//! We want to have control over certain long running
//! tasks that we care about.
//! If a task that is added to the task manager ends
//! then a reaction can be set.
//! An example would be a websocket closes with an error
//! and you want to restart it.

mod error;
pub use error::*;

use futures::stream::FuturesUnordered;
use holochain_types::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::*;

use super::{conductor::StopBroadcaster, ConductorHandle};

const CHANNEL_SIZE: usize = 1000;

/// For a task to be "managed" simply means that it will shut itself down
/// when it receives a message on the the "stop" channel passed in
pub(crate) type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
pub(crate) type TaskManagerRunHandle = JoinHandle<TaskManagerResult>;

/// A generic function to run when a task completes
pub type OnDeath = Box<dyn Fn(ManagedTaskResult) -> TaskOutcome + Send + Sync + 'static>;

/// The "kind" of a managed task determines how the Result from the task's
/// completion will be handled.
pub enum TaskKind {
    /// Log an error if there is one, but otherwise do nothing.
    Ignore,
    /// If the task returns an error, shut down the conductor.
    Unrecoverable,
    /// If the task returns an error, "freeze" the cell which caused the error,
    /// but continue running the rest of the conductor and other managed tasks.
    CellCritical(CellId),
    /// If the task returns an error, "freeze" all cells with this dna hash,
    /// but continue running the rest of the conductor and other managed tasks.
    DnaCritical(Arc<DnaHash>),
    /// A generic callback for handling the result
    // MAYBE: B-01455: reevaluate whether this should be a callback
    Generic(OnDeath),
}

/// A message sent to the TaskManager, registering an ManagedTask of a given kind.
pub struct ManagedTaskAdd {
    handle: ManagedTaskHandle,
    kind: TaskKind,
    name: String,
}

impl ManagedTaskAdd {
    fn new(handle: ManagedTaskHandle, kind: TaskKind, name: &str) -> Self {
        ManagedTaskAdd {
            handle,
            kind,
            name: name.to_string(),
        }
    }

    /// You just want the task in the task manager but don't want
    /// to react to an error
    pub fn ignore(handle: ManagedTaskHandle, name: &str) -> Self {
        Self::new(handle, TaskKind::Ignore, name)
    }

    /// If this task fails, the entire conductor must be shut down
    pub fn unrecoverable(handle: ManagedTaskHandle, name: &str) -> Self {
        Self::new(handle, TaskKind::Unrecoverable, name)
    }

    /// If this task fails, only the Cell which it runs under must be stopped
    pub fn cell_critical(handle: ManagedTaskHandle, cell_id: CellId, name: &str) -> Self {
        Self::new(handle, TaskKind::CellCritical(cell_id), name)
    }

    /// If this task fails, only the Cells with this DnaHash must be stopped
    pub fn dna_critical(handle: ManagedTaskHandle, dna_hash: Arc<DnaHash>, name: &str) -> Self {
        Self::new(handle, TaskKind::DnaCritical(dna_hash), name)
    }

    /// Handle a task's completion with a generic callback
    pub fn generic(handle: ManagedTaskHandle, f: OnDeath) -> Self {
        Self::new(handle, TaskKind::Generic(f), "unnamed")
    }
}

impl Future for ManagedTaskAdd {
    type Output = TaskOutcome;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let p = std::pin::Pin::new(&mut self.handle);
        match JoinHandle::poll(p, cx) {
            Poll::Ready(r) => Poll::Ready(handle_completed_task(
                &self.kind,
                r.unwrap_or_else(|e| Err(e.into())),
                self.name.clone(),
            )),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl std::fmt::Debug for ManagedTaskAdd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedTaskAdd").finish()
    }
}

/// The outcome of a task that has finished.
pub enum TaskOutcome {
    /// Spawn a new managed task.
    NewTask(ManagedTaskAdd),
    /// Log an info trace and take no other action.
    LogInfo(String),
    /// Log an error and take no other action.
    MinorError(Box<ManagedTaskError>, String),
    /// Close the conductor down because this is an unrecoverable error.
    ShutdownConductor(Box<ManagedTaskError>, String),
    /// Either pause or disable all apps which contain the problematic Cell,
    /// depending upon the specific error.
    StopApps(CellId, Box<ManagedTaskError>, String),
    /// Either pause or disable all apps which contain the problematic Dna,
    /// depending upon the specific error.
    StopAppsWithDna(Arc<DnaHash>, Box<ManagedTaskError>, String),
}

struct TaskManager {
    stream: FuturesUnordered<ManagedTaskAdd>,
}

impl TaskManager {
    fn new() -> Self {
        let stream = FuturesUnordered::new();
        TaskManager { stream }
    }
}

pub(crate) fn spawn_task_manager(
    handle: ConductorHandle,
) -> (mpsc::Sender<ManagedTaskAdd>, TaskManagerRunHandle) {
    let (send, recv) = mpsc::channel(CHANNEL_SIZE);
    (send, tokio::spawn(run(handle, recv)))
}

/// A super pessimistic task that is just waiting to die
/// but gets to live as long as the process
/// so the task manager doesn't quit
pub(crate) async fn keep_alive_task(mut die: broadcast::Receiver<()>) -> ManagedTaskResult {
    die.recv().await?;
    Ok(())
}

async fn run(
    conductor: ConductorHandle,
    mut new_task_channel: mpsc::Receiver<ManagedTaskAdd>,
) -> TaskManagerResult {
    let mut task_manager = TaskManager::new();
    // Need to have at least one item in the stream or it will exit early
    if let Some(new_task) = new_task_channel.recv().await {
        task_manager.stream.push(new_task);
    } else {
        error!("All senders to task manager were dropped before starting");
        return Err(TaskManagerError::TaskManagerFailedToStart);
    }
    loop {
        tokio::select! {
            Some(new_task) = new_task_channel.recv() => {
                task_manager.stream.push(new_task);
                tracing::debug!("Task added. Total tasks: {}", task_manager.stream.len());
            }
            result = task_manager.stream.next() => {
                tracing::debug!("Task completed. Total tasks: {}", task_manager.stream.len());
                match result {
                Some(TaskOutcome::NewTask(new_task)) => task_manager.stream.push(new_task),
                Some(TaskOutcome::LogInfo(context)) => {
                    debug!("Managed task completed: {}", context)
                }
                Some(TaskOutcome::MinorError(error, context)) => {
                    error!("Minor error during managed task: {:?}\nContext: {}", error, context)
                }
                Some(TaskOutcome::ShutdownConductor(error, context)) => {
                    let error = match *error {
                        ManagedTaskError::Join(error) => {
                            match error.try_into_panic() {
                                Ok(reason) => {
                                    // Resume the panic on the main task
                                    std::panic::resume_unwind(reason);
                                }
                                Err(error) => ManagedTaskError::Join(error),
                            }
                        }
                        error => error,
                    };
                    error!("Shutting down conductor due to unrecoverable error: {:?}\nContext: {}", error, context);
                    return Err(TaskManagerError::Unrecoverable(Box::new(error)));
                },
                Some(TaskOutcome::StopApps(cell_id, error, context)) => {
                    tracing::error!("About to automatically stop apps");
                    let app_ids = conductor.list_running_apps_for_dependent_cell_id(&cell_id).await.map_err(TaskManagerError::internal)?;
                    if error.is_recoverable() {
                        conductor.remove_cells(&[cell_id]).await;

                        // The following message assumes that only the app_ids calculated will be paused, but other apps
                        // may have been paused as well.
                        tracing::error!(
                            "PAUSING the following apps due to a recoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );

                        // MAYBE: it could be helpful to modify this function so that when providing Some(app_ids),
                        //   you can also pass in a PausedAppReason override, so that the reason for the apps being paused
                        //   can be set to the specific error message encountered here, rather than having to read it from
                        //   the logs.
                        let delta = conductor.reconcile_app_status_with_cell_status(None).await.map_err(TaskManagerError::internal)?;
                        tracing::debug!(delta = ?delta);

                        tracing::error!("Apps paused.");
                    } else {
                        // Since the error is unrecoverable, we don't expect to be able to use this Cell anymore.
                        // Therefore, we disable every app which requires that cell.
                        tracing::error!(
                            "DISABLING the following apps due to an unrecoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );
                        for app_id in app_ids.iter() {
                            conductor.clone().disable_app(app_id.to_string(), DisabledAppReason::Error(error.to_string())).await.map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                },
                Some(TaskOutcome::StopAppsWithDna(dna_hash, error, context)) => {
                    tracing::error!("About to automatically stop apps with dna {}", dna_hash);
                    let app_ids = conductor.list_running_apps_for_dependent_dna_hash(dna_hash.as_ref()).await.map_err(TaskManagerError::internal)?;
                    if error.is_recoverable() {
                        let cells_with_same_dna: Vec<_> = conductor.list_cell_ids(None).into_iter().filter(|id| id.dna_hash() == dna_hash.as_ref()).collect();
                        conductor.remove_cells(&cells_with_same_dna).await;

                        // The following message assumes that only the app_ids calculated will be paused, but other apps
                        // may have been paused as well.
                        tracing::error!(
                            "PAUSING the following apps due to a recoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );

                        // MAYBE: it could be helpful to modify this function so that when providing Some(app_ids),
                        //   you can also pass in a PausedAppReason override, so that the reason for the apps being paused
                        //   can be set to the specific error message encountered here, rather than having to read it from
                        //   the logs.
                        let delta = conductor.reconcile_app_status_with_cell_status(None).await.map_err(TaskManagerError::internal)?;
                        tracing::debug!(delta = ?delta);

                        tracing::error!("Apps paused.");
                    } else {
                        // Since the error is unrecoverable, we don't expect to be able to use this Cell anymore.
                        // Therefore, we disable every app which requires that cell.
                        tracing::error!(
                            "DISABLING the following apps due to an unrecoverable error: {:?}\nError: {:?}\nContext: {}",
                            app_ids,
                            error,
                            context
                        );
                        for app_id in app_ids.iter() {
                            conductor.clone().disable_app(app_id.to_string(), DisabledAppReason::Error(error.to_string())).await.map_err(TaskManagerError::internal)?;
                        }
                        tracing::error!("Apps disabled.");
                    }
                },
                None => return Ok(()),
            }}
        };
    }
}

#[tracing::instrument(skip(kind))]
fn handle_completed_task(kind: &TaskKind, result: ManagedTaskResult, name: String) -> TaskOutcome {
    use TaskOutcome::*;
    match kind {
        TaskKind::Ignore => match result {
            Ok(_) => LogInfo(name),
            Err(err) => MinorError(Box::new(err), name),
        },
        TaskKind::Unrecoverable => match result {
            Ok(_) => LogInfo(name),
            Err(err) => ShutdownConductor(Box::new(err), name),
        },
        TaskKind::CellCritical(cell_id) => match result {
            Ok(_) => LogInfo(name),
            Err(err) => StopApps(cell_id.to_owned(), Box::new(err), name),
        },
        TaskKind::DnaCritical(dna_hash) => match result {
            Ok(_) => LogInfo(name),
            Err(err) => StopAppsWithDna(dna_hash.to_owned(), Box::new(err), name),
        },
        TaskKind::Generic(f) => f(result),
    }
}

/// Handle the result of shutting down the main thread.
pub fn handle_shutdown(result: Result<TaskManagerResult, tokio::task::JoinError>) {
    let result = result.map_err(|e| {
        error!(
            error = &e as &dyn std::error::Error,
            "Failed to join the main task"
        );
        e
    });
    match result {
        Ok(result) => result.expect("Conductor shutdown error"),
        Err(error) => match error.try_into_panic() {
            Ok(reason) => {
                // Resume the panic on the main task
                std::panic::resume_unwind(reason);
            }
            Err(error) => panic!("Error while joining threads during shutdown {:?}", error),
        },
    }
}

/// A collection of channels and handles used by the Conductor to talk to the
/// TaskManager task
pub struct TaskManagerClient {
    /// Channel on which to send info about tasks we want to manage
    task_add_sender: mpsc::Sender<ManagedTaskAdd>,

    /// Sending a message on this channel will broadcast to all managed tasks,
    /// telling them to shut down
    task_stop_broadcaster: StopBroadcaster,

    /// The main task join handle to await on.
    /// The conductor is intended to live as long as this task does.
    /// It can be moved out, hence the Option. If this is None, then the
    /// handle was already moved out.
    run_handle: Option<TaskManagerRunHandle>,
}

impl TaskManagerClient {
    /// Constructor
    pub fn new(
        task_add_sender: mpsc::Sender<ManagedTaskAdd>,
        task_stop_broadcaster: StopBroadcaster,
        run_handle: TaskManagerRunHandle,
    ) -> Self {
        Self {
            task_add_sender,
            task_stop_broadcaster,
            run_handle: Some(run_handle),
        }
    }

    /// Accessor
    pub fn task_add_sender(&self) -> &mpsc::Sender<ManagedTaskAdd> {
        &self.task_add_sender
    }

    /// Accessor
    pub fn task_stop_broadcaster(&self) -> &StopBroadcaster {
        &self.task_stop_broadcaster
    }

    /// Return the handle to be joined.
    /// This will return None if the handle was already taken.
    pub fn take_handle(&mut self) -> Option<TaskManagerRunHandle> {
        self.run_handle.take()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::{error::ConductorError, Conductor};
    use anyhow::Result;
    use holochain_state::test_utils::test_db_dir;
    use observability;

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_and_handle_dying_task() -> Result<()> {
        observability::test_run().ok();
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
        let handle = tokio::spawn(async {
            Err(Box::new(ConductorError::Other(
                anyhow::anyhow!("This task gotta die").into(),
            ))
            .into())
        });
        let handle = ManagedTaskAdd::generic(
            handle,
            Box::new(|result| match result {
                Ok(_) => panic!("Task should have died"),
                Err(ManagedTaskError::Conductor(err))
                    if matches!(*err, ConductorError::Other(_)) =>
                {
                    let handle = tokio::spawn(async { Ok(()) });
                    let handle = ManagedTaskAdd::ignore(handle, "respawned task");
                    TaskOutcome::NewTask(handle)
                }
                Err(_) => unreachable!("No other error is created by this test."),
            }),
        );
        // Check that the main task doesn't close straight away
        let main_handle = tokio::spawn(main_task);
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Now send the handle
        if let Err(_) = send_task_handle.send(handle).await {
            panic!("Failed to send the handle");
        }
        main_handle.await???;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unrecoverable_error() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
        send_task_handle
            .send(ManagedTaskAdd::ignore(
                tokio::spawn(keep_alive_task(rx)),
                "",
            ))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    Err(Box::new(ConductorError::Other(
                        anyhow::anyhow!("Unrecoverable task failed").into(),
                    ))
                    .into())
                }),
                "",
            ))
            .await
            .unwrap();

        main_task
            .await
            .expect("Failed to join the main task")
            .expect_err("The main task should return an error");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "panics in tokio break other tests, this test is here to confirm behavior but cannot be run on ci"]
    async fn unrecoverable_panic() {
        observability::test_run().ok();
        let (_tx, rx) = tokio::sync::broadcast::channel(1);
        let db_dir = test_db_dir();
        let handle = Conductor::builder().test(db_dir.path(), &[]).await.unwrap();
        let (send_task_handle, main_task) = spawn_task_manager(handle);
        send_task_handle
            .send(ManagedTaskAdd::ignore(
                tokio::spawn(keep_alive_task(rx)),
                "",
            ))
            .await
            .unwrap();

        send_task_handle
            .send(ManagedTaskAdd::unrecoverable(
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    panic!("Task has panicked")
                }),
                "",
            ))
            .await
            .unwrap();

        handle_shutdown(main_task.await);
    }
}

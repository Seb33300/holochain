//! Everything to do with App (hApp) installation and uninstallation
//!
//! An App is a essentially a collection of Cells which are intended to be
//! available for a particular Holochain use-case, such as a microservice used
//! by some UI in a broader application.
//!
//! Each Cell maintains its own identity separate from any App.
//! Access to Cells can be shared between different Apps.

mod app_bundle;
mod app_manifest;
mod dna_gamut;
pub mod error;
use crate::{dna::DnaBundle, prelude::CoordinatorBundle};
pub use app_bundle::*;
pub use app_manifest::app_manifest_validated::*;
pub use app_manifest::*;
use derive_more::{Display, Into};
pub use dna_gamut::*;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_serialized_bytes::prelude::*;
use holochain_util::ffs;
use holochain_zome_types::cell::CloneId;
use holochain_zome_types::prelude::*;
use itertools::Itertools;
use std::{collections::HashMap, path::PathBuf};

use self::error::{AppError, AppResult};

/// The unique identifier for an installed app in this conductor
pub type InstalledAppId = String;

/// The source of the DNA to be installed, either as binary data, or from a path
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnaSource {
    /// register the dna loaded from a bundle file on disk
    Path(PathBuf),
    /// register the dna as provided in the DnaBundle data structure
    Bundle(Box<DnaBundle>),
    /// register the dna from an existing registered DNA (assumes properties will be set)
    Hash(DnaHash),
}

/// The source of coordinators to be installed, either as binary data, or from a path
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinatorSource {
    /// Coordinators loaded from a bundle file on disk
    Path(PathBuf),
    /// Coordinators provided in the [`CoordinatorBundle`] data structure
    Bundle(Box<CoordinatorBundle>),
}

/// The instructions on how to get the DNA to be registered
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RegisterDnaPayload {
    /// Modifier overrides
    #[serde(default)]
    pub modifiers: DnaModifiersOpt<YamlProperties>,
    /// Where to find the DNA
    #[serde(flatten)]
    pub source: DnaSource,
}

/// The instructions on how to request NetworkInfo
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkInfoRequestPayload {
    /// Get gossip info for these DNAs
    pub dnas: Vec<DnaHash>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
/// The instructions on how to update coordinators for a dna file.
pub struct UpdateCoordinatorsPayload {
    /// The hash of the dna to swap coordinators for.
    pub dna_hash: DnaHash,
    /// Where to find the coordinators.
    #[serde(flatten)]
    pub source: CoordinatorSource,
}

/// The arguments to create a clone of an existing cell.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateCloneCellPayload {
    /// The app id that the DNA to clone belongs to
    pub app_id: InstalledAppId,
    /// The DNA's role id to clone
    pub role_name: RoleName,
    /// Modifiers to set for the new cell.
    /// At least one of the modifiers must be set to obtain a distinct hash for
    /// the clone cell's DNA.
    pub modifiers: DnaModifiersOpt<YamlProperties>,
    /// Optionally set a proof of membership for the clone cell
    pub membrane_proof: Option<MembraneProof>,
    /// Optionally a name for the DNA clone
    pub name: Option<String>,
}

/// Ways of specifying a clone cell.
#[derive(Clone, Debug, Display, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CloneCellId {
    /// Clone id consisting of role id and clone index.
    CloneId(CloneId),
    /// Cell id consisting of DNA hash and agent pub key.
    CellId(CellId),
}

/// Arguments to specify the clone cell to be disabled.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisableCloneCellPayload {
    /// The app id that the clone cell belongs to
    pub app_id: InstalledAppId,
    /// The clone id or cell id of the clone cell
    pub clone_cell_id: CloneCellId,
}

/// Argumtents to specify the clone cell to be enabled.
pub type EnableCloneCellPayload = DisableCloneCellPayload;

/// Arguments to delete a disabled clone cell of an app.
pub type DeleteCloneCellPayload = DisableCloneCellPayload;

/// An [AppBundle] along with an [AgentPubKey] and optional [InstalledAppId]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppPayload {
    /// The unique identifier for an installed app in this conductor.
    #[serde(flatten)]
    pub source: AppBundleSource,

    /// The agent to use when creating Cells for this App.
    pub agent_key: AgentPubKey,

    /// The unique identifier for an installed app in this conductor.
    /// If not specified, it will be derived from the app name in the bundle manifest.
    pub installed_app_id: Option<InstalledAppId>,

    /// Include proof-of-membrane-membership data for cells that require it,
    /// keyed by the RoleName specified in the app bundle manifest.
    pub membrane_proofs: HashMap<RoleName, MembraneProof>,

    /// Optional: overwrites all network seeds for all DNAs of Cells created by this app.
    /// The app can still use existing Cells, i.e. this does not require that
    /// all Cells have DNAs with the same overridden DNA.
    pub network_seed: Option<NetworkSeed>,
}

/// The possible locations of an AppBundle
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppBundleSource {
    /// The actual serialized bytes of a bundle
    Bundle(AppBundle),
    /// A local file path
    Path(PathBuf),
    // /// A URL
    // Url(String),
}

impl AppBundleSource {
    /// Get the bundle from the source. Consumes the source.
    pub async fn resolve(self) -> Result<AppBundle, AppBundleError> {
        Ok(match self {
            Self::Bundle(bundle) => bundle,
            Self::Path(path) => AppBundle::decode(&ffs::read(&path).await?)?,
            // Self::Url(url) => todo!("reqwest::get"),
        })
    }
}

/// Information needed to specify a DNA as part of an App
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InstallAppDnaPayload {
    /// The hash of the DNA
    pub hash: DnaHash,
    /// The RoleName which will be assigned to this DNA when installed
    pub role_name: RoleName,
    /// App-specific proof-of-membrane-membership, if required by this app
    pub membrane_proof: Option<MembraneProof>,
}

impl InstallAppDnaPayload {
    /// Create a payload from hash. Good for tests.
    pub fn hash_only(hash: DnaHash, role_name: RoleName) -> Self {
        Self {
            hash,
            role_name,
            membrane_proof: None,
        }
    }
}

/// Data about an installed Cell.
#[derive(Clone, Debug, Into, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstalledCell {
    cell_id: CellId,
    role_name: RoleName,
}

impl InstalledCell {
    /// Constructor
    pub fn new(cell_id: CellId, role_name: RoleName) -> Self {
        Self { cell_id, role_name }
    }

    /// Get the CellId
    pub fn into_id(self) -> CellId {
        self.cell_id
    }

    /// Get the RoleName
    pub fn into_role_name(self) -> RoleName {
        self.role_name
    }

    /// Get the inner data as a tuple
    pub fn into_inner(self) -> (CellId, RoleName) {
        (self.cell_id, self.role_name)
    }

    /// Get the CellId
    pub fn as_id(&self) -> &CellId {
        &self.cell_id
    }

    /// Get the RoleName
    pub fn as_role_name(&self) -> &RoleName {
        &self.role_name
    }
}

/// An app which has been installed.
/// An installed app is merely its collection of "roles", associated with an ID.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::Constructor,
    shrinkwraprs::Shrinkwrap,
)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct InstalledApp {
    #[shrinkwrap(main_field)]
    app: InstalledAppCommon,
    /// The status of the installed app
    pub status: AppStatus,
}

impl InstalledApp {
    /// Constructor for freshly installed app
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: AppStatus::Disabled(DisabledAppReason::NeverStarted),
        }
    }

    /// Constructor for freshly installed app
    #[cfg(feature = "test_utils")]
    pub fn new_running(app: InstalledAppCommon) -> Self {
        Self {
            app,
            status: AppStatus::Running,
        }
    }

    /// Return the common app info, as well as a status which encodes the remaining
    /// information
    pub fn into_app_and_status(self) -> (InstalledAppCommon, AppStatus) {
        (self.app, self.status)
    }

    /// Accessor
    pub fn status(&self) -> &AppStatus {
        &self.status
    }

    /// Accessor
    pub fn id(&self) -> &InstalledAppId {
        &self.app.installed_app_id
    }
}

impl automap::AutoMapped for InstalledApp {
    type Key = InstalledAppId;

    fn key(&self) -> &Self::Key {
        &self.app.installed_app_id
    }
}

/// A map from InstalledAppId -> InstalledApp
pub type InstalledAppMap = automap::AutoHashMap<InstalledApp>;

/// An active app
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    derive_more::From,
    shrinkwraprs::Shrinkwrap,
)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct RunningApp(InstalledAppCommon);

impl RunningApp {
    /// Convert to a StoppedApp with the given reason
    pub fn into_stopped(self, reason: StoppedAppReason) -> StoppedApp {
        StoppedApp {
            app: self.0,
            reason,
        }
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.0
    }
}

impl From<RunningApp> for InstalledApp {
    fn from(app: RunningApp) -> Self {
        Self {
            app: app.into_common(),
            status: AppStatus::Running,
        }
    }
}

/// An app which is either Paused or Disabled, i.e. not Running
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap,
)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct StoppedApp {
    #[shrinkwrap(main_field)]
    app: InstalledAppCommon,
    reason: StoppedAppReason,
}

impl StoppedApp {
    /// Constructor
    #[deprecated = "should only be constructable through conversions from other types"]
    pub fn new(app: InstalledAppCommon, reason: StoppedAppReason) -> Self {
        Self { app, reason }
    }

    /// Constructor
    pub fn new_fresh(app: InstalledAppCommon) -> Self {
        Self {
            app,
            reason: StoppedAppReason::Disabled(DisabledAppReason::NeverStarted),
        }
    }

    /// If the app is Stopped, convert into a StoppedApp.
    /// Returns None if app is Running.
    pub fn from_app(app: &InstalledApp) -> Option<Self> {
        StoppedAppReason::from_status(app.status()).map(|reason| Self {
            app: app.as_ref().clone(),
            reason,
        })
    }

    /// Convert to a RunningApp
    pub fn into_active(self) -> RunningApp {
        RunningApp(self.app)
    }

    /// Move inner type out
    pub fn into_common(self) -> InstalledAppCommon {
        self.app
    }
}

impl From<StoppedApp> for InstalledAppCommon {
    fn from(d: StoppedApp) -> Self {
        d.app
    }
}

impl From<StoppedApp> for InstalledApp {
    fn from(d: StoppedApp) -> Self {
        Self {
            app: d.app,
            status: d.reason.into(),
        }
    }
}

/// The common data between apps of any status
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledAppCommon {
    /// The unique identifier for an installed app in this conductor
    installed_app_id: InstalledAppId,
    /// The agent key used to install this app. Currently this is meaningless,
    /// but I'm leaving it here as a placeholder in case we ever want it to
    /// have formal significance.
    _agent_key: AgentPubKey,
    /// Assignments of DNA roles to cells and their clones, as specified in the AppManifest
    role_assignments: HashMap<RoleName, AppRoleAssignment>,
}

impl InstalledAppCommon {
    /// Constructor
    pub fn new<S: ToString, I: IntoIterator<Item = (RoleName, AppRoleAssignment)>>(
        installed_app_id: S,
        _agent_key: AgentPubKey,
        role_assignments: I,
    ) -> AppResult<Self> {
        let role_assignments: HashMap<_, _> = role_assignments.into_iter().collect();
        // ensure no role id contains a clone id delimiter
        if let Some((illegal_role_name, _)) = role_assignments
            .iter()
            .find(|(role_name, _)| role_name.contains(CLONE_ID_DELIMITER))
        {
            return Err(AppError::IllegalRoleName(illegal_role_name.clone()));
        }
        Ok(InstalledAppCommon {
            installed_app_id: installed_app_id.to_string(),
            _agent_key,
            role_assignments,
        })
    }

    /// Accessor
    pub fn id(&self) -> &InstalledAppId {
        &self.installed_app_id
    }

    /// Accessor
    pub fn provisioned_cells(&self) -> impl Iterator<Item = (&RoleName, &CellId)> {
        self.role_assignments
            .iter()
            .filter_map(|(role_name, role)| role.provisioned_cell().map(|c| (role_name, c)))
    }

    /// Accessor
    pub fn into_provisioned_cells(self) -> impl Iterator<Item = (RoleName, CellId)> {
        self.role_assignments
            .into_iter()
            .filter_map(|(role_name, role)| role.into_provisioned_cell().map(|c| (role_name, c)))
    }

    /// Accessor
    pub fn clone_cells(&self) -> impl Iterator<Item = (&CloneId, &CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| app_role_assignment.1.clones.iter())
    }

    /// Accessor
    pub fn disabled_clone_cells(&self) -> impl Iterator<Item = (&CloneId, &CellId)> {
        self.role_assignments
            .iter()
            .flat_map(|app_role_assignment| app_role_assignment.1.disabled_clones.iter())
    }

    /// Accessor
    pub fn clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<&HashMap<CloneId, CellId>> {
        match self.role_assignments.get(role_name) {
            None => None,
            Some(role_assignments) => Some(&role_assignments.clones),
        }
    }

    /// Accessor
    pub fn disabled_clone_cells_for_role_name(
        &self,
        role_name: &RoleName,
    ) -> Option<&HashMap<CloneId, CellId>> {
        match self.role_assignments.get(role_name) {
            None => None,
            Some(role_assignment) => Some(&role_assignment.disabled_clones),
        }
    }

    /// Accessor
    pub fn clone_cell_ids(&self) -> impl Iterator<Item = &CellId> {
        self.clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Accessor
    pub fn disabled_clone_cell_ids(&self) -> impl Iterator<Item = &CellId> {
        self.disabled_clone_cells().map(|(_, cell_id)| cell_id)
    }

    /// Iterator of all cells, both provisioned and cloned
    pub fn all_cells(&self) -> impl Iterator<Item = &CellId> {
        self.provisioned_cells()
            .map(|(_, c)| c)
            .chain(self.clone_cell_ids())
            .chain(self.disabled_clone_cell_ids())
    }

    /// Iterator of all "required" cells, meaning Cells which must be running
    /// for this App to be able to run. The notion of "required cells" is not
    /// yet solidified, so for now this placeholder equates to "all cells".
    pub fn required_cells(&self) -> impl Iterator<Item = &CellId> {
        self.all_cells()
    }

    /// Accessor for particular role
    pub fn role(&self, role_name: &RoleName) -> AppResult<&AppRoleAssignment> {
        self.role_assignments
            .get(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    fn role_mut(&mut self, role_name: &RoleName) -> AppResult<&mut AppRoleAssignment> {
        self.role_assignments
            .get_mut(role_name)
            .ok_or_else(|| AppError::RoleNameMissing(role_name.clone()))
    }

    /// Accessor
    pub fn roles(&self) -> &HashMap<RoleName, AppRoleAssignment> {
        &self.role_assignments
    }

    /// Add a clone cell.
    pub fn add_clone(&mut self, role_name: &RoleName, cell_id: &CellId) -> AppResult<CloneId> {
        let app_role_assignment = self.role_mut(role_name)?;
        assert_eq!(
            cell_id.agent_pubkey(),
            app_role_assignment.agent_key(),
            "A clone cell must use the same agent key as the role it is added to"
        );
        if app_role_assignment.is_clone_limit_reached() {
            return Err(AppError::CloneLimitExceeded(
                app_role_assignment.clone_limit,
                app_role_assignment.clone(),
            ));
        }
        let clone_id = CloneId::new(role_name, app_role_assignment.next_clone_index);
        if app_role_assignment.clones.contains_key(&clone_id) {
            return Err(AppError::DuplicateCloneIds(clone_id));
        }

        // add clone
        app_role_assignment
            .clones
            .insert(clone_id.clone(), cell_id.clone());
        // increment next clone index
        app_role_assignment.next_clone_index += 1;
        Ok(clone_id)
    }

    /// Get a clone cell id from its clone id.
    pub fn get_clone_cell_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CellId> {
        let cell_id = match clone_cell_id {
            CloneCellId::CellId(cell_id) => cell_id,
            CloneCellId::CloneId(clone_id) => self
                .role(&clone_id.as_base_role_name())?
                .clones
                .get(clone_id)
                .ok_or_else(|| {
                    AppError::CloneCellNotFound(CloneCellId::CloneId(clone_id.clone()))
                })?,
        };
        Ok(cell_id.clone())
    }

    /// Get the clone id from either clone or cell id.
    pub fn get_clone_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CloneId> {
        let clone_id = match clone_cell_id {
            CloneCellId::CloneId(id) => id,
            CloneCellId::CellId(id) => {
                self.clone_cells()
                    .find(|(_, cell_id)| *cell_id == id)
                    .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::CellId(id.clone())))?
                    .0
            }
        };
        Ok(clone_id.clone())
    }

    /// Get the clone id from either clone or cell id.
    pub fn get_disabled_clone_id(&self, clone_cell_id: &CloneCellId) -> AppResult<CloneId> {
        let clone_id = match clone_cell_id {
            CloneCellId::CloneId(id) => id.clone(),
            CloneCellId::CellId(id) => {
                self.role_assignments
                    .iter()
                    .flat_map(|(_, role_assignment)| role_assignment.disabled_clones.clone())
                    .find(|(_, cell_id)| cell_id == id)
                    .ok_or_else(|| AppError::CloneCellNotFound(CloneCellId::CellId(id.clone())))?
                    .0
            }
        };
        Ok(clone_id)
    }

    /// Disable a clone cell.
    ///
    /// Removes the cell from the list of clones, and it is not accessible any
    /// longer.
    pub fn disable_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
        // remove clone from role's clones map
        match app_role_assignment.clones.remove(clone_id) {
            None => Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                clone_id.to_owned(),
            ))),
            Some(cell_id) => {
                // insert clone into disabled clones map
                let insert_result = app_role_assignment
                    .disabled_clones
                    .insert(clone_id.to_owned(), cell_id);
                assert!(
                    insert_result.is_none(),
                    "disable: clone cell is already disabled"
                );
                Ok(())
            }
        }
    }

    /// Transformer
    /// Enable a disabled clone cell.
    ///
    /// The clone cell is added back to the list of clones and can be accessed
    /// again.
    ///
    /// # Returns
    /// The enabled clone cell.
    pub fn enable_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<InstalledCell> {
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
        // remove clone from disabled clones map
        match app_role_assignment.disabled_clones.remove(clone_id) {
            None => Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                clone_id.to_owned(),
            ))),
            Some(cell_id) => {
                // insert clone back into role's clones map
                let insert_result = app_role_assignment
                    .clones
                    .insert(clone_id.to_owned(), cell_id.clone());
                assert!(
                    insert_result.is_none(),
                    "enable: clone cell already enabled"
                );
                Ok(InstalledCell {
                    role_name: clone_id.as_app_role_name().to_owned(),
                    cell_id,
                })
            }
        }
    }

    /// Delete a disabled clone cell.
    pub fn delete_clone_cell(&mut self, clone_id: &CloneId) -> AppResult<()> {
        let app_role_assignment = self.role_mut(&clone_id.as_base_role_name())?;
        match app_role_assignment.disabled_clones.remove(clone_id) {
            None => Err(AppError::CloneCellNotFound(CloneCellId::CloneId(
                clone_id.to_owned(),
            ))),
            Some(_) => Ok(()),
        }
    }

    /// Accessor
    pub fn _agent_key(&self) -> &AgentPubKey {
        &self._agent_key
    }

    /// Constructor for apps not using a manifest.
    /// Allows for cloning up to 256 times and implies immediate provisioning.
    pub fn new_legacy<S: ToString, I: IntoIterator<Item = InstalledCell>>(
        installed_app_id: S,
        installed_cells: I,
    ) -> AppResult<Self> {
        let installed_app_id = installed_app_id.to_string();
        let installed_cells: Vec<_> = installed_cells.into_iter().collect();

        // Get the agent key of the first cell
        // NB: currently this has no significance.
        let _agent_key = installed_cells
            .get(0)
            .expect("Can't create app with 0 cells")
            .cell_id
            .agent_pubkey()
            .to_owned();

        // ensure all cells use the same agent key
        if installed_cells
            .iter()
            .any(|c| *c.cell_id.agent_pubkey() != _agent_key)
        {
            tracing::warn!(
                "It's kind of an informal convention that all cells in a legacy installation should use the same agent key. But, no big deal... Cell data: {:#?}",
                installed_cells
            );
        }

        // ensure all cells use the same agent key
        let duplicates: Vec<RoleName> = installed_cells
            .iter()
            .map(|c| c.role_name.to_owned())
            .counts()
            .into_iter()
            .filter_map(|(role_name, count)| if count > 1 { Some(role_name) } else { None })
            .collect();
        if !duplicates.is_empty() {
            return Err(AppError::DuplicateRoleNames(installed_app_id, duplicates));
        }

        let roles = installed_cells
            .into_iter()
            .map(|InstalledCell { role_name, cell_id }| {
                let role = AppRoleAssignment {
                    base_cell_id: cell_id,
                    is_provisioned: true,
                    clones: HashMap::new(),
                    clone_limit: 256,
                    next_clone_index: 0,
                    disabled_clones: HashMap::new(),
                };
                (role_name, role)
            })
            .collect();
        Ok(Self {
            installed_app_id,
            _agent_key,
            role_assignments: roles,
        })
    }
}

/// The status of an installed app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    /// The app is enabled and running normally.
    Running,

    /// Enabled, but stopped due to some recoverable problem.
    /// The app "hopes" to be Running again as soon as possible.
    /// Holochain may restart the app automatically if it can. It may also be
    /// restarted manually via the `StartApp` admin method.
    /// Paused apps will be automatically set to Running when the conductor restarts.
    Paused(PausedAppReason),

    /// Disabled and stopped, either manually by the user, or automatically due
    /// to an unrecoverable error. App must be Enabled before running again,
    /// and will not restart automaticaly on conductor reboot.
    Disabled(DisabledAppReason),
}

/// The AppStatus without the reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum AppStatusKind {
    Running,
    Paused,
    Disabled,
}

impl From<AppStatus> for AppStatusKind {
    fn from(status: AppStatus) -> Self {
        match status {
            AppStatus::Running => Self::Running,
            AppStatus::Paused(_) => Self::Paused,
            AppStatus::Disabled(_) => Self::Disabled,
        }
    }
}

/// Represents a state transition operation from one state to another
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppStatusTransition {
    /// Attempt to unpause a Paused app
    Start,
    /// Attempt to pause a Running app
    Pause(PausedAppReason),
    /// Gets an app running no matter what
    Enable,
    /// Disables an app, no matter what
    Disable(DisabledAppReason),
}

impl AppStatus {
    /// Does this status correspond to an Enabled state?
    /// If false, this indicates a Disabled state.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Running | Self::Paused(_))
    }

    /// Does this status correspond to a Running state?
    /// If false, this indicates a Stopped state.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Does this status correspond to a Paused state?
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused(_))
    }

    /// Transition a status from one state to another.
    /// If None, the transition was not valid, and the status did not change.
    pub fn transition(&mut self, transition: AppStatusTransition) -> AppStatusFx {
        use AppStatus::*;
        use AppStatusFx::*;
        use AppStatusTransition::*;
        match (&self, transition) {
            (Running, Pause(reason)) => Some((Paused(reason), SpinDown)),
            (Running, Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Running, Start) | (Running, Enable) => None,

            (Paused(_), Start) => Some((Running, SpinUp)),
            (Paused(_), Enable) => Some((Running, SpinUp)),
            (Paused(_), Disable(reason)) => Some((Disabled(reason), SpinDown)),
            (Paused(_), Pause(_)) => None,

            (Disabled(_), Enable) => Some((Running, SpinUp)),
            (Disabled(_), Pause(_)) | (Disabled(_), Disable(_)) | (Disabled(_), Start) => None,
        }
        .map(|(new_status, delta)| {
            *self = new_status;
            delta
        })
        .unwrap_or(NoChange)
    }
}

/// A declaration of the side effects of a particular AppStatusTransition.
///
/// Two values of this type may also be combined into one,
/// to capture the overall effect of a series of transitions.
///
/// The intent of this type is to make sure that any operation which causes an
/// app state transition is followed up with a call to process_app_status_fx
/// in order to reconcile the cell state with the new app state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "be sure to run this value through `process_app_status_fx` to handle any transition effects"]
pub enum AppStatusFx {
    /// The transition did not result in any change to CellState.
    NoChange,
    /// The transition may cause some Cells to be removed.
    SpinDown,
    /// The transition may cause some Cells to be added (fallibly).
    SpinUp,
    /// The transition may cause some Cells to be removed and some to be (fallibly) added.
    Both,
}

impl Default for AppStatusFx {
    fn default() -> Self {
        Self::NoChange
    }
}

impl AppStatusFx {
    /// Combine two effects into one. Think "monoidal append", if that helps.
    pub fn combine(self, other: Self) -> Self {
        use AppStatusFx::*;
        match (self, other) {
            (NoChange, a) | (a, NoChange) => a,
            (SpinDown, SpinDown) => SpinDown,
            (SpinUp, SpinUp) => SpinUp,
            (Both, _) | (_, Both) => Both,
            (SpinDown, SpinUp) | (SpinUp, SpinDown) => Both,
        }
    }
}

/// The various reasons for why an App is not in the Running state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    derive_more::From,
)]
#[serde(rename_all = "snake_case")]
pub enum StoppedAppReason {
    /// Same meaning as [`InstalledAppInfoStatus::Paused`](https://docs.rs/holochain_conductor_api/0.0.33/holochain_conductor_api/enum.InstalledAppInfoStatus.html#variant.Paused).
    Paused(PausedAppReason),

    /// Same meaning as [`InstalledAppInfoStatus::Disabled`](https://docs.rs/holochain_conductor_api/0.0.33/holochain_conductor_api/enum.InstalledAppInfoStatus.html#variant.Disabled).
    Disabled(DisabledAppReason),
}

impl StoppedAppReason {
    /// Convert a status into a StoppedAppReason.
    /// If the status is Running, returns None.
    pub fn from_status(status: &AppStatus) -> Option<Self> {
        match status {
            AppStatus::Paused(reason) => Some(Self::Paused(reason.clone())),
            AppStatus::Disabled(reason) => Some(Self::Disabled(reason.clone())),
            AppStatus::Running => None,
        }
    }
}

impl From<StoppedAppReason> for AppStatus {
    fn from(reason: StoppedAppReason) -> Self {
        match reason {
            StoppedAppReason::Paused(reason) => Self::Paused(reason),
            StoppedAppReason::Disabled(reason) => Self::Disabled(reason),
        }
    }
}

/// The reason for an app being in a Paused state.
/// NB: there is no way to manually pause an app.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum PausedAppReason {
    /// The pause was due to a RECOVERABLE error
    Error(String),
}

/// The reason for an app being in a Disabled state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, SerializedBytes)]
#[serde(rename_all = "snake_case")]
pub enum DisabledAppReason {
    /// The app is freshly installed, and never started
    NeverStarted,
    /// The disabling was done manually by the user (via admin interface)
    User,
    /// The disabling was due to an UNRECOVERABLE error
    Error(String),
}

/// App "roles" correspond to cell entries in the AppManifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppRoleAssignment {
    /// The Id of the Cell which will be provisioned for this role.
    /// This also identifies the basis for cloned DNAs, and this is how the
    /// Agent is determined for clones (always the same as the provisioned cell).
    base_cell_id: CellId,
    /// Records whether the base cell has actually been provisioned or not.
    /// If true, then `base_cell_id` refers to an actual existing Cell.
    /// If false, then `base_cell_id` is just recording what that cell will be
    /// called in the future.
    is_provisioned: bool,
    /// The number of allowed clone cells.
    clone_limit: u32,
    /// The index of the next clone cell to be created.
    next_clone_index: u32,
    /// Cells which were cloned at runtime. The length cannot grow beyond
    /// `clone_limit`.
    clones: HashMap<CloneId, CellId>,
    /// Clone cells that have been disabled. These cells cannot be called
    /// any longer and are not returned as part of the app info either.
    /// Disabled clone cells can be deleted through the Admin API.
    disabled_clones: HashMap<CloneId, CellId>,
}

impl AppRoleAssignment {
    /// Constructor. List of clones always starts empty.
    pub fn new(base_cell_id: CellId, is_provisioned: bool, clone_limit: u32) -> Self {
        Self {
            base_cell_id,
            is_provisioned,
            clone_limit,
            clones: HashMap::new(),
            next_clone_index: 0,
            disabled_clones: HashMap::new(),
        }
    }

    /// Accessor
    pub fn cell_id(&self) -> &CellId {
        &self.base_cell_id
    }

    /// Accessor
    pub fn dna_hash(&self) -> &DnaHash {
        self.base_cell_id.dna_hash()
    }

    /// Accessor
    pub fn agent_key(&self) -> &AgentPubKey {
        self.base_cell_id.agent_pubkey()
    }

    /// Accessor
    pub fn provisioned_cell(&self) -> Option<&CellId> {
        if self.is_provisioned {
            Some(&self.base_cell_id)
        } else {
            None
        }
    }

    /// Accessor
    pub fn clone_ids(&self) -> impl Iterator<Item = &CloneId> {
        self.clones.iter().map(|(clone_id, _)| clone_id)
    }

    /// Accessor
    pub fn clone_limit(&self) -> u32 {
        self.clone_limit
    }

    /// Accessor
    pub fn is_clone_limit_reached(&self) -> bool {
        self.clones.len() as u32 == self.clone_limit
    }

    /// Transformer
    pub fn into_provisioned_cell(self) -> Option<CellId> {
        if self.is_provisioned {
            Some(self.base_cell_id)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppRoleAssignment, RunningApp};
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn illegal_role_name_is_rejected() {
        let result = InstalledAppCommon::new(
            "test_app",
            fixt!(AgentPubKey),
            vec![(
                CLONE_ID_DELIMITER.into(),
                AppRoleAssignment::new(fixt!(CellId), false, 0),
            )],
        );
        assert!(result.is_err())
    }

    #[test]
    fn clone_management() {
        let base_cell_id = fixt!(CellId);
        let agent = base_cell_id.agent_pubkey().clone();
        let new_clone = || CellId::new(fixt!(DnaHash), agent.clone());
        let clone_limit = 3;
        let role1 = AppRoleAssignment::new(base_cell_id, false, clone_limit);
        let agent = fixt!(AgentPubKey);
        let role_name: RoleName = "role_name".into();
        let mut app: RunningApp =
            InstalledAppCommon::new("app", agent.clone(), vec![(role_name.clone(), role1)])
                .unwrap()
                .into();

        // Can add clones up to the limit
        let clones: Vec<_> = vec![new_clone(), new_clone(), new_clone()];
        let clone_id_0 = app.add_clone(&role_name, &clones[0]).unwrap();
        let clone_id_1 = app.add_clone(&role_name, &clones[1]).unwrap();
        let clone_id_2 = app.add_clone(&role_name, &clones[2]).unwrap();

        assert_eq!(clone_id_0, CloneId::new(&role_name, 0));
        assert_eq!(clone_id_1, CloneId::new(&role_name, 1));
        assert_eq!(clone_id_2, CloneId::new(&role_name, 2));

        assert_eq!(
            app.clone_cell_ids().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
        );
        assert_eq!(app.clone_cells().count(), 3);

        // Adding the same clone twice should return an error
        let result_add_clone_twice = app.add_clone(&role_name, &clones[0]);
        assert!(result_add_clone_twice.is_err());

        // Adding a clone beyond the clone_limit is an error
        matches::assert_matches!(
            app.add_clone(&role_name, &new_clone()),
            Err(AppError::CloneLimitExceeded(3, _))
        );

        // Disable a clone cell
        app.disable_clone_cell(&clone_id_0).unwrap();
        // Assert it is moved to disabled clone cells
        assert!(!app
            .clone_cells()
            .any(|(clone_id, _)| *clone_id == clone_id_0));
        assert_eq!(app.clone_cells().count(), 2);
        assert!(app
            .disabled_clone_cells()
            .any(|(clone_id, _)| *clone_id == clone_id_0));

        // Enable a disabled clone cell
        let enabled_cell = app.enable_clone_cell(&clone_id_0).unwrap();
        assert_eq!(
            enabled_cell.role_name,
            clone_id_0.as_app_role_name().to_owned()
        );
        // Assert it is accessible from the app again
        assert!(app
            .clone_cells()
            .find(|(clone_id, _)| **clone_id == clone_id_0)
            .is_some());
        assert_eq!(
            app.clone_cell_ids().collect::<HashSet<_>>(),
            maplit::hashset! { &clones[0], &clones[1], &clones[2] }
        );
        assert_eq!(app.clone_cells().count(), 3);

        // Disable and delete a clone cell
        app.disable_clone_cell(&clone_id_0).unwrap();
        app.delete_clone_cell(&clone_id_0).unwrap();
        // Assert the deleted cell cannot be enabled
        assert!(app.enable_clone_cell(&clone_id_0).is_err());
    }
}

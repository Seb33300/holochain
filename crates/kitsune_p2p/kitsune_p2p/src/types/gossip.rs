use crate::metrics::*;
use crate::types::*;
use crate::HostApi;
use kitsune_p2p_fetch::FetchQueue;
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::tx2::tx2_api::*;
use kitsune_p2p_types::tx2::tx2_utils::TxUrl;
use kitsune_p2p_types::*;
use std::sync::Arc;

#[derive(Clone, Debug, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
/// The type of gossip module running this gossip.
pub enum GossipModuleType {
    /// Recent sharded gossip.
    ShardedRecent,
    /// Historical sharded gossip.
    ShardedHistorical,
}

/// Represents an interchangeable gossip strategy module
pub trait AsGossipModule: 'static + Send + Sync {
    fn close(&self);
    fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        remote_url: TxUrl,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()>;
    fn local_agent_join(&self, a: Arc<KitsuneAgent>);
    fn local_agent_leave(&self, a: Arc<KitsuneAgent>);
    fn new_integrated_data(&self) {}
}

#[derive(Clone)]
pub struct GossipModule(pub Arc<dyn AsGossipModule>);

impl GossipModule {
    pub fn close(&self) {
        self.0.close()
    }

    pub fn incoming_gossip(
        &self,
        con: Tx2ConHnd<wire::Wire>,
        remote_url: TxUrl,
        gossip_data: Box<[u8]>,
    ) -> KitsuneResult<()> {
        self.0.incoming_gossip(con, remote_url, gossip_data)
    }

    pub fn local_agent_join(&self, a: Arc<KitsuneAgent>) {
        self.0.local_agent_join(a);
    }

    pub fn local_agent_leave(&self, a: Arc<KitsuneAgent>) {
        self.0.local_agent_leave(a);
    }

    /// New data has been integrated and is ready for gossiping.
    pub fn new_integrated_data(&self) {
        self.0.new_integrated_data();
    }
}

impl std::fmt::Debug for GossipModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GossipModule").finish()
    }
}

/// Represents an interchangeable gossip strategy module factory
pub trait AsGossipModuleFactory: 'static + Send + Sync {
    #[allow(clippy::too_many_arguments)]
    fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
        host: HostApi,
        metrics: MetricsSync,
        fetch_queue: FetchQueue,
    ) -> GossipModule;
}

pub struct GossipModuleFactory(pub Arc<dyn AsGossipModuleFactory>);

impl GossipModuleFactory {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_gossip_task(
        &self,
        tuning_params: KitsuneP2pTuningParams,
        space: Arc<KitsuneSpace>,
        ep_hnd: Tx2EpHnd<wire::Wire>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
        host: HostApi,
        metrics: MetricsSync,
        fetch_queue: FetchQueue,
    ) -> GossipModule {
        self.0.spawn_gossip_task(
            tuning_params,
            space,
            ep_hnd,
            evt_sender,
            host,
            metrics,
            fetch_queue,
        )
    }
}

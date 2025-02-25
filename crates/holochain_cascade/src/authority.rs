//! Functions for the various authorities to handle queries

use self::get_agent_activity_query::hashes::GetAgentActivityQuery;
use self::get_agent_activity_query::must_get_agent_activity::must_get_agent_activity;
use self::get_entry_ops_query::GetEntryOpsQuery;
use self::get_links_ops_query::GetLinksOpsQuery;
use self::{
    get_agent_activity_query::deterministic::DeterministicGetAgentActivityQuery,
    get_record_query::GetRecordOpsQuery,
};

use super::error::CascadeResult;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holochain_state::query::Query;
use holochain_state::query::Txn;
use holochain_types::prelude::*;
use holochain_zome_types::agent_activity::DeterministicGetAgentActivityFilter;
use tracing::*;

#[cfg(test)]
mod test;

pub(crate) mod get_agent_activity_query;
pub(crate) mod get_entry_ops_query;
pub(crate) mod get_links_ops_query;
pub(crate) mod get_record_query;

/// Handler for get_entry query to an Entry authority
#[instrument(skip(db))]
pub async fn handle_get_entry(
    db: DbRead<DbKindDht>,
    hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireEntryOps> {
    let query = GetEntryOpsQuery::new(hash);
    let results = db
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for get_record query to a Record authority
#[tracing::instrument(skip(env))]
pub async fn handle_get_record(
    env: DbRead<DbKindDht>,
    hash: ActionHash,
    options: holochain_p2p::event::GetOptions,
) -> CascadeResult<WireRecordOps> {
    let query = GetRecordOpsQuery::new(hash, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for get_agent_activity query to an Activity authority
#[instrument(skip(env))]
pub async fn handle_get_agent_activity(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    query: ChainQueryFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse<ActionHash>> {
    let query = GetAgentActivityQuery::new(agent, query, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for must_get_agent_activity query to an Activity authority
#[instrument(skip(env))]
pub async fn handle_must_get_agent_activity(
    env: DbRead<DbKindDht>,
    author: AgentPubKey,
    filter: ChainFilter,
) -> CascadeResult<MustGetAgentActivityResponse> {
    Ok(must_get_agent_activity(env, author, filter).await?)
}

/// Handler for get_agent_activity_deterministic query to an Activity authority
#[instrument(skip(env))]
pub async fn handle_get_agent_activity_deterministic(
    env: DbRead<DbKindDht>,
    agent: AgentPubKey,
    filter: DeterministicGetAgentActivityFilter,
    options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<DeterministicGetAgentActivityResponse> {
    let query = DeterministicGetAgentActivityQuery::new(agent, filter, options);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

/// Handler for get_links query to a Record/Entry authority
#[instrument(skip(env, _options))]
pub async fn handle_get_links(
    env: DbRead<DbKindDht>,
    link_key: WireLinkKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<WireLinkOps> {
    let query = GetLinksOpsQuery::new(link_key);
    let results = env
        .async_reader(move |txn| query.run(Txn::from(&txn)))
        .await?;
    Ok(results)
}

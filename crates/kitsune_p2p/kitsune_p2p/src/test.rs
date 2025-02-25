#[cfg(test)]
mod tests {
    use crate::test_util::*;
    use crate::types::actor::KitsuneP2pSender;
    use crate::*;
    use ghost_actor::dependencies::tracing;
    use ghost_actor::GhostControlSender;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_transport_coms() {
        observability::test_run().ok();
        observability::metrics::init();
        let (harness, _evt) = spawn_test_harness_mem().await.unwrap();

        let space = harness.add_space().await.unwrap();
        let (a1, p2p1) = harness.add_direct_agent("one".into()).await.unwrap();
        let (a2, p2p2) = harness.add_direct_agent("two".into()).await.unwrap();

        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await.unwrap();

        let r1 = p2p1
            .rpc_single(space.clone(), a1.clone(), b"m1".to_vec(), None)
            .await
            .unwrap();
        let s = std::time::Instant::now();
        let r2 = match p2p2
            .rpc_single(space.clone(), a2, b"m2".to_vec(), None)
            .await
        {
            Err(_) => {
                panic!("TIMEOUT AFTER: {} ms", s.elapsed().as_millis());
            }
            Ok(r) => r,
        };
        assert_eq!(b"echo: m1".to_vec(), r1);
        assert_eq!(b"echo: m2".to_vec(), r2);
        harness.ghost_actor_shutdown().await.unwrap();
        crate::types::metrics::print_all_metrics();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_peer_info_store() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, evt) = spawn_test_harness_mem().await?;
        let mut recv = evt.receive();

        harness.add_space().await?;
        let (agent, _p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        harness.ghost_actor_shutdown().await?;

        let mut agent_info_signed = None;

        use tokio_stream::StreamExt;
        while let Some(item) = recv.next().await {
            if let HarnessEventType::StoreAgentInfo { agent, .. } = item.ty {
                agent_info_signed = Some((agent,));
            }
        }

        if let Some(i) = agent_info_signed {
            assert_eq!(i.0, Slug::from(agent));
            return Ok(());
        }

        panic!("Failed to receive agent_info_signed")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_transport_binding() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        // Create a p2p config with a local proxy that rejects proxying anyone else
        // and binds to `kitsune-quic://0.0.0.0:0`.
        // This allows the OS to assign an interface / port.
        harness.add_space().await?;
        let (_, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        // List the bindings and assert that we have one binding that is a
        // kitsune-proxy scheme with a kitsune-quic url.
        let bindings = p2p.list_transport_bindings().await?;
        tracing::warn!("BINDINGS: {:?}", bindings);
        assert_eq!(1, bindings.len());
        let binding = &bindings[0];
        assert_eq!("kitsune-proxy", binding.scheme());
        assert_eq!(
            "kitsune-quic",
            binding.path_segments().unwrap().next().unwrap()
        );

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;
        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone(), None).await?;

        let res = p2p.rpc_single(space, a1, b"hello".to_vec(), None).await?;
        assert_eq!(b"echo: hello".to_vec(), res);

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_multi_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone(), None).await?;
        let a3: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a3.clone(), None).await?;

        let mut input = actor::RpcMulti::new(
            &Default::default(),
            space,
            TestVal::test_val(),
            b"test-multi-request".to_vec(),
        );
        input.max_remote_agent_count = 2;
        input.max_timeout = kitsune_p2p_types::KitsuneTimeout::from_millis(2000);
        let res = p2p.rpc_multi(input).await.unwrap();

        harness.ghost_actor_shutdown().await?;

        assert_eq!(1, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a1 || r.agent == a2 || r.agent == a3);
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_single_agent_multi_request_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        let mut input = actor::RpcMulti::new(
            &Default::default(),
            space,
            TestVal::test_val(),
            b"test-multi-request".to_vec(),
        );
        input.max_remote_agent_count = 1;
        input.max_timeout = kitsune_p2p_types::KitsuneTimeout::from_millis(2000);
        let res = p2p.rpc_multi(input).await.unwrap();

        assert_eq!(1, res.len());
        for r in res {
            let data = String::from_utf8_lossy(&r.response);
            assert_eq!("echo: test-multi-request", &data);
            assert!(r.agent == a1);
        }

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_gossip_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;
        // TODO when networking works, just add_*_agent again...
        // but for now, we need the two agents to be on the same node:
        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone(), None).await?;

        let op1 = harness
            .inject_gossip_data(a1.clone(), "agent-1-data".to_string())
            .await?;

        // TODO - This doesn't work on fake nodes
        //        we need to actually add_*_agent to do this
        //let op2 = harness.inject_gossip_data(a2, "agent-2-data".to_string()).await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let res = harness.dump_local_gossip_data(a1).await?;
        let (op_hash, data) = res.into_iter().next().unwrap();
        assert_eq!(op1, op_hash);
        assert_eq!("agent-1-data", &data);

        // TODO - This doesn't work on fake nodes
        //        we need to actually add_*_agent to do this
        //let res = harness.dump_local_gossip_data(a2).await?;
        //let (op_hash, data) = res.into_iter().next().unwrap();
        //assert_eq!(op2, op_hash);
        //assert_eq!("agent-2-data", &data);

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_peer_data_workflow() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_quic().await?;

        let space = harness.add_space().await?;
        let (a1, p2p) = harness.add_direct_agent("DIRECT".into()).await?;

        let res = harness.dump_local_peer_data(a1.clone()).await?;
        let num_agent_info = res.len();
        let (agent_hash, _agent_info) = res.into_iter().next().unwrap();
        assert_eq!(a1, agent_hash);
        assert_eq!(num_agent_info, 1);

        let a2: Arc<KitsuneAgent> = TestVal::test_val();
        p2p.join(space.clone(), a2.clone(), None).await?;

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let res = harness.dump_local_peer_data(a1.clone()).await?;
        let num_agent_info = res.len();

        assert!(res.contains_key(&a1));
        assert!(res.contains_key(&a2));
        assert_eq!(num_agent_info, 2);

        harness.ghost_actor_shutdown().await.unwrap();
        Ok(())
    }

    /// Test that we can gossip across a in memory transport layer.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "(david.b) these tests are becoming irrelevant, worth it to maintain?"]
    async fn test_gossip_transport() -> Result<(), KitsuneP2pError> {
        observability::test_run().ok();
        let (harness, _evt) = spawn_test_harness_mem().await?;

        harness.add_space().await?;

        // - Add the first agent
        let (a1, _) = harness.add_direct_agent("one".into()).await?;

        // - Insert some data for agent 1
        let op1 = harness
            .inject_gossip_data(a1.clone(), "agent-1-data".to_string())
            .await?;

        // - Check agent one has the data
        let res = harness.dump_local_gossip_data(a1.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        assert_eq!(num_gossip, 1);

        // - Add the second agent
        let (a2, _) = harness.add_direct_agent("two".into()).await?;

        // - Insert some data for agent 2
        let op2 = harness
            .inject_gossip_data(a2.clone(), "agent-2-data".to_string())
            .await?;

        // - Check agent two only has this data
        let res = harness.dump_local_gossip_data(a2.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 1);

        // TODO: remove when we have bootstrapping for tests
        // needed until we have some way of bootstrapping
        harness.magic_peer_info_exchange().await?;

        // TODO - a better way to await gossip??
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        // - Check agent one now has all the data
        let res = harness.dump_local_gossip_data(a1.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 2);

        // - Check agent two now has all the data
        let res = harness.dump_local_gossip_data(a2.clone()).await?;
        let num_gossip = res.len();
        let data = res.get(&op1);
        assert_eq!(Some(&"agent-1-data".to_string()), data);
        let data = res.get(&op2);
        assert_eq!(Some(&"agent-2-data".to_string()), data);
        assert_eq!(num_gossip, 2);

        harness.ghost_actor_shutdown().await?;
        Ok(())
    }

    /// Test that we can publish agent info.
    #[tokio::test(flavor = "multi_thread")]
    // @freesig Can anyone think of a better way to do this?
    #[ignore = "Need a better way then waiting 6 minutes to test this"]
    async fn test_publish_agent_info() {
        observability::test_run().ok();

        let (harness, _evt) = spawn_test_harness_mem().await.unwrap();

        harness.add_space().await.unwrap();

        // - Add the first agent
        let (a1, _) = harness.add_publish_only_agent("one".into()).await.unwrap();

        let peer_data = harness
            .dump_local_peer_data(dbg!(a1.clone()))
            .await
            .unwrap();
        let a1_peer_info = peer_data[&a1].clone();
        // - Add the second agent
        let (a2, _) = harness.add_publish_only_agent("two".into()).await.unwrap();

        // - Add the second agent
        let (a3, _) = harness
            .add_publish_only_agent("three".into())
            .await
            .unwrap();

        harness
            .inject_peer_info(a3.clone(), a1_peer_info.clone())
            .await
            .unwrap();

        // There's no way to trigger a publishing of peer data without waiting
        // for > five minutes.
        tokio::time::sleep(std::time::Duration::from_secs(6 * 60)).await;

        let a1_peers = harness.dump_local_peer_data(a1.clone()).await.unwrap();
        let a2_peers = harness.dump_local_peer_data(a2.clone()).await.unwrap();
        let a3_peers = harness.dump_local_peer_data(a3.clone()).await.unwrap();

        // a1 and a2 have each others peer info.
        assert!(a1_peers.get(&a3).is_some());
        assert!(a3_peers.get(&a1).is_some());

        // a2 doesn't have anyone's info and no one has a2's info.
        assert!(a1_peers.get(&a2).is_none());
        assert!(a3_peers.get(&a2).is_none());
        assert!(a2_peers.get(&a1).is_none());
        assert!(a2_peers.get(&a3).is_none());

        harness.ghost_actor_shutdown().await.unwrap();
    }
}

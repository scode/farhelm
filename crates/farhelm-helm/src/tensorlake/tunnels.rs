//! Recipe tunnels share the supervisor connection's lifetime and transport.
//!
//! Both providers use this same listener lease. The supervisor reports a
//! claimed port, never a destination: every dial uses the recipe snapshot
//! retained here, so a sandbox cannot redirect the helm to another service.

use super::Tensorlake;
use crate::{
    client::{SupervisorClient, SupervisorTransportError},
    feed::FleetEvents,
    store::{HostId, HostRow},
};
use farhelm_proto::tensorlake::{TunnelState, TunnelView, recipes::Tunnel};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpStream, sync::watch, task::JoinSet};

#[cfg(test)]
#[path = "tunnel_process_tests.rs"]
mod process_tests;

#[cfg(test)]
#[path = "tunnel_relay_tests.rs"]
mod relay_tests;

type Views = Arc<Mutex<HashMap<HostId, watch::Receiver<TunnelView>>>>;

/// Observe connected leases without retaining their listeners or connection.
/// A recipe edit is picked up by the next lease, never by a retry on this one.
#[derive(Default)]
pub(crate) struct Tunnels {
    views: Views,
}

impl Tunnels {
    /// Lifecycle tests observe readiness and worker termination directly,
    /// without a poll that could outlive the resource being measured.
    #[cfg(test)]
    pub(super) fn subscribe(&self, host: HostId) -> Option<watch::Receiver<TunnelView>> {
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .get(&host)
            .cloned()
    }

    /// Read the current lease's last publication without prolonging its life.
    pub(crate) fn view(&self, host: HostId) -> Option<TunnelView> {
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .get(&host)
            .map(|view| view.borrow().clone())
    }
}

/// The connected phase owns both the worker and the connection that claimed
/// its ports. There is deliberately no wire release: ending that connection
/// closes every listener, pending acceptance and active relay together.
pub(crate) struct TunnelLease {
    client: Arc<SupervisorClient>,
    task: tokio::task::AbortHandle,
    views: Views,
    host: HostId,
    events: Arc<FleetEvents>,
}

impl Drop for TunnelLease {
    fn drop(&mut self) {
        self.task.abort();
        self.client.retire();
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .remove(&self.host);
        self.events.bump();
    }
}

impl Tunnels {
    /// Recipe relays are provider-neutral: they ride the supervisor protocol
    /// and retain the same lease through connection acquisition and serving.
    pub(crate) async fn start(
        &self,
        store: &crate::store::HelmStore,
        host: HostId,
        recipe: &str,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> Option<TunnelLease> {
        self.start_with_fake(store, host, recipe, client, events, String::new(), None)
            .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the marked browser fixture adds sandbox identity to the shared connection lease inputs"
    )]
    async fn start_with_fake(
        &self,
        store: &crate::store::HelmStore,
        host: HostId,
        recipe: &str,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
        sandbox: String,
        fake: Option<Arc<super::e2e::E2eTensorlake>>,
    ) -> Option<TunnelLease> {
        let recipe = store.tensorlake_recipe_row(recipe).await;
        let (definitions, failure) = match recipe {
            Ok(Some((recipe, _))) => (recipe.tunnels, None),
            Ok(None) => (
                vec![],
                Some("This host's recipe no longer exists".to_string()),
            ),
            Err(_) => (
                vec![],
                Some("Farhelm could not read this host's recipe".to_string()),
            ),
        };
        if definitions.is_empty() && failure.is_none() {
            return None;
        }
        let (tx, rx) = watch::channel(TunnelView {
            tunnels: definitions.clone(),
            state: TunnelState::Retrying {
                reason: failure.clone(),
                delay_secs: 0,
            },
        });
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .insert(host, rx);
        let worker_events = events.clone();
        let worker_client = client.clone();
        let task = tokio::spawn(async move {
            let publish = |state| {
                tx.send_modify(|view| view.state = state);
                worker_events.bump();
            };
            if let Some(reason) = failure {
                publish(TunnelState::Failed { reason });
                return;
            }
            let mut delay = 1;
            loop {
                let result = tokio::select! {
                    biased;
                    () = worker_client.closed() => return,
                    result = async {
                        if let Some(fake) = &fake {
                            fake.run_tunnels(&sandbox, &tx, &worker_events).await
                        } else {
                            run_relays(worker_client.clone(), &definitions, &tx, &worker_events).await
                        }
                    } => result,
                };
                let Err(reason) = result else {
                    return;
                };
                publish(TunnelState::Retrying {
                    reason: Some(reason),
                    delay_secs: delay,
                });
                tokio::select! {
                    () = worker_client.closed() => return,
                    () = tokio::time::sleep(Duration::from_secs(delay)) => {},
                }
                delay = (delay * 2).min(30);
            }
        });
        Some(TunnelLease {
            client,
            task: task.abort_handle(),
            views: self.views.clone(),
            host,
            events,
        })
    }
}

impl Tensorlake {
    /// Keep the marked Tensorlake browser fake on the same shared relay
    /// implementation; production relays know only their supervisor client.
    pub(crate) async fn start_tunnels(
        &self,
        row: &HostRow,
        client: Arc<SupervisorClient>,
        events: Arc<FleetEvents>,
    ) -> Option<TunnelLease> {
        let binding = row.tensorlake.as_ref()?;
        self.0
            .tunnels
            .start_with_fake(
                &self.0.store,
                row.id,
                &binding.recipe,
                client,
                events,
                binding.sandbox.clone(),
                self.0.tunnel_fake.clone(),
            )
            .await
    }
}

/// Claim the whole snapshot, then serve bounded accepted connections through
/// the shared relay pump. A failed destination affects that connection alone.
/// Dropping this future aborts its owned dials and relays, including writes
/// parked behind a slow service; no task survives independently of the lease.
/// Returns normally once the connection ends, including retirement after an
/// unanswered or wrong claim reply. A failure on a still-live connection is
/// returned for retry. The failure drain can end this stream before `closed()`
/// fires; reporting that as a tunnel failure would briefly mislabel a lost host.
pub(super) async fn run_relays(
    client: Arc<SupervisorClient>,
    definitions: &[Tunnel],
    tx: &watch::Sender<TunnelView>,
    events: &FleetEvents,
) -> Result<(), String> {
    let ports = definitions
        .iter()
        .map(|tunnel| tunnel.sandbox_port)
        .collect();
    let claim = tokio::time::timeout(
        Duration::from_secs(30),
        client.claim_tcp_tunnel_ports(ports),
    )
    .await;
    let mut accepted = match claim {
        Ok(Ok(accepted)) => accepted,
        Ok(Err(error)) => {
            if matches!(
                error.downcast_ref::<SupervisorTransportError>(),
                Some(
                    SupervisorTransportError::NotSent
                        | SupervisorTransportError::SentUnanswered
                        | SupervisorTransportError::SentWrongReply { .. }
                )
            ) {
                // Wrong replies retire the connection in the claim wrapper;
                // the other two variants already establish transport loss.
                if matches!(
                    error.downcast_ref::<SupervisorTransportError>(),
                    Some(SupervisorTransportError::SentWrongReply { .. })
                ) {
                    tracing::warn!(%error, "recipe tunnel claim received a wrong reply; retiring the supervisor connection");
                }
                client.closed().await;
                return Ok(());
            }
            return Err(super::shell::shown(&format!("{error:#}")));
        }
        Err(_) => {
            // An unanswered claim is ambiguous: only ending the connection
            // can release ports the peer may already have bound. Explicit
            // retirement also covers a future that never received its first poll.
            tracing::warn!(
                "recipe tunnel claim was not confirmed within 30 seconds; retiring the supervisor connection"
            );
            client.retire();
            client.closed().await;
            return Ok(());
        }
    };
    tx.send_modify(|view| view.state = TunnelState::Up);
    events.bump();
    let mut connections = JoinSet::new();
    loop {
        let event = tokio::select! {
            biased;
            () = client.closed() => return Ok(()),
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    tracing::warn!(%error, "recipe tunnel task ended unexpectedly");
                }
                continue;
            },
            event = accepted.recv() => match event {
                Some(event) => event,
                None => {
                    client.closed().await;
                    return Ok(());
                }
            },
        };
        let Some(destination) = definitions
            .iter()
            .find(|tunnel| tunnel.sandbox_port == event.port())
        else {
            // Even a full-authority remote peer supplies no reach beyond the
            // helm's snapshot. Refuse unknown ports without making any dial.
            // Drop owns one bounded refusal and keeps admission until it is
            // queued. An explicit refusal here would leave that fallback
            // armed and send twice, while also parking the accept loop.
            drop(event);
            continue;
        };
        let destination = destination.clone();
        let client = client.clone();
        connections.spawn(async move {
            let work = async {
                let socket = tokio::time::timeout(
                    Duration::from_secs(10),
                    TcpStream::connect((destination_host(&destination.host), destination.port)),
                )
                .await;
                match socket {
                    Ok(Ok(socket)) => client.relay_accepted_tcp(event, socket).await,
                    // An unused event drops into its single refusal path.
                    // Keep destination failures readable without logging a
                    // nested Result or introducing another cleanup task.
                    Ok(Err(error)) => Err(anyhow::anyhow!(
                        "recipe tunnel destination {}:{} could not be reached: {error}",
                        destination.host,
                        destination.port,
                    )),
                    Err(_) => Err(anyhow::anyhow!(
                        "recipe tunnel destination {}:{} timed out after 10 seconds",
                        destination.host,
                        destination.port,
                    )),
                }
            };
            let result = tokio::select! {
                () = client.closed() => return,
                result = work => result,
            };
            if let Err(error) = result {
                tracing::debug!(%error, "recipe tunnel connection ended");
            }
        });
    }
}

/// Recipe syntax keeps IPv6 brackets for display; socket address resolution
/// takes the literal itself. Host names and ordinary IPv4 pass through intact.
fn destination_host(host: &str) -> &str {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dial syntax must not turn a bracketed IPv6 recipe into a DNS name,
    /// while retaining the configured host for every other destination.
    #[test]
    fn destination_uses_the_recipe_host_without_ipv6_display_brackets() {
        assert_eq!(destination_host("[::1]"), "::1");
        assert_eq!(destination_host("127.0.0.1"), "127.0.0.1");
        assert_eq!(destination_host("inference.example"), "inference.example");
    }

    /// Keep the scripted connection alive independently of the fake tunnel
    /// worker, so a status assertion cannot pass because transport died.
    struct PeerTask(tokio::task::JoinHandle<()>);
    impl Drop for PeerTask {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    /// The marked vendor fake owns retry outcomes; this peer supplies only
    /// the actor's real client lifetime and performs no network operation.
    async fn connected_client() -> (Arc<SupervisorClient>, PeerTask) {
        let (client, peer) = tokio::io::duplex(64 * 1024);
        let peer = PeerTask(tokio::spawn(async move {
            let (read, write) = tokio::io::split(peer);
            let mut reader = farhelm_proto::io::FrameReader::new(read);
            let mut writer = farhelm_proto::io::FrameWriter::new(write);
            farhelm_proto::io::handshake(&mut reader, &mut writer, "supervisor")
                .await
                .unwrap();
            std::future::pending::<()>().await;
        }));
        let (read, write) = tokio::io::split(client);
        let client =
            tokio::time::timeout(Duration::from_secs(5), SupervisorClient::start(read, write))
                .await
                .unwrap()
                .unwrap();
        (client, peer)
    }

    /// The marked fake pins retry status, the view's connection snapshot and
    /// next-lease recipe reads. The real-worker tests separately observe
    /// claimed ports and destination dials across an edit during a retry;
    /// unchanged view metadata alone cannot establish that wire behavior.
    #[farhelm_testtrace::test]
    async fn retries_recover_with_the_original_recipe_until_the_next_connection() {
        use crate::tensorlake::e2e::E2eTensorlake;
        use farhelm_proto::tensorlake::recipes::{RecipeBase, RecipeSave, RecipeSize};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fake");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("ENABLED"), "farhelm-e2e-tensorlake-v1\n").unwrap();
        std::fs::write(
            root.join("config.json"),
            r#"{"fail":{"tunnels":{"status":500,"message":"listener refused"}}}"#,
        )
        .unwrap();
        let fake = Arc::new(E2eTensorlake::new(root.clone(), dir.path()).unwrap());
        let store = crate::store::HelmStore::open(&dir.path().join("helm.db"))
            .await
            .unwrap();
        let mut recipe = RecipeSave {
            size: RecipeSize {
                cpus: 1,
                memory_gb: 1,
                disk_gb: 10,
            },
            packages: vec![],
            base: RecipeBase::TensorlakeImage {
                image: "tensorlake/ubuntu-systemd".into(),
            },
            claude: None,
            codex: None,
            github_token_credential: None,
            claude_login_credential: None,
            codex_login_credential: None,
            tunnels: vec![Tunnel {
                sandbox_port: 8080,
                host: "127.0.0.1".into(),
                port: 8080,
            }],
        };
        store
            .create_tensorlake_recipe("inference".into(), recipe.clone())
            .await
            .unwrap()
            .unwrap();
        let host = crate::tensorlake::hosts::test_support::plant(
            &store,
            "dev",
            "farhelm-host-k7q2-test-abcd",
            "inference",
            None,
        )
        .await;
        let row = store
            .list_hosts()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == host)
            .unwrap();
        let service = Tensorlake(Arc::new(super::super::Inner::new(
            store.clone(),
            fake.clone(),
            fake.clone(),
            None,
            Some(fake),
            crate::provisioning::credential_delivery::CredentialDelivery::detached(store.clone()),
            Arc::new(crate::tensorlake::prepared::UnavailablePreparer),
        )));
        let events = Arc::new(FleetEvents::new());
        let (client, _peer) = connected_client().await;
        let lease = service
            .start_tunnels(&row, client, events.clone())
            .await
            .unwrap();
        let mut view = service.0.tunnels.views.lock().unwrap()[&host].clone();
        tokio::time::timeout(Duration::from_secs(5), view.wait_for(|view| matches!(&view.state, TunnelState::Retrying { reason: Some(reason), .. } if reason == "listener refused"))).await.unwrap().unwrap();
        recipe.tunnels[0].port = 9090;
        store
            .update_tensorlake_recipe("inference".into(), recipe.clone())
            .await
            .unwrap()
            .unwrap();
        std::fs::write(root.join("config.json"), "{}").unwrap();
        tokio::time::timeout(
            Duration::from_secs(5),
            view.wait_for(|view| view.state == TunnelState::Up),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            service.tunnel_view(host).unwrap().tunnels[0].port,
            8080,
            "retry keeps the connection's original recipe"
        );
        drop(lease);
        assert!(service.tunnel_view(host).is_none());
        let (client, _peer_next) = connected_client().await;
        let next = service.start_tunnels(&row, client, events).await.unwrap();
        assert_eq!(
            service.tunnel_view(host).unwrap().tunnels[0].port,
            9090,
            "next connection reads the edit"
        );
        drop(next);
        assert!(service.tunnel_view(host).is_none());
    }
}

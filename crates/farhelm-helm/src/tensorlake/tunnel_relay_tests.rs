//! Recipe selection and concurrent sockets through the production tunnel worker.
//!
//! The transport tests cover individual relay pumps. These tests retain the
//! recipe layer too: a peer's port selects only a configured destination, and
//! several independently half-closed connections continue through one claim.

use super::{FleetEvents, SupervisorClient, run_relays};
use farhelm_proto::{
    ControlMsg, FrameKind,
    io::{FrameReader, FrameWriter, handshake},
    tensorlake::{TunnelState, TunnelView, recipes::Tunnel},
};
use farhelm_supervisor::service::{Supervisor, handle_connection};
use std::{net::Ipv4Addr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
    time::timeout,
};

const LIMIT: Duration = Duration::from_secs(10);

/// The assertion runs separately from these owners. Even a panic disconnects
/// and reaps the real connection and recipe worker before their state goes.
#[derive(Default)]
struct RelayTasks(JoinSet<anyhow::Result<()>>);

impl Drop for RelayTasks {
    fn drop(&mut self) {
        self.0.abort_all();
    }
}

impl RelayTasks {
    /// Retirement releases the worker's claim and makes both tasks finish.
    /// A cleanup timeout still aborts and reaps every owned task; the timeout
    /// remains a failure rather than being hidden by that fallback.
    async fn finish(&mut self, client: &SupervisorClient) -> Result<(), String> {
        client.retire();
        let finished = timeout(LIMIT, async {
            let mut panic = None;
            while let Some(joined) = self.0.join_next().await {
                // EOF may race an outstanding protocol write. The connection
                // may report either result, but an owned task must not panic.
                if let Err(error) = joined {
                    panic.get_or_insert_with(|| error.to_string());
                }
            }
            panic.map_or(Ok(()), Err)
        })
        .await;
        match finished {
            Ok(result) => result,
            Err(_) => {
                self.0.abort_all();
                while self.0.join_next().await.is_some() {}
                Err("recipe fixture did not finish after connection retirement".into())
            }
        }
    }
}

/// Preserve the assertion's original panic after cleanup, and retain any
/// secondary cleanup problem in its diagnostics instead of detaching tasks.
fn assert_observation(observed: Result<(), tokio::task::JoinError>, cleaned: Result<(), String>) {
    if let Err(error) = observed {
        if let Err(cleanup) = cleaned {
            eprintln!("recipe fixture cleanup also failed: {cleanup}");
        }
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        panic!("recipe observation cancelled: {error}");
    }
    cleaned.expect("recipe fixture cleanup must finish");
}

/// Keep the production worker's view and lifetime visible without adding an
/// alternate recipe runtime. Only Up establishes that the real claim passed.
fn start_worker(
    tasks: &mut RelayTasks,
    client: Arc<SupervisorClient>,
    definitions: Vec<Tunnel>,
) -> watch::Receiver<TunnelView> {
    let (tx, rx) = watch::channel(TunnelView {
        tunnels: definitions.clone(),
        state: TunnelState::Retrying {
            reason: None,
            delay_secs: 0,
        },
    });
    tasks.0.spawn(async move {
        let result = run_relays(client, &definitions, &tx, &FleetEvents::new()).await;
        if let Err(reason) = &result {
            // A released port can be taken before the production bind. Keep
            // that actual refusal visible instead of hiding it in an Up wait.
            tx.send_modify(|view| {
                view.state = TunnelState::Failed {
                    reason: reason.clone(),
                }
            });
        }
        result.map_err(anyhow::Error::msg)
    });
    rx
}

/// Up is the bind oracle. A refused claim must retain its actual reason,
/// including a fixture port collision; an expired wait reports the last view.
async fn wait_for_up(view: &mut watch::Receiver<TunnelView>) {
    let observed = timeout(LIMIT, async {
        view.wait_for(|view| matches!(view.state, TunnelState::Up | TunnelState::Failed { .. }))
            .await
            .map(|view| view.state.clone())
    })
    .await;
    assert!(
        matches!(&observed, Ok(Ok(TunnelState::Up))),
        "production recipe claim did not become Up: result={observed:?}, last view={:?}",
        view.borrow().clone()
    );
}

/// A distinct live roundtrip precedes the large transfer. Two mappings and
/// four held connections catch routing or worker serialization that an
/// isolated relay-pump test cannot exercise.
#[farhelm_testtrace::test]
async fn several_recipe_connections_cross_credit_windows_and_half_close_independently() {
    let state = tempfile::tempdir().unwrap();
    let supervisor = Supervisor::new(state.path()).await.unwrap();
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let mut tasks = RelayTasks::default();
    tasks.0.spawn(handle_connection(supervisor, server_side));
    let (read, write) = tokio::io::split(client_side);
    let client = timeout(LIMIT, SupervisorClient::start(read, write))
        .await
        .expect("real supervisor hello must finish")
        .unwrap();
    let destinations = [
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap(),
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap(),
    ];
    // Hold both reservations together so the kernel cannot choose one port
    // twice. Up, rather than these released reservations, proves ownership.
    let reservations = [
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap(),
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap(),
    ];
    let definitions: Vec<_> = reservations
        .iter()
        .zip(&destinations)
        .map(|(reservation, destination)| Tunnel {
            sandbox_port: reservation.local_addr().unwrap().port(),
            host: "127.0.0.1".into(),
            port: destination.local_addr().unwrap().port(),
        })
        .collect();
    let ports: Vec<_> = definitions.iter().map(|t| t.sandbox_port).collect();
    drop(reservations);
    let mut view = start_worker(&mut tasks, Arc::clone(&client), definitions);
    let observed = tokio::spawn(async move {
        wait_for_up(&mut view).await;
        let mut peers = Vec::new();
        for index in 0..4 {
            let mapping = index % destinations.len();
            let mut sandbox = timeout(
                LIMIT,
                TcpStream::connect((Ipv4Addr::LOCALHOST, ports[mapping])),
            )
            .await
            .expect("claimed recipe port must accept")
            .unwrap();
            let (mut destination, _) = timeout(LIMIT, destinations[mapping].accept())
                .await
                .expect("configured destination must be dialed")
                .unwrap();
            let marker = format!("recipe-live:{index}:{}", uuid::Uuid::new_v4()).into_bytes();
            timeout(LIMIT, async {
                sandbox.write_all(&marker).await.unwrap();
                let mut actual = vec![0; marker.len()];
                destination.read_exact(&mut actual).await.unwrap();
                assert_eq!(actual, marker);
                destination.write_all(&marker).await.unwrap();
                sandbox.read_exact(&mut actual).await.unwrap();
                assert_eq!(actual, marker);
            })
            .await
            .expect("each held recipe connection must be live before half-close");
            peers.push((sandbox, destination));
        }
        let mut exchanges = JoinSet::new();
        for (index, (sandbox, destination)) in peers.into_iter().enumerate() {
            exchanges.spawn(exchange_then_half_close(sandbox, destination, index));
        }
        // Observe every task before propagating a panic, so a failed exchange
        // cannot leave another connection's socket task detached.
        let mut failure = None;
        while let Some(joined) = exchanges.join_next().await {
            if let Err(error) = joined {
                failure.get_or_insert_with(|| error.to_string());
                exchanges.abort_all();
            }
        }
        assert!(failure.is_none(), "recipe exchange failed: {failure:?}");
        assert_eq!(view.borrow().state, TunnelState::Up);
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    assert_observation(observed, cleaned);
}

/// The request ends before its response starts, in alternating directions.
/// Both payloads exceed protocol credit, so a single buffered window or a
/// whole-stream close on the first EOF cannot satisfy the comparisons.
async fn exchange_then_half_close(
    mut sandbox: TcpStream,
    mut destination: TcpStream,
    index: usize,
) {
    timeout(LIMIT, async {
        let (first, second) = if index.is_multiple_of(2) {
            (&mut sandbox, &mut destination)
        } else {
            (&mut destination, &mut sandbox)
        };
        let size = farhelm_proto::UPLOAD_WINDOW_BYTES as usize + 32_781;
        let request: Vec<_> = (0..size).map(|i| ((i + 17 * index) % 251) as u8).collect();
        let response: Vec<_> = (0..size)
            .map(|i| (250 - (i + 29 * index) % 251) as u8)
            .collect();
        let mut actual = vec![0; size];
        tokio::try_join!(first.write_all(&request), second.read_exact(&mut actual)).unwrap();
        assert_eq!(actual, request);
        first.shutdown().await.unwrap();
        assert_eq!(second.read(&mut [0]).await.unwrap(), 0);
        tokio::try_join!(second.write_all(&response), first.read_exact(&mut actual)).unwrap();
        assert_eq!(actual, response);
        second.shutdown().await.unwrap();
        assert_eq!(first.read(&mut [0]).await.unwrap(), 0);
    })
    .await
    .expect("concurrent recipe connection must transfer both windows and EOFs");
}

/// Only a wire event names the unknown port. Refusal must precede any target
/// dial, and a later configured event must still reach the configured service
/// on that same supervisor connection.
#[farhelm_testtrace::test]
async fn an_unknown_accepted_port_is_refused_without_dialing_the_recipe_destination() {
    let destination = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    destination.set_nonblocking(true).unwrap();
    let reservation = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let unknown_port = if port == u16::MAX { port - 1 } else { port + 1 };
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let connecting = async {
        let (read, write) = tokio::io::split(client_side);
        SupervisorClient::start(read, write).await.unwrap()
    };
    let greeting = async {
        let (read, write) = tokio::io::split(server_side);
        let mut reader = FrameReader::new(read);
        let mut writer = FrameWriter::new(write);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        (reader, writer)
    };
    let (client, (mut reader, mut writer)) =
        timeout(LIMIT, async { tokio::join!(connecting, greeting) })
            .await
            .expect("scripted supervisor hello must finish");
    let mut tasks = RelayTasks::default();
    let mut view = start_worker(
        &mut tasks,
        Arc::clone(&client),
        vec![Tunnel {
            sandbox_port: port,
            host: "127.0.0.1".into(),
            port: destination.local_addr().unwrap().port(),
        }],
    );
    let observed = tokio::spawn(async move {
        timeout(LIMIT, async {
            let ControlMsg::ClaimTcpTunnelPorts { req_id, ports } = control(&mut reader).await
            else {
                panic!("expected production worker's claim");
            };
            assert_eq!(ports, vec![port]);
            send(&mut writer, ControlMsg::TcpTunnelPortsClaimed { req_id }).await;
            wait_for_up(&mut view).await;
            send(
                &mut writer,
                ControlMsg::TcpTunnelAccepted {
                    accepted: 31,
                    port: unknown_port,
                },
            )
            .await;
            assert_eq!(
                control(&mut reader).await,
                ControlMsg::RejectTcpTunnel { accepted: 31 }
            );
            assert_eq!(
                destination.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "unknown port must not dial the configured destination",
            );
            send(
                &mut writer,
                ControlMsg::TcpTunnelAccepted { accepted: 32, port },
            )
            .await;
            let ControlMsg::BeginAcceptedTcpRelay {
                req_id,
                accepted: 32,
                channel,
            } = control(&mut reader).await
            else {
                panic!(
                    "the refused event must not generate a second refusal before the next admission"
                );
            };
            send(&mut writer, ControlMsg::TcpRelayStarted { req_id, channel }).await;
            let (socket, _) = destination
                .accept()
                .expect("configured event dialed its destination before admission");
            socket.set_nonblocking(true).unwrap();
            let mut socket = TcpStream::from_std(socket).unwrap();
            let marker = format!("trusted-recipe:{}", uuid::Uuid::new_v4()).into_bytes();
            timeout(LIMIT, socket.write_all(&marker))
                .await
                .unwrap()
                .unwrap();
            let mut actual = Vec::new();
            while actual.len() < marker.len() {
                let frame = timeout(LIMIT, reader.read_frame())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert_eq!(frame.kind, FrameKind::Data);
                assert_eq!(frame.channel, channel);
                assert!(!frame.body.is_empty() && actual.len() + frame.body.len() <= marker.len());
                actual.extend(frame.body);
            }
            assert_eq!(actual, marker);
            assert_eq!(
                destination.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert_eq!(view.borrow().state, TunnelState::Up);
            drop(reservation);
        })
        .await
        .expect("scripted recipe selection must finish within one shared deadline");
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    assert_observation(observed, cleaned);
}

/// A stored recipe and host row with the ordinary offline test dependencies.
/// The tunnel fake is absent: start_tunnels must execute its real worker.
struct RecipeFixture {
    store: crate::store::HelmStore,
    service: crate::tensorlake::Tensorlake,
    row: crate::store::HostRow,
    recipe: farhelm_proto::tensorlake::recipes::RecipeSave,
    // Fields drop in declaration order: keep the directory until the store
    // and the service's final references have been released.
    state: tempfile::TempDir,
}

impl RecipeFixture {
    /// Keep all state outside the assertion task, so a panic cannot remove
    /// the database while the independently owned connection is cleaning up.
    async fn new(tunnels: Vec<Tunnel>) -> Self {
        use farhelm_proto::tensorlake::recipes::{RecipeBase, RecipeSave, RecipeSize};
        let state = tempfile::tempdir().unwrap();
        let store = crate::store::HelmStore::open(&state.path().join("helm.db"))
            .await
            .unwrap();
        let recipe = RecipeSave {
            size: RecipeSize {
                cpus: 2,
                memory_gb: 4,
                disk_gb: 20,
            },
            packages: vec![],
            base: RecipeBase::TensorlakeImage {
                image: "tensorlake/ubuntu-systemd".into(),
            },
            claude: None,
            codex: None,
            claude_login_credential: None,
            codex_login_credential: None,
            github_token_credential: None,
            tunnels,
        };
        store
            .create_tensorlake_recipe("relay".into(), recipe.clone())
            .await
            .unwrap()
            .unwrap();
        let host = crate::tensorlake::hosts::test_support::plant(
            &store,
            "relay",
            "farhelm-host-k7q2-relay-test",
            "relay",
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
        let service = crate::tensorlake::Tensorlake::detached(store.clone());
        assert!(
            service.0.tunnel_fake.is_none(),
            "the production worker must run"
        );
        Self {
            state,
            store,
            service,
            row,
            recipe,
        }
    }
}

/// Dropping a connected lease must release its claim even while another
/// owner keeps the client alive. Aborting the worker alone would close the
/// held relay but leave the listener behind, so all three oracles matter.
#[farhelm_testtrace::test]
async fn dropping_a_real_lease_closes_its_claim_and_held_connection() {
    let destination = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let reservation = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let fixture = RecipeFixture::new(vec![Tunnel {
        sandbox_port: port,
        host: "127.0.0.1".into(),
        port: destination.local_addr().unwrap().port(),
    }])
    .await;
    let supervisor = Supervisor::new(&fixture.state.path().join("supervisor"))
        .await
        .unwrap();
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let mut tasks = RelayTasks::default();
    tasks.0.spawn(handle_connection(supervisor, server_side));
    let (read, write) = tokio::io::split(client_side);
    let client = timeout(LIMIT, SupervisorClient::start(read, write))
        .await
        .unwrap()
        .unwrap();
    drop(reservation);
    let observed = tokio::spawn({
        let service = fixture.service.clone();
        let row = fixture.row.clone();
        let client = Arc::clone(&client);
        async move {
            let lease = timeout(
                LIMIT,
                service.start_tunnels(&row, Arc::clone(&client), Arc::new(FleetEvents::new())),
            )
            .await
            .expect("recipe acquisition must finish")
            .expect("stored recipe must acquire a lease");
            let worker = lease.task.clone();
            let mut view = service.0.tunnels.subscribe(row.id).expect("lease view");
            wait_for_up(&mut view).await;
            let mut sandbox = timeout(LIMIT, TcpStream::connect((Ipv4Addr::LOCALHOST, port)))
                .await
                .unwrap()
                .unwrap();
            let (mut destination, _) = timeout(LIMIT, destination.accept()).await.unwrap().unwrap();
            let marker = format!("lease-live:{}", uuid::Uuid::new_v4()).into_bytes();
            timeout(LIMIT, async {
                sandbox.write_all(&marker).await.unwrap();
                let mut actual = vec![0; marker.len()];
                destination.read_exact(&mut actual).await.unwrap();
                assert_eq!(actual, marker);
                destination.write_all(&marker).await.unwrap();
                sandbox.read_exact(&mut actual).await.unwrap();
                assert_eq!(actual, marker);
            })
            .await
            .expect("both held ends must be live before lease drop");
            assert!(!worker.is_finished(), "worker remains live before drop");
            assert_eq!(
                sandbox.try_read(&mut [0]).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            assert_eq!(
                destination.try_read(&mut [0]).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            drop(lease);
            assert!(
                service.tunnel_view(row.id).is_none(),
                "drop withdraws the view"
            );
            // The retained client is deliberately not retired by the fixture
            // until after this observation, so only the real lease can pass.
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            tokio::time::timeout_at(deadline, async {
                tokio::join!(
                    async {
                        assert!(
                            super::process_tests::listener_refused(
                                (Ipv4Addr::LOCALHOST, port).into(),
                                deadline
                            )
                            .await,
                            "listener must eventually refuse"
                        );
                    },
                    peer_closed(&mut sandbox),
                    peer_closed(&mut destination),
                    client.closed(),
                    worker_finished(&mut view),
                );
            })
            .await
            .expect("lease teardown exceeded the shared deadline");
        }
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    assert_observation(observed, cleaned);
}

/// A recipe edit committed before a refused claim retries cannot change that
/// connection's ports or destination. The peer holds its first reply while
/// the edit commits, making the ordering independent of the backoff duration.
#[farhelm_testtrace::test]
async fn a_real_retry_keeps_the_original_ports_and_destination_after_a_recipe_edit() {
    let original = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let edited = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    original.set_nonblocking(true).unwrap();
    edited.set_nonblocking(true).unwrap();
    let reservations = [
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap(),
        std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap(),
    ];
    let port = reservations[0].local_addr().unwrap().port();
    let edited_port = reservations[1].local_addr().unwrap().port();
    let fixture = RecipeFixture::new(vec![Tunnel {
        sandbox_port: port,
        host: "127.0.0.1".into(),
        port: original.local_addr().unwrap().port(),
    }])
    .await;
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let connecting = async {
        let (read, write) = tokio::io::split(client_side);
        SupervisorClient::start(read, write).await.unwrap()
    };
    let greeting = async {
        let (read, write) = tokio::io::split(server_side);
        let mut reader = FrameReader::new(read);
        let mut writer = FrameWriter::new(write);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        (reader, writer)
    };
    let (client, (mut reader, mut writer)) =
        timeout(LIMIT, async { tokio::join!(connecting, greeting) })
            .await
            .expect("scripted supervisor hello must finish");
    let mut tasks = RelayTasks::default();
    let observed = tokio::spawn({
        let client = Arc::clone(&client);
        let service = fixture.service.clone();
        let row = fixture.row.clone();
        let store = fixture.store.clone();
        let mut recipe = fixture.recipe.clone();
        async move {
            let lease = timeout(
                LIMIT,
                service.start_tunnels(&row, client, Arc::new(FleetEvents::new())),
            )
            .await
            .expect("stored recipe acquisition must finish")
            .expect("stored recipe must acquire a lease");
            let worker = lease.task.clone();
            let mut view = service.0.tunnels.subscribe(row.id).expect("lease view");
            let ControlMsg::ClaimTcpTunnelPorts { req_id, ports } = control(&mut reader).await
            else {
                panic!("expected original claim");
            };
            assert_eq!(ports, vec![port]);
            recipe.tunnels[0].sandbox_port = edited_port;
            recipe.tunnels[0].port = edited.local_addr().unwrap().port();
            store
                .update_tensorlake_recipe("relay".into(), recipe)
                .await
                .unwrap()
                .unwrap();
            assert!(
                !worker.is_finished(),
                "the first claim remains held while the edit commits"
            );
            send(
                &mut writer,
                ControlMsg::Error {
                    req_id,
                    kind: farhelm_proto::ErrorKind::Conflict,
                    message: "held fixture claim refused".into(),
                    create_unaccepted: false,
                },
            )
            .await;
            let ControlMsg::ClaimTcpTunnelPorts { req_id, ports } = control(&mut reader).await
            else {
                panic!("expected retry after the refused claim");
            };
            assert_eq!(
                ports,
                vec![port],
                "retry must keep the snapshot taken before the edit"
            );
            send(&mut writer, ControlMsg::TcpTunnelPortsClaimed { req_id }).await;
            wait_for_up(&mut view).await;
            send(
                &mut writer,
                ControlMsg::TcpTunnelAccepted { accepted: 81, port },
            )
            .await;
            let ControlMsg::BeginAcceptedTcpRelay {
                req_id,
                accepted: 81,
                channel,
            } = control(&mut reader).await
            else {
                panic!("configured acceptance must be admitted");
            };
            let (socket, _) = original
                .accept()
                .expect("retry must dial the original destination before admission");
            assert_eq!(
                edited.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "the edited destination must remain untouched on this connection"
            );
            socket.set_nonblocking(true).unwrap();
            let mut socket = TcpStream::from_std(socket).unwrap();
            send(&mut writer, ControlMsg::TcpRelayStarted { req_id, channel }).await;
            let marker = format!("original-recipe:{}", uuid::Uuid::new_v4()).into_bytes();
            timeout(LIMIT, socket.write_all(&marker))
                .await
                .unwrap()
                .unwrap();
            let mut actual = Vec::new();
            while actual.len() < marker.len() {
                let frame = timeout(LIMIT, reader.read_frame())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert_eq!(frame.kind, FrameKind::Data);
                assert_eq!(frame.channel, channel);
                assert!(!frame.body.is_empty() && actual.len() + frame.body.len() <= marker.len());
                actual.extend(frame.body);
            }
            assert_eq!(
                actual, marker,
                "the original destination was live after the retry"
            );
            assert_eq!(
                edited.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
            drop(lease);
            timeout(LIMIT, worker_finished(&mut view))
                .await
                .expect("real retry worker must finish after lease drop");
            drop(reservations);
        }
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    assert_observation(observed, cleaned);
}

/// Exercise peer EOF through a real lease, keeping it alive until its worker
/// ends so lease cancellation cannot satisfy the transport-loss oracle.
/// This reaches the ended-event-stream arm only when scheduling falls inside
/// the failure drain's window. The closed-client test below independently pins
/// transport-error classification without relying on that scheduling window.
#[farhelm_testtrace::test]
async fn peer_eof_ends_a_real_lease_without_tunnel_recovery() {
    // The scripted peer binds nothing; this only supplies a valid nonzero port.
    let reservation = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let fixture = RecipeFixture::new(vec![Tunnel {
        sandbox_port: port,
        host: "127.0.0.1".into(),
        port,
    }])
    .await;
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let connecting = async {
        let (read, write) = tokio::io::split(client_side);
        SupervisorClient::start(read, write).await.unwrap()
    };
    let greeting = async {
        let (read, write) = tokio::io::split(server_side);
        let mut reader = FrameReader::new(read);
        let mut writer = FrameWriter::new(write);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        (reader, writer)
    };
    let (client, (mut reader, mut writer)) =
        timeout(LIMIT, async { tokio::join!(connecting, greeting) })
            .await
            .expect("scripted supervisor hello must finish");
    // No supervisor task runs here. The shared cleanup owner is used only
    // for client retirement, including after an assertion panic.
    let mut tasks = RelayTasks::default();
    let lease = timeout(
        LIMIT,
        fixture.service.start_tunnels(
            &fixture.row,
            Arc::clone(&client),
            Arc::new(FleetEvents::new()),
        ),
    )
    .await
    .expect("recipe acquisition must finish")
    .expect("stored recipe must acquire a lease");
    let mut view = fixture
        .service
        .0
        .tunnels
        .subscribe(fixture.row.id)
        .expect("lease view");
    // This receiver survives an assertion panic, so cleanup still observes
    // the aborted worker's final drop before removing its database.
    let mut completion = view.clone();
    let observed = tokio::spawn({
        let client = Arc::clone(&client);
        async move {
            timeout(LIMIT, async {
                let ControlMsg::ClaimTcpTunnelPorts { req_id, ports } = control(&mut reader).await
                else {
                    panic!("expected production claim");
                };
                assert_eq!(ports, vec![port]);
                send(&mut writer, ControlMsg::TcpTunnelPortsClaimed { req_id }).await;
                wait_for_up(&mut view).await;
                assert!(!lease.task.is_finished(), "worker remains live before EOF");
                let mut states = vec![view.borrow_and_update().state.clone()];
                drop(reader);
                drop(writer);
                tokio::join!(
                    async {
                        while view.changed().await.is_ok() {
                            states.push(view.borrow_and_update().state.clone());
                            assert!(
                                states.len() <= 16,
                                "unexpected publication flood: {states:?}"
                            );
                        }
                    },
                    client.closed(),
                );
                // Watch may coalesce publications. The final value matters:
                // this worker cannot recover after its connection has ended.
                states.push(view.borrow().state.clone());
                assert!(
                    states.iter().all(|state| matches!(state, TunnelState::Up)),
                    "peer EOF published tunnel recovery: {states:?}"
                );
                drop(lease);
            })
            .await
            .expect("peer EOF must end the worker");
        }
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    let finished = timeout(LIMIT, worker_finished(&mut completion))
        .await
        .map_err(|_| "recipe worker did not finish during cleanup".to_string());
    assert_observation(observed, cleaned.and(finished));
    drop(reservation);
}

/// The outer worker can notice EOF before calling run_relays. Call it directly
/// on an already closed client to pin the recipe-side Closed-to-NotSent path:
/// transport loss returns normally and never publishes either Up or failure.
#[farhelm_testtrace::test]
async fn a_claim_on_a_closed_client_ends_without_publishing() {
    // This port is configuration only: the scripted peer never binds it.
    let reservation = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let (client_side, server_side) = tokio::io::duplex(64 * 1024);
    let connecting = async {
        let (read, write) = tokio::io::split(client_side);
        SupervisorClient::start(read, write).await.unwrap()
    };
    let greeting = async {
        let (read, write) = tokio::io::split(server_side);
        let mut reader = FrameReader::new(read);
        let mut writer = FrameWriter::new(write);
        handshake(&mut reader, &mut writer, "supervisor")
            .await
            .unwrap();
        (reader, writer)
    };
    let (client, (reader, writer)) = timeout(LIMIT, async { tokio::join!(connecting, greeting) })
        .await
        .expect("scripted supervisor hello must finish");
    // Reuse the common retirement cleanup even though there is no supervisor
    // task and the successful observation already establishes client closure.
    let mut tasks = RelayTasks::default();
    let observed = tokio::spawn({
        let client = Arc::clone(&client);
        async move {
            drop(reader);
            drop(writer);
            timeout(LIMIT, client.closed())
                .await
                .expect("peer EOF must close the client");
            let definitions = vec![Tunnel {
                sandbox_port: port,
                host: "127.0.0.1".into(),
                port,
            }];
            let (tx, view) = watch::channel(TunnelView {
                tunnels: definitions.clone(),
                state: TunnelState::Retrying {
                    reason: None,
                    delay_secs: 0,
                },
            });
            let result = timeout(
                LIMIT,
                run_relays(client, &definitions, &tx, &FleetEvents::new()),
            )
            .await
            .expect("closed-client claim must finish");
            assert_eq!(result, Ok(()), "transport loss is normal completion");
            assert_eq!(
                view.borrow().state,
                TunnelState::Retrying {
                    reason: None,
                    delay_secs: 0,
                }
            );
            assert!(
                !view.has_changed().unwrap(),
                "a closed client must not publish tunnel state"
            );
        }
    })
    .await;
    let cleaned = tasks.finish(&client).await;
    assert_observation(observed, cleaned);
    drop(reservation);
}

/// The held socket itself must reach EOF or reset; a withdrawn view alone
/// would not prove that cancellation stopped an already admitted connection.
async fn peer_closed(socket: &mut TcpStream) {
    match socket.read(&mut [0]).await {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            ) => {}
        other => panic!("held recipe peer did not close: {other:?}"),
    }
}

/// Abort is a request, not completion. Observe the production worker's final
/// drop before the fixture can remove its database and owned socket state.
async fn worker_finished(view: &mut watch::Receiver<TunnelView>) {
    while view.changed().await.is_ok() {}
}

/// Correlated controls are the scripted peer's ordering oracle. No arbitrary
/// traffic is skipped, and a missing worker reply fails under a short bound.
async fn control(reader: &mut FrameReader<ReadHalf<DuplexStream>>) -> ControlMsg {
    let frame = timeout(LIMIT, reader.read_frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(frame.kind, FrameKind::Control);
    serde_json::from_slice(&frame.body).unwrap()
}

/// Bound writes to the live production client independently of later cleanup.
async fn send(writer: &mut FrameWriter<WriteHalf<DuplexStream>>, message: ControlMsg) {
    timeout(LIMIT, writer.write_control(&message))
        .await
        .unwrap()
        .unwrap();
}

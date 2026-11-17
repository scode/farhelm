//! Process lifetime proofs for recipe tunnels.
//!
//! The child owns the production helm client and tunnel worker. The parent
//! owns a real supervisor and both TCP peers, so SIGKILL cannot accidentally
//! close a peer that the proof needs to observe after the helm has died.

use super::run_relays;
use crate::{client::SupervisorClient, feed::FleetEvents};
use farhelm_proto::tensorlake::{TunnelState, TunnelView, recipes::Tunnel};
use farhelm_supervisor::service::{Supervisor, handle_connection};
use std::{
    io::ErrorKind, net::SocketAddr, os::unix::process::ExitStatusExt, path::PathBuf,
    process::Stdio, time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::watch,
    time::{Instant, timeout, timeout_at},
};

const CHILD_CONFIG: &str = "FARHELM_TEST_TUNNEL_OWNER_CONFIG";
const TEST: &str =
    "tensorlake::tunnels::process_tests::tunnel_and_listener_end_when_the_owning_helm_exits";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Only the re-executed child receives this configuration. Passing it on
/// `Command` keeps concurrent tests' environment and destinations untouched.
#[derive(serde::Serialize, serde::Deserialize)]
struct ChildConfig {
    socket: PathBuf,
    ready: PathBuf,
    tunnel: Tunnel,
}

/// Kill and reap the exact owned child on every assertion-failure path.
/// Keeping the `Child` unreaped also prevents PID reuse before SIGKILL.
struct HelmChild(std::process::Child);

impl Drop for HelmChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Own the one real supervisor connection, including an accept still waiting
/// for the child. No listener or serving task should outlive a failed proof.
struct SupervisorConnection(tokio::task::JoinHandle<anyhow::Result<()>>);

impl Drop for SupervisorConnection {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Poll observable fixture state under one deadline. The delay only limits
/// polling overhead; readiness and teardown always require their own oracle.
async fn poll_until(deadline: Instant, mut observed: impl FnMut() -> bool) -> bool {
    loop {
        if observed() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        // sleep-ok: bounded polling of this fixture's readiness, child exit or listener refusal.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Run the same client and recipe relay worker as a connected Tensorlake
/// host, directly over a local socket so this proof needs no SSH or cloud.
/// The readiness file means the production worker published a successful claim.
async fn serve_child(config: ChildConfig) {
    let socket = UnixStream::connect(&config.socket).await.unwrap();
    let (reader, writer) = socket.into_split();
    let client = SupervisorClient::start(reader, writer).await.unwrap();
    let definitions = [config.tunnel];
    let (tx, mut rx) = watch::channel(TunnelView {
        tunnels: definitions.to_vec(),
        state: TunnelState::Retrying {
            reason: None,
            delay_secs: 0,
        },
    });
    let events = FleetEvents::new();
    let relays = run_relays(client, &definitions, &tx, &events);
    tokio::pin!(relays);
    tokio::select! {
        result = &mut relays => panic!(
            "recipe relay ended before claim readiness; a port-in-use error identifies \
             a collision with this fixture's released reservation: {result:?}"
        ),
        ready = timeout(STARTUP_TIMEOUT, rx.wait_for(|view| view.state == TunnelState::Up)) => {
            ready.expect("recipe claim readiness timed out").expect("recipe view closed");
        }
    }
    std::fs::write(config.ready, b"claimed").unwrap();
    panic!(
        "recipe relay ended while its owning child was alive: {:?}",
        relays.await
    );
}

/// A held peer must receive EOF or a reset, never unexplained extra bytes.
/// The shared post-kill deadline prevents sequential waits hiding slow teardown.
async fn assert_peer_closed(peer: &mut TcpStream, name: &str, deadline: Instant) {
    let mut byte = [0];
    match timeout_at(deadline, peer.read(&mut byte)).await {
        Ok(Ok(0)) => {}
        Ok(Err(error))
            if matches!(
                error.kind(),
                ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
            ) => {}
        result => panic!("{name} did not see EOF/reset after child SIGKILL: {result:?}"),
    }
}

/// Keep listener probes asynchronous on the same runtime that must tear the
/// connection down. Every connect and polling interval shares the post-kill
/// deadline, so observing refusal cannot stall the cleanup it measures.
pub(super) async fn listener_refused(address: SocketAddr, deadline: Instant) -> bool {
    loop {
        let probe_deadline = deadline.min(Instant::now() + Duration::from_millis(100));
        match timeout_at(probe_deadline, TcpStream::connect(address)).await {
            Ok(Ok(peer)) => drop(peer),
            Ok(Err(error)) if error.kind() == ErrorKind::ConnectionRefused => return true,
            // A connect already accepted as teardown starts can be reset.
            // That is transitional evidence, never the refusal oracle itself.
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
                ) => {}
            Ok(Err(error)) => panic!("unexpected listener probe error: {error}"),
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            return false;
        }
        // sleep-ok: bound asynchronous listener-refusal polling under the shared teardown deadline.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Killing the process that owns the production client must tear down its
/// recipe listener and both sides of an established relay within five seconds.
/// A unique two-way marker establishes the actual relay before SIGKILL, and
/// both peers remain parent-owned throughout the teardown observations.
#[farhelm_testtrace::test]
async fn tunnel_and_listener_end_when_the_owning_helm_exits() {
    if let Some(config) = std::env::var_os(CHILD_CONFIG) {
        serve_child(serde_json::from_str(&config.into_string().unwrap()).unwrap()).await;
        return;
    }

    // Keep the Unix socket short even when macOS's ambient temporary directory
    // has a long path. This directory belongs solely to this fixture.
    let dir = tempfile::Builder::new()
        .prefix("fh-tunnel-")
        .tempdir_in("/tmp")
        .unwrap();
    let supervisor = Supervisor::new(dir.path()).await.unwrap();
    let socket = Supervisor::socket_path(dir.path());
    let listener = UnixListener::bind(&socket).unwrap();
    let mut connection = SupervisorConnection(tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        handle_connection(supervisor, stream).await
    }));

    let destination = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    // The recipe needs a concrete port; ask the kernel for one rather than
    // sharing a fixed port with other tests. The claim itself proves it bound.
    let reservation = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let sandbox_addr = reservation.local_addr().unwrap();
    let config = ChildConfig {
        socket,
        ready: dir.path().join("claimed"),
        tunnel: Tunnel {
            sandbox_port: sandbox_addr.port(),
            host: "127.0.0.1".into(),
            port: destination.local_addr().unwrap().port(),
        },
    };
    drop(reservation);
    let mut child = HelmChild(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD_CONFIG, serde_json::to_string(&config).unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let ready = poll_until(Instant::now() + STARTUP_TIMEOUT, || {
        config.ready.exists() || child.0.try_wait().unwrap().is_some() || connection.0.is_finished()
    })
    .await;
    assert!(
        ready && config.ready.exists(),
        "claim readiness failed on reserved port {}; child={:?}, supervisor finished={}; \
         child stderr retains the actual claim error, including a port-in-use collision",
        sandbox_addr.port(),
        child.0.try_wait(),
        connection.0.is_finished()
    );
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "child exited before relay setup"
    );
    assert!(
        !connection.0.is_finished(),
        "supervisor ended before relay setup"
    );

    let mut sandbox_peer = timeout(STARTUP_TIMEOUT, TcpStream::connect(sandbox_addr))
        .await
        .expect("claimed sandbox listener did not accept")
        .unwrap();
    let (mut destination_peer, _) = timeout(STARTUP_TIMEOUT, destination.accept())
        .await
        .expect("production relay did not dial the configured destination")
        .unwrap();
    let marker = format!("recipe-process-proof:{}", uuid::Uuid::new_v4()).into_bytes();
    timeout(STARTUP_TIMEOUT, async {
        sandbox_peer.write_all(&marker).await.unwrap();
        let mut received = vec![0; marker.len()];
        destination_peer.read_exact(&mut received).await.unwrap();
        assert_eq!(received, marker, "sandbox-to-destination marker changed");
        destination_peer.write_all(&received).await.unwrap();
        sandbox_peer.read_exact(&mut received).await.unwrap();
        assert_eq!(received, marker, "destination-to-sandbox marker changed");
    })
    .await
    .expect("unique marker did not roundtrip through the production relay");
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "the owning child must still be alive after the marker roundtrip"
    );
    assert!(
        !connection.0.is_finished(),
        "supervisor ended before SIGKILL"
    );

    let deadline = Instant::now() + TEARDOWN_TIMEOUT;
    child.0.kill().expect("SIGKILL the owned helm child");
    assert!(
        poll_until(deadline, || child.0.try_wait().unwrap().is_some()).await,
        "owned child did not exit after SIGKILL"
    );
    assert_eq!(child.0.wait().unwrap().signal(), Some(libc::SIGKILL));
    let (refused, (), ()) = tokio::join!(
        listener_refused(sandbox_addr, deadline),
        assert_peer_closed(&mut sandbox_peer, "sandbox peer", deadline),
        assert_peer_closed(&mut destination_peer, "destination peer", deadline),
    );
    assert!(
        refused,
        "recipe listener still accepted after child SIGKILL"
    );
    // A killed peer can end the reader with EOF or race an outstanding write
    // with an I/O error. Both must finish the same production teardown.
    let _ = timeout_at(deadline, &mut connection.0)
        .await
        .expect("supervisor connection did not finish after child SIGKILL")
        .expect("supervisor connection task panicked");
}

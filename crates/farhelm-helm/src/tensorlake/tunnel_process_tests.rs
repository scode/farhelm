//! Owned-process proof of both standalone signal handling and Linux's
//! uncatchable-parent-death tether. The TCP stand-in uses the production
//! spawn/monitor path; no vendor, SSH daemon or live Farhelm is involved.

use super::*;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;

const CHILD_DIR: &str = "FARHELM_TEST_TUNNEL_OWNER_DIR";
const TEST: &str =
    "tensorlake::tunnels::process_tests::tunnel_and_listener_end_when_the_owning_helm_exits";

/// Retain ownership of the exact parent even if a readiness assertion fails.
struct Parent(std::process::Child);
impl Drop for Parent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A pidfd pins the tunnel across reparenting and reaping. Cleanup must
/// never signal an unrelated process that reused the former child's PID.
struct Tunnel(OwnedFd);
impl Tunnel {
    /// Pin the identity while the ready fixture is still alive, before its
    /// parent can exit and another process can reuse the numeric PID.
    fn pin(pid: i32) -> Self {
        // SAFETY: pidfd_open observes the fixture PID and returns a new FD.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0_u32) };
        assert!(
            fd >= 0,
            "pin tunnel process: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: the successful syscall transferred ownership of this FD.
        Self(unsafe { OwnedFd::from_raw_fd(fd as i32) })
    }
    /// A readable pidfd proves exit even when the reparented child is still
    /// a zombie; signal-zero probes would count that zombie as running.
    fn exited(&self) -> bool {
        let mut event = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd, with no blocking wait.
        unsafe {
            libc::poll(&mut event, 1, 0) > 0 && event.revents & (libc::POLLIN | libc::POLLHUP) != 0
        }
    }
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        // SAFETY: the pidfd names only this owned fixture, even after exit.
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            );
        }
    }
}

/// Bound fixture polling on named files, owned child exit and listener
/// closure. No observation is inferred merely from having sent a signal.
fn until(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        // sleep-ok: bounded polling of this fixture's readiness/exit/listener oracles.
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// The re-exec branch runs a real standalone helm with a test-owned tunnel
/// lease inside its runtime. The existing startup hook injects only the
/// stand-in; signal handling, runtime ownership and monitor spawning are real.
fn serve_parent(dir: &Path) {
    let fixture_dir = dir.to_path_buf();
    crate::HELM_RUNTIME_TEST_HOOK.with_borrow_mut(|hook| {
        *hook = Some(Box::new(move || {
            tokio::spawn(async move {
                let runtime = Arc::new(Mutex::new(Runtime::default()));
                let events = Arc::new(FleetEvents::new());
                let (tx, mut rx) = watch::channel(TunnelView { tunnels: vec![], state: TunnelState::Retrying { reason: None, delay_secs: 0 } });
                let mut command = tokio::process::Command::new("/usr/bin/python3");
                command.args(["-c", "import socket,os,sys,json\ns=socket.socket(); s.bind(('127.0.0.1',0)); s.listen()\nwith open(sys.argv[1],'w') as f: json.dump([os.getpid(),s.getsockname()[1]],f)\nsys.stderr.write('debug1: forwarding_success: all expected forwarding replies received\\n'); sys.stderr.flush()\nwhile True:\n c,_=s.accept(); c.close()"])
                    .arg(fixture_dir.join("listener.json")).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
                crate::provisioning::isolate_process_group(&mut command);
                let worker_runtime = runtime.clone();
                let worker_events = events.clone();
                let task = tokio::spawn(async move { monitor(command, &worker_runtime, &tx, &worker_events).await });
                let _lease = TunnelLease { runtime, task: task.abort_handle(), views: Arc::default(), host: 1, events };
                tokio::time::timeout(Duration::from_secs(20), rx.wait_for(|view| view.state == TunnelState::Up)).await.unwrap().unwrap();
                std::fs::write(fixture_dir.join("tunnel-ready"), b"confirmed").unwrap();
                std::future::pending::<()>().await;
            });
        }));
    });
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let ready_file = dir.join("helm-ready");
    let ready = std::thread::spawn(move || {
        ready_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("helm serving dependencies ready");
        std::fs::write(ready_file, b"ready").unwrap();
    });
    crate::run_with_ready(
        crate::HelmArgs {
            port: 0,
            state_dir: Some(dir.join("helm")),
            ui_dist: None,
            ensure_hosts: None,
            payload_dir: None,
            release_base_url: None,
            backstop_refresh_secs: None,
        },
        None,
        None,
        None,
        Some(ready_tx),
        None,
        crate::ServingMode::Standalone,
    )
    .unwrap();
    ready.join().unwrap();
}

/// SIGINT/SIGTERM must finish the actual standalone shutdown path with
/// success. SIGKILL cannot unwind it, so only the Linux parent-death tether
/// can end the isolated tunnel. In every case readiness is established and
/// both process exit and listener closure are observed after signalling only
/// the parent. Child-only environment injection never mutates this process.
#[farhelm_testtrace::test]
fn tunnel_and_listener_end_when_the_owning_helm_exits() {
    if let Some(dir) = std::env::var_os(CHILD_DIR) {
        serve_parent(Path::new(&dir));
        return;
    }
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGKILL] {
        let dir = tempfile::tempdir().unwrap();
        let mut parent = Parent(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD_DIR, dir.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        assert!(
            until(|| dir.path().join("helm-ready").exists()
                && dir.path().join("tunnel-ready").exists()),
            "readiness failed; parent={:?}",
            parent.0.try_wait()
        );
        assert!(
            parent.0.try_wait().unwrap().is_none(),
            "parent must remain alive before the signal"
        );
        let [pid, port]: [i32; 2] =
            serde_json::from_slice(&std::fs::read(dir.path().join("listener.json")).unwrap())
                .unwrap();
        let tunnel = Tunnel::pin(pid);
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port as u16));
        assert!(!tunnel.exited(), "tunnel must be alive before the signal");
        assert!(
            std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_ok(),
            "owned listener must accept before the signal"
        );
        // SAFETY: this unreaped Child still owns the exact parent PID.
        assert_eq!(unsafe { libc::kill(parent.0.id() as i32, signal) }, 0);
        assert!(
            until(|| parent.0.try_wait().unwrap().is_some()),
            "parent did not finish signal {signal}"
        );
        let status = parent.0.wait().unwrap();
        if signal != libc::SIGKILL {
            assert!(
                status.success(),
                "standalone signal {signal} must run orderly shutdown: {status}"
            );
        }
        assert!(
            until(|| tunnel.exited()
                && std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(100))
                    .is_err()),
            "tunnel process/listener survived parent signal {signal}"
        );
    }
}

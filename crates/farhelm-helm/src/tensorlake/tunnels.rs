//! Recipe forwards belong to one connected host, never to an ssh master.
//!
//! The actor keeps a lease only during its connected phase. Dropping that
//! lease synchronously stops the process group before a suspend/removal hold
//! can acknowledge parking. The retry task cannot spawn after that point.

use super::{Tensorlake, ssh::TensorlakeSshAccess};
use crate::{
    feed::FleetEvents,
    store::{HostId, HostRow},
};
use farhelm_proto::tensorlake::{TunnelState, TunnelView, recipes::Tunnel};
use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Child, sync::watch};

#[cfg(all(test, target_os = "linux"))]
#[path = "tunnel_process_tests.rs"]
mod process_tests;

type Views = Arc<Mutex<HashMap<HostId, watch::Receiver<TunnelView>>>>;

/// The service's read side. A missing lease means the host has no active
/// forwards; definitions are read anew at its next connection.
#[derive(Default)]
pub(crate) struct Tunnels {
    views: Views,
}

impl Tunnels {
    /// Observe one lease's readiness and eventual worker exit in lifecycle
    /// tests without polling or keeping that lease alive.
    #[cfg(test)]
    pub(super) fn subscribe(&self, host: HostId) -> Option<watch::Receiver<TunnelView>> {
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .get(&host)
            .cloned()
    }

    /// Read the current lease's last publication without prolonging its
    /// lifetime. Removing the lease withdraws the view immediately.
    pub(crate) fn view(&self, host: HostId) -> Option<TunnelView> {
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .get(&host)
            .map(|view| view.borrow().clone())
    }
}

/// Spawn and teardown share this lock. In particular cancellation cannot
/// race a retry that is about to install its next child.
#[derive(Default)]
struct Runtime {
    stopped: bool,
    child: Option<Child>,
}

impl Runtime {
    /// End the group while its direct child is still owned, then let Tokio
    /// reap the killed child. No unbounded pipe drain delays a host hold.
    fn stop_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                // SAFETY: this child was spawned in its own process group.
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
            let _ = child.start_kill();
        }
    }
}

/// The connected phase's cancellation guard. It owns the worker and all
/// its local children, including during retry backoff and failed startup.
pub(crate) struct TunnelLease {
    runtime: Arc<Mutex<Runtime>>,
    task: tokio::task::AbortHandle,
    views: Views,
    host: HostId,
    events: Arc<FleetEvents>,
}

impl Drop for TunnelLease {
    fn drop(&mut self) {
        let mut runtime = self.runtime.lock().expect("tunnel runtime poisoned");
        runtime.stopped = true;
        runtime.stop_child();
        self.task.abort();
        self.views
            .lock()
            .expect("tunnel views poisoned")
            .remove(&self.host);
        self.events.bump();
    }
}

impl Tensorlake {
    /// Snapshot the recipe once per connection. Recipe edits intentionally
    /// wait for reconnect, so a retry never changes the reach granted to an
    /// already connected sandbox.
    pub(crate) async fn start_tunnels(
        &self,
        row: &HostRow,
        events: Arc<FleetEvents>,
    ) -> Option<TunnelLease> {
        let binding = row.tensorlake.as_ref()?;
        let recipe = self.0.store.tensorlake_recipe_row(&binding.recipe).await;
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
        self.0
            .tunnels
            .views
            .lock()
            .expect("tunnel views poisoned")
            .insert(row.id, rx);
        let runtime = Arc::new(Mutex::new(Runtime::default()));
        let worker_runtime = runtime.clone();
        let worker_events = events.clone();
        let access = self.0.ssh_keys.as_ref().map(|keys| {
            TensorlakeSshAccess::new(
                keys.tensorlake_dir(),
                super::ssh_key::private_key_path(keys.tensorlake_dir()),
            )
        });
        let sandbox = binding.sandbox.clone();
        let fake = self.0.tunnel_fake.clone();
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
                let result = if let Some(fake) = &fake {
                    fake.run_tunnels(&sandbox, &tx, &worker_events).await
                } else if let Some(access) = &access {
                    run_ssh(
                        access,
                        &sandbox,
                        &definitions,
                        &worker_runtime,
                        &tx,
                        &worker_events,
                    )
                    .await
                } else {
                    Err("This helm has no Tensorlake SSH key".to_string())
                };
                worker_runtime
                    .lock()
                    .expect("tunnel runtime poisoned")
                    .stop_child();
                let reason = result
                    .err()
                    .unwrap_or_else(|| "SSH closed the tunnels".to_string());
                publish(TunnelState::Retrying {
                    reason: Some(reason),
                    delay_secs: delay,
                });
                tokio::time::sleep(Duration::from_secs(delay)).await;
                delay = (delay * 2).min(30);
            }
        });
        Some(TunnelLease {
            runtime,
            task: task.abort_handle(),
            views: self.0.tunnels.views.clone(),
            host: row.id,
            events,
        })
    }
}

/// Own all forwards in one non-multiplexed, sessionless ssh connection.
/// OpenSSH's forwarding-confirmation diagnostic, rather than process
/// survival or authentication alone, is the readiness boundary.
fn argv(
    access: &TensorlakeSshAccess,
    sandbox: &str,
    tunnels: &[Tunnel],
) -> Result<Vec<String>, String> {
    // A tunnel grants exactly the recipe's listeners. Reading user config
    // would also enable its LocalForward/RemoteForward directives; ssh has
    // no option to clear those while keeping command-line forwards.
    let mut args: Vec<String> = ["-F", "/dev/null", "-v", "-N", "-T"]
        .into_iter()
        .map(str::to_string)
        .collect();
    for option in [
        "BatchMode=yes",
        "ControlMaster=no",
        "ControlPath=none",
        "ExitOnForwardFailure=yes",
        "ForwardAgent=no",
        "ForwardX11=no",
        "RemoteCommand=none",
        "RequestTTY=no",
        "PermitLocalCommand=no",
    ] {
        args.extend(["-o".to_string(), option.to_string()]);
    }
    for option in access.options()? {
        args.extend(["-o".to_string(), option]);
    }
    for tunnel in tunnels {
        args.extend([
            "-R".to_string(),
            format!(
                "127.0.0.1:{}:{}:{}",
                tunnel.sandbox_port, tunnel.host, tunnel.port
            ),
        ]);
    }
    args.extend(["--".to_string(), super::ssh::destination(sandbox)]);
    Ok(args)
}

/// Drain both pipes incrementally and retain only a bounded diagnostic
/// tail. The child is polled under the same lock teardown uses, so the
/// lease can kill it synchronously even while a read is waiting.
async fn run_ssh(
    access: &TensorlakeSshAccess,
    sandbox: &str,
    tunnels: &[Tunnel],
    runtime: &Arc<Mutex<Runtime>>,
    tx: &watch::Sender<TunnelView>,
    events: &FleetEvents,
) -> Result<(), String> {
    let mut command = tokio::process::Command::new("ssh");
    command
        .args(argv(access, sandbox, tunnels)?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::provisioning::isolate_process_group(&mut command);
    monitor(command, runtime, tx, events).await
}

/// Monitor a prepared, isolated command. Keeping command construction at
/// the boundary lets tests exercise real pipes and cancellation without
/// changing PATH or ever dialing a vendor.
async fn monitor(
    command: tokio::process::Command,
    runtime: &Arc<Mutex<Runtime>>,
    tx: &watch::Sender<TunnelView>,
    events: &FleetEvents,
) -> Result<(), String> {
    let (_spawn_thread, (mut stdout, mut stderr)) = spawn_owned(command, runtime.clone()).await?;
    let mut out = [0; 128];
    let mut err = [0; 1024];
    let mut tail = Vec::new();
    let mut ready = false;
    let mut out_open = true;
    let mut err_open = true;
    let mut ended = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut poll = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            read = stdout.read(&mut out), if out_open => {
                let count = read.map_err(|error| error.to_string())?;
                out_open = count != 0;
            },
            read = stderr.read(&mut err), if err_open => {
                let count = read.map_err(|error| error.to_string())?;
                err_open = count != 0;
                tail.extend_from_slice(&err[..count]);
                if tail.len() > 8192 { tail.drain(..tail.len()-8192); }
                if !ready && forwarding_confirmed(&tail) {
                    ready = true;
                    tx.send_modify(|view| view.state = TunnelState::Up);
                    events.bump();
                }
            },
            _ = poll.tick() => {
                if ended.is_none() {
                    let exited = runtime.lock().expect("tunnel runtime poisoned").child.as_mut().and_then(|child| child.try_wait().transpose());
                    if let Some(status) = exited { ended = Some((status.map_err(|error| error.to_string())?, tokio::time::Instant::now())); }
                }
                // Exit and pipe readiness are scheduled independently. Give
                // the final SSH refusal a bounded drain before rendering it.
                if let Some((status, at)) = ended
                    && (!err_open || at.elapsed() >= Duration::from_secs(1)) {
                    return Err(format!("SSH tunnels closed ({status}){}", diagnostic(&tail)));
                }
                if !ready && tokio::time::Instant::now() >= deadline { return Err(format!("SSH did not confirm the tunnels within 30 seconds{}", diagnostic(&tail))); }
            },
        }
    }
}

/// Linux's parent-death signal belongs to the thread that forks, not merely
/// its process. A Tokio worker can retire after handing off its work, so a
/// dedicated thread spawns this child and stays parked until its monitor
/// ends. The sender returned here keeps that thread alive across every await.
/// No watcher process or polling is involved. macOS uses lease teardown and
/// the helm's orderly shutdown; it has no equivalent parent-death primitive.
async fn spawn_owned(
    mut command: tokio::process::Command,
    runtime: Arc<Mutex<Runtime>>,
) -> Result<
    (
        std::sync::mpsc::Sender<()>,
        (tokio::process::ChildStdout, tokio::process::ChildStderr),
    ),
    String,
> {
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as libc::pid_t;
        // SAFETY: pre_exec calls only async-signal-safe syscalls. The parent
        // check closes the race where the helm dies before prctl is armed.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                Ok(())
            });
        }
    }
    let handle = tokio::runtime::Handle::current();
    let (keep_alive, lifetime) = std::sync::mpsc::channel();
    let (sent, received) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("tensorlake-tunnel-spawn".into())
        .spawn(move || {
            let _entered = handle.enter();
            let result = (|| {
                let mut runtime = runtime.lock().expect("tunnel runtime poisoned");
                if runtime.stopped {
                    return Err("Host disconnected".to_string());
                }
                let mut child = command
                    .spawn()
                    .map_err(|error| format!("Could not start SSH tunnels: {error}"))?;
                let pipes = (
                    child.stdout.take().expect("stdout piped"),
                    child.stderr.take().expect("stderr piped"),
                );
                runtime.child = Some(child);
                Ok(pipes)
            })();
            if sent.send(result).is_err() {
                runtime
                    .lock()
                    .expect("tunnel runtime poisoned")
                    .stop_child();
            }
            // Command::spawn waits for pre_exec/exec to finish before returning,
            // so this thread cannot exit before the child's tether is installed.
            let _ = lifetime.recv();
        })
        .map_err(|error| format!("Could not start the SSH owner thread: {error}"))?;
    let pipes = received
        .await
        .map_err(|_| "The SSH owner thread stopped before spawning".to_string())??;
    Ok((keep_alive, pipes))
}

/// OpenSSH changed the summary's wording between releases. Both are sent
/// only after every requested remote listener has answered successfully;
/// authentication and the individual request-sent messages prove less.
fn forwarding_confirmed(tail: &[u8]) -> bool {
    String::from_utf8_lossy(tail).lines().any(|line| {
        matches!(
            line.trim_end(),
            "debug1: All remote forwarding requests processed"
                | "debug1: forwarding_success: all expected forwarding replies received"
        )
    })
}

/// SSH output is bounded and sanitized before entering any host view.
fn diagnostic(tail: &[u8]) -> String {
    let clean = String::from_utf8_lossy(tail)
        .lines()
        .filter(|line| !line.starts_with("debug1:"))
        .collect::<Vec<_>>()
        .join("\n");
    let text = super::shell::shown(&clean);
    if text.is_empty() {
        String::new()
    } else {
        format!(": {text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed SSH and newer OpenSSH use different completion
    /// summaries. Both must count; request-sent and per-listener successes
    /// alone must not report the complete recipe as ready.
    #[test]
    fn confirmation_accepts_both_openssh_summaries_only() {
        for line in [
            "debug1: All remote forwarding requests processed\r\n",
            "debug1: forwarding_success: all expected forwarding replies received\r\n",
        ] {
            assert!(forwarding_confirmed(line.as_bytes()));
        }
        for line in [
            "debug1: ssh_init_forwarding: expecting replies for 1 forwards\r\n",
            "debug1: remote forward success for: listen 127.0.0.1:8080, connect 127.0.0.1:8080\r\n",
        ] {
            assert!(!forwarding_confirmed(line.as_bytes()));
        }
    }

    /// Forwards must carry only recipe listeners on a separate connection.
    /// Sharing a supervisor master silently clears them; reading ssh config
    /// could grant listeners the user never put in the recipe.
    #[test]
    fn argv_is_sessionless_unshared_and_recipe_scoped() {
        let access = TensorlakeSshAccess::new(
            std::path::Path::new("/state/tensorlake"),
            "/state/key".into(),
        );
        let args = argv(
            &access,
            "farhelm-host-k7q2-dev-abcd",
            &[Tunnel {
                sandbox_port: 8080,
                host: "[::1]".into(),
                port: 9090,
            }],
        )
        .unwrap();
        for option in [
            "ControlMaster=no",
            "ControlPath=none",
            "ExitOnForwardFailure=yes",
            "ServerAliveInterval=30",
            "HostKeyAlias=sandbox.tensorlake.ai",
            "PermitLocalCommand=no",
        ] {
            assert!(
                args.windows(2).any(|pair| pair == ["-o", option]),
                "{option}"
            );
        }
        assert_eq!(&args[..2], ["-F", "/dev/null"]);
        assert!(args.iter().any(|arg| arg == "-N"));
        assert!(!args.iter().any(|arg| arg == "ClearAllForwardings=yes"));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["-R", "127.0.0.1:8080:[::1]:9090"])
        );
        assert_eq!(
            &args[args.len() - 2..],
            [
                "--",
                "farhelm-host-k7q2-dev-abcd@farhelm-host-k7q2-dev-abcd.sandbox.tensorlake.ai"
            ]
        );
    }

    /// An owned stand-in models SSH's actual pipe lifetime without changing
    /// the test process's environment or ever reaching a network.
    fn command(script: &str) -> tokio::process::Command {
        let mut command = tokio::process::Command::new("/bin/sh");
        command
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::provisioning::isolate_process_group(&mut command);
        command
    }

    /// Authentication/process survival cannot prove forwards accepted. The
    /// explicit confirmation moves health to Up; dropping the lease kills
    /// the owned process and publishes no remaining tunnel view.
    #[farhelm_testtrace::test]
    async fn confirmed_forward_is_up_and_lease_drop_ends_the_child() {
        let runtime = Arc::new(Mutex::new(Runtime::default()));
        let events = Arc::new(FleetEvents::new());
        let (tx, mut rx) = watch::channel(TunnelView {
            tunnels: vec![],
            state: TunnelState::Retrying {
                reason: None,
                delay_secs: 0,
            },
        });
        let worker_runtime = runtime.clone();
        let worker_events = events.clone();
        let task = tokio::spawn(async move {
            monitor(command("printf 'debug1: forwarding_success: all expected forwarding replies received\\n' >&2; exec /usr/bin/sleep 600"), &worker_runtime, &tx, &worker_events).await
        });
        // Own teardown before any readiness wait or assertion can fail;
        // a detached monitor otherwise keeps its isolated child alive.
        let views: Views = Arc::default();
        views.lock().unwrap().insert(1, rx.clone());
        let lease = TunnelLease {
            runtime: runtime.clone(),
            task: task.abort_handle(),
            views: views.clone(),
            host: 1,
            events,
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            rx.wait_for(|view| view.state == TunnelState::Up),
        )
        .await
        .unwrap()
        .unwrap();
        let (pid, alive) = {
            let mut runtime = runtime.lock().unwrap();
            let child = runtime.child.as_mut().expect("live child before drop");
            (child.id().unwrap(), child.try_wait())
        };
        assert!(alive.unwrap().is_none(), "fixture must still run");
        // SAFETY: observe only the child whose live handle is held here.
        assert_eq!(
            unsafe { libc::getpgid(pid as i32) },
            pid as i32,
            "fixture owns a separate process group"
        );
        drop(lease);
        assert!(runtime.lock().unwrap().stopped);
        assert!(runtime.lock().unwrap().child.is_none());
        assert!(views.lock().unwrap().is_empty());
        match task.await {
            Err(error) if error.is_cancelled() => {}
            Ok(Err(_)) => {} // The killed child may settle before abort is polled.
            other => panic!("unexpected monitor completion: {other:?}"),
        }
        // Tokio owns reaping after kill-on-drop; process disappearance is
        // the observable, not the cancellation flag alone.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                // SAFETY: signal zero observes only this test-owned PID.
                if unsafe { libc::kill(pid as i32, 0) } != 0 {
                    break;
                }
                // sleep-ok: bounded polling for Tokio's child reaper after the lease killed it.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("tunnel child reaped");
    }

    /// A refusal must carry the SSH reason rather than show Up, even when
    /// the process starts successfully and exits before the first poll.
    #[farhelm_testtrace::test]
    async fn rejected_forward_keeps_failure_reason_and_never_reports_up() {
        let runtime = Arc::new(Mutex::new(Runtime::default()));
        let (tx, rx) = watch::channel(TunnelView {
            tunnels: vec![],
            state: TunnelState::Retrying {
                reason: None,
                delay_secs: 0,
            },
        });
        let result = monitor(command("printf 'Error: remote port forwarding failed for listen port 8080\\n' >&2; exit 255"), &runtime, &tx, &FleetEvents::new()).await.unwrap_err();
        runtime.lock().unwrap().stop_child();
        assert!(result.contains("remote port forwarding failed"), "{result}");
        assert!(!matches!(rx.borrow().state, TunnelState::Up));
    }

    /// Retry must recover without reconnecting the supervisor, but must not
    /// apply a recipe edit midway through an existing connection. A new
    /// lease reads the edited mappings, and dropping either withdraws it.
    #[farhelm_testtrace::test]
    async fn retries_recover_with_the_original_recipe_until_the_next_connection() {
        use crate::tensorlake::e2e::E2eTensorlake;
        use farhelm_proto::tensorlake::recipes::{RecipeSave, RecipeSize, RecipeSource};
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
            source: RecipeSource::Image {
                image: "tensorlake/ubuntu-systemd".into(),
            },
            claude: None,
            codex: None,
            github_token_credential: None,
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
        )));
        let events = Arc::new(FleetEvents::new());
        let lease = service.start_tunnels(&row, events.clone()).await.unwrap();
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
        let next = service.start_tunnels(&row, events).await.unwrap();
        assert_eq!(
            service.tunnel_view(host).unwrap().tunnels[0].port,
            9090,
            "next connection reads the edit"
        );
        drop(next);
        assert!(service.tunnel_view(host).is_none());
    }
}

//! Stopping a real `farhelm supervisor run` process the way systemd does.
//!
//! The generated units use `KillMode=process`, so every planned stop,
//! restart, and upgrade is a SIGTERM to the supervisor alone while every
//! session keeps running in its terminal host. This module pins that each
//! way a supervisor is stopped (SIGTERM, a terminal hangup, a closed stdin
//! tether) reaches the orderly shutdown, which finishes inside its budget,
//! that the process then exits cleanly instead of being killed by the
//! signal, and that the session's agent outlives it and still answers
//! input; and that an inherited ignored hangup is honored.

use crate::harness::*;

/// How the test asks the supervisor to stop.
#[derive(Clone, Copy, Debug)]
enum Stop {
    /// What `systemctl stop`/`restart` sends under `KillMode=process`, and
    /// what the desktop app and uninstall send to stop the app's detached
    /// supervisor (`farhelm_supervisor::managed_supervisor`).
    Sigterm,
    /// The parent of a supervisor run with `--exit-on-stdin-close` closing
    /// its end of that stdin tether. The desktop app used to run its
    /// supervisor that way and no longer does; the option remains for tests
    /// and other parents that want the supervisor's lifetime tied to theirs.
    TetherClosed,
    /// A terminal hangup: the terminal the supervisor's stderr is on goes
    /// away, then SIGHUP reaches the whole process group the supervisor
    /// leads, as when a hand-started supervisor's terminal closes or its ssh
    /// connection drops.
    HangupToGroup,
}

/// Spec: a terminal hangup (the pseudo-terminal holding the supervisor's
/// stderr closes, then SIGHUP reaches the supervisor's whole process group)
/// runs the same orderly shutdown as SIGTERM: the supervisor exits with
/// status 0, and the session's agent survives in its terminal host and
/// answers a fresh input line after the shutdown.
///
/// Why it matters: a terminal hangup is how a hand-started
/// `farhelm supervisor run` (or a desktop app launched from a terminal) is
/// most often stopped, and two separate failures hid behind it. Unhandled,
/// SIGHUP killed the supervisor outright. And with its stderr terminal
/// gone, the supervisor's first log line after the hangup panicked the
/// process before the shutdown ran. The supervisor is started as a group
/// leader so the signal reaches its group and not the test runner's; the
/// agent surviving shows the hangup to the group did not reach the
/// session's host either.
#[farhelm_testtrace::test]
async fn a_hangup_to_the_supervisors_group_stops_it_in_order() {
    stop_attached_supervisor(Stop::HangupToGroup).await;
}

/// Spec: a supervisor holding a live, streaming terminal attachment answers
/// SIGTERM through the orderly shutdown and exits with status 0, and the
/// agent it was serving still answers fresh input.
///
/// Why it matters: exit status 0 plus the "stopping" log line without the
/// budget-exceeded warning is the evidence that the handler ran and
/// completed, rather than the process dying of the signal. The session
/// check guards the other direction: an orderly shutdown must not take the
/// agents down with it; a host process that still runs would not prove that
/// its agent does.
#[farhelm_testtrace::test]
async fn sigterm_stops_the_supervisor_in_order_and_exits_cleanly() {
    stop_attached_supervisor(Stop::Sigterm).await;
}

/// Spec: a closed stdin tether (`--exit-on-stdin-close`) runs the same
/// orderly shutdown as SIGTERM before the supervisor exits, and its agent
/// still answers fresh input after the tether closes.
///
/// Why it matters: the tether used to be raced against the supervisor and
/// win by dropping it mid-stream, which is the same abrupt close as a
/// signal death; a tethering parent going away is a routine stop, not a
/// crash.
#[farhelm_testtrace::test]
async fn a_closed_desktop_tether_stops_the_supervisor_in_order() {
    stop_attached_supervisor(Stop::TetherClosed).await;
}

/// Start a real supervisor, give it a live attachment on a running fake
/// agent, stop it the way `stop` says, and check the orderly shutdown ran
/// inside its budget, the process exited cleanly, and the agent still
/// answers fresh input in its terminal host, typed and read over a runtime
/// of the test's own (no supervisor runs to attach through). A host process
/// that still runs alone cannot distinguish a surviving agent from a host
/// showing a dead program's last screen.
async fn stop_attached_supervisor(stop: Stop) {
    let state = farhelm_teststate::tempdir().expect("supervisor state dir");
    let stderr_path = state.path().join("supervisor-stderr.log");
    // A hangup needs a terminal to hang up: the supervisor's stderr is then a
    // pseudo-terminal whose controlling side this test closes, so every
    // later write to it fails as it does after a real terminal closes. The
    // other stops log to a file the assertions below read.
    let mut pty_master = None;
    let stderr: std::process::Stdio = if matches!(stop, Stop::HangupToGroup) {
        let (master, slave) = open_pty();
        pty_master = Some(master);
        std::process::Stdio::from(slave)
    } else {
        std::fs::File::create(&stderr_path)
            .expect("stderr capture file")
            .into()
    };
    let mut command = tokio::process::Command::new(farhelm_bin());
    command
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path());
    if matches!(stop, Stop::TetherClosed) {
        command
            .arg("--exit-on-stdin-close")
            .stdin(std::process::Stdio::piped());
    }
    if matches!(stop, Stop::HangupToGroup) {
        // A group of its own, led by the supervisor: the hangup below goes
        // to that group, the way a closing terminal signals its foreground
        // job, and never to the test runner's own group.
        command.process_group(0);
    }
    let mut child = command
        .stdout(std::process::Stdio::null())
        .stderr(stderr)
        .kill_on_drop(true)
        .spawn()
        .expect("spawn supervisor");
    wait_for_supervisor_ready(state.path()).await;

    let stream = farhelm_supervisor::service::connect(state.path())
        .await
        .expect("connect to the supervisor");
    let (read, write) = tokio::io::split(stream);
    let client = SupervisorClient::start(read, write)
        .await
        .expect("handshake");
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = client
        .create_session(
            &work.path().to_string_lossy(),
            &fixture_cmd("fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    // A live attachment, so the stop below meets a supervisor that is
    // really serving a terminal, not merely holding one.
    let (channel, replay, mut rx) = client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = replay;
    // Fresh output through this attachment after its replay, so the stop
    // below meets an attachment that is really streaming, not merely opened.
    client.send_keys(channel, b"before-stop\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "before-stop", 20).await;

    match stop {
        Stop::Sigterm => {
            let pid = child.id().expect("supervisor pid") as libc::pid_t;
            // SAFETY: `pid` is our own live child, which `kill_on_drop`
            // still owns; nothing has waited on it, so it cannot have been
            // reaped and recycled.
            assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0, "send SIGTERM");
        }
        Stop::TetherClosed => drop(child.stdin.take().expect("piped tether")),
        Stop::HangupToGroup => {
            // The terminal goes first, as it does for real: the kernel hangs
            // the line up before it delivers SIGHUP.
            drop(pty_master.take().expect("the stderr pseudo-terminal"));
            let pid = child.id().expect("supervisor pid") as libc::pid_t;
            // SAFETY: as above; `process_group(0)` made the live child the
            // leader of a group with its own pid as the id, so `-pid` names
            // exactly that group.
            assert_eq!(
                unsafe { libc::kill(-pid, libc::SIGHUP) },
                0,
                "send SIGHUP to the supervisor's process group"
            );
        }
    }
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("the supervisor must exit after being stopped")
        .expect("wait for the supervisor");
    assert!(
        status.success(),
        "{stop:?}: the supervisor must exit cleanly, not die of the stop: {status:?}"
    );

    // A hangup's stderr went to the closed terminal, so for it the orderly
    // shutdown is evidenced by the exit status alone.
    if !matches!(stop, Stop::HangupToGroup) {
        let log = std::fs::read_to_string(&stderr_path).expect("read supervisor stderr");
        assert!(
            log.contains("stopping: closing terminal-output clients in order before exit"),
            "{stop:?} must reach the orderly output shutdown; stderr was:\n{log}"
        );
        assert!(
            !log.contains("did not finish closing in time"),
            "{stop:?}: the orderly shutdown must finish inside its budget; stderr was:\n{log}"
        );
    }
    // The session's agent outlived its supervisor in its terminal host:
    // the host answers a runtime of the test's own (nothing else is
    // connected once the supervisor has exited), and the agent echoes a
    // line typed there. Only the agent's own echo of a new line proves it
    // still runs; the host alone would still show a dead program's screen.
    let row = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .expect("open the store")
        .session(&session.id)
        .await
        .expect("read the row")
        .expect("the row exists");
    assert_eq!(
        row.terminal_kind,
        farhelm_supervisor::store::TerminalKind::Host,
        "premise: the session runs in a terminal host"
    );
    let socket = farhelm_supervisor::hosts::hosts_dir(state.path()).join(&row.terminal_address);
    let host = farhelm_supervisor::hosts::HostTerminal::adopt(
        &socket,
        farhelm_supervisor::hosts::Adoption::Recovered,
        farhelm_supervisor::hosts::HostTerminalOptions::default(),
    )
    .await
    .unwrap_or_else(|error| {
        panic!("{stop:?}: the session's host must survive its supervisor's orderly stop: {error}")
    });
    let probe = format!("after-stop-{}", uuid::Uuid::new_v4());
    crate::host_restarts::type_line(&host, &probe).await;
    crate::host_terminals::wait_for_screen(&host, &format!("echo:{probe}")).await;
}

/// Open a pseudo-terminal pair: the controlling (master) side, whose drop
/// hangs the line up, and the terminal (slave) side to hand a child as a
/// standard stream.
fn open_pty() -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
    use std::os::fd::FromRawFd as _;
    let (mut master, mut slave) = (-1, -1);
    // SAFETY: both out-pointers are valid, and null name, termios and
    // window-size arguments are documented as "use the defaults".
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert_eq!(rc, 0, "openpty: {}", std::io::Error::last_os_error());
    // Close-on-exec on both, which openpty does not set: a child that
    // inherited the controlling side would keep the line up after the test
    // closes its own copy, and the hangup would never happen. (The terminal
    // side still reaches the child as the stream it is handed.)
    for fd in [master, slave] {
        // SAFETY: `fd` is a descriptor openpty just returned.
        let set = unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
        assert_eq!(
            set,
            0,
            "set FD_CLOEXEC: {}",
            std::io::Error::last_os_error()
        );
    }
    // SAFETY: openpty succeeded, so both are fresh descriptors nothing else
    // owns.
    unsafe {
        (
            std::os::fd::OwnedFd::from_raw_fd(master),
            std::os::fd::OwnedFd::from_raw_fd(slave),
        )
    }
}

/// Spec: a supervisor started with SIGHUP already ignored (under `nohup`)
/// keeps running when it receives SIGHUP, and still stops cleanly on
/// SIGTERM afterwards.
///
/// Why it matters: `nohup farhelm supervisor run &` is the conventional way
/// to keep a hand-started supervisor running past an ssh logout. Handling
/// SIGHUP for an orderly shutdown must not override that explicit request;
/// a listener installed unconditionally would replace the inherited ignore
/// and stop the supervisor at logout, leaving the host offline.
#[farhelm_testtrace::test]
async fn an_inherited_ignored_hangup_leaves_the_supervisor_running() {
    let state = farhelm_teststate::tempdir().expect("supervisor state dir");
    // `nohup` sets SIGHUP to ignored and execs the program, so the child's
    // pid is the supervisor's own.
    let mut command = tokio::process::Command::new("nohup");
    command
        .arg(farhelm_bin())
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path());
    let mut child = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn supervisor under nohup");
    wait_for_supervisor_ready(state.path()).await;

    let pid = child.id().expect("supervisor pid") as libc::pid_t;
    // SAFETY: `pid` is our own live child, which `kill_on_drop` still owns;
    // nothing has waited on it, so it cannot have been reaped and recycled.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGHUP) }, 0, "send SIGHUP");
    // An unignored SIGHUP stops the supervisor within milliseconds, and
    // nothing positive marks "still ignoring", so this waits and then looks.
    // sleep-ok: observation window for an ignored SIGHUP
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        child.try_wait().expect("poll the supervisor").is_none(),
        "the supervisor must ignore SIGHUP it inherited as ignored"
    );
    farhelm_supervisor::service::connect(state.path())
        .await
        .expect("the supervisor must still be serving after the ignored SIGHUP");

    // SAFETY: as above.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0, "send SIGTERM");
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("the supervisor must exit after SIGTERM")
        .expect("wait for the supervisor");
    assert!(
        status.success(),
        "SIGTERM must still stop it cleanly: {status:?}"
    );
}

/// Spec: a tethered supervisor (`--exit-on-stdin-close`) whose tether closes
/// while it is still starting up exits promptly, even when a tmux command it
/// is waiting on never returns.
///
/// Why: building a supervisor runs tmux commands with no timeout of their
/// own (adopting a private server left running with old terminals, which
/// lists its sessions). If the stop were only
/// watched once serving began, a tethering parent that went away during a
/// hung startup would leave its supervisor running with no parent.
/// Nothing streams before startup finishes, so there is nothing to close in
/// order; the process should simply go.
#[farhelm_testtrace::test]
async fn a_closed_tether_stops_a_supervisor_stuck_in_startup() {
    use std::os::unix::fs::PermissionsExt as _;
    let state = farhelm_teststate::tempdir().expect("supervisor state dir");
    let real = farhelm_supervisor::tmux::resolve_tmux_program(None, None);
    let pidfile = state.path().join("stuck-list-sessions.pid");
    let quote = |path: &std::path::Path| shell_words::quote(&path.to_string_lossy()).into_owned();
    // `list-sessions`, the adoption's listing, parks in a sleep whose pid
    // the test records (so it can be reaped afterwards); everything else,
    // including the version probes, is real tmux.
    let script = format!(
        "#!/bin/sh\n\
         case \" $* \" in\n\
         *\" list-sessions \"*) echo $$ > {pidfile}; exec sleep 120;;\n\
         esac\n\
         exec {real} \"$@\"\n",
        pidfile = quote(&pidfile),
        real = quote(&real),
    );
    let wrapper = state.path().join("tmux-stuck-list-sessions");
    std::fs::write(&wrapper, script).expect("write the tmux wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("make the tmux wrapper executable");

    let mut command = tokio::process::Command::new(farhelm_bin());
    command
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .arg("--tmux")
        .arg(&wrapper)
        .arg("--exit-on-stdin-close")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let _tmux = tmux_guard_for_supervisor_child(state.path().join("tmux.sock"), command.as_std());
    // Premise: a private server holding an old terminal, which is what makes
    // startup ask tmux at all (a supervisor with none runs no tmux command).
    let started = tokio::process::Command::new(&real)
        .arg("-S")
        .arg(state.path().join("tmux.sock"))
        .args([
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            "old",
            "sleep",
            "600",
        ])
        .status()
        .await
        .expect("start the private server's old terminal");
    assert!(started.success(), "premise: the private server started");
    let mut child = command.spawn().expect("spawn supervisor");
    let stuck = wait_for_pid_file(&pidfile, 20).await;
    let _reap_stuck = PidKillGuard::arm(stuck);

    drop(child.stdin.take().expect("piped tether"));
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("a closed tether must stop a supervisor stuck in startup")
        .expect("wait for the supervisor");
    assert!(status.success(), "the stop must exit cleanly: {status:?}");
}

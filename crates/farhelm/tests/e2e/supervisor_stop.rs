//! Stopping a real `farhelm supervisor run` process the way systemd does.
//!
//! The generated units use `KillMode=process`, so every planned stop,
//! restart, and upgrade is a SIGTERM to the supervisor alone while tmux and
//! every session keep running. Before the supervisor handled that signal it
//! died on the spot, closing every tmux control client with output still
//! streaming, which is the shape that can make tmux abort its whole private
//! server (BUGS.md, "Abrupt supervisor death can crash the private tmux
//! server"). The abort itself is too rare to reproduce on demand, so this
//! module pins the part Farhelm controls: the signal reaches the orderly
//! output shutdown, which finishes inside its budget, and the process then
//! exits cleanly instead of being killed by the signal, having switched
//! every output-bearing client to `no-output` first. The last part is
//! observed through a `--tmux` wrapper that records each acknowledged
//! `refresh-client -f no-output` the supervisor runs.

use crate::harness::*;

/// How the test asks the supervisor to stop.
#[derive(Clone, Copy, Debug)]
enum Stop {
    /// What `systemctl stop`/`restart` sends under `KillMode=process`.
    Sigterm,
    /// The desktop app exiting: its end of the stdin tether closes.
    TetherClosed,
    /// A terminal hangup: the terminal the supervisor's stderr is on goes
    /// away, then SIGHUP reaches the whole process group the supervisor
    /// leads, as when a hand-started supervisor's terminal closes or its ssh
    /// connection drops.
    HangupToGroup,
}

/// Spec: a terminal hangup (the pseudo-terminal holding the supervisor's
/// stderr closes, then SIGHUP reaches the supervisor's whole process group)
/// runs the same orderly output shutdown as SIGTERM: the supervisor exits
/// with status 0 after both output clients acknowledged `no-output`, and
/// the session survives.
///
/// Why it matters: a terminal hangup is how a hand-started
/// `farhelm supervisor run` (or a desktop app launched from a terminal) is
/// most often stopped, and three separate failures hid behind it. Unhandled,
/// SIGHUP killed the supervisor outright. Because the signal goes to the
/// whole group, the tmux clients the supervisor spawned got it too and were
/// torn down before the supervisor could switch them off. And with its
/// stderr terminal gone, the supervisor's first log line after the hangup
/// panicked the process before the shutdown ran. The supervisor is started
/// as a group leader so the signal reaches its group and not the test
/// runner's.
#[farhelm_testtrace::test]
async fn a_hangup_to_the_supervisors_group_closes_output_clients_in_order() {
    stop_attached_supervisor(Stop::HangupToGroup).await;
}

/// Spec: a supervisor holding a live, streaming terminal attachment answers
/// SIGTERM by closing its output clients through the orderly path and
/// exiting with status 0, and the session it was serving survives in tmux.
///
/// Why it matters: a SIGTERM-killed process (exit by signal) is exactly the
/// abrupt close that risks the whole tmux server; exit status 0 plus the
/// "stopping" log line without the budget-exceeded warning is the evidence
/// that the handler ran and completed. The session check guards the other
/// direction: an orderly shutdown must not take the sessions down with it.
#[farhelm_testtrace::test]
async fn sigterm_closes_output_clients_in_order_and_exits_cleanly() {
    stop_attached_supervisor(Stop::Sigterm).await;
}

/// Spec: the desktop app's stdin tether closing runs the same orderly output
/// shutdown as SIGTERM before the managed supervisor exits.
///
/// Why it matters: the tether used to be raced against the supervisor and
/// win by dropping it mid-stream, which is the same abrupt close as a
/// signal death; quitting the desktop app is a routine stop, not a crash.
#[farhelm_testtrace::test]
async fn a_closed_desktop_tether_closes_output_clients_in_order() {
    stop_attached_supervisor(Stop::TetherClosed).await;
}

/// Write a tmux wrapper that execs the real tmux for everything except the
/// supervisor's output-disabling command, which it runs, then records with
/// its exit status in `log`, one line each.
///
/// That command is a separate tmux process whose successful exit is the
/// supervisor's acknowledgement that tmux applied `no-output` to the named
/// client (`TmuxDriver::disable_control_client_output`), and it can only
/// succeed while that client still exists, so a recorded success is proof
/// that the client was switched off before it was closed. Everything else
/// is passed through by `exec`, so long-lived control clients are real tmux
/// processes with the pids and client names the supervisor expects.
fn no_output_recording_tmux(dir: &std::path::Path, log: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let real = farhelm_supervisor::tmux::resolve_tmux_program(None, None);
    let quote = |path: &std::path::Path| shell_words::quote(&path.to_string_lossy()).into_owned();
    let script = format!(
        "#!/bin/sh\n\
         case \" $* \" in\n\
         *\" refresh-client \"*\" -f no-output \"*)\n\
         \x20 {real} \"$@\"; rc=$?\n\
         \x20 printf 'no-output rc=%s args=%s\\n' \"$rc\" \"$*\" >> {log}\n\
         \x20 exit \"$rc\";;\n\
         esac\n\
         exec {real} \"$@\"\n",
        real = quote(&real),
        log = quote(log),
    );
    let wrapper = dir.join("tmux-no-output-recorder");
    std::fs::write(&wrapper, script).expect("write the tmux wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("make the tmux wrapper executable");
    wrapper
}

/// Start a real supervisor, give it a live attachment on a running fake
/// agent, stop it the way `stop` says, and check the orderly shutdown ran
/// and acknowledged `no-output` on the output-bearing clients, the process
/// exited cleanly, and the session survived.
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
    let no_output_log = state.path().join("no-output.log");
    let wrapper = no_output_recording_tmux(state.path(), &no_output_log);
    let mut command = tokio::process::Command::new(farhelm_bin());
    command
        .args(["supervisor", "run", "--state-dir"])
        .arg(state.path())
        .arg("--tmux")
        .arg(&wrapper);
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
    let sock = state.path().join("tmux.sock");
    let _tmux = tmux_guard_for_supervisor_child(sock.clone(), command.as_std());
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
    wait_for_agent_ready(&sock, &session.id).await;
    // A live attachment is what gives the supervisor output-bearing clients
    // (the terminal's forwarder and the session sink) to shut down.
    let (channel, replay, mut rx) = client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = replay;
    // Fresh output through this attachment after its replay, so the stop
    // below meets clients that are really streaming, not merely opened.
    client.send_input(channel, b"before-stop\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "before-stop", 20).await;
    // Everything the wrapper records from here on is the stop's doing.
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&no_output_log)
        .and_then(|mut log| std::io::Write::write_all(&mut log, b"STOP\n"))
        .expect("mark the stop in the no-output log");

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
    // shutdown is evidenced by the acknowledgements and exit status alone.
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
    // Both output-bearing clients of the attachment, the terminal's
    // forwarder and the session sink, must have been switched off with an
    // acknowledgement after the stop began. (The input client never carried
    // output and is not expected here.)
    let recorded = std::fs::read_to_string(&no_output_log).expect("read the no-output log");
    let after_stop = recorded
        .split_once("STOP\n")
        .map(|(_, after)| after)
        .unwrap_or_default();
    let acknowledged: std::collections::HashSet<&str> = after_stop
        .lines()
        .filter(|line| line.starts_with("no-output rc=0 "))
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            words.by_ref().find(|word| *word == "-t")?;
            words.next()
        })
        .collect();
    assert!(
        acknowledged.len() >= 2,
        "{stop:?}: both output clients must acknowledge no-output before exit; recorded:\n{recorded}"
    );
    // Sessions live in tmux as `fh-<id>` (see `wait_for_agent_ready`).
    let target = format!("=fh-{}", session.id);
    let has_session = tmux_query(&sock, &["has-session", "-t", &target]).await;
    assert!(
        has_session.status.success(),
        "{stop:?}: the session must survive its supervisor's orderly stop: {}",
        String::from_utf8_lossy(&has_session.stderr)
    );
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
    let sock = state.path().join("tmux.sock");
    let _tmux = tmux_guard_for_supervisor_child(sock, command.as_std());
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

/// Spec: a desktop-managed supervisor whose tether closes while it is still
/// starting up exits promptly, even when a tmux command it is waiting on
/// never returns.
///
/// Why: building a supervisor runs tmux commands with no timeout of their
/// own (starting the private server among them). If the stop were only
/// watched once serving began, a desktop app that quit during a hung
/// startup would leave its managed supervisor running with no parent.
/// Nothing streams before startup finishes, so there is nothing to close in
/// order; the process should simply go.
#[farhelm_testtrace::test]
async fn a_closed_tether_stops_a_supervisor_stuck_in_startup() {
    use std::os::unix::fs::PermissionsExt as _;
    let state = farhelm_teststate::tempdir().expect("supervisor state dir");
    let real = farhelm_supervisor::tmux::resolve_tmux_program(None, None);
    let pidfile = state.path().join("stuck-start-server.pid");
    let quote = |path: &std::path::Path| shell_words::quote(&path.to_string_lossy()).into_owned();
    // `start-server` parks in a sleep whose pid the test records (so it can
    // be reaped afterwards); everything else, including the version probe,
    // is real tmux.
    let script = format!(
        "#!/bin/sh\n\
         case \" $* \" in\n\
         *\" start-server \"*) echo $$ > {pidfile}; exec sleep 120;;\n\
         esac\n\
         exec {real} \"$@\"\n",
        pidfile = quote(&pidfile),
        real = quote(&real),
    );
    let wrapper = state.path().join("tmux-stuck-start-server");
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

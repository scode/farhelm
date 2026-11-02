//! The supervisor's host terminal runtime
//! (`farhelm_supervisor::hosts::terminal`) against real terminal hosts,
//! started by the supervisor's spawn from the real `farhelm` binary.
//!
//! These live here rather than in the supervisor's own tests because a host
//! forks: started in-process, its second stage would be a fork of a
//! multi-threaded test runner (D-19). What a real host only does under
//! circumstances a test cannot arrange on cue (a read straddling the
//! replay's end, a resumed stream, discarded input, an unrestorable
//! snapshot) is tested against stand-ins in the supervisor's unit tests
//! (`hosts::terminal::tests`).
//!
//! Every host a test starts lives in a test-state directory of its own,
//! whose drop ends it through the test-state cleanup's own client
//! (`farhelm_teststate::hosts`), never through the runtime under test. Each
//! test declares its host before its terminal, so the terminal (and with it
//! the runtime's connection) is dropped first.
//!
//! Sections: fixtures; output, queries and the child; the clock; adoption;
//! resizes and shutdown.

use farhelm_supervisor::hosts::terminal::ManualClock;
use farhelm_supervisor::hosts::{
    AdoptError, Adoption, ChildSpec, Clock, HostRole, HostTag, HostTerminal, HostTerminalOptions,
    InputFate, Liveness, LivenessNotifier, Unavailability, spawn_host,
};
use farhelm_supervisor::tmux::{PaneProbe, PaneProcess};
use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use terminal_host::protocol::{ChildExit, MAX_SNAPSHOT, Size};

/// How long any single wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

// ---- Fixtures ------------------------------------------------------------------

fn farhelm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_farhelm"))
}

/// A child running `script` under `/bin/sh`, at 80x24.
fn shell(script: &str) -> ChildSpec {
    ChildSpec {
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: "/".into(),
        env: vec![("PATH".into(), "/usr/bin:/bin".into())],
        size: Size { cols: 80, rows: 24 },
        journal_cap: 1024 * 1024,
        initial_snapshot: None,
    }
}

fn agent_tag() -> HostTag {
    HostTag {
        session_id: "0f8e2b9c-4c1d-4e7a-9a53-7d2f61c0b8e4".into(),
        created_ns: farhelm_supervisor::hosts::tag::creation_stamp(),
        role: HostRole::Agent { generation: 3 },
    }
}

/// A real host this test started, in a state directory of its own whose
/// drop ends the host (see the module docs).
struct Host {
    socket: PathBuf,
    _state: farhelm_teststate::TestDir,
}

impl Host {
    async fn start(child: ChildSpec) -> Host {
        let state = farhelm_teststate::tempdir().expect("state dir");
        let spawned = spawn_host(&farhelm(), state.path(), child, &agent_tag())
            .await
            .expect("the host starts");
        Host {
            socket: spawned.socket,
            _state: state,
        }
    }

    /// Start the runtime for this host.
    async fn adopt(&self, adoption: Adoption, options: HostTerminalOptions) -> HostTerminal {
        HostTerminal::adopt(&self.socket, adoption, options)
            .await
            .expect("the runtime adopts the host")
    }
}

/// `future`, failing the test with `what` if it takes longer than [`WAIT`].
async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(WAIT, future)
        .await
        .unwrap_or_else(|_| panic!("{what} did not finish in time"))
}

/// Whether the process `pid` ends within [`WAIT`]: no process has that pid
/// any more (`ESRCH`), or it is a zombie, which runs nothing. A process that
/// exists and is not a zombie, or cannot be inspected, has not ended.
fn ended_within(pid: u32) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        // SAFETY: signal 0 only checks for the process.
        let missing = unsafe { libc::kill(pid as libc::pid_t, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        let zombie = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .is_ok_and(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .starts_with('Z')
            });
        if missing || zombie {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        // sleep-ok: polling interval for a process the test started.
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Wait until `condition` holds, checking again on every change of the
/// terminal's status (which every change of its emulator makes).
async fn wait_until(
    terminal: &HostTerminal,
    what: &str,
    condition: impl Fn(&HostTerminal) -> bool,
) {
    let mut status = terminal.subscribe();
    let waited = tokio::time::timeout(WAIT, async {
        while !condition(terminal) {
            if status.changed().await.is_err() {
                // The runtime ended; nothing will change any more.
                return condition(terminal);
            }
        }
        true
    })
    .await;
    assert!(
        waited == Ok(true),
        "{what} did not happen; status {:?}, screen {:?}",
        terminal.status(),
        terminal.screen_tail(4096)
    );
}

fn screen_has(text: &'static str) -> impl Fn(&HostTerminal) -> bool {
    move |terminal| terminal.screen_tail(4096).contains(text)
}

/// The number the program printed after `prefix` on the screen.
fn printed_number(terminal: &HostTerminal, prefix: &str) -> u32 {
    let screen = terminal.screen_tail(4096);
    let start = screen.find(prefix).expect("the prefix on the screen") + prefix.len();
    screen[start..]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|digits| digits.parse().ok())
        .expect("a number after the prefix")
}

// ---- Output, queries and the child -------------------------------------------------

/// A fresh host's output and title reach the runtime's screen, and its
/// child is the running root.
///
/// Why: this is the path every new terminal's screen, its status detection
/// and its kill root take. Spec: a child that sets its title, prints its
/// own pid and execs a sleep shows that line and that title; the terminal
/// is running, its root is the printed pid (the exec keeps the pid), and
/// the probe says it is alive with that pid.
#[farhelm_testtrace::test]
async fn a_fresh_hosts_output_title_and_child_reach_the_runtime() {
    let host = Host::start(shell(
        "printf '\\033]2;the title\\007pid=%s;\\n' $$; exec /bin/sleep 600",
    ))
    .await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    wait_until(&terminal, "the output", screen_has("pid=")).await;
    let pid = printed_number(&terminal, "pid=");
    assert_eq!(terminal.title(), "the title");
    let root = terminal.child_root().expect("a running root");
    assert_eq!(root.pid, pid);
    assert_eq!(terminal.status().child_pid, pid);
    assert_eq!(
        terminal.probe().unwrap(),
        PaneProbe::Owned(PaneProcess {
            pid: Some(pid),
            dead: false
        })
    );
}

/// A device attributes query is answered end to end: the program reads the
/// emulator's answer from its own terminal.
///
/// Why: agents probe their terminal at startup and some wait for the
/// answer; a terminal host answers nothing itself, so the runtime's answer
/// is the only one the program gets. Spec: a child in raw mode that sends
/// DA1 and prints the next seven bytes it reads, in hex, shows
/// `1b 5b 3f 31 3b 32 63` (`ESC [ ? 1 ; 2 c`, the answer the emulator gives
/// as today's terminal does).
#[farhelm_testtrace::test]
async fn a_device_attributes_query_is_answered_end_to_end() {
    let host = Host::start(shell(
        "stty raw -echo; printf '\\033[c'; od -An -tx1 -N7; exec /bin/sleep 600",
    ))
    .await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    wait_until(
        &terminal,
        "the answer read back",
        screen_has("1b 5b 3f 31 3b 32 63"),
    )
    .await;
}

/// Input reaches the program, and the child's exit records its status and
/// ends its root for good.
///
/// Why: Stop's and Delete's kill and sweep root at the child's pid only
/// while the host reports it alive, since a reaped pid may name any process
/// by now; the exit's status is what the session records; and the tab reap
/// learns of exits from the notifier rather than by polling. Spec: a child
/// that reads a line and exits 3, sent `go` and Enter, has that input
/// written, shows `read go`, and is then exited with status 3, with no
/// root, a probe that says dead with no pid, exit code 3, and the exit
/// announced on the notifier.
#[farhelm_testtrace::test]
async fn input_reaches_the_child_and_its_exit_ends_its_root() {
    let host = Host::start(shell(
        "printf 'ready\\n'; read line; printf 'read %s\\n' \"$line\"; exit 3",
    ))
    .await;
    let (notifier, mut changes) = LivenessNotifier::channel();
    let options = HostTerminalOptions {
        notifier: Some(notifier),
        ..HostTerminalOptions::default()
    };
    let terminal = host.adopt(Adoption::Fresh, options).await;
    wait_until(&terminal, "the prompt", screen_has("ready")).await;
    assert!(terminal.child_root().is_some(), "premise: the child runs");

    let receipt = within("queueing input", terminal.send_input(b"go\r"))
        .await
        .expect("the input queued");
    assert_eq!(
        within("the input's fate", receipt.fate()).await,
        InputFate::Written
    );
    wait_until(&terminal, "the exit", |terminal| {
        matches!(terminal.liveness(), Liveness::Exited { .. })
    })
    .await;
    assert!(terminal.screen_tail(4096).contains("read go"));
    assert_eq!(
        terminal.liveness(),
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
    assert_eq!(terminal.child_root(), None);
    assert_eq!(
        terminal.probe().unwrap(),
        PaneProbe::Owned(PaneProcess {
            pid: None,
            dead: true
        })
    );
    assert_eq!(terminal.exit_code(), Some(3));
    let change = within("the exit's announcement", changes.recv())
        .await
        .expect("the notifier is open");
    assert_eq!(change.socket, host.socket);
    assert_eq!(
        change.liveness,
        Liveness::Exited {
            exit: ChildExit::Code(3)
        }
    );
}

// ---- The clock ---------------------------------------------------------------------

/// A synchronized update the program never ends is drawn once the runtime's
/// clock passes its deadline, with the program silent.
///
/// Why: vte only records the 150 ms safety deadline, and a program that
/// opens an update and goes silent sends nothing more to trigger it; without
/// the runtime's wake-up its frame would never be drawn. Spec: with a manual
/// clock, a child that starts an update, prints `hidden` and sleeps has
/// those 14 bytes processed without `hidden` drawn; advancing the clock by
/// 150 ms draws it.
#[farhelm_testtrace::test]
async fn an_update_never_ended_is_drawn_at_its_deadline() {
    let host = Host::start(shell("printf '\\033[?2026hhidden'; exec /bin/sleep 600")).await;
    let clock = ManualClock::new();
    let options = HostTerminalOptions {
        clock: Clock::Manual(clock.clone()),
        ..HostTerminalOptions::default()
    };
    let terminal = host.adopt(Adoption::Fresh, options).await;
    wait_until(&terminal, "the update's bytes", |terminal| {
        terminal.status().position.offset >= 14
    })
    .await;
    assert!(
        !terminal.screen_tail(4096).contains("hidden"),
        "premise: the open update is held"
    );
    clock.advance(Duration::from_millis(150));
    wait_until(&terminal, "the flush", screen_has("hidden")).await;
}

// ---- Adoption ----------------------------------------------------------------------

/// A host spawned with an initial snapshot starts its terminal from it.
///
/// Why: Restart starts a new host seeded with the old terminal's history, and
/// the runtime must show that history above what the new program prints.
/// Spec: a host seeded with a snapshot of an emulator that printed
/// `from the seed`, whose child prints `after the seed`, shows both, in that
/// order.
#[farhelm_testtrace::test]
async fn a_host_seeded_with_a_snapshot_starts_from_it() {
    let mut seed = farhelm_terminal_emulator::Emulator::new(
        farhelm_terminal_emulator::Size {
            columns: 80,
            lines: 24,
        },
        1000,
    );
    let mut ignored = farhelm_terminal_emulator::Collected::default();
    seed.feed(
        b"from the seed\r\n",
        Instant::now(),
        farhelm_terminal_emulator::FeedMode::Live,
        &mut ignored,
    );
    let mut child = shell("printf 'after the seed\\n'; exec /bin/sleep 600");
    child.initial_snapshot = Some(seed.snapshot(MAX_SNAPSHOT).expect("clean").blob);
    let host = Host::start(child).await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    wait_until(&terminal, "the output", screen_has("after the seed")).await;
    let screen = terminal.screen_tail(4096);
    let seeded = screen.find("from the seed").expect("the seed's line");
    assert!(
        seeded < screen.find("after the seed").unwrap(),
        "{screen:?}"
    );
}

/// A large seed is restored before adoption returns, and the host is then
/// caught up, with nothing else to read; a seed that cannot be restored
/// fails the adoption.
///
/// Why: caught up is what a caller waits on before it reads the screen or a
/// Restart seed, and a host whose only state is its seed has nothing after
/// it to wait for; a large record is the one most likely to arrive after
/// the host's first answer following `Read`. Spec: a host seeded with 5,000
/// lines of history (several hundred KiB) and a silent child is caught up
/// when adoption returns, with the seed's last line on the screen; a host
/// seeded with 1 MiB that is not a snapshot fails the adoption as
/// unavailable for its snapshot.
#[farhelm_testtrace::test]
async fn a_large_seed_is_restored_before_adoption_returns() {
    let mut seed = farhelm_terminal_emulator::Emulator::new(
        farhelm_terminal_emulator::Size {
            columns: 80,
            lines: 24,
        },
        10_000,
    );
    let mut ignored = farhelm_terminal_emulator::Collected::default();
    for n in 0..5_000 {
        let line = format!("seed line {n:05} {}\r\n", "x".repeat(55));
        seed.feed(
            line.as_bytes(),
            Instant::now(),
            farhelm_terminal_emulator::FeedMode::Live,
            &mut ignored,
        );
    }
    let blob = seed.snapshot(MAX_SNAPSHOT).expect("clean").blob;
    assert!(blob.len() > 256 * 1024, "premise: a large seed");
    let mut child = shell("exec /bin/sleep 600");
    child.initial_snapshot = Some(blob);
    let host = Host::start(child).await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    assert!(terminal.status().caught_up, "{:?}", terminal.status());
    assert!(
        terminal.screen_tail(4096).contains("seed line 04999"),
        "{:?}",
        terminal.screen_tail(4096)
    );

    let mut child = shell("exec /bin/sleep 600");
    child.initial_snapshot = Some(vec![b'x'; 1 << 20]);
    let unreadable = Host::start(child).await;
    match HostTerminal::adopt(
        &unreadable.socket,
        Adoption::Fresh,
        HostTerminalOptions::default(),
    )
    .await
    {
        Err(AdoptError::Unavailable {
            reason: Unavailability::Snapshot(farhelm_terminal_emulator::SnapshotError::NotASnapshot),
            peer: Some(_),
        }) => {}
        other => panic!("expected the adoption refused for its snapshot, got {other:?}"),
    }
}

/// A runtime that recovers a running host rebuilds the same screen from the
/// host's journal, and pins the same child.
///
/// Why: a host outlives the supervisor that started it; the next one adopts
/// it, and its terminal must look as it did. Spec: after a first runtime
/// showed `first line` and was dropped, a recovered runtime catches up,
/// shows `first line`, and is running with the same root.
#[farhelm_testtrace::test]
async fn a_recovered_runtime_rebuilds_the_screen() {
    let host = Host::start(shell("printf 'first line\\n'; exec /bin/sleep 600")).await;
    let first = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    wait_until(&first, "the output", screen_has("first line")).await;
    let root = first.child_root().expect("a running root");
    drop(first);

    let second = host
        .adopt(Adoption::Recovered, HostTerminalOptions::default())
        .await;
    wait_until(&second, "the catch-up", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(second.screen_tail(4096).contains("first line"));
    assert_eq!(second.child_root(), Some(root));
}

// ---- Resizes and shutdown ----------------------------------------------------------

/// A resize reaches the host, and the emulator takes the size from the
/// host's resize record.
///
/// Why: a viewer's size must reach the program (through the host's
/// `TIOCSWINSZ`) and the emulator alike, in the output's order. Spec: a
/// resize to 100x30 leaves the terminal 100x30.
#[farhelm_testtrace::test]
async fn a_resize_reaches_the_host_and_the_emulator() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    let size = Size {
        cols: 100,
        rows: 30,
    };
    terminal.resize(size).expect("a valid size");
    wait_until(&terminal, "the resize", |terminal| terminal.size() == size).await;
}

/// Shutdown ends the host, confirmed by its process, and ends the runtime.
///
/// Why: Delete ends a session's hosts through the protocol, and must know
/// whether a host is really gone before it falls back to a signal. Spec:
/// shutting down a host whose child sleeps reports a goodbye and a
/// confirmed end with the host's process as the peer, which the test's own
/// check then finds ended (no such pid, or a zombie); the terminal is then
/// gone and refuses input.
#[farhelm_testtrace::test]
async fn shutdown_ends_the_host() {
    let host = Host::start(shell("exec /bin/sleep 600")).await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    let report = within("the shutdown", terminal.shutdown())
        .await
        .expect("the runtime shuts down");
    assert!(report.goodbye, "{report:?}");
    assert!(report.ended, "{report:?}");
    let host_pid = report.peer.expect("the host's process").pid;
    assert!(ended_within(host_pid), "the host {host_pid} still runs");
    assert_eq!(terminal.liveness(), Liveness::Gone);
    let refused = match within("input to an ended terminal", terminal.send_input(b"x")).await {
        Err(_) => true,
        Ok(receipt) => within("its fate", receipt.fate()).await == InputFate::NotSent,
    };
    assert!(refused, "input reached an ended terminal");
}

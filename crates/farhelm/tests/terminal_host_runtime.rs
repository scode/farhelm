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
//! recovery (checkpoints, the replay policy, gaps); resizes and shutdown.

mod terminal_host_support;

use farhelm_supervisor::hosts::terminal::ManualClock;
use farhelm_supervisor::hosts::{
    AdoptError, Adoption, CheckpointOutcome, ChildSpec, Clock, HostConnection, HostRole, HostTag,
    HostTerminal, HostTerminalOptions, InputFate, Liveness, LivenessNotifier, NotTaken,
    Unavailability, spawn_host,
};
use farhelm_supervisor::terminal_process::{TerminalProbe, TerminalProcess};
use farhelm_terminal_emulator::{ClipboardTarget, Notice};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use terminal_host::protocol::{ChildExit, MAX_SNAPSHOT, MIN_JOURNAL_CAP, Size};

/// How long any single wait in these tests may take before it fails.
const WAIT: Duration = Duration::from_secs(10);

// ---- Fixtures ------------------------------------------------------------------

/// Force a snapshot of the small streams used to distinguish replay,
/// checkpoint refusal and gap recovery. Skipping these snapshots would
/// erase the state each fixture is meant to recover or reject.
fn checkpointing_options() -> HostTerminalOptions {
    HostTerminalOptions {
        checkpoints: farhelm_supervisor::hosts::CheckpointPolicy {
            stop_min_bytes: 0,
            ..Default::default()
        },
        ..HostTerminalOptions::default()
    }
}

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

/// A shell pipeline stage that prints the bytes on its stdin as bare
/// lowercase hex pairs, with no separator and no line end (`ESC [ c` is
/// `1b5b63`), so that a test can find what a child read on its screen.
///
/// The spacing is removed rather than matched because `od` spaces its
/// columns differently per platform: GNU `od -An -tx1` writes ` 1b 5b 63`,
/// macOS's writes a wide indent and two spaces between bytes. Deleting every
/// blank leaves the same text on both. It loses nothing: `-tx1` writes every
/// byte as exactly two hex digits, so the digits alone still split into
/// bytes unambiguously, and `-v` makes `od` write every row, where it would
/// otherwise replace a row repeating the one before with a lone `*`.
const HEX_DUMP: &str = "od -An -v -tx1 | tr -d '[:space:]'";

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
        TerminalProbe::Owned(TerminalProcess {
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
/// DA1 and prints the next seven bytes it reads, in hex ([`HEX_DUMP`]),
/// shows `1b5b3f313b3263` (`ESC [ ? 1 ; 2 c`, the answer the emulator gives
/// as today's terminal does).
#[farhelm_testtrace::test]
async fn a_device_attributes_query_is_answered_end_to_end() {
    // `dd` reads the seven bytes, rather than `od -N7`, only so that this
    // test shares `HEX_DUMP` (which takes no byte count) with the replay test.
    let host = Host::start(shell(&format!(
        "stty raw -echo; printf '\\033[c'; dd bs=1 count=7 2>/dev/null | {HEX_DUMP}; \
         exec /bin/sleep 600"
    )))
    .await;
    let terminal = host
        .adopt(Adoption::Fresh, HostTerminalOptions::default())
        .await;
    wait_until(
        &terminal,
        "the answer read back",
        screen_has("1b5b3f313b3263"),
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
        TerminalProbe::Owned(TerminalProcess {
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

// ---- Recovery ------------------------------------------------------------------------

/// A FIFO named `name` in `dir`, for a child to wait on (`read go < FIFO`)
/// until the test releases it, or to report on (`echo done > FIFO`).
fn fifo(dir: &Path, name: &str) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    let path = dir.join(name);
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0, "mkfifo");
    path
}

/// Whether process `pid` still exists (a zombie included), for a FIFO
/// helper's diagnostics.
fn still_runs(pid: u32) -> &'static str {
    // SAFETY: signal 0 only checks for the process.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        "still runs"
    } else {
        "has ended"
    }
}

/// Let a child (process `child`) waiting on `fifo` go on, within `bound`.
///
/// Never blocks: the FIFO is opened for writing without blocking, which
/// fails until a reader has it open, and is tried again every 20 ms. A
/// blocking open would wait for the reader for good, and a blocked thread
/// cannot be cancelled, so a child that never came would hang the test.
async fn try_release(fifo: &Path, child: u32, bound: Duration) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let deadline = Instant::now() + bound;
    loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(fifo)
        {
            Ok(mut writer) => {
                return writer
                    .write_all(b"go\n")
                    .map_err(|error| format!("writing {}: {error}", fifo.display()));
            }
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {}
            Err(error) => return Err(format!("opening {}: {error}", fifo.display())),
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "nobody read {} within {bound:?}; the child {child} {}",
                fifo.display(),
                still_runs(child)
            ));
        }
        // sleep-ok: polling interval for a FIFO's reader, bounded above.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Wait, within `bound`, for a child (process `child`) to report a line on
/// `fifo`, and return it.
///
/// Never blocks: the FIFO is opened for reading without blocking, with a
/// writer of the test's own held open beside it, so that a read finds no
/// data yet rather than an end of file until the child writes; it is read
/// every 20 ms.
async fn try_reported(fifo: &Path, child: u32, bound: Duration) -> Result<String, String> {
    use std::io::Read as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let open = |write: bool| {
        std::fs::OpenOptions::new()
            .read(!write)
            .write(write)
            .custom_flags(libc::O_NONBLOCK)
            .open(fifo)
            .map_err(|error| format!("opening {}: {error}", fifo.display()))
    };
    let mut reader = open(false)?;
    let _writer = open(true)?;
    let deadline = Instant::now() + bound;
    let mut text = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        match reader.read(&mut chunk) {
            Ok(n) => text.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(format!("reading {}: {error}", fifo.display())),
        }
        if text.contains(&b'\n') {
            return Ok(String::from_utf8_lossy(&text).into_owned());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "nothing was reported on {} within {bound:?}; the child {child} {}",
                fifo.display(),
                still_runs(child)
            ));
        }
        // sleep-ok: polling interval for a FIFO's writer, bounded above.
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// [`try_release`] within [`WAIT`], failing the test if it gives up.
async fn release(fifo: &Path, child: u32) {
    try_release(fifo, child, WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
}

/// [`try_reported`] within [`WAIT`], failing the test if it gives up.
async fn reported(fifo: &Path, child: u32) {
    try_reported(fifo, child, WAIT)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
}

/// The FIFO helpers give up within their bound when nobody is at the
/// other end, and say whether the child still runs.
///
/// Why: a fixture whose child died or never reached its FIFO must fail
/// with a message within the test's bound, not hang the test process in
/// an open that waits for a peer that never comes (a thread blocked there
/// cannot be cancelled, and the test runtime would wait for it at exit).
/// Spec: on FIFOs nobody opens, with this process named as the child,
/// releasing and waiting for a report each fail within 300 ms bounds, the
/// two within 2 s in all, saying the child still runs.
#[farhelm_testtrace::test]
async fn fifo_helpers_give_up_within_their_bound() {
    let fifos = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let bound = Duration::from_millis(300);
    let own = std::process::id();
    let release = try_release(&fifo(fifos.path(), "nobody reads"), own, bound).await;
    let report = try_reported(&fifo(fifos.path(), "nobody writes"), own, bound).await;
    for failed in [release.map(|()| String::new()), report] {
        let error = failed.expect_err("nobody was at the other end");
        assert!(error.contains("still runs"), "{error}");
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
}

/// Drop `terminal`, standing in for a supervisor that crashed: no
/// checkpoint, no shutdown. Waits until its runtime has let go of the host,
/// so that output after this reaches no supervisor.
async fn crash(terminal: HostTerminal) {
    let mut status = terminal.subscribe();
    drop(terminal);
    within("the runtime to end", async {
        while status.changed().await.is_ok() {}
    })
    .await;
}

/// A child line that writes about 170 KB of plain lines, more than
/// [`MIN_JOURNAL_CAP`].
const FLOOD: &str = "i=0; while [ $i -lt 3000 ]; do \
     printf 'filler line %05d xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n' $i; i=$((i+1)); done";

/// A recovered runtime answers only the query no supervisor saw, and
/// reports only live clipboard writes (bells are never reported: a viewer
/// has no use for one): nothing from output in the
/// snapshot, nothing from output an earlier supervisor was sent after it,
/// and of output written while no supervisor was attached, the query only.
///
/// Why: this is the replay policy (D-7) end to end, the goal's three cases.
/// A query answered twice arrives at the program as typed garbage, one
/// never answered leaves it waiting, and a bell or clipboard write
/// delivered again surprises the user. The program here reads each answer
/// in turn, so a second answer to an old query would be what it reads
/// next. Spec: a raw-mode child asks DA1 and writes the clipboard and a
/// bell (answered live, then checkpointed and accepted), does the same
/// again (after the checkpoint, answered live), and, once the first runtime
/// has gone as if crashed, asks for status, writes the clipboard and a bell,
/// and reports that it asked; the first runtime shows everything before
/// that, and an `Info`-only connection (no `Read`) shows all 21 bytes of
/// the query, clipboard write and bell journaled past `delivered_end`
/// before the recovery adopts the host. The recovered runtime makes the
/// child print `1b5b306e` (in [`HEX_DUMP`]'s form), the status answer;
/// once released, the child writes `four` to the clipboard and rings, and
/// a notice subscription taken on the recovered runtime receives that
/// clipboard write first, and nothing else.
#[farhelm_testtrace::test]
async fn the_replay_answers_only_what_no_supervisor_saw() {
    let fifos = tempfile::tempdir().unwrap();
    let first = fifo(fifos.path(), "first");
    let second = fifo(fifos.path(), "second");
    let asked = fifo(fifos.path(), "asked");
    let third = fifo(fifos.path(), "third");
    let host = Host::start(shell(&format!(
        "stty raw -echo; \
         printf '\\033[c\\033]52;c;b25l\\007\\007'; \
         a=$(dd bs=1 count=7 2>/dev/null | {HEX_DUMP}); printf 'one:%s\\r\\n' \"$a\"; \
         read go < '{first}'; \
         printf '\\033[c\\033]52;c;dHdv\\007\\007'; \
         b=$(dd bs=1 count=7 2>/dev/null | {HEX_DUMP}); printf 'two:%s\\r\\nend-two' \"$b\"; \
         read go < '{second}'; \
         printf '\\033[5n\\033]52;c;dGhyZWU=\\007\\007'; echo asked > '{asked}'; \
         c=$(dd bs=1 count=4 2>/dev/null | {HEX_DUMP}); printf 'three:%s\\r\\n' \"$c\"; \
         read go < '{third}'; \
         printf '\\033]52;c;Zm91cg==\\007\\007'; \
         exec /bin/sleep 600",
        first = first.display(),
        second = second.display(),
        asked = asked.display(),
        third = third.display(),
    )))
    .await;
    let terminal = host.adopt(Adoption::Fresh, checkpointing_options()).await;
    let child = terminal.status().child_pid;
    wait_until(&terminal, "the first answer", screen_has("one:1b5b3f")).await;
    let deadline = tokio::time::Instant::now() + WAIT;
    let checkpoint = terminal.checkpoint_now(deadline).await;
    assert!(
        matches!(checkpoint, Ok(CheckpointOutcome::Accepted(_))),
        "{checkpoint:?}"
    );
    release(&first, child).await;
    // `end-two` is the last thing the child writes before the downtime
    // block: shown, it was delivered, so `delivered_end` is where that
    // block starts.
    wait_until(&terminal, "the second answer", screen_has("end-two")).await;
    crash(terminal).await;

    release(&second, child).await;
    reported(&asked, child).await;
    // The premise: the whole downtime block (the status query, 4 bytes;
    // the clipboard write, 16; the bell, 1) is journaled, past what any
    // client was sent. Less, and the rest of the block would reach the
    // recovered runtime live, its clipboard write and bell with it.
    const DOWNTIME_BLOCK: u64 = 4 + 16 + 1;
    let socket = host.socket.as_path();
    let journaled = within(
        "the downtime block to be journaled",
        terminal_host_support::poll_until(move || async move {
            let mut probe = HostConnection::connect(socket)
                .await
                .expect("connecting to the host");
            let info = probe.info().await.expect("the host's Info");
            info.end >= info.delivered_end + DOWNTIME_BLOCK
        }),
    )
    .await;
    assert!(journaled, "the downtime block was never journaled");

    let recovered = host
        .adopt(Adoption::Recovered, HostTerminalOptions::default())
        .await;
    let mut notices = recovered.subscribe_notices();
    wait_until(&recovered, "the third answer", screen_has("three:")).await;
    assert!(
        recovered.screen_tail(4096).contains("three:1b5b306e"),
        "{:?}",
        recovered.screen_tail(4096)
    );
    // Only now the live writes: the subscription, taken after adoption,
    // would miss notices from before it.
    release(&third, child).await;
    assert_eq!(
        within("the live clipboard write", notices.recv()).await,
        Some(Notice::Clipboard {
            target: ClipboardTarget::Clipboard,
            text: "four".into()
        })
    );
    // The bell came in the same write, so it would be queued by now.
    assert_eq!(notices.try_recv(), None, "a notice besides the write");
}

/// Output that overflows the journal while no supervisor is attached
/// leaves a gap that a recovery marks and repairs by asking the program to
/// redraw, which brings back a mode set in the lost output.
///
/// Why: a supervisor can be away for long (a crash not restarted at once)
/// while a program keeps writing; what the host trimmed is gone, and the
/// screen and modes rebuilt without it are stale until the program draws
/// again. Spec: a shell child with the smallest journal cap prints
/// `before`, is checkpointed, and with the first runtime gone as if
/// crashed, turns bracketed paste on, writes about 170 KB and prints
/// `after the gap`; on `SIGWINCH` it turns bracketed paste on again and
/// prints `winch`. A recovered runtime shows `after the gap`, marks the
/// history trimmed, and asks for a redraw: `winch` appears, and bracketed
/// paste is on.
#[farhelm_testtrace::test]
async fn an_overflow_while_away_is_marked_and_redrawn() {
    let fifos = tempfile::tempdir().unwrap();
    let go = fifo(fifos.path(), "go");
    let done = fifo(fifos.path(), "done");
    let mut child = shell(&format!(
        "trap 'printf \"\\033[?2004hwinch\\n\"' WINCH; \
         printf 'before\\n'; read go < '{go}'; \
         printf '\\033[?2004h'; {FLOOD}; printf 'after the gap\\n'; \
         echo done > '{done}'; \
         while :; do sleep 1; done",
        go = go.display(),
        done = done.display(),
    ));
    child.journal_cap = MIN_JOURNAL_CAP;
    let host = Host::start(child).await;
    let terminal = host.adopt(Adoption::Fresh, checkpointing_options()).await;
    let child = terminal.status().child_pid;
    wait_until(&terminal, "before", screen_has("before")).await;
    let deadline = tokio::time::Instant::now() + WAIT;
    let checkpoint = terminal.checkpoint_now(deadline).await;
    assert!(
        matches!(checkpoint, Ok(CheckpointOutcome::Accepted(_))),
        "{checkpoint:?}"
    );
    crash(terminal).await;

    release(&go, child).await;
    reported(&done, child).await;
    let recovered = host
        .adopt(Adoption::Recovered, HostTerminalOptions::default())
        .await;
    wait_until(&recovered, "the recovery", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(recovered.screen_tail(4096).contains("after the gap"));
    assert!(recovered.status().history_trimmed);
    wait_until(&recovered, "the redraw", screen_has("winch")).await;
    assert!(recovered.modes().bracketed_paste);
}

/// A program left mid-sequence and silent gets no checkpoint, so output
/// after it that overflows the journal while no supervisor is attached
/// ends in a gap, from which a recovery carries on with a clean parser.
///
/// Why: a snapshot is only valid at a clean boundary, and a program can
/// stay mid-sequence for good; the orderly stop must not wait for one, and
/// the recovery must not inherit the unfinished sequence. Spec: a child
/// prints `start` and an unfinished CSI and waits; an orderly checkpoint is
/// refused as `NotClean`; with the runtime gone, the child finishes the
/// sequence, writes about 170 KB with the smallest journal cap and prints
/// `end of output`; a recovered runtime shows `end of output` and marks
/// the history trimmed.
#[farhelm_testtrace::test]
async fn a_sequence_left_open_then_silence_ends_in_a_gap() {
    let fifos = tempfile::tempdir().unwrap();
    let go = fifo(fifos.path(), "go");
    let done = fifo(fifos.path(), "done");
    let mut child = shell(&format!(
        "printf 'start\\n\\033['; read go < '{go}'; printf 'm'; {FLOOD}; \
         printf 'end of output\\n'; echo done > '{done}'; exec /bin/sleep 600",
        go = go.display(),
        done = done.display(),
    ));
    child.journal_cap = MIN_JOURNAL_CAP;
    let host = Host::start(child).await;
    let terminal = host.adopt(Adoption::Fresh, checkpointing_options()).await;
    let child = terminal.status().child_pid;
    wait_until(&terminal, "the open sequence", |terminal| {
        terminal.status().position.offset >= 8
    })
    .await;
    let deadline = tokio::time::Instant::now() + WAIT;
    assert_eq!(
        terminal.checkpoint_now(deadline).await,
        Ok(CheckpointOutcome::Kept(NotTaken::NotClean))
    );
    crash(terminal).await;

    release(&go, child).await;
    reported(&done, child).await;
    let recovered = host
        .adopt(Adoption::Recovered, HostTerminalOptions::default())
        .await;
    wait_until(&recovered, "the recovery", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(recovered.screen_tail(4096).contains("end of output"));
    assert!(recovered.status().history_trimmed);
}

/// A single sequence longer than the journal cap, written while no
/// supervisor is attached, ends in a gap that a recovery carries on from.
///
/// Why: no snapshot can be taken inside a sequence, and the host keeps no
/// more than its cap, so a sequence longer than the cap always loses its
/// start; the recovery must take what follows as ordinary output, with a
/// clean parser, rather than wedge or wait for an end it never sees the
/// start of. Spec: with the smallest journal cap, a child prints `start`
/// and is checkpointed; with that runtime gone as if crashed, it prints a
/// title of 200,000 bytes and `done`; a recovered runtime shows `done` and
/// marks the history trimmed.
#[farhelm_testtrace::test]
async fn a_sequence_longer_than_the_cap_ends_in_a_gap() {
    let fifos = tempfile::tempdir().unwrap();
    let go = fifo(fifos.path(), "go");
    let done = fifo(fifos.path(), "done");
    let mut child = shell(&format!(
        "printf 'start\\n'; read go < '{go}'; \
         printf '\\033]2;'; head -c 200000 /dev/zero | tr '\\000' x; \
         printf '\\007done\\n'; echo done > '{done}'; exec /bin/sleep 600",
        go = go.display(),
        done = done.display(),
    ));
    child.journal_cap = MIN_JOURNAL_CAP;
    let host = Host::start(child).await;
    let terminal = host.adopt(Adoption::Fresh, checkpointing_options()).await;
    let child = terminal.status().child_pid;
    wait_until(&terminal, "start", screen_has("start")).await;
    let deadline = tokio::time::Instant::now() + WAIT;
    let checkpoint = terminal.checkpoint_now(deadline).await;
    assert!(
        matches!(checkpoint, Ok(CheckpointOutcome::Accepted(_))),
        "{checkpoint:?}"
    );
    crash(terminal).await;

    release(&go, child).await;
    reported(&done, child).await;
    let recovered = host
        .adopt(Adoption::Recovered, HostTerminalOptions::default())
        .await;
    wait_until(&recovered, "the recovery", |terminal| {
        terminal.status().caught_up
    })
    .await;
    assert!(recovered.screen_tail(4096).contains("done"));
    assert!(recovered.status().history_trimmed);
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

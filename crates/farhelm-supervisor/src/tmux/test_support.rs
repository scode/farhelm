//! Shared scratch-server harness for driver, stream, and sink tests.
//!
//! One harness keeps every real-tmux test under the same process cap and teardown rules.

use super::TmuxDriver;
use super::stream::{OutputEvent, OutputStream};
use std::process::Stdio;

/// A tmux server on a throwaway socket, killed on drop.
///
/// Tests that use this harness are the subset that start real tmux servers;
/// the remaining tests do not start a server, though some invoke tmux or use
/// other process-backed fixtures. Teardown is drop-based for the same reason
/// the e2e harness uses it: a test that fails an assertion never reaches an
/// explicit cleanup call, and leaked tmux servers accumulate across runs.
pub(super) struct ScratchServer {
    pub(super) driver: TmuxDriver,
    /// Also the scratch space tests put out-of-band fixtures in — the
    /// progress files a filtered pane writes so its liveness is
    /// observable when its output reaches nobody (see
    /// [`read_progress`]). It outlives the server by drop order.
    pub(super) dir: tempfile::TempDir,
    /// Released once this server is gone, letting the next real-tmux
    /// test start its own — see [`REAL_TMUX_SLOTS`]. Declared last so
    /// it is released only after the tempdir and driver have been,
    /// which is after `Drop` has killed the server.
    _slot: tokio::sync::SemaphorePermit<'static>,
}

/// Caps how many real-tmux tests in THIS binary run at once.
///
/// libtest runs every test in a binary concurrently and bounds only
/// the thread count, not what those threads start. Some stream and sink
/// stress tests run high-volume panes specifically designed to flood in
/// bursts. Every real-tmux test shares this cap so those flooders cannot load
/// the machine enough that the multi-second timeouts elsewhere stop meaning
/// what they say, or make a pane that simply never got scheduled read as a
/// filter that failed or a client that vanished.
///
/// Two permits rather than one: these tests are dominated by waiting
/// on a real tmux, so serializing them outright would roughly double
/// the suite's wall time for no extra signal, while the step from two
/// to unbounded is where the flooders start competing. Mirrors the
/// e2e suite's own `SLOTS`, which exists for the same reason.
static REAL_TMUX_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

impl Drop for ScratchServer {
    fn drop(&mut self) {
        let _ = std::process::Command::new("tmux")
            .arg("-S")
            .arg(&self.driver.socket)
            .arg("kill-server")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl ScratchServer {
    /// Take a concurrency slot, then start a server on a fresh
    /// socket. The slot is acquired here rather than in each test so
    /// that standing up a real tmux is the thing that is bounded —
    /// every scratch server supplied by this shared harness goes
    /// through this constructor.
    pub(super) async fn start() -> ScratchServer {
        let slot = REAL_TMUX_SLOTS
            .acquire()
            .await
            .expect("semaphore is never closed");
        let dir = tempfile::tempdir().expect("tempdir");
        let driver = TmuxDriver::new(dir.path());
        driver.ensure_server().await.expect("tmux server");
        ScratchServer {
            driver,
            dir,
            _slot: slot,
        }
    }
}

/// Pull events until one satisfies `want`, failing rather than
/// hanging if the stream goes quiet. Returns everything decoded along
/// the way so callers can assert on the bytes that preceded the event.
pub(super) async fn pump_until(
    stream: &mut OutputStream,
    secs: u64,
    want: impl Fn(&OutputEvent) -> bool,
) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut bytes = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, stream.next_output())
            .await
            .unwrap_or_else(|_| panic!("stream went quiet after {} bytes", bytes.len()))
            .expect("control stream failed")
            .expect("control client exited");
        if want(&event) {
            return bytes;
        }
        if let OutputEvent::Bytes(chunk) = event {
            bytes.extend_from_slice(&chunk);
        }
    }
}

/// Finish an output stream's fail-closed shutdown in real-tmux tests.
///
/// A one-shot failure deliberately returns ownership instead of dropping
/// the client. Production publishes that reaper through the per-terminal
/// registry; a direct unit test has no registry, so it must drive the same
/// retry object itself or risk leaking the very unsafe state under test.
pub(super) async fn shutdown_test_stream(stream: OutputStream) {
    if let Err(reaper) = stream.shutdown().await {
        reaper
            .run()
            .await
            .expect("the output client must eventually shut down safely");
    }
}
/// Read this stream continuously until `ticks` of its OWN pane's
/// heartbeat have arrived, failing on anything that is not a clean
/// read.
///
/// Every loop here drives [`OutputStream::next_output`] to completion
/// rather than racing it against a timer. That is not style: that
/// future is documented as NOT cancel-safe, so a `timeout(_,
/// next_output())` loop — the obvious way to "read for a while" —
/// abandons a partially-read line on every tick and, now that the
/// foreign-pane path writes, can abandon a partially-written command
/// too. A test built that way exercises a state production never
/// reaches and can pass or fail for reasons unrelated to its subject.
/// Own-pane ticks are the clock instead, which also makes the
/// observation window a number of PROVEN round trips rather than a
/// duration that a loaded machine can turn into nothing.
///
/// An `Err` or an EOF fails outright. Both mean the control client is
/// gone, and a loop that swallowed them would keep counting an empty
/// stream as "no foreign notifications arrived" — the exact reading
/// these tests are trying to earn. That failure is reported through
/// [`client_gone_report`] rather than a bare `expect`, because it has
/// happened on CI three times and produced no evidence whatsoever
/// about WHY the client vanished.
///
/// `server` is taken purely for that report: the stream itself cannot
/// say anything about the tmux server behind it.
pub(super) async fn pump_own_pane_ticks(
    server: &ScratchServer,
    stream: &mut OutputStream,
    ticks: usize,
    secs: u64,
) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut seen = 0;
    while seen < ticks {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let event = tokio::time::timeout(remaining, stream.next_output())
            .await
            .unwrap_or_else(|_| {
                panic!("only {seen} of {ticks} own-pane ticks arrived within {secs}s")
            });
        let event = match event {
            Ok(Some(event)) => event,
            Ok(None) => {
                let report = client_gone_report(server, stream).await;
                panic!("the control client exited after {seen} of {ticks} ticks; {report}");
            }
            Err(e) => {
                let report = client_gone_report(server, stream).await;
                panic!(
                    "the control stream failed after {seen} of {ticks} ticks: {e:#}; \
                     {report}"
                );
            }
        };
        match event {
            OutputEvent::Bytes(bytes) if contains(&bytes, b"AGENT-TICK") => seen += 1,
            OutputEvent::Bytes(_) => {}
            // Recovered exactly the way production recovers (the
            // forwarder's `catch_up_after_tmux_pause`), and then the
            // tick count simply carries on. A pause is a legitimate
            // thing for tmux to do to a client that fell behind — this
            // helper's own callers can provoke one by flooding a
            // neighbouring pane — and treating it as fatal, or merely
            // failing to resume, would turn it into a 30-second "ticks
            // never arrived" that blames the wrong mechanism entirely.
            // The replay's own content is deliberately NOT counted:
            // those are history, not the fresh round trips these tests
            // use as their clock.
            OutputEvent::Paused => {
                stream
                    .resume_paused_with_replay()
                    .await
                    .expect("a paused own pane must recover through the replay path");
            }
        }
    }
}

/// Everything cheap to know about why a control client is no longer
/// there, gathered at the moment of failure.
///
/// Exists because the alternative is what this suite actually had: a
/// bare "the control client must not exit mid-test", which says only
/// that the thing being measured stopped existing. Each piece answers
/// a different candidate cause — tmux's own `%exit` reason says
/// whether tmux announced the end or the pipe simply died; the child's
/// wait status distinguishes a crashed or signalled `tmux -C` from one
/// still running with a broken pipe; and the server probes say whether
/// the whole scratch server went away underneath it (the shape a
/// loaded CI box, an OOM kill, or a stray `kill-server` takes).
///
/// Every probe is best-effort and its failure is folded into the text
/// rather than raised: this runs on a path that is already panicking,
/// and a probe that panics first would destroy the very report it was
/// gathering. The same reasoning is why the two tmux probes are
/// bounded by [`REPORT_PROBE_TIMEOUT`] — the most likely reason a
/// control client vanished is a tmux in trouble, and asking a wedged
/// tmux a question through the UNBOUNDED [`TmuxDriver::run`] would
/// turn a test that was about to fail with a diagnosis into one that
/// hangs until the harness kills it.
async fn client_gone_report(server: &ScratchServer, stream: &mut OutputStream) -> String {
    let exit_reason = match stream.exit_reason.as_deref() {
        Some("") => "tmux announced a bare %exit with no reason".to_string(),
        Some(reason) => format!("tmux said: %exit {reason}"),
        None => "clean EOF, no %exit was ever announced".to_string(),
    };
    let child = match stream.child.try_wait() {
        Ok(Some(status)) => format!("the tmux -C child exited: {status}"),
        Ok(None) => "the tmux -C child is still running".to_string(),
        Err(e) => format!("the tmux -C child could not be waited on: {e}"),
    };
    let sessions = probe(
        server,
        "sessions",
        &["list-sessions", "-F", "#{session_name}"],
    )
    .await;
    let clients = probe(
        server,
        "clients",
        &["list-clients", "-F", "#{client_flags}"],
    )
    .await;
    format!("{exit_reason}; {child}; {sessions}; {clients}")
}

/// How long one of [`client_gone_report`]'s tmux probes may take
/// before the report records that it hung instead of what it found.
///
/// Short on purpose. These questions are one subprocess round trip
/// against a server that is, at worst, on the same box — anything
/// slower is itself the answer, and the panic this report decorates is
/// already overdue by the time it runs.
const REPORT_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// One bounded tmux query for [`client_gone_report`], rendered as a
/// report fragment whether it answered, failed, or hung.
async fn probe(server: &ScratchServer, label: &str, args: &[&str]) -> String {
    match tokio::time::timeout(REPORT_PROBE_TIMEOUT, server.driver.run(args)).await {
        Ok(Ok(out)) => format!("{label}: [{}]", out.trim().replace('\n', ", ")),
        Ok(Err(e)) => format!("{label} query failed: {e:#}"),
        Err(_) => format!(
            "{label} query did not answer within {REPORT_PROBE_TIMEOUT:?} — tmux itself \
             is not responding"
        ),
    }
}
/// The counter a [`bursting_pane`] keeps outside tmux, or 0 before it
/// has written one.
///
/// Read from a file rather than from the pane's own output because the
/// whole question is what happens when that output is NOT being
/// delivered to anyone the test can see. A frozen pane and a filtered
/// pane look identical on a control client; they differ only here.
pub(super) fn read_progress(path: &std::path::Path) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

/// A pane command that prints `<label>-TICK` five times a second,
/// forever — a terminal whose liveness a test can keep re-checking
/// instead of racing one echo.
pub(super) fn ticking_pane(label: &str) -> Vec<String> {
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!("while :; do echo {label}-TICK; sleep 0.2; done"),
    ]
}

/// A pane command that emits bursts of 5000 lines a second apart, and
/// records how many bursts it has completed in `progress`.
///
/// Bursty rather than free-running on purpose. An unpaced `echo` loop
/// pins one tmux server at 100% CPU for as long as the test runs, and
/// with this suite's parallelism that starved unrelated panes badly
/// enough to make a quiet terminal's own output arrive seconds late —
/// the test's producer becoming the test's flakiness. A burst is just
/// as unmistakable to a filter that is not working (thousands of
/// notifications per second while it lasts) at a fraction of the
/// average cost.
///
/// The counter is written BEFORE each burst rather than after, so it
/// advances as soon as the pane is running again, and it is the only
/// evidence a test has that the pane is alive at all once its output
/// is filtered away from every client (see [`read_progress`]). A pane
/// tmux has stopped reading blocks inside the burst, mid-`echo`, and
/// so simply stops updating it.
pub(super) fn bursting_pane(label: &str, progress: &std::path::Path) -> Vec<String> {
    let progress = progress.display();
    vec![
        "sh".to_string(),
        "-c".to_string(),
        format!(
            "n=0; while :; do n=$((n+1)); echo $n > {progress}; \
             i=0; while [ $i -lt 5000 ]; do echo {label}-$i; i=$((i+1)); done; \
             sleep 1; done"
        ),
    ]
}

/// Whether `haystack` contains `needle`, for assertions over decoded
/// pane bytes.
pub(super) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
/// Poll `capture_pane_tail` until the pane has actually rendered
/// `want`, rather than sampling once and racing tmux's own writes.
///
/// Returns the tail that satisfied the wait so the caller can assert
/// on the same bytes it waited for — sampling twice would reintroduce
/// exactly the race this exists to close.
pub(super) async fn tail_containing(
    driver: &TmuxDriver,
    session: &str,
    pane: &str,
    want: &str,
) -> String {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let tail = driver
            .capture_pane_tail(session, pane, 64 * 1024)
            .await
            .expect("capture the pane's visible grid");
        if tail.contains(want) {
            return tail;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the pane never rendered {want:?}; last tail was:\n{tail}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

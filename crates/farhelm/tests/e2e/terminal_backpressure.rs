//! Pause/resume backpressure on the terminal path, both the shallow and
//! deep recovery regimes.

use crate::harness::*;

// ---------------------------------------------------------------------
// Terminal-path backpressure (PLAN_M2_5.md)
//
// Everything below drives the real pause/resume control messages against
// a real tmux with `pause-after` set, which is the only way to observe
// the two genuinely different catch-up regimes: a SHALLOW pause, lifted
// before tmux gives up, where delivery must be lossless and continuous;
// and a DEEP one, where tmux has cut the stream and the supervisor must
// recover by resetting the client's terminal and replaying history.
// ---------------------------------------------------------------------

/// Extract `FLOOD-NNNNNNNN` record numbers from a raw transcript, in
/// order.
///
/// Byte-oriented and deliberately tolerant of records the stream split
/// (at a notification boundary, or across the catch-up's reset): a
/// half-record simply is not a record. That tolerance is what lets the
/// assertions below be about ORDER — strictly increasing numbers prove
/// both no reordering and no duplicated replay — rather than about exact
/// framing, which no layer on this path promises.
fn flood_records(transcript: &[u8]) -> Vec<u64> {
    const PREFIX: &[u8] = b"FLOOD-";
    const DIGITS: usize = 8;

    transcript
        .windows(PREFIX.len() + DIGITS)
        .filter(|record| record.starts_with(PREFIX))
        .filter_map(|record| {
            std::str::from_utf8(&record[PREFIX.len()..])
                .ok()?
                .parse()
                .ok()
        })
        .collect()
}

/// Assert flood records are exactly consecutive, naming the offending
/// pair.
///
/// Consecutive rather than merely increasing, deliberately: "increasing"
/// is satisfied by a bug that drops every second record, which is exactly
/// the class of loss a flow-control change could introduce. Duplication
/// and reordering both show up as a step that is not +1 as well, so this
/// one predicate covers every way the byte stream could go wrong that a
/// numbered producer can express.
fn assert_records_consecutive(records: &[u64], what: &str, allowed_seams: usize) {
    let mut seams = 0;
    for pair in records.windows(2) {
        if pair[1] == pair[0] + 1 {
            continue;
        }
        // Exactly one record missing, at most `allowed_seams` times: a
        // record straddling a replay/live boundary is delivered as two
        // halves with the replay's own mode sequences between them, so
        // the scanner matches neither half. `counter_records` documents
        // the same effect for the attach cutover. Every OTHER shape —
        // a wider gap, a repeat, a step backwards — is real loss,
        // duplication, or reordering and fails immediately.
        if pair[1] == pair[0] + 2 && seams < allowed_seams {
            seams += 1;
            continue;
        }
        panic!(
            "{what}: record {} follows {} — output was lost, duplicated, or reordered",
            pair[1], pair[0]
        );
    }
}

/// A quiet period that rearms on data without moving its hard outer bound.
///
/// This state is separate from the receive loop so the load-bearing timing
/// rule has deterministic coverage: late data rearms the quiet deadline, but
/// the effective wait is clipped by the fixed overall bound.
struct QuietDeadline {
    quiet_window: Duration,
    quiet_deadline: tokio::time::Instant,
    overall_deadline: tokio::time::Instant,
}

impl QuietDeadline {
    /// Start a quiet window inside a strictly longer overall wait budget.
    ///
    /// Panics when the quiet window is not shorter than the outer bound:
    /// that shape cannot demonstrate rearming before the hard stop clips it.
    fn new(now: tokio::time::Instant, quiet_window: Duration, overall_bound: Duration) -> Self {
        assert!(quiet_window < overall_bound);
        Self {
            quiet_window,
            quiet_deadline: now + quiet_window,
            overall_deadline: now + overall_bound,
        }
    }

    /// Rearm the nominal quiet deadline without moving the hard outer bound.
    fn observe_data(&mut self, now: tokio::time::Instant) {
        self.quiet_deadline = now + self.quiet_window;
    }

    /// Return the rearmed quiet deadline clipped to the hard outer bound.
    fn next_deadline(&self) -> tokio::time::Instant {
        self.quiet_deadline.min(self.overall_deadline)
    }
}

/// Numbered flood progress and its no-progress deadline.
///
/// Duplicate, old, and unrelated bytes must not keep a stalled producer
/// alive forever. Only a record newer than every one already observed earns
/// another complete stall window.
struct FloodProgress {
    latest: Option<u64>,
    records_seen: u64,
    deadline: tokio::time::Instant,
    stall_timeout: Duration,
}

impl FloodProgress {
    /// Start a no-progress window with no numbered record observed yet.
    fn new(now: tokio::time::Instant, stall_timeout: Duration) -> Self {
        Self {
            latest: None,
            records_seen: 0,
            deadline: now + stall_timeout,
            stall_timeout,
        }
    }

    /// Record strictly newer progress and rearm its stall deadline.
    ///
    /// Duplicate or older records change nothing: replayed bytes must not
    /// keep a genuinely stalled producer alive forever.
    fn observe(&mut self, record: u64, now: tokio::time::Instant) {
        if self.latest.is_some_and(|previous| record <= previous) {
            return;
        }
        self.latest = Some(record);
        self.records_seen += 1;
        self.deadline = now + self.stall_timeout;
    }
}

/// Create a session running the `flood` script — the fast producer every
/// backpressure test needs. Returns the workdir for the caller to hold,
/// exactly like [`basic_session`].
pub(crate) async fn flood_session(h: &Harness) -> (SessionInfo, farhelm_teststate::TestDir) {
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script flood"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    (session, work)
}

/// Drain an attachment into `seen` for `window`, returning any detach
/// reason that arrived.
///
/// Distinct from [`wait_for`] because these tests need to observe the
/// ABSENCE of something (no reset during a shallow pause) or to keep
/// reading through a period with no particular marker due — neither of
/// which a needle-driven wait can express.
pub(crate) async fn drain_for(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    window: Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // The replay-complete marker (PLAN_M5.md item 4) is
            // presentation metadata this backpressure suite does not
            // assert on — it is about pause/resume byte delivery, not
            // the catch-up boundary, which has its own coverage in
            // replay_marker.rs and, helm-side, in farhelm-helm's
            // client.rs/lib.rs.
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => return Some(reason),
            Ok(None) => return Some("closed".to_string()),
            Err(_) => return None,
        }
    }
}

/// Drain output left in flight by a pause until delivery stays quiet.
///
/// A fixed sleep is not a pause barrier: on a loaded runner the supervisor
/// may still be emptying frames already queued toward this attachment when
/// that sleep ends. RSS sampled there measures the transition into a stall,
/// not the bounded stalled state. The quiet window rearms on every data
/// frame, while the overall bound makes a backlog that never settles fail
/// instead of waiting forever. Silence here does not acknowledge tmux's
/// internal pause state; the producer is idle during this drain.
async fn drain_until_quiet(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    quiet_window: Duration,
    overall_bound: Duration,
) {
    let mut deadline = QuietDeadline::new(tokio::time::Instant::now(), quiet_window, overall_bound);

    loop {
        let now = tokio::time::Instant::now();
        assert!(
            now < deadline.overall_deadline,
            "paused delivery never stayed quiet for {quiet_window:?} within {overall_bound:?}; \
             {} bytes seen, recent records: {:?}",
            seen.len(),
            flood_records(&seen[seen.len().saturating_sub(4096)..])
                .into_iter()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
        );
        let next_deadline = deadline.next_deadline();
        match tokio::time::timeout(next_deadline.saturating_duration_since(now), rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => {
                seen.extend_from_slice(&bytes);
                deadline.observe_data(tokio::time::Instant::now());
            }
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!("stream detached ({reason}) while paused output was settling")
            }
            Ok(None) => panic!("stream closed while paused output was settling"),
            Err(_) if tokio::time::Instant::now() >= deadline.quiet_deadline => return,
            Err(_) => panic!(
                "paused delivery did not settle within {overall_bound:?}; {} bytes seen",
                seen.len()
            ),
        }
    }
}

/// Read until `needle` appears in the transcript, scanning only what is
/// newly arrived.
///
/// [`wait_for`] cannot be used by these tests: it re-runs
/// `String::from_utf8_lossy(seen).contains(...)` over the WHOLE
/// transcript after every chunk, which allocates a fresh copy of it each
/// time and is quadratic in its length. That is fine for the kilobyte
/// transcripts the rest of this file works with and ruinous for the
/// multi-megabyte ones here — ruinous in a particularly misleading way,
/// too: the test itself becomes the slow consumer, which provokes the
/// very tmux-side pause it is trying to observe under controlled
/// conditions. This keeps a cursor instead, overlapping by `needle.len()
/// - 1` bytes so a needle straddling a chunk boundary is still found.
async fn wait_for_bytes(rx: &mut TermStream, seen: &mut Vec<u8>, needle: &[u8], secs: u64) {
    assert!(!needle.is_empty(), "an empty needle is always present");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    let mut scanned = 0;
    loop {
        if seen[scanned..]
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return;
        }
        scanned = seen.len().saturating_sub(needle.len() - 1);
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            // See `drain_for`'s twin arm above: presentation-only, not
            // asserted on by this needle scan.
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!(
                    "stream ended ({reason}) without {needle:?} in {} bytes",
                    seen.len()
                )
            }
            Ok(None) => panic!("stream closed without {needle:?} in {} bytes", seen.len()),
            Err(_) => panic!(
                "timed out waiting for {needle:?}; {} bytes seen, last records: {:?}",
                seen.len(),
                flood_records(&seen[seen.len().saturating_sub(4096)..])
                    .into_iter()
                    .rev()
                    .take(3)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// Wait for a flood phase marker while numbered delivery keeps advancing.
///
/// A loaded runner may spend longer than one flat deadline draining the
/// phase's megabytes while record numbers visibly advance. Every new number
/// therefore rearms a no-progress deadline; bytes that arrive without record
/// progress do not. This is a bounded stall detector, not a larger timeout.
///
/// Like [`wait_for_bytes`], this scans only newly arrived bytes plus one
/// record's overlap. Re-scanning the full flood would make the test itself
/// the slow consumer whose memory behavior it is trying to measure.
async fn wait_for_flood_marker_with_progress(
    rx: &mut TermStream,
    seen: &mut Vec<u8>,
    marker: &[u8],
) {
    const RECORD_BYTES: usize = b"FLOOD-00000000".len();
    const STALL_TIMEOUT: Duration = Duration::from_secs(30);

    assert!(!marker.is_empty(), "an empty marker is always present");
    let mut scanned = 0;
    let mut progress = FloodProgress::new(tokio::time::Instant::now(), STALL_TIMEOUT);

    loop {
        let new_bytes = &seen[scanned..];
        if new_bytes
            .windows(marker.len())
            .any(|window| window == marker)
        {
            return;
        }
        for record in flood_records(new_bytes) {
            progress.observe(record, tokio::time::Instant::now());
        }

        scanned = seen
            .len()
            .saturating_sub(RECORD_BYTES.max(marker.len()) - 1);
        let remaining = progress
            .deadline
            .saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(TermEvent::Data(bytes))) => seen.extend_from_slice(&bytes),
            Ok(Some(TermEvent::ReplayComplete)) => {}
            Ok(Some(TermEvent::Detached(reason))) => {
                panic!(
                    "stream detached ({reason}) before flood marker {marker:?}; {} bytes, \
                     latest={:?}, records_seen={}",
                    seen.len(),
                    progress.latest,
                    progress.records_seen
                )
            }
            Ok(None) => panic!(
                "stream closed before flood marker {marker:?}; {} bytes, latest={:?}, \
                 records_seen={}",
                seen.len(),
                progress.latest,
                progress.records_seen
            ),
            Err(_) => panic!(
                "flood delivery made no record progress for {STALL_TIMEOUT:?} while waiting \
                 for {marker:?}; {} bytes, latest={:?}, records_seen={}, \
                 recent={:?}",
                seen.len(),
                progress.latest,
                progress.records_seen,
                flood_records(&seen[seen.len().saturating_sub(4096)..])
                    .into_iter()
                    .rev()
                    .take(5)
                    .collect::<Vec<_>>()
            ),
        }
    }
}

/// Late data rearms the quiet deadline, subject to the fixed hard bound.
#[test]
fn quiet_deadline_rearms_but_keeps_its_overall_bound() {
    let base = tokio::time::Instant::now();
    let mut deadline = QuietDeadline::new(base, Duration::from_secs(1), Duration::from_secs(2));

    deadline.observe_data(base + Duration::from_millis(900));
    assert_eq!(deadline.quiet_deadline, base + Duration::from_millis(1900));
    assert_eq!(deadline.next_deadline(), deadline.quiet_deadline);

    deadline.observe_data(base + Duration::from_millis(1500));
    assert_eq!(deadline.quiet_deadline, base + Duration::from_millis(2500));
    assert_eq!(deadline.next_deadline(), deadline.overall_deadline);
}

/// Only a newer numbered record may rearm the flood's stall detector.
#[test]
fn flood_progress_deadline_ignores_duplicate_old_and_unrelated_bytes() {
    let base = tokio::time::Instant::now();
    let stall = Duration::from_secs(1);
    let mut progress = FloodProgress::new(base, stall);

    progress.observe(10, base + Duration::from_millis(100));
    let first_deadline = progress.deadline;
    progress.observe(10, base + Duration::from_millis(500));
    progress.observe(9, base + Duration::from_millis(700));
    assert_eq!(progress.deadline, first_deadline);
    assert!(flood_records(b"noise without a numbered record").is_empty());
    assert_eq!(progress.deadline, first_deadline);

    progress.observe(11, base + Duration::from_millis(900));
    assert_eq!(progress.deadline, base + Duration::from_millis(1900));
    assert_eq!(progress.records_seen, 2);
}

/// How many records the `flood` fake-agent script emits. Duplicated from
/// `fake_agent::FLOOD_RECORDS` because that module is private to the bin
/// crate. Only ever used to recognize a COMPLETE producer run, so drift
/// here weakens an assertion rather than causing a false failure.
const FLOOD_RECORDS: u64 = 800_000;

/// Force tmux to pause the SUPERVISOR's own output client for `pane`,
/// exactly as `pause-after` would after a stall — but immediately and
/// deterministically.
///
/// This is what makes the reset-then-replay catch-up testable at all.
/// Whether the delay-driven pause actually fires depends on how far tmux
/// happened to read ahead of the client before it stalled, and both
/// outcomes occur on every supported tmux generation (see the
/// either-behavior test below), so a test that waits for one is asserting
/// a race. tmux's documented on-demand form reaches the identical pane
/// state.
///
/// It needs no test-only seam in the supervisor, which is why it is done
/// this way: `refresh-client -A` acts on a NAMED client, and the
/// supervisor's control clients are distinguishable from outside by their
/// flags. The discriminator is `pause-after`, and it has to be: three
/// shapes are attached to a session with one live terminal, and only this
/// one carries that flag. The input client keeps `no-output` set forever
/// (see `InputClient`); the session sink clears `no-output` exactly as the
/// output client does but deliberately never takes `pause-after`, because
/// it is the client that must never be paused (see `tmux::SessionSink`) —
/// which is what makes the same flag both the sink's defining absence and
/// this helper's positive match.
pub(crate) async fn force_tmux_pause(h: &Harness, pane: &str) {
    let sock = h.state.path().join("tmux.sock");
    let listed = tmux_query(
        &sock,
        &["list-clients", "-F", "#{client_name}\t#{client_flags}"],
    )
    .await;
    assert!(
        listed.status.success(),
        "listing tmux clients failed: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed = String::from_utf8_lossy(&listed.stdout).into_owned();
    let target = listed
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, flags)| {
            flags
                .split(',')
                .any(|flag| flag.starts_with("pause-after="))
        })
        .map(|(name, _)| name)
        .unwrap_or_else(|| panic!("no output control client found among tmux clients:\n{listed}"));
    let paused = tmux_query(
        &sock,
        &[
            "refresh-client",
            "-t",
            target,
            "-A",
            &format!("{pane}:pause"),
        ],
    )
    .await;
    assert!(
        paused.status.success(),
        "forcing a tmux pane pause failed: {}",
        String::from_utf8_lossy(&paused.stderr)
    );
}

/// A forced tmux-side pause must be recovered THROUGH THE REAL
/// ATTACHMENT: terminal reset, history replayed, live output resuming.
///
/// The deterministic counterpart to the either-behavior test below, and
/// the only coverage that runs the FORWARDER's reset-then-replay send on
/// every CI run. The either-behavior test exercises this path only when
/// tmux happens to choose the read-ahead branch, which it did in 1 of 13
/// measured runs across 3.3a/3.4/3.7b — real coverage, but not coverage
/// anything may depend on. Here the pause is forced, so a regression in
/// the reset, the replay, or the continue cutover fails every time.
#[tokio::test]
async fn a_forced_tmux_pause_is_recovered_through_the_real_attachment() {
    let h = harness().await;
    // The counter fixture, not the flood: this test asserts that LIVE
    // output resumes after the replay, and a producer that can finish
    // makes that unfalsifiable — "no further records" would then be
    // correct rather than a pane left paused. `counter` runs until its
    // session is killed.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    // Enough history accumulated that the replay below is unmistakably a
    // history replay rather than a screenful.
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-00001200", 60).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;

    // The reset proves the catch-up ran rather than the stream merely
    // continuing; without it the replay would land on top of content the
    // client still held.
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    // `wait_for_bytes` returns on the reset itself, so the replay that
    // FOLLOWS it has not been read yet — keep draining before asserting.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let reset_at = before_pause
        + seen[before_pause..]
            .windows(2)
            .position(|window| window == b"\x1bc")
            .expect("wait_for_bytes already proved the reset arrived");

    let replayed = counter_records(&seen[reset_at..]);
    assert!(
        replayed.len() > 1000,
        "the catch-up replayed only {} records; history was not replayed",
        replayed.len()
    );
    assert_records_consecutive(&replayed, "forced-pause catch-up replay", 1);

    // Live output must resume after the replay: a continue that returned
    // a snapshot but left the pane paused looks identical up to here and
    // leaves the terminal dead.
    let last_replayed = *replayed.last().expect("non-empty");
    let target = format!("CUTOVER-{:08}", last_replayed + 50);
    wait_for_bytes(&mut rx, &mut seen, target.as_bytes(), 60).await;
}

/// The same forced catch-up against an ALTERNATE-SCREEN pane, which
/// selects a different snapshot and a different mode-replay path.
///
/// PLAN_M2_5.md requires the catch-up to be correct on the alternate
/// screen as well as the normal one, and the two share only the command
/// group: alt-screen replay must select the VISIBLE snapshot (never the
/// normal screen's history, which would splice unrelated scrollback into
/// a full-screen app) and must re-enter the alternate buffer BEFORE the
/// content, since `\x1b[?1049h` clears the buffer it switches to. A
/// regression in either shows up here and in no normal-screen test.
#[tokio::test]
async fn a_forced_tmux_pause_recovers_an_alternate_screen_pane() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script altscreen"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FAKE-AGENT READY", 30).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    // The replay follows the reset marker; drain it before asserting.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let recovered = &seen[before_pause..];
    let reset_at = recovered
        .windows(2)
        .position(|window| window == b"\x1bc")
        .expect("wait_for_bytes already proved the reset arrived");
    let after_reset = String::from_utf8_lossy(&recovered[reset_at..]).into_owned();
    let enter_alt = after_reset
        .find("\x1b[?1049h")
        .expect("an alternate-screen pane must re-enter the alternate buffer after the reset");
    let content = after_reset
        .find("ALT-SCREEN APP")
        .expect("the catch-up must replay the alternate screen's own content");
    assert!(
        enter_alt < content,
        "the alternate-screen switch must precede the replayed content — it CLEARS the buffer \
         it switches to, so emitting it afterwards would wipe the replay"
    );
}

/// A forced catch-up must restore INPUT MODES and cursor state, not just
/// content.
///
/// PLAN_M2_5.md requires the catch-up to be a reattach in full, and mode
/// restoration is the half that fails silently: content looks right while
/// bracketed paste and application cursor keys are quietly off, which is
/// the audited silent-loss case SPEC_impl.md calls out. The ordinary
/// reattach path has covered this since M1; the CATCH-UP path reaches it
/// through a different caller, so a regression there (dropping the mode
/// replay, or emitting it before the content that overwrites it) would go
/// unnoticed by every content-only assertion.
#[tokio::test]
async fn a_forced_tmux_pause_restores_modes_and_cursor_state() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let pane = pane_id_of(
        &h.state.path().join("tmux.sock"),
        &format!("fh-{}", session.id),
    )
    .await;

    let (_chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FAKE-AGENT READY", 30).await;
    let before_pause = seen.len();

    force_tmux_pause(&h, &pane).await;
    wait_for_bytes(&mut rx, &mut seen, b"\x1bc", 30).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(detached, None, "the catch-up must not end the attachment");

    let reset_at = before_pause
        + seen[before_pause..]
            .windows(2)
            .position(|window| window == b"\x1bc")
            .expect("wait_for_bytes already proved the reset arrived");
    let after_reset = String::from_utf8_lossy(&seen[reset_at..]).into_owned();

    // Cursor placement is re-synthesized on every replay, and must come
    // AFTER the content — writing the content moves the cursor, so a
    // position emitted first would be immediately wrong.
    let content = after_reset
        .find("FAKE-AGENT READY")
        .expect("the catch-up must replay the pane's content");
    let cursor = after_reset[content..]
        .find("\x1b[")
        .and_then(|offset| {
            after_reset[content + offset..]
                .find('H')
                .map(|end| content + offset + end)
        })
        .expect("the catch-up must re-synthesize a cursor position after the content");
    assert!(
        cursor > content,
        "cursor placement must follow the replayed content, not precede it"
    );

    // Bracketed paste is the mode a real agent most visibly loses. Only
    // assertable where tmux can report it (3.7+); below that the
    // supervisor degrades that one mode by design.
    if tmux_has_format(&h, "bracket_paste_flag").await {
        assert!(
            after_reset[content..].contains("\x1b[?2004h"),
            "the catch-up must restore bracketed paste — content alone passing here is exactly \
             the audited silent-loss case"
        );
    } else {
        eprintln!("tmux lacks bracket_paste_flag; skipping the mode-restoration assertion");
    }
}

/// A stall teardown that lands AFTER a takeover has installed a new
/// attachment must not detach the winner.
///
/// The dangerous shape is narrow and entirely invisible to ordinary
/// tests: a stalled forwarder hands its teardown to a separate task
/// (it must — forwarders may never take the attachments lock), so between
/// deciding to detach and actually detaching, a takeover can install a
/// DIFFERENT attachment for the same session. Since the winner is a
/// different connection using the same channel id — every helm numbers
/// channels from 1 — a teardown that checked only the channel, or checked
/// nothing, would tear down the innocent winner and send it a stall
/// notice it has no way to interpret.
///
/// Timing is swept rather than blocked on a barrier: the window is
/// between two lock acquisitions inside the supervisor and nothing
/// outside it can synchronize on that. Each iteration aims the takeover
/// at a slightly different offset around the stall deadline, so the sweep
/// covers before, during, and after. Any iteration that lands in the
/// window and gets this wrong fails the test.
#[tokio::test]
async fn a_stall_teardown_racing_a_takeover_never_detaches_the_winner() {
    let stall = Duration::from_millis(800);
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: stall,
        ..SupervisorTimeouts::default()
    })
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    for offset_ms in [0i64, 20, 40, 60, 80, 120, 160, 200] {
        // A FRESH connection per iteration for each side: channel ids are
        // per-connection and never recycled, so reusing one client would
        // hand out 1, 2, 3... and the id collision this test depends on
        // would only happen on the first pass.
        let loser = h.second_client().await;
        let (loser_chan, mut loser_rx) = loser.attach(&session.id, 80, 24).await.expect("attach");
        let mut loser_seen = Vec::new();
        wait_for_bytes(&mut loser_rx, &mut loser_seen, b"CUTOVER-", 30).await;
        loser.pause_output(loser_chan).await;

        // Aim the takeover at the moment the stall teardown fires.
        let aim = stall + Duration::from_millis(offset_ms as u64);
        tokio::time::sleep(aim).await;

        let winner = h.second_client().await;
        let (winner_chan, mut winner_rx) = winner
            .attach(&session.id, 80, 24)
            .await
            .expect("takeover attach");
        assert_eq!(
            loser_chan, winner_chan,
            "test premise: both clients must use the same channel id, or the identity check \
             is not being exercised"
        );

        // The winner must survive and keep receiving. A stale teardown
        // detaching it would show up as either a Detached event or a
        // terminal that has gone silent.
        let mut winner_seen = Vec::new();
        wait_for_bytes(&mut winner_rx, &mut winner_seen, b"CUTOVER-", 30).await;
        let before = winner_seen.len();
        let detached = drain_for(&mut winner_rx, &mut winner_seen, Duration::from_secs(2)).await;
        assert_eq!(
            detached, None,
            "offset {offset_ms}ms: the winner was detached by the loser's stall teardown"
        );
        assert!(
            winner_seen.len() > before,
            "offset {offset_ms}ms: the winner stopped receiving output after the loser's \
             stall teardown"
        );
        winner.detach(winner_chan).await;
    }
}

/// Resident memory of the tmux server and the supervisor must stay FLAT
/// while a viewer is stalled against a producer larger than every queue.
///
/// This is the milestone's headline promise and the one nothing else
/// measures: every other test asserts the CONSEQUENCES of bounded queues
/// (a detach fires, delivery is lossless, order holds), all of which an
/// unbounded implementation satisfies perfectly right up until it
/// exhausts memory. The plan's own audit found an undrained control
/// client grew the tmux server at ~3.5 MB/s without `pause-after`; at that
/// rate a stall of a few seconds is unmistakable against the tolerance
/// below, and a regression that drops the flag or unbounds a queue shows
/// up here and nowhere else.
///
/// Two processes are sampled for two different claims. tmux is the one
/// the audit measured and the one `pause-after` protects. The supervisor
/// is this test process — the harness runs it in-process — so its number
/// carries libtest and the harness itself and is necessarily noisier;
/// it gets the looser bound, and is included because an unbounded
/// per-connection queue would grow it without limit while tmux stayed
/// flat.
///
/// Sampled across several windows rather than as a before/after pair:
/// a single pair cannot tell a leak from an allocator that grabbed one
/// chunk early, while a trend across a stall can. A first gated burst fills
/// tmux's fixed history, then a second producer runs past the five-second
/// `pause-after` window. The baseline follows a seven-second allowance for
/// that five-second policy, and an out-of-band acknowledgement plus a final
/// test-controlled gate prove the producer spans the full sample; stable
/// memory cannot be explained by a producer that finished during setup.
#[tokio::test]
async fn memory_stays_flat_while_a_viewer_is_stalled() {
    /// Resident bytes of a process, from `/proc/<pid>/statm` (field 2 is
    /// resident pages).
    fn rss_bytes(pid: u32) -> Option<u64> {
        let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
        let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        Some(pages * 4096)
    }

    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script flood-memory"),
            None,
            80,
            24,
        )
        .await
        .expect("create");
    let sock = h.state.path().join("tmux.sock");

    let tmux_pid: u32 = {
        let out = tmux_query(&sock, &["display-message", "-p", "#{pid}"]).await;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("tmux must report its server pid")
    };
    let own_pid = std::process::id();

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FAKE-AGENT READY", 30).await;
    h.client.send_input(chan, vec![b'w']).await;
    wait_for_flood_marker_with_progress(&mut rx, &mut seen, b"FLOOD-WARMED").await;

    h.client.pause_output(chan).await;
    // Drain residual frames from the completed warm-up burst before opening
    // the second gate. Silence here does not prove tmux's internal pause
    // state; ordering PauseOutput before the next producer is the barrier.
    drain_until_quiet(
        &mut rx,
        &mut seen,
        Duration::from_secs(1),
        Duration::from_secs(30),
    )
    .await;
    h.client.send_input(chan, vec![b'm']).await;
    let started = work.path().join("flood-memory-started");
    let progress = work.path().join("flood-memory-progress");
    wait_for_file(&started, 30).await;
    wait_for_file(&progress, 30).await;
    // `pause-after` is an age limit, not a byte limit. Bytes queued during
    // its five-second allowance are legitimate. The out-of-band files above
    // distinguish producer startup from a completed output batch; this wait
    // gives the stalled output path time to settle before its growth is
    // sampled.
    tokio::time::sleep(Duration::from_secs(7)).await;
    let progress_before_sampling: u64 = std::fs::read_to_string(&progress)
        .expect("read flood progress before RSS sampling")
        .parse()
        .expect("flood progress must be a record count");
    let tmux_baseline = rss_bytes(tmux_pid).expect("tmux rss");
    let own_baseline = rss_bytes(own_pid).expect("own rss");

    let mut tmux_peak = tmux_baseline;
    let mut own_peak = own_baseline;
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        tmux_peak = tmux_peak.max(rss_bytes(tmux_pid).expect("tmux rss"));
        own_peak = own_peak.max(rss_bytes(own_pid).expect("own rss"));
    }
    let progress_after_sampling: u64 = std::fs::read_to_string(&progress)
        .expect("read flood progress after RSS sampling")
        .parse()
        .expect("flood progress must be a record count");
    assert!(
        progress_after_sampling > progress_before_sampling,
        "the producer completed no output batch during RSS sampling — flat memory without \
         sustained terminal pressure proves nothing (progress stayed at \
         {progress_before_sampling})"
    );

    // Six seconds of stall. Unbounded, the audited growth rate would put
    // tmux ~21 MB over baseline; 8 MB is comfortably above ordinary
    // allocator noise and far below that.
    let tmux_growth = tmux_peak.saturating_sub(tmux_baseline);
    assert!(
        tmux_growth < 8 * 1024 * 1024,
        "the tmux server grew {tmux_growth} bytes during a stalled viewer — `pause-after` is \
         not bounding it"
    );
    // Looser, for the reason in this test's docs: this number is the
    // whole test process. Still far below what an unbounded per-connection
    // queue would reach against this producer.
    let own_growth = own_peak.saturating_sub(own_baseline);
    assert!(
        own_growth < 64 * 1024 * 1024,
        "the supervisor process grew {own_growth} bytes during a stalled viewer — a queue on \
         the terminal path is unbounded"
    );

    // The stall must not have been "flat" merely because everything died.
    h.client.resume_output(chan).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match rx.recv().await {
                Some(TermEvent::Data(bytes)) if !bytes.is_empty() => break,
                Some(TermEvent::Data(_)) | Some(TermEvent::ReplayComplete) => {}
                Some(TermEvent::Detached(reason)) => {
                    panic!("the stalled viewer detached before output resumed: {reason}")
                }
                None => panic!("the stalled viewer closed before output resumed"),
            }
        }
    })
    .await
    .expect("no output resumed within five seconds of releasing the stalled viewer");
    h.client.send_input(chan, vec![b'd']).await;
    let stopped = work.path().join("flood-memory-stopped");
    wait_for_file(&stopped, 10).await;
    assert_eq!(
        std::fs::read_to_string(&stopped).expect("read the producer stop acknowledgement"),
        "released",
        "the flood producer reached its self-expiry instead of the test-controlled stop gate"
    );
}

/// A paused attachment must actually stop delivering output.
///
/// The assertion no other pause test makes, and the one a broken
/// implementation would most easily survive: a forwarder that kept
/// reading tmux and only ran the stall timer passes every
/// end-state-shaped test in this file, because the end state after a
/// resume looks the same either way. This observes the QUIET INTERVAL
/// itself — nothing new arrives while paused — and then that delivery
/// resumes.
///
/// The counter fixture rather than the flood: it paces itself, so
/// "nothing arrived" cannot be an artifact of the producer having
/// finished, and the in-flight backlog to drain first is small.
#[tokio::test]
async fn a_paused_attachment_stops_receiving_until_it_resumes() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    h.client.pause_output(chan).await;
    // Drain whatever was already in flight when the pause landed: the
    // pause stops the supervisor PULLING from tmux, it does not retract
    // frames already queued toward this client.
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(2)).await;
    assert_eq!(
        detached, None,
        "a paused-but-live attachment must not be detached"
    );

    let quiet_from = seen.len();
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(3)).await;
    assert_eq!(detached, None, "a paused attachment must stay attached");
    assert_eq!(
        seen.len(),
        quiet_from,
        "output kept arriving while paused: the forwarder is still reading tmux and only the \
         stall timer is honoring the pause — {} bytes arrived",
        seen.len() - quiet_from
    );

    h.client.resume_output(chan).await;
    let resumed_from = seen.len();
    drain_for(&mut rx, &mut seen, Duration::from_secs(5)).await;
    assert!(
        seen.len() > resumed_from,
        "no output resumed after ResumeOutput"
    );
}

/// Repeated short pauses that add up to longer than the stall timeout
/// must NOT detach: the timeout is a hard maximum on ONE pause, not a
/// cumulative budget.
///
/// This is the test that fails an implementation keeping a single timer
/// across resumes — the obvious wrong simplification of "detach a pause
/// that lasts too long", and one every end-state test would otherwise
/// miss. It is the direct complement to the stall-detach test: together
/// they pin both halves of what "continuous" means.
#[tokio::test]
async fn repeated_short_pauses_never_accumulate_into_a_stall_detach() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    // Five pauses of 1.2s each: every one comfortably inside the 2s
    // maximum, together three times over it.
    for cycle in 0..5 {
        h.client.pause_output(chan).await;
        let detached = drain_for(&mut rx, &mut seen, Duration::from_millis(1200)).await;
        assert_eq!(
            detached, None,
            "cycle {cycle}: a pause shorter than the stall timeout must never detach"
        );
        h.client.resume_output(chan).await;
        let detached = drain_for(&mut rx, &mut seen, Duration::from_millis(300)).await;
        assert_eq!(
            detached, None,
            "cycle {cycle}: a resumed attachment must stay attached"
        );
    }

    // Still live afterwards, not merely un-detached during the cycles.
    let before = seen.len();
    drain_for(&mut rx, &mut seen, Duration::from_secs(3)).await;
    assert!(
        seen.len() > before,
        "the attachment survived the pause cycles but stopped delivering output"
    );
}

/// A pause held across a large replay, with `PauseOutput` re-sent
/// repeatedly, must still detach — through the REAL attach/forwarder/
/// connection stack, not a synthetic stand-in for it.
///
/// Two failures in one test, both of which every other pause test
/// survives. First, the stall deadline must be ABSOLUTE: an
/// implementation that restarts its timer per chunk, per phase, or on
/// every observed pause message would keep this attachment alive forever
/// while a client sat paused, which is exactly the unbounded pin the
/// timeout exists to prevent. Second, the pause must gate the REPLAY
/// itself and not merely the live pump — pausing mid-replay is the case
/// where the forwarder has megabytes already in hand, so a version that
/// consulted the pause only between live events would push all of it at a
/// client that had said stop.
///
/// The spam is what makes the first failure observable: `PauseOutput`
/// repeated every 300ms is well inside the shortened timeout, so an
/// implementation that lets a repeat overwrite the stored pause start
/// never detaches at all — this test's `reason.expect` below is what
/// catches that outright, by timing out the whole test if it happens.
///
/// # What this test does NOT pin
///
/// It deliberately asserts no numeric bound on how LONG the detach took.
/// An earlier version tried one (`elapsed < 9s`, then `elapsed <
/// STALL_DETACH + SPAM_PERIOD + 1s`) and a review swarm found both wrong in
/// the same two ways every such bound here would be: real wall-clock
/// elapsed time is measured from `pause_output`'s fire-and-forget SEND,
/// so a CORRECT implementation can still miss a tight bound under nothing
/// worse than ordinary delivery and forwarder-teardown latency on a loaded
/// runner; and the loop below is itself capped at `spam_count *
/// SPAM_PERIOD` (a few seconds) before `reason.expect` would fail it
/// anyway, so any bound loose enough to absorb real scheduling noise is
/// also too loose to catch a wrong-instant anchor (one anchored to the
/// second spam rather than the first drifts by only one `SPAM_PERIOD`).
/// There is no honest number to put here that is both tight enough to
/// discriminate and loose enough not to flake.
///
/// The precise property — the stored anchor never moves under spam, and
/// the deadline fires within one virtual-time tick of
/// `anchor + stall_timeout` — is pinned deterministically and at zero
/// wall-clock cost by
/// `connection::tests::repeated_pause_spam_never_moves_the_stall_anchor`
/// (a supervisor-level unit test against a paused clock, where "measured
/// from the first pause" and "measured from anything else" land at
/// different virtual instants with no timing noise to hide behind). What
/// THIS test proves that the unit test cannot: that a real, continuously
/// spammed client attached through the real stack genuinely gets detached
/// with the right reason, rather than wedging forever.
#[tokio::test]
#[ignore = "load flake: its shared wait times out under a loaded runner (CI 2026-09-02, release gate 2026-09-03); TODO.md has the evidence"]
async fn a_paused_replay_detaches_relative_to_the_first_pause_despite_pause_spam() {
    let stall_detach = Duration::from_secs(3);
    let spam_period = Duration::from_millis(300);

    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach,
        ..SupervisorTimeouts::default()
    })
    .await;
    // The flood fixture builds a full history quickly, so the reattach
    // below has a large replay for the pause to land in the middle of.
    let (session, _work) = flood_session(&h).await;
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;
    h.client.detach(chan).await;

    // Reattach and pause immediately, while the replay is still being
    // written rather than after it has drained.
    let (chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach");
    h.client.pause_output(chan2).await;

    let mut replay = Vec::new();
    let mut reason = None;
    // Spam pause throughout, never resuming. Every repeat is inside the
    // 3s maximum, so only an absolute deadline detaches at all.
    for _ in 0..20 {
        h.client.pause_output(chan2).await;
        if let Some(seen_reason) = drain_for(&mut rx2, &mut replay, spam_period).await {
            reason = Some(seen_reason);
            break;
        }
    }

    let reason = reason.expect(
        "a continuously paused attachment was never detached — repeated PauseOutput is \
         restarting the hard maximum instead of being ignored",
    );
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the detach must be the stall detach"
    );
}

/// A stall detaches exactly ONE attachment, leaving every other
/// attachment on the same CONNECTION alive (PLAN_M4.md item 3).
///
/// The stall bound is a property of one control-mode client — tmux's
/// `pause-after`/`%pause` are per client — so the teardown it triggers
/// must be scoped to that client's own attachment key. A teardown that
/// swept by connection, or by anything wider than the key, would let one
/// wedged view take down terminals the user is actively watching, which
/// is exactly the outcome the per-terminal design exists to avoid; a
/// genuinely wedged client converges on a whole-client detach anyway, one
/// stall bound at a time.
///
/// The two attachments are two SESSIONS rather than two terminals of one
/// session, because tabs do not exist yet — so this is a
/// connection-scoped over-detach guard, not a tab-isolation test. Their
/// leases are deliberately DISTINCT and deliberately irrelevant: takeover
/// is session-scoped, so two sessions never displace each other whatever
/// their leases say, and distinct leases keep this test from implying
/// otherwise. The same-session variant PLAN_M4.md acceptance item 5
/// describes lands with the tabs PR.
///
/// Both sessions run the QUIET fixture on purpose. The survivor sits
/// undrained for as long as the stall takes to fire, and a chatty fixture
/// would overflow the client's own per-terminal queue in that window —
/// which this client answers with a local stall detach of its own
/// (`SupervisorClient::dispatch`), indistinguishable here from the
/// supervisor-side over-detach under test. Liveness is asserted by typing
/// instead: an echo proves both the attachment and its input route
/// survived.
#[tokio::test]
async fn a_stall_detaches_only_its_own_attachment_not_the_connections_others() {
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(2),
        ..SupervisorTimeouts::default()
    })
    .await;
    let (stalling_session, _stalling_work) = basic_session(&h).await;
    let (live_session, _live_work) = basic_session(&h).await;

    // Two sessions on one connection, under two different leases: no
    // takeover is possible between them in either direction, so anything
    // that detaches the survivor came from the stall teardown.
    let (stalling_chan, mut stalling_rx) = h
        .client
        .attach_terminal(
            &stalling_session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-of-the-stalled-view",
        )
        .await
        .expect("attach the terminal that will stall");
    let (live_chan, mut live_rx) = h
        .client
        .attach_terminal(
            &live_session.id,
            80,
            24,
            TerminalSelector::Agent,
            "lease-of-the-live-view",
        )
        .await
        .expect("attach the terminal that must survive");
    let mut stalling_seen = Vec::new();
    let mut live_seen = Vec::new();
    wait_for(&mut stalling_rx, &mut stalling_seen, "FAKE-AGENT READY", 20).await;
    wait_for(&mut live_rx, &mut live_seen, "FAKE-AGENT READY", 20).await;

    h.client.pause_output(stalling_chan).await;
    let reason = expect_detached(&mut stalling_rx, 15).await;
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the paused attachment must take the stall detach"
    );

    // The connection's other attachment: no notice of its own, and still
    // authorized to type — the stall teardown must not have removed its
    // attachment or its input route along with the stalled one's.
    let live_detached = drain_for(&mut live_rx, &mut live_seen, Duration::from_secs(2)).await;
    assert_eq!(
        live_detached, None,
        "a stall on one attachment detached another attachment on the same connection"
    );
    h.client
        .send_input(live_chan, b"still-mine\r".to_vec())
        .await;
    wait_for(&mut live_rx, &mut live_seen, "still-mine", 15).await;
}

/// A pause from a client that LOST a takeover must not silence the
/// winner.
///
/// Pause carries only a channel id, and channel ids are unique only
/// within a connection — every browser tab rides the helm's single
/// supervisor connection, so two connections trivially collide on id 1.
/// Without both halves of the ownership check (owning connection AND
/// channel), a losing client's pause would silence a terminal it no
/// longer holds, which is a denial of service one tab can inflict on
/// another. This is the same trust boundary the input and resize arms
/// enforce, and it had no test.
#[tokio::test]
async fn a_pause_from_a_client_that_lost_a_takeover_cannot_silence_the_winner() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    // The loser attaches first, on its own connection.
    let (loser_chan, mut loser_rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut loser_seen = Vec::new();
    wait_for_bytes(&mut loser_rx, &mut loser_seen, b"CUTOVER-", 30).await;

    // A second connection takes over. Its channel ids number from 1 too,
    // which is exactly the collision this test needs.
    let winner = h.second_client().await;
    let (winner_chan, mut winner_rx) = winner
        .attach(&session.id, 80, 24)
        .await
        .expect("takeover attach");
    assert_eq!(
        loser_chan, winner_chan,
        "test premise: both clients must use the same channel id, or the connection half of \
         the ownership check is not being exercised"
    );
    let mut winner_seen = Vec::new();
    wait_for_bytes(&mut winner_rx, &mut winner_seen, b"CUTOVER-", 30).await;

    // The loser, which has been detached, pauses "its" channel.
    h.client.pause_output(loser_chan).await;

    let before = winner_seen.len();
    let detached = drain_for(&mut winner_rx, &mut winner_seen, Duration::from_secs(3)).await;
    assert_eq!(
        detached, None,
        "the winner must not be detached by the loser's pause"
    );
    assert!(
        winner_seen.len() > before,
        "the loser's pause silenced the winner's terminal — the ownership check on \
         PauseOutput is not enforcing both channel and owning connection"
    );
}

/// The DEEP-pause contract: a client pause held well past tmux's
/// `pause-after` must still leave the terminal correct — under BOTH of
/// the flow-control behaviors tmux exhibits.
///
/// # Why this test has two branches
///
/// With `pause-after` set and a control client that stops reading, tmux
/// does one of two things, and which one is not something this code gets
/// to choose:
///
/// - **It throttles the pane.** tmux stops reading the PTY, the producer
///   blocks on `write`, and nothing is ever dropped. On resume, delivery
///   continues from exactly where it stopped. This is a genuine
///   end-to-end degrade-to-slow.
/// - **It reads ahead into history and pauses the client's stream.** The
///   producer free-runs, tmux fills its scrollback, and the bytes queued
///   for this client age past `pause-after`, at which point tmux cuts the
///   stream with `%pause` and discards what it had queued. Recovery is
///   then the supervisor's reset-then-replay catch-up, and history is
///   what makes it lossless within the replay floor.
///
/// Audited directly on 2026-07-29 (see SPEC_impl.md's backpressure
/// paragraph): tmux 3.7b took the read-ahead path in every trial, while
/// 3.4 took either path across repeated identical trials. The deciding
/// factor is how far tmux happens to have read ahead of the client at the
/// moment it stalls, which no test can pin down — so asserting one
/// behavior would be asserting a race. This follows the version-tolerant
/// precedent in the harness (see `harness::wait_for_after`): detect which
/// happened, then assert that branch's FULL contract rather than
/// weakening both.
///
/// Both branches are real coverage, and the read-ahead branch is the only
/// end-to-end exercise of the forwarder's reset-then-replay path.
#[tokio::test]
async fn a_deep_pause_ends_correctly_under_either_tmux_flow_control_behavior() {
    let h = harness().await;
    let (session, _work) = flood_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;

    let paused_at = seen.len();
    let last_before_pause = flood_records(&seen)
        .last()
        .copied()
        .expect("test setup: records must have been delivered before the pause");
    h.client.pause_output(chan).await;

    // Hold the pause well past `pause-after` so tmux has to make its
    // choice. Draining throughout is deliberate and not a contradiction:
    // the pause stops the SUPERVISOR pulling from tmux, so what arrives
    // here is only what was already in flight — and NOT reading it would
    // instead trip the helm's own detach-not-block rule, ending the
    // attachment for an unrelated reason.
    let detached = drain_for(
        &mut rx,
        &mut seen,
        Duration::from_secs(farhelm_supervisor::tmux::TMUX_PAUSE_AFTER_SECS + 5),
    )
    .await;
    assert!(
        detached.is_none(),
        "a paused-but-live attachment must not be detached: {detached:?}"
    );

    h.client.resume_output(chan).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(20)).await;
    assert_eq!(detached, None, "the attachment must survive the resume");

    // The catch-up reset is what tells the branches apart: it exists only
    // on the path where tmux cut the stream. Searched from the pause
    // point so an earlier attach-time byte pattern cannot be mistaken for
    // one, and taken as the LAST occurrence so a second stall during the
    // post-resume drain is analyzed rather than ignored.
    let reset_at = seen[paused_at..]
        .windows(2)
        .rposition(|window| window == b"\x1bc")
        .map(|offset| paused_at + offset);

    match reset_at {
        None => {
            eprintln!("deep-pause branch: tmux throttled the pane (lossless continuation)");
            // Nothing was dropped, so delivery must be exactly
            // consecutive ACROSS the pause boundary, not merely within
            // the suffix after it — a gap exactly at the boundary is the
            // failure this branch exists to rule out, and slicing at
            // `paused_at` would hide it. Including the last record
            // delivered before the pause is what tests the seam itself.
            let records = flood_records(&seen);
            assert!(
                records.len() > 500,
                "test setup: only {} records arrived, too few for the continuity assertion to \
                 mean anything",
                records.len()
            );
            let boundary = flood_records(&seen[..paused_at]).len().saturating_sub(1);
            assert_records_consecutive(
                &records[boundary..],
                "throttle-branch delivery across the pause boundary",
                1,
            );
        }
        Some(reset_at) => {
            eprintln!("deep-pause branch: tmux cut the stream (reset-then-replay catch-up)");
            // What the client had immediately before this reset. Compared
            // against the replay's first record below: the replay must
            // resume PAST it, never re-deliver content the client still
            // held, which is the "never replay into a populated terminal"
            // rule (PLAN_M2_5.md) observed from the outside.
            let last_before_reset = flood_records(&seen[..reset_at])
                .last()
                .copied()
                .unwrap_or(last_before_pause);
            let after_reset = flood_records(&seen[reset_at..]);
            let first_after = *after_reset
                .first()
                .expect("the catch-up replay must carry records");

            // Consecutive, not merely increasing: the replay is one
            // contiguous history capture followed by live output, so any
            // step other than +1 is loss, duplication, or reordering.
            // "Increasing" alone would pass a bug that dropped every
            // second record.
            assert_records_consecutive(&after_reset, "post-catch-up transcript", 1);
            // Deliberately NOT asserted: that `first_after` exceeds
            // `last_before_reset`. The replay is a fresh capture of
            // retained history, so it legitimately starts BEFORE the last
            // pre-pause record — resetting the terminal first is exactly
            // what makes re-delivering that overlap correct rather than
            // duplication (PLAN_M2_5.md's "never replay into a populated
            // terminal"). The reset is the assertion that matters, and it
            // is `reset_at`'s own existence.
            let _ = (first_after, last_before_reset);
            assert!(
                after_reset.len() > 1000,
                "the catch-up replayed only {} records; history was not replayed",
                after_reset.len()
            );

            // Delivery must actually be live again afterwards, not a
            // one-shot replay into a still-paused pane. Either the
            // producer had already finished during the stall — in which
            // case the replay carries its true tail, the strongest end
            // state available — or records keep arriving.
            let last_after_catch_up = *after_reset.last().expect("non-empty");
            drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
            let finished = seen
                .windows(b"FLOOD-DONE".len())
                .any(|window| window == b"FLOOD-DONE");
            let latest = flood_records(&seen[reset_at..])
                .last()
                .copied()
                .expect("non-empty");
            if finished {
                assert_eq!(
                    latest,
                    FLOOD_RECORDS - 1,
                    "the producer finished, so the recovered terminal must hold its true tail"
                );
            } else {
                assert!(
                    latest > last_after_catch_up,
                    "no records arrived after the catch-up ({latest} still the last): the pane \
                     was replayed but never continued"
                );
            }
        }
    }
}

/// The SHALLOW-pause contract: a pause lifted before tmux's own
/// `pause-after` fires must be lossless and continuous — no reset, no
/// replay, delivery simply resuming with the very next record.
///
/// The complement to the deep-stall test, and the reason the supervisor
/// keys its catch-up on tmux's `%pause` notification rather than on "the
/// client was paused at some point". Recovering unconditionally would be
/// correct-looking but wasteful and visibly disruptive: every watermark
/// pause a busy terminal makes — which is the STEADY STATE this milestone
/// designs for — would clear and repaint the user's screen.
///
/// Scoped to a window around the pause rather than the producer's whole
/// run, for the same load-sensitivity reason as the deep-stall test
/// above, and for a sharper one: asserting "no reset ever" across a
/// multi-megabyte run would fail whenever unrelated load stalled the
/// pipeline past `pause-after`, which is correct behavior, not a bug.
#[tokio::test]
#[ignore = "load flake: its shared wait times out under a loaded runner (release gate 2026-09-03); TODO.md has the evidence"]
async fn shallow_pause_resumes_without_reset_or_replay() {
    let h = harness().await;
    let (session, _work) = flood_session(&h).await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"FLOOD-000000", 30).await;

    let paused_at = seen.len();
    h.client.pause_output(chan).await;
    // Comfortably inside tmux's own window, so it has no reason to cut
    // this client off.
    tokio::time::sleep(Duration::from_millis(500)).await;
    h.client.resume_output(chan).await;
    let detached = drain_for(&mut rx, &mut seen, Duration::from_secs(10)).await;
    assert_eq!(
        detached, None,
        "a shallow pause must not end the attachment"
    );

    assert!(
        !seen[paused_at..]
            .windows(2)
            .any(|window| window == b"\x1bc"),
        "a pause lifted inside tmux's pause-after window must not trigger a catch-up reset"
    );
    let records = flood_records(&seen);
    assert!(
        records.len() > 1000,
        "test setup: too little output arrived ({} records) for the continuity assertion below \
         to mean anything",
        records.len()
    );
    // Lossless, not merely ordered — and asserted ACROSS the pause
    // boundary rather than only after it, since a gap exactly at the seam
    // is the failure this test exists to rule out. Including the last
    // record delivered before the pause is what tests the seam itself.
    let boundary = flood_records(&seen[..paused_at]).len().saturating_sub(1);
    assert_records_consecutive(
        &records[boundary..],
        "shallow-pause delivery across the pause",
        1,
    );
}

/// A pause that never ends must detach the attachment with the stall
/// reason, and must leave the session itself untouched and reattachable.
///
/// Both halves matter. The detach is what bounds memory when a viewer
/// wedges — every hop's buffers stay pinned for exactly as long as the
/// pause lasts, so "forever" is not an option. The session surviving is
/// what makes the detach an acceptable answer at all: SPEC.md promises a
/// stuck viewer never harms the agent, so the pane must still be running
/// and must still replay correctly to the next client.
#[tokio::test]
async fn a_pause_past_the_stall_timeout_detaches_and_leaves_the_session_healthy() {
    // Short enough to wait out, long enough that ordinary scheduling
    // jitter on a loaded CI runner cannot trip it early.
    let h = harness_with_timeouts(SupervisorTimeouts {
        stall_detach: Duration::from_secs(3),
        ..SupervisorTimeouts::default()
    })
    .await;
    // The counter fixture, NOT the flood: this test has to prove the
    // agent is producing again AFTER the detach, and a producer that can
    // finish during the stall makes that unfalsifiable — its tail would
    // then be present no matter how wedged the pane still was. `counter`
    // runs until its session is killed.
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = h
        .client
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script counter"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for_bytes(&mut rx, &mut seen, b"CUTOVER-", 30).await;

    h.client.pause_output(chan).await;
    let reason = drain_for(&mut rx, &mut seen, Duration::from_secs(30))
        .await
        .expect("a pause past the stall timeout must produce a detach");
    assert_eq!(
        reason,
        farhelm_proto::DETACH_REASON_STALLED,
        "the stall detach must use the reason both emitters share verbatim"
    );

    // The session is unharmed: still listed live. The await IS the
    // assertion — `wait_for_live_status` panics if the session never
    // reaches a live status inside its window — so a stalled viewer having
    // harmed the agent fails right here.
    //
    // Waited for rather than read once, per that helper's own rationale: a
    // single list can report `Exited { exit_code: None }` for a perfectly
    // healthy session when tmux's `pane_states` degrades to an empty map,
    // which under a loaded machine (the whole suite running in parallel) is
    // frequent enough to have flaked a single-shot assertion here. Waiting
    // is not the weaker claim — an agent the stall really killed never
    // becomes live again, so this still fails, just after giving the truth
    // a bounded number of chances to be observed.
    wait_for_live_status(&h.client, &session.id, 10).await;

    // ...and, the part that actually matters, the AGENT IS RUNNING AGAIN.
    // Metadata saying the session is live, plus a replay of pre-stall
    // bytes, proves neither: on the tmux behavior that throttles the pane,
    // the agent's
    // writes were blocked for the whole stall, and a detach that failed to
    // release the pane would leave them blocked forever while every
    // assertion above still passed. Requiring records strictly PAST the
    // last one seen before the detach is what makes a still-wedged pane
    // fail.
    let last_before_detach = counter_records(&seen)
        .last()
        .copied()
        .expect("records must have been delivered before the stall");
    let (_chan2, mut rx2) = h
        .client
        .attach(&session.id, 80, 24)
        .await
        .expect("reattach after a stall detach");
    let mut replay = Vec::new();
    let target = format!("CUTOVER-{:08}", last_before_detach + 50);
    wait_for_bytes(&mut rx2, &mut replay, target.as_bytes(), 60).await;
}

/// The cross-language invariant PLAN_M2_5.md's honesty argument rests on:
/// the browser's scrollback capacity must sit between SPEC.md's promised
/// floor and tmux's actual history floor, never outside either end.
///
/// Why the UPPER bound matters: after a deep stall the catch-up replays at
/// most `HISTORY_LIMIT` lines. If xterm.js could retain MORE than that, a
/// user would watch scrollback they already had get truncated by the
/// recovery — visible, unexplained loss. Holding the browser at or below
/// the floor is what makes the catch-up's end state observably equivalent
/// to lossless slow delivery instead.
///
/// Why the LOWER bound matters, and why it is pinned HERE rather than left
/// implicit: SPEC.md's own product promise is "at least the current screen
/// plus 10,000 lines of scrollback" — a real minimum, not merely "whatever
/// happens to be at most `HISTORY_LIMIT`". Before this bound was added,
/// `scrollback: 0` (or any value far below the promised floor) satisfied
/// the upper-bound check just as well as a correct value, silently
/// defeating the whole product guarantee this test exists to protect.
///
/// Asserted by reading the UI asset directly, because nothing else
/// connects these numbers: they live in different languages, in different
/// crates, with no shared build step. A test that pinned only the Rust
/// constants would go green while the JavaScript drifted, which is
/// precisely the failure this exists to catch.
#[test]
fn browser_scrollback_stays_within_the_product_floor_and_the_tmux_history_ceiling() {
    /// SPEC.md: "the terminal retains, and replay covers, at least the
    /// current screen plus 10,000 lines of scrollback."
    const SPEC_MINIMUM_SCROLLBACK: u32 = 10_000;

    let terminal_js =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../farhelm-ui/assets/terminal.js");
    let source = std::fs::read_to_string(&terminal_js)
        .unwrap_or_else(|e| panic!("reading {}: {e}", terminal_js.display()));
    let (_, after) = source
        .split_once("scrollback:")
        .expect("terminal.js must configure an explicit xterm.js scrollback");
    let scrollback: u32 = after
        .trim_start()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|digits| digits.parse().ok())
        .expect("terminal.js's scrollback must be a plain integer literal");
    assert!(
        scrollback <= farhelm_supervisor::tmux::HISTORY_LIMIT,
        "terminal.js keeps {scrollback} lines of scrollback but tmux only guarantees {} — a \
         post-stall catch-up would visibly truncate history the user already had",
        farhelm_supervisor::tmux::HISTORY_LIMIT
    );
    assert!(
        scrollback >= SPEC_MINIMUM_SCROLLBACK,
        "terminal.js keeps only {scrollback} lines of scrollback but SPEC.md promises at least \
         {SPEC_MINIMUM_SCROLLBACK} — this is a broken product promise, not merely a cosmetic gap"
    );
}

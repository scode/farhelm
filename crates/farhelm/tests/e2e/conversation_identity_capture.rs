//! Conversation-identity capture against a real supervisor and the
//! record-writing fake agent: the wiring and claim discipline built on
//! top of farhelm-supervisor's unit-tested `agent_kind` module.

use crate::harness::*;

use crate::boot_id_durable_outcome::listed;

// ---------------------------------------------------------------------
// Conversation-identity capture (PLAN_M3.md item 8, acceptance 8)
//
// Every test below drives a REAL supervisor against the record-writing
// fake agent, because the properties under test are about the interaction
// of three things a unit test cannot put in one room: when the supervisor
// confirms delivery of its first input, when the agent writes its record,
// and what the rescan concludes from the two. The per-kind parsing, the
// scan's completeness and budgets, and the pure correlation rules are
// unit-tested in farhelm-supervisor's `agent_kind` module; what is here is
// the wiring and the claim discipline built on top of it.
// ---------------------------------------------------------------------

/// The capture window every test in this section runs with.
///
/// Short deliberately, and each part matters. `AFTER` bounds how long
/// after first input a record may appear and still be attributed, so it is
/// also what two sessions in one directory must be spaced by to avoid
/// poisoning each other. `GRACE` is how long past the window's close the
/// supervisor waits before the one COMPLETE scan that may commit — every
/// test that expects a durable claim has to outlast it, so a production
/// value would put minutes on the clock. `BEFORE` only absorbs clock
/// granularity between the supervisor's reading and the agent's.
const TEST_CAPTURE_BEFORE: Duration = Duration::from_secs(1);
pub(crate) const TEST_CAPTURE_AFTER: Duration = Duration::from_secs(2);
pub(crate) const TEST_CAPTURE_GRACE: Duration = Duration::from_secs(1);

/// The bounds every capture harness injects.
pub(crate) fn test_capture_bounds() -> CaptureWindowBounds {
    CaptureWindowBounds::new(TEST_CAPTURE_BEFORE, TEST_CAPTURE_AFTER, TEST_CAPTURE_GRACE)
}

/// Everything a capture test needs beyond the harness itself: the private
/// agent home the supervisor observes and the fixture writes into, and a
/// directory of kind-named symlinks to the farhelm binary.
///
/// The symlinks are what let these tests exercise DERIVATION rather than
/// routing around it. A session launched as `farhelm internal fake-agent
/// ...` has basename `farhelm` and correctly classifies as generic, so
/// running the fixture through `<bin>/claude` is the only way to reach the
/// integrated path the way a real user does — and it simultaneously pins
/// PLAN_M3.md item 7's other promise, that the default resume template is
/// built from the ORIGINAL first token (this absolute path) rather than
/// from a bare command name. The binary is multi-call by SUBCOMMAND, not
/// by argv0, so it behaves identically under either name.
pub(crate) struct CaptureFixtures {
    home: tempfile::TempDir,
    bin: tempfile::TempDir,
}

impl CaptureFixtures {
    /// Private record root observed by the supervisor and fake agents.
    pub(crate) fn home(&self) -> &std::path::Path {
        self.home.path()
    }

    /// Directory containing the kind-named fake-agent entry points.
    pub(crate) fn bin(&self) -> &std::path::Path {
        self.bin.path()
    }
}

/// A harness whose supervisor observes a private agent home, with the
/// short capture window above.
pub(crate) async fn capture_harness() -> (Harness, CaptureFixtures) {
    capture_harness_with_fault(None).await
}

/// [`capture_harness`] with a durable-write fault injected, for the
/// pending-durability tests.
async fn capture_harness_with_fault(
    fault: Option<CaptureStoreFault>,
) -> (Harness, CaptureFixtures) {
    let home = tempfile::tempdir().expect("agent home");
    let bin = tempfile::tempdir().expect("agent bin");
    for kind in ["claude", "codex"] {
        std::os::unix::fs::symlink(farhelm_bin(), bin.path().join(kind))
            .expect("symlink the farhelm binary under an agent's own name");
    }
    let h = harness_with_seams(
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            capture_store_fault: fault,
            scopes: Arc::new(farhelm_supervisor::scope::ScopeManager::disabled()),
            ..SupervisorSeams::default()
        },
    )
    .await;
    (h, CaptureFixtures { home, bin })
}

/// Create a session running the record-writing fake agent for `kind`
/// (`claude` or `codex`) in `cwd`, launched through the kind-named symlink
/// so the supervisor derives the integration itself.
pub(crate) async fn record_session(
    h: &Harness,
    fixtures: &CaptureFixtures,
    cwd: &std::path::Path,
    kind: &str,
) -> SessionInfo {
    let invocation = format!(
        "{} internal fake-agent --script {kind}-record --record-home {}",
        shell_words::quote(&fixtures.bin.path().join(kind).to_string_lossy()),
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    );
    h.client
        .create_session(&cwd.to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create a record-writing session")
}

/// Attach, wait for the fixture to be listening, send one line, and wait
/// for the record it writes in response — the shape of "the agent's first
/// prompt", which is the only moment a record can appear.
///
/// Returns the conversation id the fixture reported, so a test can assert
/// the supervisor captured THAT id rather than merely some id.
pub(crate) async fn provoke_record(
    h: &Harness,
    session: &SessionInfo,
) -> (u32, TermStream, Vec<u8>, String) {
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    let id = marker_value(&seen, "RECORD-WRITTEN:");
    (chan, rx, seen, id)
}

/// The value the fixture printed after `marker`, up to the line ending.
///
/// The fixture's markers are its own contract with these tests (the same
/// discipline `FAKE-AGENT READY` established), and reading the id back out
/// is what lets a test assert the supervisor captured the RIGHT
/// conversation rather than just any one — the property that separates
/// this feature working from it appearing to.
pub(crate) fn marker_value(transcript: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(transcript);
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("no {marker} in transcript:\n{text}"))
        + marker.len();
    text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect()
}

/// The value after the LAST occurrence of `marker`, for transcripts that
/// span a restart: a reattached client's replay carries the previous run's
/// markers too, so "the first one" is the wrong run's answer whenever a
/// terminal was reused.
pub(crate) fn last_marker_value(transcript: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(transcript);
    let start = text
        .rfind(marker)
        .unwrap_or_else(|| panic!("no {marker} in transcript:\n{text}"))
        + marker.len();
    text[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect()
}

/// This session's durable snapshot, as the supervisor would answer a
/// restart.
pub(crate) async fn snapshot_of(h: &Harness, session_id: &str) -> SessionSnapshot {
    h.sup
        .session_snapshot(session_id)
        .await
        .expect("reading the snapshot")
        .expect("the session exists")
}

/// Poll until this session's durable first-input time is recorded, and
/// return it.
///
/// Every window assertion below is arithmetic on THIS value rather than on
/// wall-clock sleeps, because the correlator is truncated to whole seconds:
/// a 3.5-second sleep can produce a 3-second separation, which would
/// silently break a disjointness premise a sleep-based test only *assumes*.
/// Waiting on the recorded value lets the premise be asserted instead.
async fn wait_for_first_input(h: &Harness, session_id: &str, secs: u64) -> i64 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(at) = snapshot_of(h, session_id).await.first_input_at {
            return at;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never recorded a durable first-input time within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Sleep until a first input taken NOW would own a window disjoint from
/// the one anchored at `earlier`.
///
/// Disjointness is `t2 - before > t1 + after`, so this waits past
/// `t1 + after + before` with a whole second of margin for the truncation
/// on both readings. The caller still asserts the premise afterwards — this
/// only makes the assertion likely to hold rather than assuming it does.
async fn wait_until_window_disjoint_from(earlier: i64) {
    let target =
        earlier + TEST_CAPTURE_AFTER.as_secs() as i64 + TEST_CAPTURE_BEFORE.as_secs() as i64 + 1;
    while farhelm_supervisor::agent_kind::now_unix() <= target {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Assert that two sessions' capture windows really are disjoint, from the
/// DURABLE first-input times rather than from how long the test slept.
///
/// Without this the two-sessions-in-one-directory tests would silently
/// become vacuous the day a slow machine (or a second-boundary) shrank the
/// separation below the window: both sessions would bail, and "each
/// captured its own" would fail for a reason that looks like a capture bug.
fn assert_windows_disjoint(first: i64, second: i64) {
    let bounds = test_capture_bounds();
    let a = CaptureWindow::around(first, bounds);
    let b = CaptureWindow::around(second, bounds);
    assert!(
        !a.overlaps(&b),
        "this test's premise is that these windows do not overlap, but {a:?} meets {b:?}"
    );
}

/// Assert the opposite premise, for the ambiguity tests: the two windows
/// really do overlap, so a bail is the correct answer rather than an
/// accident of timing.
fn assert_windows_overlap(first: i64, second: i64) {
    let bounds = test_capture_bounds();
    let a = CaptureWindow::around(first, bounds);
    let b = CaptureWindow::around(second, bounds);
    assert!(
        a.overlaps(&b),
        "this test's premise is that these windows overlap, but {a:?} misses {b:?}"
    );
}

/// Poll until this session's stored snapshot reports a captured identity.
///
/// Polling because capture rides the list/reload cadence rather than an
/// event: nothing pushes, so a test must ask. `list_sessions` is what
/// drives the pass, so it is called each round rather than only reading the
/// store — a test that read the store alone would hang forever waiting for
/// a pass nothing triggered. The wait has to outlast the publication grace,
/// since nothing is committed until the horizon closes.
pub(crate) async fn wait_for_capture(h: &Harness, session_id: &str, secs: u64) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    loop {
        h.client.list_sessions().await.expect("list drives capture");
        if let Some(conversation) = snapshot_of(h, session_id).await.captured_conversation {
            return conversation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "session {session_id} never captured a conversation identity within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Drive list passes until every session's horizon has closed and then
/// some, for the tests asserting a capture must NOT happen.
///
/// Sleeping past the horizon is what makes this real negative evidence: a
/// session still inside its window is only ever `Provisional` anyway, so
/// asserting "nothing captured" before the horizon would pass on a broken
/// implementation too.
pub(crate) async fn settle_past_horizon(h: &Harness) {
    let deadline = tokio::time::Instant::now()
        + TEST_CAPTURE_AFTER
        + TEST_CAPTURE_GRACE
        + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    // A few more passes with the clock already past every horizon, so the
    // final complete scan has certainly run.
    for _ in 0..3 {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// SPEC.md's per-session resume promise, at its hardest: two sessions in
/// ONE working directory each capture their OWN conversation, and each
/// resumes exactly that one.
///
/// This is the case the whole correlation design exists for — "even when
/// several sessions share a working directory" is SPEC.md's own wording —
/// and it is where a naive implementation (take the newest record in the
/// project directory) silently hands both sessions the same conversation.
/// The inputs are spaced past the capture window so the two windows are
/// disjoint, and that premise is ASSERTED from the durable first-input
/// times rather than assumed from how long the test slept.
///
/// The filled resume argv is asserted, not just the id: SPEC.md's promise
/// is that restart resumes that conversation, and an id captured into a
/// template that never gets filled would satisfy the letter of a weaker
/// test while failing the actual promise.
#[tokio::test]
async fn two_claude_sessions_in_one_directory_each_capture_their_own_conversation() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan_a, _rx_a, _seen_a, id_a) = provoke_record(&h, &first).await;
    let at_a = wait_for_first_input(&h, &first.id, 20).await;

    wait_until_window_disjoint_from(at_a).await;

    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan_b, _rx_b, _seen_b, id_b) = provoke_record(&h, &second).await;
    let at_b = wait_for_first_input(&h, &second.id, 20).await;
    assert_ne!(id_a, id_b, "the fixture must mint distinct conversations");
    assert_windows_disjoint(at_a, at_b);

    assert_eq!(wait_for_capture(&h, &first.id, 30).await, id_a);
    assert_eq!(wait_for_capture(&h, &second.id, 30).await, id_b);

    for (session, conversation) in [(&first, &id_a), (&second, &id_b)] {
        let snapshot = snapshot_of(&h, &session.id).await;
        assert_eq!(snapshot.restart_offer, farhelm_proto::RestartOffer::Resume);
        assert_eq!(
            snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
            conversation,
            "the resume template must be filled with THIS session's conversation"
        );
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::Resume,
            "the offer must reach the wire, not only the store"
        );
    }
}

/// SPEC.md requires BOTH integrations in v1, so Codex gets the same
/// shared-directory proof rather than being assumed to follow from
/// Claude's. It is not a formality: Codex's records live in a date-nested
/// tree that is NOT partitioned by working directory at all, so the
/// recorded-cwd filter carries all the weight here, and the scan cache is
/// keyed on a root every Codex session on the host shares.
#[tokio::test]
async fn two_codex_sessions_in_one_directory_each_capture_their_own_conversation() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_chan_a, _rx_a, _seen_a, id_a) = provoke_record(&h, &first).await;
    let at_a = wait_for_first_input(&h, &first.id, 20).await;

    wait_until_window_disjoint_from(at_a).await;

    let second = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_chan_b, _rx_b, _seen_b, id_b) = provoke_record(&h, &second).await;
    let at_b = wait_for_first_input(&h, &second.id, 20).await;
    assert_ne!(id_a, id_b);
    assert_windows_disjoint(at_a, at_b);

    assert_eq!(wait_for_capture(&h, &first.id, 30).await, id_a);
    assert_eq!(wait_for_capture(&h, &second.id, 30).await, id_b);

    let snapshot = snapshot_of(&h, &first.id).await;
    let template = snapshot.resume_template.as_deref().unwrap();
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap(),
        // The audited codex shape: a subcommand, not a flag.
        [template[0].clone(), "resume".to_string(), id_a.clone()]
    );
}

/// Two sessions of DIFFERENT kinds in one working directory must not
/// poison each other, even with overlapping windows: a Claude record can
/// only ever be a Claude session's, so the ambiguity rule is scoped to the
/// kind as well as the directory. Without that scoping the natural
/// implementation (group by directory) would make a mixed pair — which is
/// an ordinary thing for a user to do — permanently uncapturable.
#[tokio::test]
async fn a_claude_and_a_codex_session_in_one_directory_do_not_poison_each_other() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let claude = record_session(&h, &fixtures, work.path(), "claude").await;
    let codex = record_session(&h, &fixtures, work.path(), "codex").await;
    let (_c1, _r1, _s1, id_claude) = provoke_record(&h, &claude).await;
    let (_c2, _r2, _s2, id_codex) = provoke_record(&h, &codex).await;
    let at_claude = wait_for_first_input(&h, &claude.id, 20).await;
    let at_codex = wait_for_first_input(&h, &codex.id, 20).await;
    assert_windows_overlap(at_claude, at_codex);

    assert_eq!(wait_for_capture(&h, &claude.id, 30).await, id_claude);
    assert_eq!(wait_for_capture(&h, &codex.id, 30).await, id_codex);
}

/// The audited constraint that shapes the entire correlator: the record
/// appears at first PROMPT submission, not at launch, and the gap between
/// them is unbounded. So a session left sitting well past every window
/// constant in the code must still capture the moment its user finally
/// types — there is no deadline running from creation, and this test fails
/// loudly if one is ever introduced.
///
/// The idle period is longer than the window AND the publication grace
/// together, which is the whole span any timeout-shaped implementation
/// could plausibly have used; list passes run throughout, so such an
/// implementation would have settled the session `UncapturedFinal` before
/// the prompt ever arrived.
#[tokio::test]
async fn a_first_prompt_delayed_past_every_window_constant_still_captures() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;

    let idle =
        TEST_CAPTURE_BEFORE + TEST_CAPTURE_AFTER + TEST_CAPTURE_GRACE + Duration::from_secs(2);
    let deadline = tokio::time::Instant::now() + idle;
    while tokio::time::Instant::now() < deadline {
        h.client.list_sessions().await.expect("list");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "nothing may be claimed before the agent has written anything"
    );
    assert_eq!(
        snapshot_of(&h, &session.id).await.first_input_at,
        None,
        "and no correlator clock may have started either"
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// The munged-cwd collision, end to end through two real sessions.
///
/// `a.b` and `a-b` munge to the SAME Claude project directory, so both
/// sessions' records land side by side in one place. Only the recorded
/// `cwd` FIELD can tell them apart, which is exactly why SPEC_impl.md
/// records the munging as non-injective. Their first inputs are close
/// together on purpose: if directory membership were doing the work, the
/// two would look like a shared-directory collision and BOTH would bail —
/// so a passing test proves the field filter ran before the ambiguity rule
/// ever had anything to complain about.
#[tokio::test]
async fn two_directories_that_munge_alike_are_separated_by_the_recorded_cwd() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let dotted = parent.path().join("a.b");
    let dashed = parent.path().join("a-b");
    std::fs::create_dir(&dotted).expect("mkdir a.b");
    std::fs::create_dir(&dashed).expect("mkdir a-b");
    assert_eq!(
        farhelm_supervisor::agent_kind::munge_cwd(
            &std::fs::canonicalize(&dotted).unwrap().to_string_lossy()
        ),
        farhelm_supervisor::agent_kind::munge_cwd(
            &std::fs::canonicalize(&dashed).unwrap().to_string_lossy()
        ),
        "the premise of this test is that these two collide"
    );

    let one = record_session(&h, &fixtures, &dotted, "claude").await;
    let two = record_session(&h, &fixtures, &dashed, "claude").await;
    let (_c1, _r1, _s1, id_one) = provoke_record(&h, &one).await;
    let (_c2, _r2, _s2, id_two) = provoke_record(&h, &two).await;

    assert_eq!(wait_for_capture(&h, &one.id, 30).await, id_one);
    assert_eq!(wait_for_capture(&h, &two.id, 30).await, id_two);
}

/// Correlation uses the CANONICAL working directory, not the spelling the
/// caller sent, because the agent records its own `getcwd()` — which the
/// kernel has already resolved. A session created through a symlink, or
/// with a dot component, or with a trailing slash, must therefore still
/// find its own records; without the resolution its munged directory name
/// and its recorded-cwd comparison would both miss, and capture would
/// simply never happen for anyone whose path was not already canonical.
#[tokio::test]
async fn a_symlinked_or_dotted_working_directory_still_correlates() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let real = parent.path().join("real");
    std::fs::create_dir(&real).expect("mkdir");
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let canonical = std::fs::canonicalize(&real).expect("canonicalize");

    // Through the symlink, with a dot component and a trailing slash for
    // good measure — three different ways of naming the same directory.
    let spelled = link.join(".").join("");
    let session = record_session(&h, &fixtures, &spelled, "claude").await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.canonical_cwd.as_deref(),
        Some(canonical.to_string_lossy().as_ref()),
        "the resolved spelling is what correlation must use"
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// Codex gets the same canonical-cwd proof as Claude, because the two
/// consume it differently: Claude uses it to build the project DIRECTORY
/// name, while Codex has no per-directory tree at all and uses it only for
/// the recorded-field comparison. A fix that resolved the path for one
/// path and not the other would pass a Claude-only test.
#[tokio::test]
async fn a_symlinked_working_directory_still_correlates_for_codex() {
    let (h, fixtures) = capture_harness().await;
    let parent = tempfile::tempdir().expect("workdir");
    let real = parent.path().join("real");
    std::fs::create_dir(&real).expect("mkdir");
    let link = parent.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");

    let session = record_session(&h, &fixtures, &link, "codex").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
}

/// The claim discipline's central rule: nothing is made durable while the
/// window is still open, so a rival record arriving LATE inside the window
/// flips a provisional match to ambiguous instead of finding an identity
/// already committed.
///
/// The rival is planted directly rather than launched as a second session,
/// which is the sharper test: a second session would ALSO be caught by the
/// overlapping-windows rule, so this would pass even with the record-level
/// rule removed. A bare file in the same project directory, carrying the
/// same recorded cwd and a timestamp inside the window, can only be caught
/// by re-deriving the verdict from scratch on every pass — which is
/// exactly what the provisional state exists to make happen.
#[tokio::test]
async fn a_rival_record_arriving_late_in_the_window_flips_a_provisional_claim() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, _id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // One pass with only the real record present: the match exists, but it
    // is provisional, so nothing may be stored yet.
    h.client.list_sessions().await.expect("list drives capture");
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        None,
        "a match inside an open window must not be committed"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "nor advertised: Resume promises a stored identity a restart can fill in"
    );

    // Now a second record for the same directory, timestamped inside the
    // same window — the shape another agent running here would produce.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    let rival_line = serde_json::json!({
        "type": "user",
        "sessionId": "planted-rival-conversation",
        "cwd": canonical.to_string_lossy(),
        "timestamp": farhelm_supervisor::agent_kind::format_rfc3339(at),
    });
    std::fs::write(
        project.join("planted-rival.jsonl"),
        format!("{rival_line}\n"),
    )
    .expect("plant a rival record");

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation, None,
        "the late rival makes the correlation ambiguous, so nothing is claimed"
    );
    assert!(
        snapshot.capture_ambiguous,
        "and the refusal is recorded durably, not merely inferred each pass"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
}

/// A plain resume APPENDS to the existing record under the same id
/// (audited), so the watcher must treat an append as a confirmation rather
/// than as a new conversation — and an explicit fork, which writes a NEW
/// id, must not displace the identity already claimed.
///
/// Both halves are in one test because the second is only meaningful after
/// the first: the fork is written into the same directory the append just
/// touched, so a rescan that re-derived identity from "whatever is in this
/// directory now" would find two records and either bail or switch. The
/// captured identity must simply stay put — and the stored STAMP must
/// advance, which is what proves the re-verification actually re-read the
/// file rather than skipping it.
#[tokio::test]
async fn an_append_re_verifies_the_identity_and_a_fork_never_displaces_it() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (chan, mut rx, mut seen, id) = provoke_record(&h, &session).await;
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);

    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let record = fixtures
        .home
        .path()
        .join(".claude")
        .join("projects")
        .join(farhelm_supervisor::agent_kind::munge_cwd(
            &canonical.to_string_lossy(),
        ))
        .join(format!("{id}.jsonl"));
    let before = std::fs::metadata(&record).expect("the record exists").len();

    h.client.send_input(chan, b"append\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-APPENDED:", 20).await;
    // The append must be observable as a size change, or re-verification
    // is legitimately entitled to skip the re-read and this test would
    // assert nothing.
    let after = std::fs::metadata(&record).expect("the record exists").len();
    assert!(
        after > before,
        "the fixture's append must actually grow the record ({before} -> {after})"
    );

    for _ in 0..3 {
        h.client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        Some(id.clone()),
        "an append confirms the identity; it must not duplicate or replace it"
    );

    h.client.send_input(chan, b"fork\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-FORKED:", 20).await;
    let forked = marker_value(&seen, "RECORD-FORKED:");
    assert_ne!(forked, id, "a fork is a genuinely different conversation");
    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.captured_conversation.as_deref(),
        Some(id.as_str()),
        "the ORIGINAL conversation is this session's; the fork belongs to another"
    );
    assert_eq!(
        snapshot.resume_argv.as_deref().unwrap().last().unwrap(),
        &id
    );
}

/// The ambiguity bail, which is the mechanical form of SPEC.md's
/// never-silently-resume-the-wrong-conversation rule.
///
/// Two sessions launched near-simultaneously in one working directory have
/// overlapping windows — asserted from their durable first-input times, not
/// assumed from their ordering — so a record landing in the shared span
/// could honestly belong to either. Neither is captured, the refusal is
/// durable, and both keep offering the honest fresh launch.
///
/// The sticky half is what the second stage pins: DELETING the rival's
/// evidence entirely must not let the survivor change its mind. A pass
/// that re-derived the verdict from what is on disk right now would see
/// one clean candidate and claim it — on strictly worse evidence than the
/// pass that bailed.
#[tokio::test]
async fn two_near_simultaneous_sessions_in_one_directory_stay_uncaptured() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, id_first) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id_second) = provoke_record(&h, &second).await;
    let at_first = wait_for_first_input(&h, &first.id, 20).await;
    let at_second = wait_for_first_input(&h, &second.id, 20).await;
    assert_windows_overlap(at_first, at_second);

    settle_past_horizon(&h).await;
    for session in [&first, &second] {
        let snapshot = snapshot_of(&h, &session.id).await;
        assert_eq!(
            snapshot.captured_conversation, None,
            "an ambiguous correlation must claim nothing at all"
        );
        assert!(snapshot.capture_ambiguous, "and must record the refusal");
        assert_eq!(snapshot.resume_argv, None);
        assert_eq!(
            listed(&h.client, &session.id).await.restart_offer,
            farhelm_proto::RestartOffer::FreshOnly,
            "restart must offer the honest fallback, never a guessed resume"
        );
    }

    // Remove the SECOND session's record, leaving the first's alone in the
    // directory. A rescan that re-decided from present evidence would now
    // find exactly one candidate and claim it.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    for entry in std::fs::read_dir(&project).expect("project dir") {
        let entry = entry.expect("dir entry");
        if !entry.file_name().to_string_lossy().contains(&id_first) {
            std::fs::remove_file(entry.path()).expect("remove the rival's record");
        }
    }
    settle_past_horizon(&h).await;
    assert_eq!(
        snapshot_of(&h, &first.id).await.captured_conversation,
        None,
        "an ambiguity does not become less ambiguous because its evidence was tidied away"
    );
}

/// The snapshot is immutable and the captured identity is durable, both
/// across a supervisor restart — which is the only reason capture is worth
/// doing at all, since SPEC.md's resume offer exists precisely for the
/// sessions that outlived their supervisor.
///
/// The capture is deliberately provoked at RELOAD rather than by a list
/// before the shutdown: nothing calls `list_sessions` on the first
/// supervisor, so the identity this test finds afterwards can only have
/// been claimed by the successor's own reload pass. That is the path a
/// real restart takes — a session whose agent wrote its record while the
/// supervisor was down — and it is the one a list-driven test would never
/// exercise. Only the DURABLE first-input time is polled for, because that
/// is the fact the successor needs to correlate at all.
#[tokio::test]
async fn a_capture_missed_while_the_supervisor_was_down_lands_on_reload() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // Past the horizon, so the successor's very first pass is allowed to
    // commit — but with no list on THIS supervisor, so nothing here can.
    while farhelm_supervisor::agent_kind::now_unix()
        <= at + (TEST_CAPTURE_AFTER + TEST_CAPTURE_GRACE).as_secs() as i64
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.captured_conversation,
        None,
        "nothing drove a pass on this supervisor, so nothing can have been claimed"
    );

    // Release the first supervisor before constructing its replacement: an
    // overlapping successor starts read-only and reconciles nothing, so a
    // test that skipped this would exercise a path production never takes
    // (see `Supervisor::owns_state_dir`).
    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    assert!(
        restarted.owns_state_dir(),
        "the predecessor must be gone, or this proves nothing"
    );
    let after = restarted
        .session_snapshot(&session.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert_eq!(
        after.captured_conversation.as_deref(),
        Some(id.as_str()),
        "the successor's own reload pass is what captured this"
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::Resume);
    assert_eq!(after.kind, farhelm_proto::AgentKind::Claude);
    assert_eq!(
        after.resume_argv.as_deref().unwrap().last().unwrap(),
        &id,
        "and the snapshot it fills is the immutable one from create"
    );
    drop(_slot);
}

/// An ambiguity verdict survives a restart, and that durability is
/// load-bearing rather than tidy: after a restart the rival's evidence may
/// be gone (its session deleted, its record cleaned up), so a successor
/// that re-derived the verdict from what is on disk would see one clean
/// candidate and claim it — resuming a conversation the first supervisor
/// had already established it could not attribute.
#[tokio::test]
async fn an_ambiguity_survives_a_restart_even_when_its_evidence_does_not() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let first = record_session(&h, &fixtures, work.path(), "claude").await;
    let second = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_c1, _r1, _s1, id_first) = provoke_record(&h, &first).await;
    let (_c2, _r2, _s2, _id_second) = provoke_record(&h, &second).await;
    settle_past_horizon(&h).await;
    assert!(snapshot_of(&h, &first.id).await.capture_ambiguous);

    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    // The rival's record is gone by the time the successor looks.
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    for entry in std::fs::read_dir(&project).expect("project dir") {
        let entry = entry.expect("dir entry");
        if !entry.file_name().to_string_lossy().contains(&id_first) {
            std::fs::remove_file(entry.path()).expect("remove the rival's record");
        }
    }

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    let after = restarted
        .session_snapshot(&first.id)
        .await
        .expect("snapshot")
        .expect("present");
    assert!(after.capture_ambiguous, "the refusal survived");
    assert_eq!(
        after.captured_conversation, None,
        "and still refuses, even though only one candidate remains on disk"
    );
    assert_eq!(after.restart_offer, farhelm_proto::RestartOffer::FreshOnly);
    drop(_slot);
}

/// A durable write that FAILS must never yield a session that advertises
/// `Resume`: the offer promises a stored identity a restart can fill in,
/// and there is none. The retry then has to ride the polling cadence, not
/// the input path — so clearing the fault and polling again is what lands
/// the claim.
///
/// The same shape covers the first-input write, whose failure is quieter
/// and worse: correlation still works for this process, but a restart
/// would lose the anchor entirely, so the retry is the only thing that
/// makes capture survivable across the restart it exists for.
#[tokio::test]
async fn a_failed_durable_write_never_advertises_resume_and_is_retried() {
    let failing = Arc::new(AtomicBool::new(true));
    let armed = Arc::clone(&failing);
    let fault: CaptureStoreFault = Arc::new(move |_write, _session| {
        if armed.load(Ordering::SeqCst) {
            anyhow::bail!("injected capture-write failure")
        }
        Ok(())
    });
    let (h, fixtures) = capture_harness_with_fault(Some(fault)).await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, id) = provoke_record(&h, &session).await;

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.first_input_at, None,
        "the first-input write was refused, so nothing is stored"
    );
    assert_eq!(
        snapshot.captured_conversation, None,
        "and no identity may be committed either"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly,
        "a claim this process holds but could not store must not advertise Resume"
    );

    // The retry rides the poll, not the input path: nothing more is typed.
    failing.store(false, Ordering::SeqCst);
    assert_eq!(wait_for_capture(&h, &session.id, 30).await, id);
    assert!(
        snapshot_of(&h, &session.id).await.first_input_at.is_some(),
        "the first-input write is retried on the same cadence"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::Resume
    );
}

/// Input that never reaches the pane must not start the correlator's
/// clock. An empty data frame is the case that actually occurs (a client
/// flushing nothing), and starting the window on it would anchor the
/// session before its user has typed — narrowing, and possibly missing,
/// the window the real prompt lands in.
#[tokio::test]
async fn an_empty_input_frame_never_starts_the_correlator() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;

    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, Vec::new()).await;
    for _ in 0..5 {
        h.client.list_sessions().await.expect("list");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        snapshot_of(&h, &session.id).await.first_input_at,
        None,
        "nothing reached the pane, so nothing may have anchored the window"
    );

    // A real byte does anchor it, which is what makes the assertion above
    // about emptiness rather than about the hook never running.
    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    wait_for_first_input(&h, &session.id, 20).await;
}

/// Capture iterates EVERY session, not the capped subset `ListSessions`
/// replies with: the ambiguity rule is a statement about all sessions
/// sharing a working directory, so a session beyond the reply cap must
/// still poison a window it occupies. Otherwise a busy host would turn a
/// bail into a wrong capture — the one outcome this design exists to
/// exclude — and would do it only under load, which is the worst possible
/// way to find out.
///
/// The extra sessions are inserted straight into the store and brought
/// into memory by a restart, because what is under test is the pass's
/// iteration over the session map, not five hundred tmux panes. The
/// poisoning rival is one of them: it is a Claude session in the same
/// canonical directory whose first input lands inside the real session's
/// window, and it exists only as a row.
#[tokio::test]
async fn capture_considers_sessions_beyond_the_list_reply_cap() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let canonical = std::fs::canonicalize(work.path())
        .expect("canonicalize")
        .to_string_lossy()
        .into_owned();

    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    let (_chan, _rx, _seen, _id) = provoke_record(&h, &session).await;
    let at = wait_for_first_input(&h, &session.id, 20).await;

    // `_tmux` LAST, and that is not cosmetic: destructuring rebinds these
    // fields as ordinary locals, which drop in reverse declaration order —
    // so listing the guard before `state` would delete the state tempdir
    // (and with it the socket the guard kills through) before the guard
    // ever ran, leaking the tmux server. That leak was real and measured;
    // see `TmuxServerGuard`'s docs.
    let Harness {
        client,
        sup,
        state,
        _tmux,
        _slot,
    } = h;
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);

    let store = SessionStore::open(&state.path().join("supervisor.db"), false)
        .await
        .expect("open the store directly");
    for i in 0..=LIST_SESSION_CAP {
        // The last row is the rival: same kind, same canonical directory,
        // and a first input inside the real session's window.
        let rival = i == LIST_SESSION_CAP;
        store
            .insert_session(
                StoredSession {
                    archived: false,
                    id: format!("extra-{i}"),
                    parent: None,
                    title: format!("extra-{i}"),
                    created_at: now_unix(),
                    creation_seq: 0,
                    cwd: work.path().to_string_lossy().into_owned(),
                    invocation: "agent".to_string(),
                    tmux_name: format!("fh-extra-{i}"),
                    pane: String::new(),
                    outcome: LastOutcome::Exited {
                        exit_code: Some(0),
                        annotation: None,
                    },
                    agent_kind: if rival {
                        farhelm_proto::AgentKind::Claude
                    } else {
                        farhelm_proto::AgentKind::Generic
                    },
                    resume_template: rival.then(|| {
                        vec![
                            "claude".to_string(),
                            "--resume".to_string(),
                            "{conversation}".to_string(),
                        ]
                    }),
                    canonical_cwd: rival.then(|| canonical.clone()),
                    captured_conversation: None,
                    captured_record: None,
                    capture_ambiguous: false,
                    first_input_at: rival.then_some(at),
                    generation: 0,
                    launch_scoped: false,
                    source_profile: None,
                },
                None,
            )
            .await
            .expect("insert an extra session row");
    }
    drop(store);

    let restarted = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            agent_home: Some(fixtures.home.path().to_path_buf()),
            capture_window: test_capture_bounds(),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("restarted supervisor");
    let client = connect_client(&restarted).await;
    let listing = client.list_sessions().await.expect("list");
    assert!(
        listing.total > LIST_SESSION_CAP as u64,
        "this test's premise is that there are more sessions than the reply cap"
    );
    for _ in 0..5 {
        client.list_sessions().await.expect("list drives capture");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        restarted
            .session_snapshot(&session.id)
            .await
            .expect("snapshot")
            .expect("present")
            .capture_ambiguous,
        "a rival beyond the reply cap must still poison this session's window"
    );
    drop(_slot);
}

/// A session whose kind basename recognition would miss (`env claude`, a
/// wrapper) still captures once the caller says what it is — the reason
/// PLAN_M3.md item 7 carries explicit overrides at all. And a
/// placeholder-free template on a NON-integrated kind is the fallback shape
/// SPEC.md describes, which must reach the wire as `FallbackTemplate`
/// rather than being flattened into a fresh launch.
///
/// All three are asserted here because they are the same override slot, and
/// because none has a UI caller — the API and these tests are the only
/// consumers until M6.75's profiles, so an untested override is an unexercised
/// one.
#[tokio::test]
async fn an_overridden_kind_captures_and_a_generic_fallback_template_is_offered() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");

    // `farhelm internal fake-agent ...`: basename `farhelm`, so derivation
    // says generic. The override is what makes it claude.
    let invocation = agent_cmd(&format!(
        "internal fake-agent --script claude-record --record-home {}",
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    ));
    let overridden = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Claude),
                resume_template: Some(vec![
                    "my-wrapper".to_string(),
                    "--resume".to_string(),
                    "{conversation}".to_string(),
                ]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with overrides");
    let (_chan, _rx, _seen, id) = provoke_record(&h, &overridden).await;
    assert_eq!(wait_for_capture(&h, &overridden.id, 30).await, id);
    assert_eq!(
        snapshot_of(&h, &overridden.id)
            .await
            .resume_argv
            .as_deref()
            .unwrap(),
        ["my-wrapper", "--resume", &id]
    );

    // A generic session with a verbatim, placeholder-free resume
    // invocation: nothing to capture, but a real fallback to offer.
    let fallback = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                resume_template: Some(vec!["some-agent".to_string(), "--continue".to_string()]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect("create with a fallback template");
    assert_eq!(
        fallback.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate,
        "the create reply already knows this session has a fallback"
    );
    assert_eq!(
        listed(&h.client, &fallback.id).await.restart_offer,
        farhelm_proto::RestartOffer::FallbackTemplate
    );

    // ...and the invariant that keeps the promise honest: an INTEGRATED
    // kind may not carry a placeholder-free template, because once capture
    // succeeded such a template could only discard the identity.
    let refused = h
        .client
        .create_session_with_extras(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
            farhelm_helm::CreateExtras {
                agent_kind: Some(farhelm_proto::AgentKind::Codex),
                resume_template: Some(vec!["codex".to_string(), "resume".to_string()]),
                ..farhelm_helm::CreateExtras::default()
            },
        )
        .await
        .expect_err("a placeholder-free template on an integrated kind is refused");
    assert!(
        format!("{refused:#}").contains("{conversation}"),
        "the refusal must name what is missing: {refused:#}"
    );
}

/// A keyed create REPLAYED after its session captured must report the
/// capture, not the create-time placeholder: the replay is "the same
/// answer to the same request", and the honest answer to "what would
/// restart do for this session" changes the moment an identity is claimed.
/// A replay frozen at create time would tell a retrying client `FreshOnly`
/// for a session that can in fact resume.
#[tokio::test]
async fn a_keyed_replay_after_capture_reports_the_resume_offer() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let invocation = format!(
        "{} internal fake-agent --script claude-record --record-home {}",
        shell_words::quote(&fixtures.bin.path().join("claude").to_string_lossy()),
        shell_words::quote(&fixtures.home.path().to_string_lossy())
    );
    let created = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            Some("intent-capture-replay".to_string()),
        )
        .await
        .expect("create");
    assert_eq!(
        created.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );

    let (_chan, _rx, _seen, id) = provoke_record(&h, &created).await;
    assert_eq!(wait_for_capture(&h, &created.id, 30).await, id);

    let replayed = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &invocation,
            None,
            80,
            24,
            Some("intent-capture-replay".to_string()),
        )
        .await
        .expect("replay");
    assert_eq!(replayed.id, created.id, "still one session for one intent");
    assert_eq!(
        replayed.restart_offer,
        farhelm_proto::RestartOffer::Resume,
        "the replay reports what restart would do NOW, not at create time"
    );
}

/// PLAN_M3.md's recorded M3 limitation, pinned so it cannot be lost: a
/// session whose own invocation resumes an existing conversation appends to
/// a record whose header timestamp predates its window, so nothing is
/// captured and the honest fresh-launch fallback is offered.
///
/// The shape is reproduced by planting an OLD record for this working
/// directory and running an integrated session that writes none of its
/// own. That is exactly what `claude --resume <id>` looks like from the
/// outside, and pinning it here is what makes a future change that starts
/// correlating on appends a deliberate decision rather than an accident —
/// see the plan for why that correlation is not free.
#[tokio::test]
async fn a_session_resuming_an_old_conversation_is_not_captured() {
    let (h, fixtures) = capture_harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let canonical = std::fs::canonicalize(work.path()).expect("canonicalize");
    let project = fixtures.home.path().join(".claude").join("projects").join(
        farhelm_supervisor::agent_kind::munge_cwd(&canonical.to_string_lossy()),
    );
    std::fs::create_dir_all(&project).expect("project dir");
    let old = serde_json::json!({
        "type": "user",
        "sessionId": "a-conversation-from-last-week",
        "cwd": canonical.to_string_lossy(),
        "timestamp": farhelm_supervisor::agent_kind::format_rfc3339(
            farhelm_supervisor::agent_kind::now_unix() - 7 * 24 * 3600,
        ),
    });
    std::fs::write(project.join("old.jsonl"), format!("{old}\n")).expect("plant an old record");

    // An integrated session that writes no record of its own — the `basic`
    // script under the `claude` name, which is what a resume looks like
    // from the supervisor's side.
    let invocation = format!(
        "{} internal fake-agent --script basic",
        shell_words::quote(&fixtures.bin.path().join("claude").to_string_lossy())
    );
    let session = h
        .client
        .create_session(&work.path().to_string_lossy(), &invocation, None, 80, 24)
        .await
        .expect("create");
    let (chan, mut rx) = h.client.attach(&session.id, 80, 24).await.expect("attach");
    let mut seen = Vec::new();
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"hello\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "echo:", 20).await;
    wait_for_first_input(&h, &session.id, 20).await;

    settle_past_horizon(&h).await;
    let snapshot = snapshot_of(&h, &session.id).await;
    assert_eq!(
        snapshot.kind,
        farhelm_proto::AgentKind::Claude,
        "the session IS integrated; the limitation is about correlation, not derivation"
    );
    assert_eq!(
        snapshot.captured_conversation, None,
        "the record's header predates the window, so it is not a candidate"
    );
    assert!(
        !snapshot.capture_ambiguous,
        "and this is a clean miss, not an ambiguity"
    );
    assert_eq!(
        listed(&h.client, &session.id).await.restart_offer,
        farhelm_proto::RestartOffer::FreshOnly
    );
}

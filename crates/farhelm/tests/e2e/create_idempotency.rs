//! Server-enforced create idempotency: dropped-reply retries and
//! genuine crash-partway-through cases.

use crate::harness::*;

use crate::boot_id_durable_outcome::{crash_at, listed, supervisor_believing_boot};

// ---------------------------------------------------------------------
// PLAN_M3.md item 6 / acceptance 7: server-enforced create idempotency.
//
// "The reply was dropped" is simulated throughout by simply calling
// create again with the same key: from the supervisor's side a retried
// create and a create whose reply never arrived are the same request, and
// nothing about the guarantee depends on how the first answer was lost.
// The crash-stage tests below are the cases where that is NOT enough —
// there the first attempt has to genuinely die partway through, which is
// what `crash_at` provides.
//
// Every test that "restarts" a supervisor does so through
// `handoff_to_new_supervisor`, which releases the predecessor's state-dir
// claim first. That is not tidiness: a second supervisor constructed while
// the first still holds the claim runs READ-ONLY (no reconciliation, no
// settlement), so a handoff test that skipped it would exercise a path
// production never takes.
// ---------------------------------------------------------------------

/// Create with an intent key through the real client, for the tests that
/// only vary the key and the client.
async fn create_keyed(
    client: &SupervisorClient,
    work: &std::path::Path,
    key: &str,
) -> anyhow::Result<SessionInfo> {
    client
        .create_session_with_key(
            &work.to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("keyed".to_string()),
            80,
            24,
            Some(key.to_string()),
        )
        .await
}

/// How many tmux sessions this state directory's private server holds —
/// the check that actually proves "no second process", since a duplicate
/// create leaves a duplicate agent behind whether or not anything lists it.
async fn tmux_session_count(sock: &std::path::Path) -> usize {
    tmux_session_names(sock).await.len()
}

/// The `fh-*` tmux session names this server holds.
async fn tmux_session_names(sock: &std::path::Path) -> Vec<String> {
    let out = tmux_query(sock, &["list-sessions", "-F", "#{session_name}"]).await;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| line.starts_with("fh-"))
        .map(str::to_string)
        .collect()
}

/// The supervisor's own view of a session's durable row, read through a
/// SECOND store handle on the same database.
///
/// Reading the database directly is the point: these tests assert on what
/// a crash left DURABLY, which the session list deliberately does not
/// expose (it reports classified status, not stored state), and doing it
/// through the real `SessionStore` keeps the assertions in the same
/// vocabulary the supervisor uses rather than in raw SQL. `may_migrate:
/// false` because the supervisor under test owns that right.
async fn stored_reservation(state: &std::path::Path, key: &str) -> Option<Reservation> {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    store.reservation(key).await.expect("read the reservation")
}

/// Every session row the database holds, in id order.
async fn stored_sessions(state: &std::path::Path) -> Vec<StoredSession> {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    let mut rows = store.load_all().await.expect("load");
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

/// Retire `sup` (and the client whose connection task holds its own
/// reference) and construct its replacement, which must genuinely OWN the
/// state directory.
///
/// The wait is the whole point: `StateDirOwnership` releases the `flock`
/// when the LAST `Supervisor` for the directory drops, and a connection
/// task holds an `Arc` of its own for as long as its pipe is open. Without
/// draining that first, the replacement is constructed alongside a live
/// predecessor and silently starts read-only — which would make every
/// assertion below about reload's reconciliation vacuous.
pub(crate) async fn handoff_to_new_supervisor(
    state: &std::path::Path,
    sup: Arc<Supervisor>,
    client: Arc<SupervisorClient>,
) -> Arc<Supervisor> {
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup) > 1 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the old supervisor's connection tasks never released it"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup);
    let replacement = Supervisor::new_with_exe(state, farhelm_bin().into())
        .await
        .expect("the restarted supervisor");
    assert!(
        replacement.owns_state_dir(),
        "the replacement must hold the state directory's claim, or it reconciles nothing \
         and this test would pass for the wrong reason"
    );
    replacement
}

/// M3 acceptance 7's first clause: a create whose reply is lost AFTER the
/// session durably exists, retried with the same key, returns the SAME
/// session and starts no second process — including when the supervisor
/// restarts between the two attempts.
///
/// The restart half is not a bonus: it is the case a purely in-memory
/// dedup table would pass the first half of and fail here, and it is
/// exactly the shape of a real ambiguous failure (the supervisor being
/// restarted or crashing is one of the reasons a reply goes missing in the
/// first place).
#[tokio::test]
async fn a_retried_create_returns_the_same_session_across_a_supervisor_restart() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client1 = connect_client(&sup1).await;

    let first = create_keyed(&client1, work.path(), "intent-1")
        .await
        .expect("create");
    let replay = create_keyed(&client1, work.path(), "intent-1")
        .await
        .expect("a retry of the same intent must succeed, not conflict with itself");
    assert_eq!(
        replay.id, first.id,
        "the retry must replay the session the first attempt created"
    );

    let sup2 = handoff_to_new_supervisor(state.path(), sup1, client1).await;
    let client2 = connect_client(&sup2).await;
    let after_restart = create_keyed(&client2, work.path(), "intent-1")
        .await
        .expect("the retry must still replay after a restart");
    assert_eq!(
        after_restart.id, first.id,
        "the reservation is durable, so a restart cannot forget which session an intent made"
    );
    // Dimensions are deliberately NOT part of the fingerprint — they shape
    // the attachment, not the session — so the same intent retried from a
    // differently-sized client is still the same intent.
    let resized = client2
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("keyed".to_string()),
            132,
            50,
            Some("intent-1".to_string()),
        )
        .await
        .expect("a retry from a different-sized client is the same intent");
    assert_eq!(resized.id, first.id);

    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "four creates of one intent must leave exactly one session: {:?}",
        listing.sessions
    );
    assert_eq!(
        tmux_session_count(&state.path().join("tmux.sock")).await,
        1,
        "and exactly one agent process behind it"
    );
    drop(slot);
}

/// The other direction of the same key: reused for a DIFFERENT request, it
/// is refused rather than merged.
///
/// A reused key is a client bug — two distinct intents cannot share one
/// identity — and answering with the first intent's session would hand the
/// caller a session it never asked for while silently discarding the one it
/// did. `Conflict` is the classification the helm renders as a 409.
#[tokio::test]
async fn a_key_reused_for_a_different_request_is_refused() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let other = tempfile::tempdir().expect("other workdir");
    let first = create_keyed(&h.client, work.path(), "intent-2")
        .await
        .expect("create");

    let error = create_keyed(&h.client, other.path(), "intent-2")
        .await
        .expect_err("a different cwd under the same key must be refused");
    let downcast = error
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .expect("the refusal must carry a classified kind");
    assert_eq!(downcast.kind, ErrorKind::Conflict);
    assert!(
        downcast.message.contains("intent-2"),
        "the refusal must name the key: {}",
        downcast.message
    );

    let listing = h.client.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "the refused request must not have created anything: {:?}",
        listing.sessions
    );
    assert_eq!(listing.sessions[0].id, first.id);
}

/// Concurrent creates sharing one intent key collapse to ONE launch, and
/// both callers get the same session.
///
/// The barrier is what makes this a real concurrency test rather than two
/// requests that happened to be issued close together: the first create is
/// HELD inside its launch (through the create-lifecycle seam) until the
/// second has demonstrably been issued, so the two genuinely overlap. A
/// plain `join!` proves nothing — the runtime is free to run them one after
/// the other, and usually does.
///
/// Two connections, not one: the supervisor handles each connection's
/// control messages in a single serial read loop, so two creates on one
/// connection would serialize before ever reaching the idempotency
/// machinery.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_creates_under_one_intent_key_yield_one_session() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    // Released by the test once the second create is in flight; the first
    // create parks inside its launch until then.
    let (release, held) = std::sync::mpsc::channel::<()>();
    let held = std::sync::Mutex::new(held);
    let sup = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(Arc::new(move |stage| {
                if stage == CreateStage::DuringLaunch {
                    // Blocking, not awaiting: the seam is synchronous, and
                    // `block_in_place` is what keeps the rest of the
                    // runtime — including the second create — running.
                    tokio::task::block_in_place(|| {
                        let held = held.lock().expect("barrier mutex");
                        let _ = held.recv_timeout(Duration::from_secs(30));
                    });
                }
                Ok(())
            })),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let first_client = connect_client(&sup).await;
    let second_client = connect_client(&sup).await;

    let held_create = {
        let client = Arc::clone(&first_client);
        let work = work.path().to_path_buf();
        tokio::spawn(async move { create_keyed(&client, &work, "intent-3").await })
    };
    // The second create is issued while the first is parked in its launch.
    let racing_create = {
        let client = Arc::clone(&second_client);
        let work = work.path().to_path_buf();
        tokio::spawn(async move { create_keyed(&client, &work, "intent-3").await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !held_create.is_finished() && !racing_create.is_finished(),
        "test fixture: both creates must still be in flight when the barrier releases"
    );
    let _ = release.send(());

    let a = held_create.await.expect("join").expect("first create");
    let b = racing_create.await.expect("join").expect("second create");
    assert_eq!(
        a.id, b.id,
        "both callers must be answered with the same session"
    );

    let listing = first_client.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "one intent, one session, however many requests raced: {:?}",
        listing.sessions
    );
    assert_eq!(
        tmux_session_count(&state.path().join("tmux.sock")).await,
        1,
        "and one agent process — a second launch is the failure this collapses"
    );
    drop(slot);
}

/// A replay whose session was since DELETED returns an explicit
/// gone-error: never a live-looking success carrying a dead id, and never
/// a fresh duplicate.
///
/// Both wrong answers are worth naming. Handing back the deleted id would
/// have the client attach to nothing; creating a second session would
/// resurrect work the user explicitly threw away, under a key they have no
/// reason to think is still live. The honest answer is that the key is
/// spent, and the message says which session it was spent on.
#[tokio::test]
async fn a_replay_for_a_deleted_session_reports_that_it_is_gone() {
    let h = harness().await;
    let work = tempfile::tempdir().expect("workdir");
    let session = create_keyed(&h.client, work.path(), "intent-4")
        .await
        .expect("create");
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete the session the intent created");

    let error = create_keyed(&h.client, work.path(), "intent-4")
        .await
        .expect_err("a replay for a deleted session must not succeed");
    let downcast = error
        .chain()
        .find_map(|c| c.downcast_ref::<SupervisorError>())
        .expect("the gone-error must carry a classified kind");
    assert_eq!(
        downcast.kind,
        ErrorKind::Conflict,
        "nothing is missing — the KEY is spent, which is the same rule a reuse gets"
    );
    assert!(
        downcast.message.contains(&session.id) && downcast.message.contains("deleted"),
        "the error must name the session and what happened to it: {}",
        downcast.message
    );
    assert!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .is_empty(),
        "and it must not have created a replacement"
    );
}

/// The durable state a crash at `stage` leaves behind, as the next
/// supervisor's reload finds it.
struct CrashScene {
    /// The session row the interrupted attempt left, if any.
    row: Option<StoredSession>,
    /// tmux session names alive at the moment of inspection.
    tmux: Vec<String>,
    /// The reservation, after the replacement supervisor's reload has run
    /// but BEFORE any retry — so reload's own settlement pass is visible
    /// rather than masked by the retry's reconciliation.
    reservation_after_reload: Reservation,
}

/// Crash the first attempt at `stage`, inspect what it left, hand the
/// state directory to a fresh supervisor, inspect again, then retry — and
/// assert what every stage must equally produce: exactly one session,
/// exactly one live agent, and a usable answer.
///
/// The per-stage evidence is returned rather than asserted here because it
/// is the one thing that DIFFERS: the shared assertions below would pass
/// even if two stages left identical state, which would mean the seam was
/// not injecting where it claims to.
async fn retry_after_a_crash_at(stage: CreateStage) -> CrashScene {
    let state = tempfile::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));
    let sock = state.path().join("tmux.sock");
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            create_crash: Some(crash_at(stage)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    create_keyed(&client1, work.path(), "intent-crash")
        .await
        .expect_err("the injected crash must fail the create");

    // What the crash left, before anything reconciles it.
    let rows = stored_sessions(state.path()).await;
    assert!(rows.len() <= 1, "one create can leave at most one row");
    let scene_row = rows.into_iter().next();
    let scene_tmux = tmux_session_names(&sock).await;

    // The retry arrives at a supervisor that never saw the first attempt
    // in memory — the only honest way to model a crash, and what makes
    // reload's own reconciliation load-bearing rather than incidental.
    let sup2 = handoff_to_new_supervisor(state.path(), sup1, client1).await;
    let reservation_after_reload = stored_reservation(state.path(), "intent-crash")
        .await
        .expect("the reservation outlives the crash");
    let client2 = connect_client(&sup2).await;

    let stranded: Vec<String> = client2
        .list_sessions()
        .await
        .expect("list what the crash left")
        .sessions
        .into_iter()
        .map(|s| s.id)
        .collect();
    let session = create_keyed(&client2, work.path(), "intent-crash")
        .await
        .expect("the retry must produce a session");
    assert_eq!(
        stranded,
        vec![session.id.clone()],
        "the retry must land on the identity the reservation already assigned — replaying it \
         or relaunching under it, never minting a second one beside it"
    );

    let listing = client2.list_sessions().await.expect("list");
    assert_eq!(
        listing.sessions.len(),
        1,
        "a crash at {stage:?} then a retry must leave exactly one session: {:?}",
        listing.sessions
    );
    assert_eq!(listing.sessions[0].id, session.id);
    assert_eq!(
        tmux_session_names(&sock).await,
        vec![format!("fh-{}", session.id)],
        "tmux must hold exactly the session the retry handed back"
    );
    // Waited for rather than read once: the agent must be running in it,
    // and a single list landing on a tolerated tmux diagnostic reports a
    // live session as `Exited { None }` (see `wait_for_listing`).
    wait_for_live_status(&client2, &session.id, 30).await;
    // The retried session is a normal session: deleting it tears down
    // everything the retry built (the other half of item 6's
    // retry-versus-delete ordering — when the retry wins, the delete that
    // follows cleans up its work).
    client2
        .delete_session(&session.id)
        .await
        .expect("delete the retried session");
    assert!(
        tmux_session_names(&sock).await.is_empty(),
        "the delete must have torn down the relaunched agent too"
    );

    CrashScene {
        row: scene_row,
        tmux: scene_tmux,
        reservation_after_reload,
    }
}

/// Crash after the reservation and its launching row commit, before any
/// side effect: the retry PERFORMS the create, under the identities the
/// reservation already carries.
///
/// The distinguishing fact is that nothing was ever launched, so a retry
/// that replayed instead of launching would hand back a session id with no
/// agent behind it — which is why "absent side effects means redo" is a
/// real branch and not an optimization. The reservation must therefore
/// still be PENDING after reload: settling it as created would mean
/// claiming a session that never existed.
#[tokio::test]
async fn a_crash_after_the_reservation_lets_the_retry_perform_the_create() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::AfterRecord).await;
    assert_eq!(
        scene
            .row
            .expect("the launching row is the evidence")
            .outcome,
        LastOutcome::Launching
    );
    assert!(
        scene.tmux.is_empty(),
        "the crash landed before any side effect, so tmux must hold nothing: {:?}",
        scene.tmux
    );
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Pending,
        "reload must not settle an intent whose launch left no trace"
    );
    drop(slot);
}

/// Crash between tmux having the session and the launch being confirmed
/// durably: the tmux session and its pane exist under a row that still
/// says `Launching`.
///
/// The retry must not launch a second one. What makes that work is the
/// unification item 6 asks for — reload rediscovers the pane and confirms
/// the row, and the reservation settles from that same verdict instead of
/// probing tmux a second time and risking a different answer. Settled by
/// RELOAD, before any retry arrives, which is what the reservation
/// assertion pins.
#[tokio::test]
async fn a_crash_during_the_launch_reconciles_to_the_session_that_started() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::DuringLaunch).await;
    let row = scene.row.expect("the launching row survives");
    assert_eq!(row.outcome, LastOutcome::Launching);
    assert!(
        row.pane.is_empty(),
        "the confirmation never ran, so no pane was recorded"
    );
    assert_eq!(
        scene.tmux,
        vec![format!("fh-{}", row.id)],
        "but tmux really does hold the session"
    );
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Created,
        "reload's own pass must settle this intent, with no retry involved"
    );
    drop(slot);
}

/// Crash after the launch is confirmed and before the reservation's
/// outcome is recorded — acceptance 7's "the reply is dropped AFTER the
/// session durably exists", from the inside.
///
/// This is the window a reservation alone could never cover: the session
/// is complete and only the intent table does not know it, which is
/// exactly why the lifecycle is a state machine with a reconciling PENDING
/// state rather than one durable "this key is spent" flag.
#[tokio::test]
async fn a_crash_before_the_outcome_commit_still_replays_the_same_session() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let scene = retry_after_a_crash_at(CreateStage::BeforeOutcome).await;
    let row = scene.row.expect("the confirmed row survives");
    assert_eq!(
        row.outcome,
        LastOutcome::Running,
        "the launch was confirmed durably before the crash"
    );
    assert!(!row.pane.is_empty(), "with its pane recorded alongside");
    assert_eq!(scene.tmux, vec![format!("fh-{}", row.id)]);
    assert_eq!(
        scene.reservation_after_reload.outcome,
        ReservationOutcome::Created,
        "reload settles the intent whose session is demonstrably complete"
    );
    drop(slot);
}

/// PLAN_M3.md item 6 meeting item 2's reboot conversion: a create that
/// crashed BEFORE reaching tmux, then survived a reboot, must not be
/// replayed as though it had succeeded.
///
/// The reboot converts its launching row to `interrupted`, which looks
/// terminal — and settling a reservation from a terminal status alone
/// would record "created" for a session that never launched and can never
/// run, permanently. Provenance, not status, is what the settlement needs:
/// this row never had a pane, so nothing ever saw it in tmux.
///
/// The retry must therefore still perform the create, under the same
/// identity, exactly as it would have without the reboot.
#[tokio::test]
async fn a_reboot_does_not_turn_a_never_launched_intent_into_a_created_one() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = tempfile::tempdir().expect("tempdir");
    let guard = TmuxServerGuard(state.path().join("tmux.sock"));
    let work = tempfile::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_seams(
        state.path(),
        farhelm_bin().into(),
        SupervisorTimeouts::default(),
        SupervisorSeams {
            boot_id: Arc::new(|| Ok(Some("boot-a".to_string()))),
            create_crash: Some(crash_at(CreateStage::AfterRecord)),
            ..SupervisorSeams::default()
        },
    )
    .await
    .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    create_keyed(&client1, work.path(), "intent-reboot")
        .await
        .expect_err("the injected crash must fail the create");

    // A reboot: a different boot id AND no surviving tmux server.
    drop(client1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while Arc::strong_count(&sup1) > 1 {
        assert!(tokio::time::Instant::now() < deadline, "connection drain");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(sup1);
    // Dropping the guard kills the private tmux server, which is the other
    // half of a reboot: a boot-id change alone would leave live panes for
    // the reload to find, which is not a reboot at all. The replacement
    // guard covers the server the next supervisor starts.
    drop(guard);
    let _tmux = TmuxServerGuard(state.path().join("tmux.sock"));

    let sup2 = supervisor_believing_boot(state.path(), Some("boot-b")).await;
    assert!(
        sup2.owns_state_dir(),
        "the replacement must own the state dir"
    );
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed(&client2, &stored_sessions(state.path()).await[0].id)
            .await
            .status,
        SessionStatus::Interrupted,
        "test fixture: the reboot must have converted the stranded row"
    );
    assert_eq!(
        stored_reservation(state.path(), "intent-reboot")
            .await
            .expect("the reservation survives")
            .outcome,
        ReservationOutcome::Pending,
        "an interrupted row with no pane is no evidence a launch ever happened"
    );

    let session = create_keyed(&client2, work.path(), "intent-reboot")
        .await
        .expect("the retry must perform the create the crash never did");
    // The session it hands back must be a real, running one — not the
    // interrupted placeholder the reboot left. Waited for rather than read
    // once, for the reason `wait_for_listing` documents.
    wait_for_live_status(&client2, &session.id, 30).await;
    assert_eq!(
        stored_sessions(state.path()).await.len(),
        1,
        "still one session for one intent"
    );
    drop(slot);
}

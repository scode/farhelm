//! Session rename: the two-part write, the reply it builds, its refusals,
//! and what it must leave alone.
//!
//! Driven at the raw protocol level because `SupervisorClient` has no
//! rename method yet — the helm's REST route and client convenience method
//! are PLAN_M5.md item 4's PR. Everything asserted here is therefore the
//! supervisor's own contract, which is where SPEC.md puts the authority
//! anyway: the helm neither validates nor rewrites a title.

use crate::harness::*;

use crate::conversation_identity_capture::{
    TEST_CAPTURE_AFTER, TEST_CAPTURE_GRACE, capture_harness, provoke_record, record_session,
    wait_for_capture,
};
use crate::create_idempotency::handoff_to_new_supervisor;
use crate::terminal_backpressure::drain_for;
use farhelm_proto::RestartOffer;

// ---------------------------------------------------------------------
// Rename (PLAN_M5.md item 3)
//
// The half worth stating up front: a rename is durable AND in-memory. The
// supervisor serves list replies from `SessionEntry` values it never
// re-reads from SQLite mid-process, so a store-only implementation would
// keep serving the old title until a restart — it passes the persistence
// test and every refusal test here, and is caught by two:
// `a_rename_is_visible_in_the_next_list_reply_without_a_restart` directly,
// and the concurrent-rename test through the store-versus-list comparison
// it makes.
//
// The other half is the reply. `SessionRenamed` carries a `SessionInfo`
// built the way `ListSessions` builds one — live-probed status (launch
// sentinel included), rediscovered tabs, freshly derived restart offer —
// and three tests below exist only to keep that from decaying into "the
// stored row with a new title spliced in".
// ---------------------------------------------------------------------

/// Send one `RenameSession` over a connection of its own and return the
/// supervisor's answer, whatever it is.
///
/// A fresh connection per call rather than a shared peer: several tests
/// below need two renames genuinely in flight at once, and the answer is
/// correlated by `req_id` on a connection that carries nothing else, so
/// there is no reply to disambiguate.
async fn rename(sup: &Arc<Supervisor>, session_id: &str, title: &str) -> ControlMsg {
    let (client_side, server_side) = tokio::io::duplex(1 << 20);
    let sup = Arc::clone(sup);
    tokio::spawn(async move {
        let _ = handle_connection(sup, server_side).await;
    });
    let (read_half, write_half) = tokio::io::split(client_side);
    let mut reader = FrameReader::new(read_half);
    let mut writer = FrameWriter::new(write_half);
    handshake(&mut reader, &mut writer, "helm")
        .await
        .expect("handshake");
    writer
        .write_control(&ControlMsg::RenameSession {
            req_id: 1,
            session_id: session_id.to_string(),
            title: title.to_string(),
        })
        .await
        .expect("write rename");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let frame = tokio::time::timeout(remaining, reader.read_frame())
            .await
            .expect("timed out waiting for the rename reply")
            .expect("read frame")
            .expect("connection closed before the rename was answered");
        if frame.kind == FrameKind::Control {
            return parse_control(&frame).expect("parse control");
        }
    }
}

/// The `SessionInfo` a successful rename replied with, failing the test
/// with the supervisor's own words on a refusal.
fn renamed(reply: ControlMsg) -> SessionInfo {
    match reply {
        ControlMsg::SessionRenamed { session, .. } => session,
        ControlMsg::Error { message, kind, .. } => {
            panic!("rename refused ({kind:?}): {message}")
        }
        other => panic!("expected SessionRenamed, got {other:?}"),
    }
}

/// The `(kind, message)` of a refused rename, failing the test if the
/// supervisor accepted it instead.
fn refusal(reply: ControlMsg) -> (ErrorKind, String) {
    match reply {
        ControlMsg::Error { kind, message, .. } => (kind, message),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// One session's title as the CURRENT process reports it in a list reply.
async fn listed_title(client: &SupervisorClient, session_id: &str) -> String {
    client
        .list_sessions()
        .await
        .expect("list")
        .sessions
        .into_iter()
        .find(|s| s.id == session_id)
        .unwrap_or_else(|| panic!("session {session_id} vanished from the list"))
        .title
}

/// One session's captured conversation as the DATABASE holds it, read
/// through a second store handle.
///
/// Read-only and pass-free, which is the point wherever it is used: every
/// other way of asking (a list reply, `session_snapshot`) DRIVES a capture
/// pass, so a test that asked that way could not tell an identity the code
/// under test captured from one the question itself captured.
async fn stored_conversation(state: &std::path::Path, session_id: &str) -> Option<String> {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    store
        .session(session_id)
        .await
        .expect("read the session row")
        .unwrap_or_else(|| panic!("session {session_id} has no durable row"))
        .captured_conversation
}

/// One session's title as the DATABASE holds it, read through a second
/// store handle on the same file.
///
/// The durable half, independent of anything this process holds in memory
/// — which makes it the only way to tell a refused rename from one that
/// wrote the row and then reported a failure, and the only way to see
/// which of two concurrent writers the store ended up agreeing with.
async fn stored_title(state: &std::path::Path, session_id: &str) -> String {
    let store = SessionStore::open(&state.join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    store
        .session(session_id)
        .await
        .expect("read the session row")
        .unwrap_or_else(|| panic!("session {session_id} has no durable row"))
        .title
}

/// The load-bearing half of the write: a renamed session lists under its
/// new title IMMEDIATELY, in the same process, with no restart.
///
/// `SessionEntry` values are immutable once created and never re-read from
/// SQLite mid-process, so a rename that wrote only the durable row would
/// keep serving the old title from every list reply until the supervisor
/// was restarted — invisible to a persistence test, and the whole feature
/// broken for the user who just typed a new name.
///
/// The reply's own `SessionInfo` is checked alongside, because it is what
/// the UI paints before its next poll arrives.
#[farhelm_testtrace::test]
async fn a_rename_is_visible_in_the_next_list_reply_without_a_restart() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let reply = renamed(rename(&h.sup, &session.id, "renamed-in-place").await);
    assert_eq!(reply.title, "renamed-in-place");
    assert_eq!(reply.id, session.id);
    assert_eq!(
        listed_title(&h.client, &session.id).await,
        "renamed-in-place",
        "the renamed title must reach the very next list reply, with no restart"
    );
}

/// The reply's tabs are rediscovered from tmux, not echoed from the entry.
///
/// Tabs are never stored anywhere — they exist only as tmux window markers
/// — so a tab opened after the session was created can appear in a rename
/// reply only if that reply really was rebuilt from a live probe. This is
/// the cheapest of the three built-not-echoed pins; the other two cover
/// status and the restart offer, which need a session in a more particular
/// state to say anything.
#[farhelm_testtrace::test]
async fn the_rename_reply_rediscovers_tabs_from_tmux() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    assert!(
        session.tabs.is_empty(),
        "test premise: a fresh session has no tabs"
    );
    let tab = h.client.open_tab(&session.id).await.expect("open tab");

    let reply = renamed(rename(&h.sup, &session.id, "with-a-tab").await);
    assert_eq!(reply.title, "with-a-tab");
    assert_eq!(
        reply.tabs.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
        vec![tab.id],
        "the reply's tabs must be rediscovered from tmux, not echoed from the stored row"
    );
}

/// A rename reply reports the launch-sentinel **error** a list reply would
/// — a status no stored field holds and no pane probe can produce.
///
/// The sharpest form of the built-like-ListSessions contract. A failed exec
/// leaves an ordinary dead pane, indistinguishable from a command that ran
/// and exited, so the only thing that can tell them apart is the sentinel
/// the launch shim wrote. A reply that probed liveness but skipped the
/// sentinel — the obvious cheap implementation — would answer **exited**
/// here while every list reply for the same session says **error**, which
/// is the reply lying about the one field the user acts on.
///
/// The sentinel is planted at its own derived path rather than raced out
/// of a real failed launch, exactly as the sentinel tests do (see
/// `launch_sentinel_error_status`): reaching this class end to end means
/// corrupting supervisor-internal state either way, and planting it tests
/// the reader rather than whichever shim path produced the file.
#[farhelm_testtrace::test]
async fn a_rename_reply_reports_the_launch_sentinel_error_a_list_would() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec()).await;
    // The sentinel path is only consulted for a pane that is dead or gone,
    // so the agent has to be genuinely finished before it means anything.
    wait_for_non_live_status(&h.client, &session.id, 30).await;

    let detail = "exec failed: no such file or directory".to_string();
    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, &detail).expect("plant the sentinel");

    let reply = renamed(rename(&h.sup, &session.id, "renamed-after-a-failed-launch").await);
    assert_eq!(reply.title, "renamed-after-a-failed-launch");
    assert_eq!(
        reply.status,
        SessionStatus::Error {
            detail: detail.clone()
        },
        "the reply must surface the sentinel, exactly as a list reply does"
    );

    // The reply is only half of what a list pass does with a sentinel: it
    // also RECORDS the outcome and consumes the file. Both are asserted
    // from outside this process before anything lists, because a later
    // `ListSessions` would perform them itself and mask a rename that had
    // done neither — the reply would look right for the wrong reason.
    let store = SessionStore::open(&h.state.path().join("supervisor.db"), false)
        .await
        .expect("open the database a second time");
    assert_eq!(
        store
            .session(&session.id)
            .await
            .expect("read the session row")
            .expect("the session still exists")
            .outcome,
        LastOutcome::Error {
            detail: detail.clone()
        },
        "the rename's own pass must have recorded the sentinel's outcome durably"
    );
    assert!(
        !status_path.exists(),
        "a consumed sentinel is deleted once its Error outcome commits durably"
    );

    assert_eq!(
        h.client
            .list_sessions()
            .await
            .expect("list")
            .sessions
            .into_iter()
            .find(|s| s.id == session.id)
            .expect("the session is still listed")
            .status,
        SessionStatus::Error { detail },
        "and the recorded outcome keeps the list reply agreeing with the rename's"
    );
}

/// A rename reply offers **resume** for a conversation NOTHING has
/// captured yet — the rename's own pass is what captures it.
///
/// Two things at once, and the ordering is what makes the second one
/// testable. The offer moves from `FreshOnly` to `Resume` when a capture
/// pass commits an identity, so a reply echoing
/// `SessionEntry::info.restart_offer` would carry what it was at create —
/// `FreshOnly` forever — and the UI would keep offering a fresh launch for
/// a session that can be resumed.
///
/// But capture rides the passes the supervisor already performs, and a
/// test that drove those passes itself (by listing until the identity
/// landed) would leave the reply nothing to do but read a value already
/// committed — it would pass against a reply that never ran a capture pass
/// at all. So NOTHING here drives a pass, before or after: the record is
/// provoked, the horizon is slept past, and the rename is the first pass
/// of any kind to run afterwards. `Resume` in its reply can then only mean
/// the rename's own pass captured the identity, which is the
/// `ListSessions` behavior the protocol promises this reply matches.
///
/// The identity is then confirmed through a READ-ONLY store handle rather
/// than through `wait_for_capture`, for the same reason: that helper polls
/// `list_sessions`, and a list would have captured the identity itself,
/// retroactively making the assertion above pass for the wrong reason.
#[farhelm_testtrace::test]
async fn a_rename_reply_captures_and_offers_resume_without_a_list_first() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;
    assert_eq!(
        session.restart_offer,
        RestartOffer::FreshOnly,
        "test premise: nothing is capturable until the agent writes its record"
    );
    let (_chan, _rx, _seen, conversation) = provoke_record(&h, &session).await;

    // Past the horizon, without a single list: nothing may have committed
    // this session's identity before the rename runs. The margin is the
    // same shape the capture tests use — the window plus its publication
    // grace, plus a second for clock granularity.
    tokio::time::sleep(TEST_CAPTURE_AFTER + TEST_CAPTURE_GRACE + Duration::from_secs(1)).await;

    let reply = renamed(rename(&h.sup, &session.id, "renamed-after-capture").await);
    assert_eq!(reply.title, "renamed-after-capture");
    assert_eq!(
        reply.restart_offer,
        RestartOffer::Resume,
        "the rename's own capture pass must have claimed the identity and the reply must \
         reflect it, exactly as a list reply would"
    );
    // And it is genuinely the identity the agent reported, committed
    // durably — not a `Resume` derived from something weaker.
    assert_eq!(
        stored_conversation(h.state.path(), &session.id)
            .await
            .as_deref(),
        Some(conversation.as_str()),
        "the identity the rename captured must be the one the fixture wrote, and must have \
         been committed by that same pass"
    );
}

/// Renaming an ATTACHED session before its first input must not cost it
/// its conversation capture.
///
/// The regression this pins is invisible everywhere else and permanent
/// when it happens. A rename publishes a rebuilt entry, while the input
/// path writes the first-input anchor through the entry its `InputRoute`
/// pinned at attach time — an entry the rename has already replaced. If
/// the rebuild COPIED that cell instead of sharing it, the anchor would
/// land in the abandoned copy, the capture pass would go on reading the
/// published entry's empty one, and this session would never become
/// resumable: SPEC.md's resume promise silently broken by renaming a
/// session at the wrong moment, with nothing anywhere reporting it.
///
/// The ordering is therefore load-bearing: attach first (so a route pins
/// the pre-rename entry), rename second, and only then type.
#[farhelm_testtrace::test]
async fn a_rename_before_first_input_still_captures_the_conversation() {
    let (h, fixtures) = capture_harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let session = record_session(&h, &fixtures, work.path(), "claude").await;

    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    renamed(rename(&h.sup, &session.id, "renamed-before-typing").await);

    h.client.send_input(chan, b"first prompt\r".to_vec()).await;
    wait_for(&mut rx, &mut seen, "RECORD-WRITTEN:", 20).await;
    let conversation = crate::conversation_identity_capture::marker_value(&seen, "RECORD-WRITTEN:");

    assert_eq!(
        wait_for_capture(&h, &session.id, 30).await,
        conversation,
        "a rename must not strand the first-input anchor the capture window is measured from"
    );
}

/// The new title survives the supervisor process that applied it.
///
/// The durable half of the two-part write, and the one that makes rename
/// a metadata change rather than a per-process relabeling. A restarted
/// supervisor rebuilds every entry from SQLite, so this is also the only
/// way to observe that the row itself was written and not merely the map.
#[farhelm_testtrace::test]
async fn a_renamed_title_survives_a_supervisor_restart() {
    let slot = SLOTS.acquire().await.expect("semaphore is never closed");
    let state = farhelm_teststate::tempdir().expect("tempdir");
    let _tmux = TmuxServerGuard::new(state.path().join("tmux.sock"));
    let work = farhelm_teststate::tempdir().expect("workdir");
    let sup1 = Supervisor::new_with_exe(state.path(), farhelm_bin().into())
        .await
        .expect("supervisor");
    let client1 = connect_client(&sup1).await;
    let session = client1
        .create_session(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            None,
            80,
            24,
        )
        .await
        .expect("create");

    renamed(rename(&sup1, &session.id, "survives-a-restart").await);

    let sup2 = handoff_to_new_supervisor(state.path(), sup1, client1).await;
    let client2 = connect_client(&sup2).await;
    assert_eq!(
        listed_title(&client2, &session.id).await,
        "survives-a-restart",
        "a rename must outlive the supervisor that applied it"
    );
    drop(slot);
}

/// Renaming a session that no longer exists is `NotFound`.
///
/// The honest answer for a client whose session list is a poll behind
/// reality — someone deleted it in another window, or on another client
/// entirely. Inventing the row back, or reporting success against nothing,
/// would leave the caller believing a session exists that does not.
#[farhelm_testtrace::test]
async fn renaming_a_deleted_session_is_not_found() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    h.client
        .delete_session(&session.id)
        .await
        .expect("delete the session out from under the rename");

    let (kind, message) = refusal(rename(&h.sup, &session.id, "gone").await);
    assert_eq!(kind, ErrorKind::NotFound);
    assert!(
        message.contains(&session.id),
        "the refusal must name the session it could not find: {message}"
    );
}

/// A malformed rename of a session that does not exist reports the
/// MALFORMED half — `InvalidRequest`, not `NotFound`.
///
/// Validation runs ahead of the lookup on purpose: the caller can fix a
/// control character in a title, and telling them "no such session"
/// instead sends them looking for the wrong problem — most misleadingly
/// when the session really does exist and only the ID was mistyped
/// alongside a title that was also wrong. Pinning the precedence keeps a
/// later reordering from silently swapping the two answers.
#[farhelm_testtrace::test]
async fn a_malformed_rename_of_a_missing_session_reports_the_malformed_title() {
    let h = harness().await;
    let (kind, message) = refusal(
        rename(
            &h.sup,
            "3d4f4b1e-0000-4000-8000-000000000000",
            "escape\x1b[31m sequence",
        )
        .await,
    );
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert_eq!(message, "title must not contain control characters");
}

/// A control character in a title is refused with create's exact words,
/// and neither the listed nor the stored title moves.
///
/// The refusal exists because a title is echoed into terminals by
/// `tracing` consumers, where an embedded escape sequence is terminal
/// injection (SPEC.md names control characters as the refusal for a
/// supplied title's CONTENT). Rename shares `validate_create`'s
/// explicit-title rule rather than restating it, and this pins both halves
/// of that sharing: the same refusal, and the same message — the client's
/// whole contract for a refused title is the supervisor's own words, so a
/// divergent phrasing between the two verbs is a user-visible regression.
///
/// The DURABLE title is asserted alongside the listed one because they can
/// disagree: an implementation that wrote the row before validating would
/// leave the refused title in SQLite and only look correct until the next
/// restart.
#[farhelm_testtrace::test]
async fn a_control_character_title_is_refused_with_the_supervisors_words() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (kind, message) = refusal(rename(&h.sup, &session.id, "escape\x1b[31m sequence").await);
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert_eq!(message, "title must not contain control characters");
    assert_eq!(
        listed_title(&h.client, &session.id).await,
        session.title,
        "a refused rename must leave the old title in place"
    );
    assert_eq!(
        stored_title(h.state.path(), &session.id).await,
        session.title,
        "a refused rename must not have written the row either"
    );
}

/// A title past the field cap is refused, and the refusal names the bound.
///
/// The cap is create's own, applied to the title alone. It is not the
/// point at which a reply becomes undeliverable — the frame limit is two
/// orders of magnitude higher — but deliberately conservative headroom: a
/// title is echoed back in the reply and again in every list reply that
/// carries the session, and bounding the input this far below the frame
/// limit makes an oversized reply structurally impossible instead of
/// merely unlikely. SPEC.md documents the bound as of this milestone, so
/// it is user-visible behavior rather than an internal guard.
#[farhelm_testtrace::test]
async fn an_oversized_title_is_refused_before_anything_changes() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let huge = "t".repeat(64 * 1024 + 1);
    let (kind, message) = refusal(rename(&h.sup, &session.id, &huge).await);
    assert_eq!(kind, ErrorKind::InvalidRequest);
    assert!(
        message.contains("65536"),
        "the refusal must name the byte limit: {message}"
    );
    assert_eq!(listed_title(&h.client, &session.id).await, session.title);
    assert_eq!(
        stored_title(h.state.path(), &session.id).await,
        session.title,
        "a refused rename must not have written the row either"
    );
}

/// A title of exactly the cap is ACCEPTED, and survives the round trip.
///
/// The other side of the boundary, and the half a `>=`-instead-of-`>` slip
/// would break silently: the refusal test above passes either way. Taking
/// it all the way through a list reply also proves the accepted maximum is
/// genuinely deliverable — a cap that refused nothing but produced replies
/// the frame layer had to degrade would be worse than no cap at all.
#[farhelm_testtrace::test]
async fn a_title_of_exactly_the_cap_is_accepted_end_to_end() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let exact = "t".repeat(64 * 1024);
    let reply = renamed(rename(&h.sup, &session.id, &exact).await);
    assert_eq!(reply.title, exact);
    assert_eq!(
        listed_title(&h.client, &session.id).await,
        exact,
        "a title at the cap must survive the reply that echoes it AND the next list"
    );
}

/// An explicitly empty title is ACCEPTED.
///
/// Pins a decision, not an accident: create accepts an explicit empty
/// title, SPEC.md names control characters as the only CONTENT-based
/// refusal for a supplied title (the size cap is the other refusal, and it
/// is about size), and rename inventing a stricter rule would be an
/// asymmetry between two verbs that share one validation. A future "reject
/// blank titles" change is welcome to exist — but it has to change create,
/// this test, and SPEC.md together rather than drifting in on one side.
#[farhelm_testtrace::test]
async fn an_explicit_empty_title_is_accepted() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let reply = renamed(rename(&h.sup, &session.id, "").await);
    assert_eq!(reply.title, "");
    assert_eq!(
        listed_title(&h.client, &session.id).await,
        "",
        "an accepted empty title must be what the session now lists as"
    );
}

/// Two renames issued at once both succeed, and the winner is the same
/// everywhere.
///
/// Last-write-wins with no version token is the deliberate choice (this is
/// one mutable label, and optimistic concurrency would add a conflict
/// surface no user flow can hit on purpose), so BOTH callers get a success
/// reply carrying their OWN title — a success reply that reported somebody
/// else's would make the optimistic UI paint a name the user never typed.
///
/// What "wins" then has to mean is that the durable row and the in-memory
/// map hold the SAME title. A rename whose two halves could interleave
/// would leave the database saying one thing and every list reply saying
/// the other for the rest of the process's life — a torn write dressed up
/// as last-write-wins.
///
/// Stated honestly: `join!` does not FORCE the two renames to overlap
/// inside the critical section, so this cannot prove the protection by
/// construction — the serialization rests on the session's lifecycle
/// claim, which is verifiable by reading `Supervisor::rename_session` and
/// which nothing here can force a schedule against without a seam. What
/// this test does pin is the observable contract: both callers succeed,
/// neither is refused for conflicting with the other, and the store and
/// the list agree on one winner.
#[farhelm_testtrace::test]
async fn two_concurrent_renames_both_succeed_and_agree_on_one_winner() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    let (first, second) = tokio::join!(
        rename(&h.sup, &session.id, "racer-one"),
        rename(&h.sup, &session.id, "racer-two"),
    );
    assert_eq!(renamed(first).title, "racer-one");
    assert_eq!(renamed(second).title, "racer-two");

    let listed = listed_title(&h.client, &session.id).await;
    let stored = stored_title(h.state.path(), &session.id).await;
    assert!(
        listed == "racer-one" || listed == "racer-two",
        "the surviving title must be one of the two that were written, got {listed:?}"
    );
    assert_eq!(
        listed, stored,
        "the durable row and the in-memory entry disagree about which rename won"
    );
}

/// A rename changes nothing about a live attachment.
///
/// A title is a label; the terminal stream is a pane. Nothing in an
/// attachment depends on the session's name, so a rename must not detach,
/// interrupt, or reset it — the user renaming a session from the session
/// view is watching that terminal while they do it. The input round trip
/// afterwards is what proves the attachment is still whole rather than
/// merely un-detached.
#[farhelm_testtrace::test]
async fn a_rename_leaves_an_active_attachment_alone() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;

    renamed(rename(&h.sup, &session.id, "renamed-mid-attach").await);

    assert_eq!(
        drain_for(&mut rx, &mut seen, Duration::from_secs(1)).await,
        None,
        "a rename detached a live attachment"
    );
    h.client
        .send_input(chan, b"still-attached\r".to_vec())
        .await;
    wait_for(&mut rx, &mut seen, "still-attached", 20).await;
}

/// A rename does not disturb the intent key its create was made under.
///
/// The create-idempotency fingerprint records the CREATE request as it was
/// SENT — including the title that request carried. A rename that folded
/// the new title into that record would make the original create's own
/// retry look like a DIFFERENT request under a used key, which the
/// supervisor refuses as key reuse: the client would be told its create
/// conflicted with itself, for having renamed the session in between.
///
/// The replay must also come back describing the session as it is NOW,
/// renamed — a replay is the authoritative answer to "what did this
/// intent produce", not a recording of an old reply.
#[farhelm_testtrace::test]
async fn a_rename_does_not_disturb_the_create_intent_key() {
    let h = harness().await;
    let work = farhelm_teststate::tempdir().expect("workdir");
    let created = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("as-created".to_string()),
            80,
            24,
            Some("intent-across-a-rename".to_string()),
        )
        .await
        .expect("create with an intent key");

    renamed(rename(&h.sup, &created.id, "renamed-after-create").await);

    let replayed = h
        .client
        .create_session_with_key(
            &work.path().to_string_lossy(),
            &agent_cmd("internal fake-agent --script basic"),
            Some("as-created".to_string()),
            80,
            24,
            Some("intent-across-a-rename".to_string()),
        )
        .await
        .expect("the original create, retried verbatim, must still replay");
    assert_eq!(
        replayed.id, created.id,
        "the retry must replay the same session rather than launching a second one"
    );
    assert_eq!(
        replayed.title, "renamed-after-create",
        "the replay must describe the session as it is now, not as it was created"
    );
    assert_eq!(
        h.client.list_sessions().await.expect("list").sessions.len(),
        1,
        "a replayed create must not have started a second session"
    );
}

/// More concurrent renames than the supervisor has admission slots all
/// complete — the request must never hold one slot while waiting for
/// another.
///
/// This is a deadlock regression, and it is worth the concurrency it
/// costs. A rename has two phases with different cancellation rules (a
/// commit that must outlive its connection, a reply that must not), and
/// the obvious way to admit both — take a slot for the commit, then let
/// `spawn_admitted` take one for the reply — livelocks the whole
/// supervisor the moment `HANDLER_ADMISSION_PERMITS` renames are in
/// flight: every one of them holds a slot nothing can release while
/// waiting for a slot nobody will free. Every rename here would then hang
/// forever, and so would every later request on those connections.
///
/// Each rename gets its own connection, which is what makes the read
/// loops independent — the deadlock needs several loops parked in
/// acquisition at once, and one connection could only ever park one. The
/// count is comfortably past the permit count so the test does not depend
/// on knowing it exactly, and the timeout is generous because a healthy
/// run finishes these in well under a second: what is being detected is
/// "never", not "slow".
///
/// The REFUSED renames in the second half matter for the same structural
/// reason: an error path that acquired its own slot, or that took a
/// second one to send its refusal, would wedge here exactly as the
/// success path would. What no external test can observe is the other
/// half of the failure-path rule — that the slot is held until the
/// refusal has been handed to the writer queue rather than freed before
/// it — since the difference is task accumulation against a peer that
/// never reads. That one rests on the single-acquisition structure being
/// visible in `Supervisor::rename_session`, which returns the permit with
/// BOTH outcomes for this reason.
#[farhelm_testtrace::test]
async fn more_concurrent_renames_than_admission_slots_all_complete() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    // Refusals first, against a session that does not exist: the failure
    // path runs the same acquisition, and nothing may hold a slot waiting
    // for one.
    const REFUSALS: usize = 24;
    let mut refused = tokio::task::JoinSet::new();
    for n in 0..REFUSALS {
        let sup = Arc::clone(&h.sup);
        refused.spawn(async move {
            refusal(
                rename(
                    &sup,
                    "3d4f4b1e-0000-4000-8000-000000000000",
                    &format!("nope-{n}"),
                )
                .await,
            )
        });
    }
    let kinds = tokio::time::timeout(Duration::from_secs(60), async {
        let mut kinds = Vec::new();
        while let Some(joined) = refused.join_next().await {
            kinds.push(joined.expect("a refused-rename task panicked").0);
        }
        kinds
    })
    .await
    .expect("refused renames past the admission bound must all complete, not deadlock");
    assert_eq!(kinds.len(), REFUSALS);
    assert!(kinds.iter().all(|kind| *kind == ErrorKind::NotFound));

    // Past `HANDLER_ADMISSION_PERMITS` (8) with room to spare.
    const RENAMES: usize = 24;
    let mut renames = tokio::task::JoinSet::new();
    for n in 0..RENAMES {
        let sup = Arc::clone(&h.sup);
        let id = session.id.clone();
        renames.spawn(async move { renamed(rename(&sup, &id, &format!("racer-{n}")).await) });
    }
    let finished = tokio::time::timeout(Duration::from_secs(60), async {
        let mut titles = Vec::new();
        while let Some(joined) = renames.join_next().await {
            titles.push(joined.expect("a rename task panicked").title);
        }
        titles
    })
    .await
    .expect("renames past the admission bound must all complete, not deadlock");
    assert_eq!(finished.len(), RENAMES);

    // And the session is left in one of the states somebody asked for,
    // rather than half-written by whichever writers were interrupted.
    let listed = listed_title(&h.client, &session.id).await;
    assert!(
        finished.contains(&listed),
        "the surviving title must be one of the ones written, got {listed:?}"
    );
    assert_eq!(listed, stored_title(h.state.path(), &session.id).await);
}

/// A rename whose client vanishes before the reply still lands.
///
/// The two-part write runs on a task the SUPERVISOR owns, not on the
/// connection's, precisely for this: a client that disconnects between the
/// committed row and the map install would otherwise leave the durable
/// title changed while every list reply from this still-running process
/// served the old one, until a restart. Nothing about the request is
/// abandoned by the caller going away — only the reply is.
///
/// The connection is dropped immediately after the frame goes out, with
/// nothing read back, which is as close to "the reply was lost" as a test
/// can get without a seam.
///
/// Two things it deliberately does not prove. It cannot distinguish a
/// commit owned by the SUPERVISOR from one owned by the connection: this
/// disconnect is clean, so the connection's shutdown tail drains its
/// tasks rather than aborting them, and a connection-owned commit would
/// land here too. Forcing the abort needs the cancellation seam that is
/// parked. And it says nothing about the admission slot the task holds —
/// that it is not handed back early is structural (the permit is moved
/// into the task) and would need instrumentation to observe. What this
/// pins is the OUTCOME: a rename nobody is left to reply to still lands,
/// both halves of it.
#[farhelm_testtrace::test]
async fn a_rename_whose_client_vanishes_still_lands() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;

    {
        let (client_side, server_side) = tokio::io::duplex(1 << 20);
        let sup = Arc::clone(&h.sup);
        tokio::spawn(async move {
            let _ = handle_connection(sup, server_side).await;
        });
        let (read_half, write_half) = tokio::io::split(client_side);
        let mut reader = FrameReader::new(read_half);
        let mut writer = FrameWriter::new(write_half);
        handshake(&mut reader, &mut writer, "helm")
            .await
            .expect("handshake");
        writer
            .write_control(&ControlMsg::RenameSession {
                req_id: 1,
                session_id: session.id.clone(),
                title: "renamed-by-a-client-that-left".to_string(),
            })
            .await
            .expect("write rename");
        // Both halves go here, taking the whole connection with them
        // without a single reply frame ever being read.
    }

    wait_for_listing(
        &h.client,
        30,
        "the rename landed after its client disconnected",
        |sessions| {
            sessions
                .iter()
                .any(|row| row.id == session.id && row.title == "renamed-by-a-client-that-left")
        },
    )
    .await;
    assert_eq!(
        stored_title(h.state.path(), &session.id).await,
        "renamed-by-a-client-that-left",
        "both halves of the write must have landed, not just the in-memory one"
    );
}

/// When the rename lands but its reply cannot be built, the caller is told
/// exactly that — and the rename stays.
///
/// The two-part write commits before the reply is assembled, so a failure
/// while reading the session's current state arrives AFTER the title has
/// changed. Fabricating the dynamic fields to paper over it was rejected:
/// the reply promises a live-probed `SessionInfo`, and inventing one is a
/// worse answer than an honest error. What must not happen is the error
/// implying the rename did not happen, since the caller's next poll will
/// show the new title regardless.
///
/// The failure is provoked with an EMPTY launch sentinel, which the reader
/// treats as corrupt state rather than as absence — the supervisor refuses
/// to base a reply on an inference an unreadable sentinel might contradict
/// (PLAN_M3.md item 3), and that refusal is what propagates here. It needs
/// a dead pane, since that is the only state whose sentinel is consulted.
#[farhelm_testtrace::test]
async fn a_rename_whose_reply_cannot_be_built_reports_that_it_landed_anyway() {
    let h = harness().await;
    let (session, _work) = basic_session(&h).await;
    let (chan, initial_replay, mut rx) = h
        .client
        .attach_live(&session.id, 80, 24)
        .await
        .expect("attach");
    let mut seen = initial_replay;
    wait_for(&mut rx, &mut seen, "FAKE-AGENT READY", 20).await;
    h.client.send_input(chan, b"quit\r".to_vec()).await;
    wait_for_non_live_status(&h.client, &session.id, 30).await;

    let status_path = status_path_for_spec(&spec_path_for_launch(h.state.path(), &session.id, 0));
    std::fs::write(&status_path, "").expect("plant an empty (corrupt) sentinel");

    let (kind, message) = refusal(rename(&h.sup, &session.id, "renamed-but-unreadable").await);
    assert_eq!(kind, ErrorKind::Internal);
    assert!(
        message.contains("was renamed, but reading back its current state failed"),
        "the error must say the rename itself landed: {message}"
    );
    assert_eq!(
        stored_title(h.state.path(), &session.id).await,
        "renamed-but-unreadable",
        "the durable rename must stand even though the reply could not be built"
    );

    // With the corrupt sentinel gone, the ordinary reply path recovers and
    // the new title is simply there — the poll interval this costs is the
    // whole price of refusing to fabricate a reply.
    std::fs::remove_file(&status_path).expect("remove the planted sentinel");
    assert_eq!(
        listed_title(&h.client, &session.id).await,
        "renamed-but-unreadable"
    );
}

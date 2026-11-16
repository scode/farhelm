//! Tests of the status sounds' owner in the real main view over the helm client's fake: which sound it hands the
//! player, and when it hands none.
//!
//! They port the old spec's acceptance cases (`e2e/tests/sounds.spec.ts` at reference commit `24bc0580`), which watched
//! Web Audio's oscillators start; here a fake player records each sound it is asked to play, and checks it was handed
//! that sound's one rendered file. The rules' own cases (each status edge, priority, consumed events) are
//! `sounds::rules`'s unit tests; these are about what the owner feeds them: the store's sessions as the sidebar's query
//! selects them, every approval, the shared switches, the quiet session and the view changes. The window's activity
//! is pinned through the ticker's override, never read from the test platform, so whether the open session is quiet
//! is the test's choice. Every step settles through the shared harness, which checks the focus and layer invariants.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, PendingApproval};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionState, KeyedEntry, Preferences,
    SESSION_QUERY_PATH,
};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use crate::main_view::MainView;
use crate::session_list::tests::{
    Fixture, change, listing, listing_meta, preferences_state, selected, session,
};
use crate::sounds::{Player, Sound, install_player, synth};
use crate::test_support::{self, click, meta, settle, started};
use crate::ticker;

// ---- Fixtures --------------------------------------------------------------------------------------------------------

/// What the player heard when nothing played.
const NOTHING: [Sound; 0] = [];

/// A player that records what it was asked to play, as the old spec's recording audio context did.
#[derive(Default)]
struct Recorder {
    played: RefCell<Vec<Sound>>,
}

impl Player for Recorder {
    fn play(&self, sound: Sound, wav: &'static [u8]) {
        assert!(
            std::ptr::eq(wav, synth::wav(sound)),
            "{sound:?} is handed its one rendered file"
        );
        self.played.borrow_mut().push(sound);
    }
}

impl Recorder {
    /// What was played since the last look.
    fn heard(&self) -> Vec<Sound> {
        self.played.take()
    }
}

/// A session in the status the helm's word `state` names.
fn in_state(key: &'static str, state: &str) -> Fixture {
    session(key).status(json!({ "state": state }))
}

/// A pending stop request `id`, asked by the session `asker`.
fn ask(id: &str, asker: &str) -> PendingApproval {
    PendingApproval {
        id: id.to_owned(),
        host_id: 1,
        host_name: "fixture host".to_owned(),
        session: ApprovalSession {
            id: asker.to_owned(),
            title: None,
            host_name: None,
        },
        action: ApprovalAction::Stop {
            target: ApprovalSession {
                id: "target".to_owned(),
                title: None,
                host_name: None,
            },
        },
        own_host_session_action: false,
        expires_at_ms: i64::MAX,
    }
}

/// The approvals collection holding `approvals`, oldest first.
fn approvals(approvals: &[PendingApproval]) -> CollectionState {
    let ids: Vec<&str> = approvals
        .iter()
        .map(|approval| approval.id.as_str())
        .collect();
    CollectionState {
        collection: Collection::Approvals,
        meta: meta(&ids),
        entries: approvals
            .iter()
            .map(|approval| KeyedEntry {
                key: approval.id.clone(),
                value: serde_json::to_value(approval).unwrap(),
            })
            .collect(),
    }
}

/// The helm's pending approvals become `pending`, oldest first.
fn push_approvals(helm: &mut FakeHelm, pending: &[PendingApproval], cx: &mut VisualTestContext) {
    let state = approvals(pending);
    helm.push(CollectionChanges {
        collection: Collection::Approvals,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
}

/// The listing becomes `rows` in that order, with `gone` taken away: one change of the sessions collection, as the
/// helm pushes it.
fn push_listing(helm: &mut FakeHelm, rows: &[Fixture], gone: &[&str], cx: &mut VisualTestContext) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    helm.push(change(
        listing_meta(&order, rows.len() as u64, false),
        rows,
        gone,
    ));
    settle(cx);
}

/// A preference row that has shown the Welcome card (so no card covers the window) and stores these sound switches.
fn row(waiting: Option<bool>, approval: Option<bool>, turn_finished: Option<bool>) -> Preferences {
    Preferences {
        welcome_seen: Some(true),
        sound_waiting: waiting,
        sound_approval: approval,
        sound_turn_finished: turn_finished,
        ..Preferences::default()
    }
}

/// The main view over the fake helm, connected with the sessions `rows` (in that order), the pending approvals
/// `pending` and the preference row `preferences`, with a recording player installed and the window counted as
/// `active` ([`open_with`]).
fn open<'a>(
    cx: &'a mut TestAppContext,
    rows: &[Fixture],
    pending: &[PendingApproval],
    preferences: Preferences,
    active: bool,
) -> (FakeHelm, &'a mut VisualTestContext, Rc<Recorder>) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    open_with(cx, listing(&order, rows), pending, preferences, active)
}

/// [`open`] with the sessions collection given whole, for a snapshot whose sessions read failed. The snapshot arrives
/// before the window opens, as the embedded helm's usually does, so the owner's first observation is that snapshot.
fn open_with<'a>(
    cx: &'a mut TestAppContext,
    sessions: CollectionState,
    pending: &[PendingApproval],
    preferences: Preferences,
    active: bool,
) -> (FakeHelm, &'a mut VisualTestContext, Rc<Recorder>) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![
        sessions,
        approvals(pending),
        preferences_state(preferences),
    ]);
    cx.run_until_parked();
    let recorder = Rc::new(Recorder::default());
    let player = recorder.clone();
    cx.update(|cx| {
        install_player(player, cx);
        ticker::set_active_override(Some(active), cx);
    });
    let (_, window) = test_support::open_view(cx, MainView::new);
    settle(window);
    (helm, window, recorder)
}

/// Answer every command the window sent since the last look, in order, as the fake requires: the session queries among
/// them as the helm does once it serves a query's listing, and the rest (the remembered selection, the seen mark) with
/// the same empty success, which nothing here reads.
fn answer_queries(helm: &mut FakeHelm, cx: &mut VisualTestContext) {
    let commands = helm.commands();
    assert!(
        commands
            .iter()
            .any(|command| command.path == SESSION_QUERY_PATH),
        "the premise: a session query is out"
    );
    for command in commands {
        helm.reply(command.id, 204, None);
    }
    settle(cx);
}

// ---- The start and the reset -----------------------------------------------------------------------------------------

/// SPEC.md: "the state loaded when the window starts or resets makes no sound". A window that rang for what was already
/// waiting when it opened would ring at every start and every reconnect. Ported from the old spec's "first read only
/// records existing attention". Specified: a start snapshot holding a waiting session and a pending approval plays
/// nothing; a new approval then plays the approval sound and a session that starts waiting the waiting sound; after a
/// reset, a snapshot holding a newly waiting session and a new approval plays nothing, and a later edge plays again.
#[gpui::test]
fn the_start_and_a_reset_record_what_is_there_silently(cx: &mut TestAppContext) {
    let rows = [in_state("a", "waiting"), in_state("b", "running")];
    let (mut helm, cx, recorder) =
        open(cx, &rows, &[ask("old", "a")], row(None, None, None), false);
    assert_eq!(recorder.heard(), NOTHING, "the start snapshot");

    push_approvals(&mut helm, &[ask("old", "a"), ask("new", "a")], cx);
    assert_eq!(recorder.heard(), [Sound::Approval]);
    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "waiting")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), [Sound::Waiting]);

    helm.break_channel();
    settle(cx);
    helm.connect(vec![
        listing(
            &["a", "b", "c"],
            &[
                in_state("a", "waiting"),
                in_state("b", "waiting"),
                in_state("c", "waiting"),
            ],
        ),
        approvals(&[ask("old", "a"), ask("fresh", "c")]),
        preferences_state(row(None, None, None)),
    ]);
    settle(cx);
    assert_eq!(recorder.heard(), NOTHING, "the reset's snapshot");
    push_listing(
        &mut helm,
        &[
            in_state("a", "running"),
            in_state("b", "waiting"),
            in_state("c", "waiting"),
        ],
        &[],
        cx,
    );
    push_listing(
        &mut helm,
        &[
            in_state("a", "waiting"),
            in_state("b", "waiting"),
            in_state("c", "waiting"),
        ],
        &[],
        cx,
    );
    assert_eq!(
        recorder.heard(),
        [Sound::Waiting],
        "a later edge rings again"
    );
}

// ---- The quiet session -----------------------------------------------------------------------------------------------

/// SPEC.md: "The open session stays quiet while its window is active". The user is looking at it, so a sound would only
/// interrupt; once the window is not active the same edges must ring again. Ported from the old spec's "active open
/// session is quiet for waiting and its approval". Specified, with `a` open: in an active window, `a` starting to wait
/// and its own approval request play nothing, while `b` starting to wait plays the waiting sound; in an inactive window,
/// `a` running and waiting again plays the waiting sound.
#[gpui::test]
fn the_open_session_is_quiet_while_its_window_is_active(cx: &mut TestAppContext) {
    let preferences = Preferences {
        last_selected: Some("a".to_owned()),
        ..row(None, None, None)
    };
    let rows = [in_state("a", "running"), in_state("b", "running")];
    let (mut helm, cx, recorder) = open(cx, &rows, &[], preferences, true);
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");

    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "running")],
        &[],
        cx,
    );
    push_approvals(&mut helm, &[ask("mine", "a")], cx);
    assert_eq!(
        recorder.heard(),
        NOTHING,
        "the open session in an active window"
    );
    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "waiting")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), [Sound::Waiting], "another session rings");

    cx.update(|_, cx| ticker::set_active_override(Some(false), cx));
    push_listing(
        &mut helm,
        &[in_state("a", "running"), in_state("b", "waiting")],
        &[],
        cx,
    );
    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "waiting")],
        &[],
        cx,
    );
    assert_eq!(
        recorder.heard(),
        [Sound::Waiting],
        "an inactive window rings for it"
    );
}

// ---- The sidebar's view ----------------------------------------------------------------------------------------------

/// SPEC.md: "Status sounds follow the sidebar's current host selector and filters; changing that view silently records
/// its existing statuses. New approval requests sound regardless of those filters, because their cards are always
/// shown." A sessions listing pushed for a new filter shows sessions this client never saw change, and it arrives before
/// the query's reply, so ringing for it would ring on every filter click. Ported from the old spec's "status sounds
/// follow the current view", "new view keeps later status transitions" and "status segments seed sounds silently and
/// do not filter approval bells". Specified, starting under All with `a` running and `b` idle: Waiting's listing,
/// holding `c` waiting, plays nothing while its query is unanswered, but a new request from `a`, which Waiting hides,
/// plays the approval sound; the query's answer plays nothing; `d` starting to wait in that view then plays the waiting
/// sound; All's listing again, where `a` began waiting while hidden, plays nothing, and `a` running and waiting again
/// then plays the waiting sound.
#[gpui::test]
fn a_view_change_records_the_new_views_statuses_silently(cx: &mut TestAppContext) {
    let rows = [in_state("a", "running"), in_state("b", "idle")];
    let (mut helm, cx, recorder) = open(cx, &rows, &[], row(None, None, None), false);
    answer_queries(&mut helm, cx);

    click(cx, "session-status-Waiting");
    push_listing(&mut helm, &[in_state("c", "waiting")], &["a", "b"], cx);
    assert_eq!(
        recorder.heard(),
        NOTHING,
        "the new view's listing, its query unanswered"
    );
    push_approvals(&mut helm, &[ask("hidden", "a")], cx);
    assert_eq!(
        recorder.heard(),
        [Sound::Approval],
        "a request from a session the view hides"
    );
    answer_queries(&mut helm, cx);
    assert_eq!(recorder.heard(), NOTHING, "the query's answer");
    push_listing(
        &mut helm,
        &[in_state("c", "waiting"), in_state("d", "waiting")],
        &[],
        cx,
    );
    assert_eq!(
        recorder.heard(),
        [Sound::Waiting],
        "an edge inside the new view"
    );

    click(cx, "session-status-All");
    let all = [
        in_state("a", "waiting"),
        in_state("b", "idle"),
        in_state("c", "waiting"),
        in_state("d", "waiting"),
    ];
    push_listing(&mut helm, &all, &[], cx);
    answer_queries(&mut helm, cx);
    assert_eq!(recorder.heard(), NOTHING, "back under All");
    let mut running = all.clone();
    running[0] = in_state("a", "running");
    push_listing(&mut helm, &running, &[], cx);
    push_listing(&mut helm, &all, &[], cx);
    assert_eq!(recorder.heard(), [Sound::Waiting]);
}

/// A snapshot whose sessions read failed holds no rows, and the helm's next successful read lists every session
/// at once; taking the empty snapshot as the baseline would ring for every session already waiting the moment the read
/// recovers, a sound about nothing that happened. The old app kept its last statuses through a failed listing read.
/// Specified: a start snapshot with the sessions collection unavailable plays nothing; the recovered listing, holding
/// `a` waiting, plays nothing; `b` starting to wait after that plays the waiting sound.
#[gpui::test]
fn the_listing_that_recovers_from_a_failed_read_is_recorded_silently(cx: &mut TestAppContext) {
    let mut unavailable = listing(&[], &[]);
    unavailable.meta.availability = Availability::Unavailable {
        error: "fixture: the sessions read failed".to_owned(),
    };
    let (mut helm, cx, recorder) = open_with(cx, unavailable, &[], row(None, None, None), false);
    assert_eq!(recorder.heard(), NOTHING, "the start snapshot");

    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "running")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), NOTHING, "the recovered listing");
    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "waiting")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), [Sound::Waiting]);
}

// ---- The switches ----------------------------------------------------------------------------------------------------

/// The switches are the helm's shared preference row, so a change made in Settings arrives as a push, and the owner
/// must play by the row as it is at each observation rather than as it was when the window opened. The choice among
/// simultaneous events is the rules' own (`sounds::rules::tests`). Specified, with waiting stored off: `a` starting to
/// wait plays nothing; once the helm pushes waiting on, `b` starting to wait plays the waiting sound.
#[gpui::test]
fn the_switches_follow_the_pushed_preference_row(cx: &mut TestAppContext) {
    let start = [in_state("a", "running"), in_state("b", "running")];
    let (mut helm, cx, recorder) = open(cx, &start, &[], row(Some(false), None, None), false);

    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "running")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), NOTHING, "waiting is off");

    let state = preferences_state(row(Some(true), None, None));
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: Vec::new(),
    });
    settle(cx);
    push_listing(
        &mut helm,
        &[in_state("a", "waiting"), in_state("b", "waiting")],
        &[],
        cx,
    );
    assert_eq!(recorder.heard(), [Sound::Waiting], "waiting is on");
}

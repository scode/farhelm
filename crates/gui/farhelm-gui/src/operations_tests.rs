//! Tests of the operation store and of saving drafts through it, against the helm client's fake.
//!
//! The fake answers commands in the order they were sent, as the helm does, so "overlapping" here means both commands
//! are in flight at once and each settles on its own reply; which reply comes first is the order the helm admitted
//! them. The fake-helm helpers come from the shared test harness (`test_support.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, SessionRow};
use gpui::{
    AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, px,
};
use gpui_component::Root;
use serde_json::json;

use crate::components::{HoverHelp, TextInput, TextInputState};
use crate::drafts::{self, Draft, DraftInput};
use crate::host_text::HostText;
use crate::main_view::MainView;
use crate::operations::{
    self, Command, Failure, NotStarted, OpKey, Operations, Outcome, Settled, Work,
};
use crate::session::{self, Connection};
use crate::test_support::{self, keys, meta, session_entry, sessions, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Every completion delivered, labeled by the test's name for the operation.
type Delivered = Rc<RefCell<Vec<(&'static str, Settled)>>>;

fn operations(cx: &mut TestAppContext) -> Entity<Operations> {
    match cx.update(|cx| session::current_operations(cx)) {
        Some(operations) => operations,
        None => panic!("the premise: a UI session is live"),
    }
}

/// Start `command` on session `session`, recording its completion under `label`.
fn start(
    operations: &Entity<Operations>,
    (session, command): (&str, Command),
    label: &'static str,
    delivered: &Delivered,
    cx: &mut TestAppContext,
) -> Result<(), NotStarted> {
    start_on(
        operations,
        &OpKey::session(session),
        command,
        label,
        delivered,
        cx,
    )
}

/// Start `command` as work on `key`, recording its completion under `label`.
fn start_on(
    operations: &Entity<Operations>,
    key: &OpKey,
    command: Command,
    label: &'static str,
    delivered: &Delivered,
    cx: &mut TestAppContext,
) -> Result<(), NotStarted> {
    let delivered = delivered.clone();
    cx.update(|cx| {
        operations.update(cx, |operations, cx| {
            operations.start(key, command, cx, move |settled, _| {
                delivered.borrow_mut().push((label, settled.clone()));
            })
        })
    })
}

/// A Rename of `session` to `title`, built here with [`Command::new`] rather than by the session list's own route code
/// (`session_list::rename::command`), so the store's tests stay the store's: a change to how the session list builds
/// its commands cannot break them, and they do not test it.
fn rename(session: &str, title: &str) -> Command {
    let path = ApiPath::api(&["sessions", session, "rename"]).unwrap();
    let body = Some(json!({ "title": title }));
    Command::new(
        Work::Rename,
        Method::Post,
        path,
        body,
        "POST /api/sessions/[session]/rename",
    )
}

/// A Delete of `session` with `guard`, built here as [`rename`] is. The guard's query flag is the session list's
/// route knowledge (its own tests check it); here the guard only has to travel as the command's work.
fn delete(session: &str, guard: DeleteGuard) -> Command {
    let path = ApiPath::api(&["sessions", session]).unwrap();
    Command::new(
        Work::Delete(guard),
        Method::Delete,
        path,
        None,
        "DELETE /api/sessions/[session]",
    )
}

/// A connected app whose sessions are `keys`.
fn connected(keys: &[&str], cx: &mut TestAppContext) -> (FakeHelm, Entity<Connection>) {
    let (mut fake, connection) = started(cx);
    fake.connect(vec![sessions(keys)]);
    cx.run_until_parked();
    (fake, connection)
}

// ---- The operation store -------------------------------------------------------------------------------------------

/// SPEC_impl.md, "Work that finishes later", names this test: independent actions on different sessions each keep
/// their completion handling until they settle. Specified: a Rename on `a` and a Delete on `b` are in flight together;
/// a second action on `a` is refused while its Rename runs; `a`'s refusal settles only `a` and is recorded on `a`'s
/// row while `b` stays in flight; then `b` settles with its own outcome. The row's failure stays until the next
/// operation on `a` starts, which clears it (the old app's rule: a row's error lasts until the retry that supersedes
/// it), and a refusal with no words names the command and its status with the session's id hidden.
#[gpui::test]
fn independent_work_on_two_sessions_keeps_its_own_completion(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["a", "b"], cx);
    let operations = operations(cx);
    let delivered = Delivered::default();
    let renaming = ("a", rename("a", "new"));
    start(&operations, renaming, "rename a", &delivered, cx).unwrap();
    let deleting = ("b", delete("b", DeleteGuard::NothingAlive));
    start(&operations, deleting, "delete b", &delivered, cx).unwrap();
    let again = ("a", delete("a", DeleteGuard::Unconditional));
    assert_eq!(
        start(&operations, again, "delete a", &delivered, cx),
        Err(NotStarted::Busy(Work::Rename)),
        "one operation per session"
    );
    cx.run_until_parked();

    let sent = fake.commands();
    let shown: Vec<_> = sent
        .iter()
        .map(|sent| (sent.method, sent.path.as_str()))
        .collect();
    assert_eq!(
        shown,
        vec![
            (Method::Post, "/api/sessions/a/rename"),
            (Method::Delete, "/api/sessions/b"),
        ],
        "both commands reached the helm, and the refused one did not"
    );
    assert_eq!(sent[0].body, Some(json!({ "title": "new" })));

    fake.reply(sent[0].id, 400, Some(json!("titles may not be empty ")));
    cx.run_until_parked();
    let refused = Outcome::Refused(HostText::from_host("titles may not be empty"));
    let a_settled = Settled {
        outcome: refused.clone(),
        user_moved_on: false,
    };
    assert_eq!(*delivered.borrow(), vec![("rename a", a_settled.clone())]);
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(
            operations.in_flight(&OpKey::session("a")),
            None,
            "a settled"
        );
        assert_eq!(
            operations.in_flight(&OpKey::session("b")),
            Some(Work::Delete(DeleteGuard::NothingAlive)),
            "b is still in flight, as the Delete the command sent, guard included"
        );
        let failure = operations
            .failure(&OpKey::session("a"))
            .map(|failure| (failure.work, &failure.message));
        assert_eq!(
            failure,
            Some((
                Work::Rename,
                &HostText::from_host("titles may not be empty")
            ))
        );
        assert_eq!(
            operations.failure(&OpKey::session("b")),
            None,
            "a's refusal is a's alone"
        );
    });

    // A bare successful Delete answers `{}`: `archived` and the notice are left out when they say nothing.
    fake.reply(sent[1].id, 200, Some(json!({})));
    cx.run_until_parked();
    let b_settled = Settled {
        outcome: Outcome::Done(Some(json!({}))),
        user_moved_on: false,
    };
    assert_eq!(
        *delivered.borrow(),
        vec![("rename a", a_settled), ("delete b", b_settled)]
    );
    assert_eq!(
        cx.update(|cx| operations.read(cx).in_flight(&OpKey::session("b"))),
        None
    );
    assert!(
        cx.update(|cx| operations.read(cx).failure(&OpKey::session("a")).is_some()),
        "the premise: a's failure is still on its row"
    );

    let retry = ("a", rename("a", "newer"));
    start(&operations, retry, "rename a again", &delivered, cx).unwrap();
    assert_eq!(
        cx.update(|cx| operations.read(cx).failure(&OpKey::session("a")).cloned()),
        None,
        "starting again cleared the earlier failure"
    );
    cx.run_until_parked();
    let sent = fake.commands();
    fake.reply(sent[0].id, 409, None);
    cx.run_until_parked();
    let message = cx.update(|cx| {
        operations
            .read(cx)
            .failure(&OpKey::session("a"))
            .map(|failure| failure.message.clone())
    });
    assert_eq!(
        message,
        Some(HostText::from_host(
            "POST /api/sessions/[session]/rename: 409"
        )),
        "a refusal with no words names the command, not the session's id"
    );
}

/// Why this matters: while a Delete runs, its row says what is happening ("stopping the agent", "stopping its terminal
/// tabs" or "deleting", SPEC.md, "Lifecycle operations"), and the only honest source for that is the precondition the
/// request actually carries. Were the in-flight work recorded apart from the command, a caller could start an
/// unconditional Delete while the row claimed there was nothing alive to stop.
///
/// Specification: for each guard, a started Delete is in flight as `Work::Delete` with the guard its command carries.
/// (Which query flag each guard puts on the route is the session list's, and its Delete tests check that.)
#[gpui::test]
fn a_delete_in_flight_reports_the_guard_its_command_sent(cx: &mut TestAppContext) {
    let guards = [
        ("a", DeleteGuard::Unconditional),
        ("b", DeleteGuard::AgentEnded),
        ("c", DeleteGuard::NothingAlive),
    ];
    let (mut fake, _connection) = connected(&["a", "b", "c"], cx);
    let operations = operations(cx);
    let delivered = Delivered::default();
    for (key, guard) in guards {
        let deleting = (key, delete(key, guard));
        start(&operations, deleting, "delete", &delivered, cx).unwrap();
    }
    cx.run_until_parked();
    assert_eq!(
        fake.commands().len(),
        3,
        "the premise: every Delete was sent"
    );
    for (key, guard) in guards {
        assert_eq!(
            cx.update(|cx| operations.read(cx).in_flight(&OpKey::session(key))),
            Some(Work::Delete(guard)),
            "{key} is in flight with its own guard"
        );
    }
}

/// The design's core claim: the store, not the view that started the work, owns its completion. The old GUI once lost
/// the outcome of a Delete, Stop or rename because the component whose scope held its task was unmounted (`auth.rs`'s
/// `GateState` at reference commit `24bc0580`). Specified: a Rename saved from an editor whose view is gone before the
/// reply still settles in the store: the session is no longer busy and its refusal is on its row.
#[gpui::test]
fn work_started_from_a_view_settles_after_the_view_is_gone(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![titled("old")]);
    cx.run_until_parked();
    let editor = open_editor("a", cx);
    cx.update(|cx| {
        editor.update(cx, |editor, cx| {
            assert!(editor.title.edit("new".to_owned()));
            editor.save(cx)
        })
    })
    .unwrap();
    let gone = editor.downgrade();
    drop(editor);
    cx.run_until_parked();
    assert!(gone.upgrade().is_none(), "the premise: the view is gone");
    let operations = operations(cx);
    assert_eq!(
        cx.update(|cx| operations.read(cx).in_flight(&OpKey::session("a"))),
        Some(Work::Rename),
        "the work outlived its view"
    );

    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the save reached the helm");
    fake.reply(sent[0].id, 400, Some(json!("refused")));
    cx.run_until_parked();
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(
            operations.in_flight(&OpKey::session("a")),
            None,
            "the store settled it"
        );
        let failure = operations
            .failure(&OpKey::session("a"))
            .map(|failure| (failure.work, failure.message.clone()));
        assert_eq!(
            failure,
            Some((Work::Rename, HostText::from_host("refused")))
        );
    });
}

/// A Stop whose host could not close a tab, or a Replace whose removal of its source reported something, succeeded but
/// left words the row must show, in the place and for as long as a refusal's words. Specified: a note on a key shows as
/// its failure with the work and words given; a second note replaces it; starting work on the key clears it; and a
/// note on a retired store records nothing.
#[gpui::test]
fn a_partial_result_shows_where_a_refusal_does_until_the_next_start(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["a"], cx);
    let first = operations(cx);
    let key = OpKey::session("a");
    // Stop and Replace, the notes' first users, have no work kind yet; any kind shows the same way.
    let note = |operations: &Entity<Operations>, words: &str, cx: &mut TestAppContext| {
        let message = HostText::from_host(words);
        cx.update(|cx| {
            operations.update(cx, |operations, cx| {
                operations.note(&key, Work::Save, message, cx)
            })
        });
    };
    note(&first, "the tab stayed open", cx);
    note(&first, "the host kept one tab", cx);
    assert_eq!(
        cx.update(|cx| first.read(cx).failure(&key).cloned()),
        Some(Failure {
            work: Work::Save,
            message: HostText::from_host("the host kept one tab"),
        }),
        "the later note is what the row shows"
    );
    let delivered = Delivered::default();
    start(
        &first,
        ("a", rename("a", "new")),
        "rename a",
        &delivered,
        cx,
    )
    .unwrap();
    assert_eq!(
        cx.update(|cx| first.read(cx).failure(&key).cloned()),
        None,
        "starting work on the key cleared the note"
    );

    fake.break_channel();
    fake.connect(vec![sessions(&["a"])]);
    cx.run_until_parked();
    assert_eq!(
        start(&first, ("a", rename("a", "late")), "late", &delivered, cx),
        Err(NotStarted::Retired),
        "the premise: the reconnect retired the first store"
    );
    note(&first, "after the reset", cx);
    assert_eq!(
        cx.update(|cx| first.read(cx).failure(&key).cloned()),
        None,
        "the retired store recorded nothing"
    );
}

/// A reset drops the UI's waiting, never the helm's work: SPEC_impl.md says dropping a waiter never cancels work the
/// helm accepted, and that a completion queued across a reset must not reach the new UI session. Specified: with a
/// Rename and a Delete in flight, a reply already on the feed when the channel breaks reaches no completion, the
/// rebuilt session starts with nothing in flight, the GUI sends the helm nothing more (no cancel), the failures the
/// retired store recorded are gone with it, and the retired store starts nothing.
#[gpui::test]
fn a_reset_drops_pending_completions_without_cancelling_the_helms_work(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["a", "b", "c"], cx);
    let first = operations(cx);
    let delivered = Delivered::default();
    start(&first, ("c", rename("c", "x")), "rename c", &delivered, cx).unwrap();
    cx.run_until_parked();
    let sent = fake.commands();
    fake.reply(sent[0].id, 400, Some(json!("refused")));
    cx.run_until_parked();
    assert!(
        cx.update(|cx| first.read(cx).failure(&OpKey::session("c")).is_some()),
        "the premise: c's refusal is on its row"
    );
    delivered.borrow_mut().clear();

    start(
        &first,
        ("a", rename("a", "new")),
        "rename a",
        &delivered,
        cx,
    )
    .unwrap();
    let deleting = ("b", delete("b", DeleteGuard::Unconditional));
    start(&first, deleting, "delete b", &delivered, cx).unwrap();
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(sent.len(), 2, "the premise: both commands reached the helm");

    fake.reply(sent[0].id, 200, Some(json!({})));
    fake.break_channel();
    fake.connect(vec![sessions(&["a", "b", "c"])]);
    cx.run_until_parked();

    assert!(
        delivered.borrow().is_empty(),
        "no completion reached anyone"
    );
    assert!(
        fake.commands().is_empty(),
        "nothing was sent to undo the helm's work"
    );
    let second = operations(cx);
    assert_ne!(
        first.entity_id(),
        second.entity_id(),
        "the reconnect built a new store"
    );
    cx.update(|cx| {
        assert_eq!(
            first.read(cx).in_flight(&OpKey::session("b")),
            None,
            "the retired store holds nothing"
        );
        assert_eq!(
            first.read(cx).failure(&OpKey::session("c")),
            None,
            "its failures went with it"
        );
        assert_eq!(second.read(cx).in_flight(&OpKey::session("a")), None);
        assert_eq!(second.read(cx).in_flight(&OpKey::session("b")), None);
        assert_eq!(second.read(cx).failure(&OpKey::session("c")), None);
    });
    let late = ("a", rename("a", "late"));
    assert_eq!(
        start(&first, late, "late", &delivered, cx),
        Err(NotStarted::Retired)
    );
}

/// A completion that would move focus, open a dialog or switch sessions must first ask whether the user moved on
/// (SPEC.md, "Who may move focus"), and the answer has to come from the moment the work started, not when it settled.
/// Specified: work during which the user pressed a key settles with `user_moved_on`; work during which they did
/// nothing settles without it.
#[gpui::test]
fn a_completion_says_whether_the_user_moved_on_since_the_work_started(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["a", "b"], cx);
    let (_, window) = cx.add_window_view(|window, cx| {
        let main = cx.new(|cx| MainView::new(window, cx));
        Root::new(main, window, cx)
    });
    // Simulated windows start inactive, and an inactive window has no focus path for keys to follow.
    window.update(|window, _| window.activate_window());
    window.run_until_parked();
    // The main view's session list opens a session and tells the helm. Answer that first: the fake answers commands
    // in the order they were sent, and the replies below are for the renames.
    for command in fake.commands() {
        fake.reply(command.id, 204, None);
    }
    let operations = operations(window);
    let delivered = Delivered::default();

    start(
        &operations,
        ("a", rename("a", "x")),
        "typed",
        &delivered,
        window,
    )
    .unwrap();
    window.simulate_keystrokes("x");
    window.run_until_parked();
    let sent = fake.commands();
    fake.reply(sent[0].id, 200, None);
    window.run_until_parked();

    start(
        &operations,
        ("b", rename("b", "y")),
        "idle",
        &delivered,
        window,
    )
    .unwrap();
    window.run_until_parked();
    let sent = fake.commands();
    fake.reply(sent[0].id, 200, None);
    window.run_until_parked();

    let moved: Vec<_> = delivered
        .borrow()
        .iter()
        .map(|(label, settled)| (*label, settled.user_moved_on))
        .collect();
    assert_eq!(moved, vec![("typed", true), ("idle", false)]);
}

/// A save of something other than a session, as the settings and templates editors send: `PUT` on `path` with an
/// empty body. The route does not matter to the store, only that the command names its work.
fn save_on(path: &[&str], shown: &'static str) -> Command {
    let path = ApiPath::api(path).unwrap();
    Command::new(Work::Save, Method::Put, path, Some(json!({})), shown)
}

/// Adding a host and a confirmed Update answer 202: the helm accepted long work and carries on with it, reporting its
/// progress and result in the hosts it pushes. Settling that as done would claim a success nobody has seen yet, and
/// keeping the key busy would block the next action until the work ends. A new host has no id until the helm accepts
/// it, so its work goes under a fresh key of its own. Specified: a host addition under `OpKey::new_host_add` settles on
/// a 202 as accepted with the reply's body, its key is no longer busy, and nothing is recorded as a failure; a second
/// addition under another fresh key runs beside the first; and any other 2xx (a 201) still settles as done.
#[gpui::test]
fn an_accepted_reply_ends_the_work_without_claiming_success_or_failure(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&[], cx);
    let operations = operations(cx);
    let delivered = Delivered::default();
    let (first, second) = (OpKey::new_host_add(), OpKey::new_host_add());
    assert_ne!(
        first, second,
        "the premise: each addition has a key of its own"
    );
    let add = || {
        let path = ApiPath::api(&["hosts", "provision"]).unwrap();
        Command::new(
            Work::Save,
            Method::Post,
            path,
            Some(json!({})),
            "POST /api/hosts/provision",
        )
    };
    start_on(&operations, &first, add(), "add first", &delivered, cx).unwrap();
    start_on(&operations, &second, add(), "add second", &delivered, cx).unwrap();
    assert_eq!(
        cx.update(|cx| operations.read(cx).in_flight(&first)),
        Some(Work::Save),
        "the premise: the first addition holds its key"
    );
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(
        sent.len(),
        2,
        "the premise: both additions reached the helm"
    );
    fake.reply(
        sent[0].id,
        202,
        Some(json!({ "host_id": "h1", "run_id": "r1" })),
    );
    fake.reply(sent[1].id, 201, Some(json!({})));
    cx.run_until_parked();
    let outcomes: Vec<_> = delivered
        .borrow()
        .iter()
        .map(|(label, settled)| (*label, settled.outcome.clone()))
        .collect();
    assert_eq!(
        outcomes,
        vec![
            (
                "add first",
                Outcome::Accepted(Some(json!({ "host_id": "h1", "run_id": "r1" })))
            ),
            ("add second", Outcome::Done(Some(json!({})))),
        ]
    );
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(
            operations.in_flight(&first),
            None,
            "the accepted work no longer holds its key"
        );
        assert_eq!(operations.failure(&first), None, "and nothing failed");
    });
}

/// Settings saves the preferences and a host's settings, templates saves a template, and none of them is a session;
/// before the store took keys of other kinds, such work could not use it, and its busy state and refusals had nowhere
/// to live. Specified: work keyed by a host and by a template runs beside a session's, all three named by the same id
/// string, which must not make them collide; each is busy only on its own key (a second save of the host is refused,
/// the session is not); each refusal (the host's, then the template's) is recorded on its own key alone; and the
/// session's work settles with its own outcome.
#[gpui::test]
fn work_on_a_host_and_a_template_keeps_its_own_busy_state_and_refusal(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["x"], cx);
    let operations = operations(cx);
    let delivered = Delivered::default();
    let (host, template, session) = (OpKey::host("x"), OpKey::template("x"), OpKey::session("x"));
    let host_save = || save_on(&["hosts", "x"], "PUT /api/hosts/[host]");
    start_on(&operations, &host, host_save(), "save host", &delivered, cx).unwrap();
    let template_save = save_on(&["templates", "x"], "PUT /api/templates/[template]");
    start_on(
        &operations,
        &template,
        template_save,
        "save template",
        &delivered,
        cx,
    )
    .unwrap();
    start(
        &operations,
        ("x", rename("x", "new")),
        "rename session",
        &delivered,
        cx,
    )
    .unwrap();
    assert_eq!(
        start_on(
            &operations,
            &host,
            host_save(),
            "save host again",
            &delivered,
            cx
        ),
        Err(NotStarted::Busy(Work::Save)),
        "one operation per key: the host's second save waits for its first"
    );
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(operations.in_flight(&host), Some(Work::Save));
        assert_eq!(operations.in_flight(&template), Some(Work::Save));
        assert_eq!(
            operations.in_flight(&session),
            Some(Work::Rename),
            "the same id string is another key"
        );
    });
    cx.run_until_parked();
    let sent = fake.commands();
    let paths: Vec<_> = sent.iter().map(|sent| sent.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/api/hosts/x", "/api/templates/x", "/api/sessions/x/rename"],
        "the three commands reached the helm in order, and the refused one did not"
    );

    fake.reply(sent[0].id, 409, None);
    cx.run_until_parked();
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(operations.in_flight(&host), None, "the host's save settled");
        let words = HostText::from_host("PUT /api/hosts/[host]: 409");
        assert_eq!(
            operations
                .failure(&host)
                .map(|failure| (failure.work, &failure.message)),
            Some((Work::Save, &words)),
            "the refusal names the route with the host's id hidden"
        );
        assert_eq!(
            operations.failure(&template),
            None,
            "the host's refusal is the host's alone"
        );
        assert_eq!(operations.failure(&session), None);
        assert_eq!(
            operations.in_flight(&template),
            Some(Work::Save),
            "the template's save is still in flight"
        );
    });

    fake.reply(sent[1].id, 422, Some(json!("a template needs an agent")));
    fake.reply(sent[2].id, 200, None);
    cx.run_until_parked();
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(
            operations
                .failure(&template)
                .map(|failure| (failure.work, &failure.message)),
            Some((
                Work::Save,
                &HostText::from_host("a template needs an agent")
            )),
            "the template's refusal is on the template's key"
        );
        assert_eq!(operations.failure(&session), None, "and on no other");
        assert!(
            operations.failure(&host).is_some(),
            "the host's is still the host's"
        );
    });
    let outcomes: Vec<_> = delivered
        .borrow()
        .iter()
        .map(|(label, settled)| (*label, settled.outcome.clone()))
        .collect();
    assert_eq!(
        outcomes,
        vec![
            (
                "save host",
                Outcome::Refused(HostText::from_host("PUT /api/hosts/[host]: 409"))
            ),
            (
                "save template",
                Outcome::Refused(HostText::from_host("a template needs an agent"))
            ),
            ("rename session", Outcome::Done(None)),
        ]
    );
}

// ---- Saving drafts -------------------------------------------------------------------------------------------------

/// A stand-in for the Rename editor: the session it edits, its title draft, and what its completion saw.
struct Editor {
    session: String,
    title: Draft<String>,
    settled: Vec<Outcome>,
}

impl Editor {
    /// The editor's Save, as the Rename dialog will call it.
    fn save(&mut self, cx: &mut gpui::Context<Editor>) -> Result<(), NotStarted> {
        let session = self.session.clone();
        drafts::save(
            self,
            |editor| &mut editor.title,
            &OpKey::session(&session),
            |title| Ok(rename(&session, title)),
            cx,
            |editor, settled, _| editor.settled.push(settled.outcome.clone()),
        )
    }
}

/// An editor over session `key`'s title as the store holds it now.
fn open_editor(key: &str, cx: &mut TestAppContext) -> Entity<Editor> {
    let title = cx.update(|cx| {
        let store = session::current_store(cx).unwrap();
        store
            .read(cx)
            .collections()
            .get::<SessionRow>(key)
            .unwrap()
            .info
            .title
            .clone()
    });
    cx.new(|_| Editor {
        session: key.to_owned(),
        title: Draft::load(title),
        settled: Vec::new(),
    })
}

/// The sessions collection with one row, `a`, titled `title`.
fn titled(title: &str) -> CollectionState {
    let mut entry = session_entry("a");
    entry.value["title"] = json!(title);
    CollectionState {
        collection: Collection::Sessions,
        meta: meta(&["a"]),
        entries: vec![entry],
    }
}

/// SPEC_impl.md, "Drafts", names this test: load, save untouched, compare. An untouched draft must send back the
/// helm's own bytes; an editor that sent its escaped presentation, or trimmed or repaired the title, would rename the
/// session to something the user never typed. Specified: a title holding a direction override, a zero-width
/// character, a literal `<U+` and surrounding spaces, loaded from the store and saved untouched, goes out as exactly
/// those bytes.
#[gpui::test]
fn a_draft_saved_untouched_sends_the_helms_own_bytes(cx: &mut TestAppContext) {
    let raw = " fix\u{202E}lmth.exe <U+0041>\u{200B} ";
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![titled(raw)]);
    cx.run_until_parked();
    let editor = open_editor("a", cx);
    assert_eq!(
        cx.update(|cx| editor.read(cx).title.value().clone()),
        raw,
        "the premise: loaded raw"
    );

    cx.update(|cx| editor.update(cx, |editor, cx| editor.save(cx)))
        .unwrap();
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].path, "/api/sessions/a/rename");
    assert_eq!(
        sent[0].body,
        Some(json!({ "title": raw })),
        "the title went out byte for byte"
    );
}

/// SPEC.md's Rename rule: a refusal leaves the draft and its error visible for correction, and SPEC_impl.md adds that
/// a failed save is reported, never retried and never treated as saved. Specified: a touched draft follows no push;
/// a draft whose save is in flight says so and refuses edits, and a second Save meanwhile is refused; the refusal keeps
/// the typed title and the touched mark, records the helm's words, reaches the editor's completion, and sends nothing
/// more.
#[gpui::test]
fn a_refused_save_keeps_the_draft_and_is_not_retried(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![titled("old")]);
    cx.run_until_parked();
    let editor = open_editor("a", cx);
    let took =
        cx.update(|cx| editor.update(cx, |editor, _| editor.title.edit("bad\ttitle".to_owned())));
    assert!(took, "the premise: the field is touched");
    fake.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: meta(&["a"]),
        upsert: titled("renamed elsewhere").entries,
        remove: vec![],
    });
    cx.run_until_parked();
    let pushed = cx.update(|cx| {
        let store = session::current_store(cx).unwrap();
        store
            .read(cx)
            .collections()
            .get::<SessionRow>("a")
            .unwrap()
            .info
            .title
            .clone()
    });
    assert_eq!(
        pushed, "renamed elsewhere",
        "the premise: the push reached the store"
    );
    let followed = cx.update(|cx| editor.update(cx, |editor, _| editor.title.follow(pushed)));
    assert!(!followed, "the push did not overwrite the touched draft");

    cx.update(|cx| editor.update(cx, |editor, cx| editor.save(cx)))
        .unwrap();
    assert!(
        cx.update(|cx| editor.read(cx).title.is_saving()),
        "the editor can draw its Save busy"
    );
    let late = cx.update(|cx| {
        editor.update(cx, |editor, _| {
            editor.title.edit("typed while saving".to_owned())
        })
    });
    assert!(!late, "an edit while saving is refused");
    let again = cx.update(|cx| editor.update(cx, |editor, cx| editor.save(cx)));
    assert_eq!(
        again,
        Err(NotStarted::Busy(Work::Rename)),
        "Save again while saving does nothing"
    );
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(sent.len(), 1);
    fake.reply(sent[0].id, 400, Some(json!("a title is a single line")));
    cx.run_until_parked();

    let words = HostText::from_host("a title is a single line");
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert_eq!(
            editor.title.value(),
            "bad\ttitle",
            "the typed title is still there"
        );
        assert!(editor.title.is_touched() && !editor.title.is_saving());
        assert_eq!(editor.title.refusal(), Some(&words));
        assert_eq!(editor.settled, vec![Outcome::Refused(words.clone())]);
    });
    assert!(
        fake.commands().is_empty(),
        "the failed save was not retried"
    );
}

/// A stand-in for a template editor: one draft field, saved as work on the template it edits.
struct TemplateEditor {
    prompt: Draft<String>,
    settled: Vec<Outcome>,
}

impl TemplateEditor {
    /// The editor's Save: the prompt as the template's whole body, keyed by the template's name.
    fn save(&mut self, cx: &mut gpui::Context<TemplateEditor>) -> Result<(), NotStarted> {
        drafts::save(
            self,
            |editor| &mut editor.prompt,
            &OpKey::template("t"),
            |prompt| {
                let path = ApiPath::api(&["templates", "t"])?;
                let body = Some(json!({ "prompt": prompt }));
                Ok(Command::new(
                    Work::Save,
                    Method::Put,
                    path,
                    body,
                    "PUT /api/templates/[template]",
                ))
            },
            cx,
            |editor, settled, _| editor.settled.push(settled.outcome.clone()),
        )
    }
}

/// Drafts are not only session titles: the template and settings editors save drafts too, and `drafts::save` used to
/// take a session id. Specified: a draft saved under a template's key goes out as its editor's command, the
/// store shows the template's key busy (and no session's) while it is in flight, and once the helm answers the draft
/// is settled as saved and the editor's completion runs.
#[gpui::test]
fn a_draft_saves_as_work_on_a_template(cx: &mut TestAppContext) {
    let (mut fake, _connection) = connected(&["t"], cx);
    let editor = cx.new(|_| TemplateEditor {
        prompt: Draft::load("old".to_owned()),
        settled: Vec::new(),
    });
    let took =
        cx.update(|cx| editor.update(cx, |editor, _| editor.prompt.edit("new prompt".to_owned())));
    assert!(took, "the premise: the field is touched");
    cx.update(|cx| editor.update(cx, |editor, cx| editor.save(cx)))
        .unwrap();
    let operations = operations(cx);
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(
            operations.in_flight(&OpKey::template("t")),
            Some(Work::Save)
        );
        assert_eq!(
            operations.in_flight(&OpKey::session("t")),
            None,
            "no session is busy"
        );
        assert!(editor.read(cx).prompt.is_saving());
    });
    cx.run_until_parked();
    let sent = fake.commands();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Put, "/api/templates/t")
    );
    assert_eq!(sent[0].body, Some(json!({ "prompt": "new prompt" })));

    fake.reply(sent[0].id, 200, None);
    cx.run_until_parked();
    cx.update(|cx| {
        let editor = editor.read(cx);
        assert!(
            !editor.prompt.is_saving() && !editor.prompt.is_touched(),
            "the save counted as saved"
        );
        assert_eq!(editor.settled, vec![Outcome::Done(None)]);
        assert_eq!(operations.read(cx).in_flight(&OpKey::template("t")), None);
    });
}

// ---- A text field bound to a draft ---------------------------------------------------------------------------------

/// A stand-in editor with one text field bound to a draft, drawn in a window so the field is real: what a settings or
/// template field will be. Its field is whichever kind `make` builds; Enter in a field that submits saves it as work
/// on a template.
struct FieldEditor {
    prompt: DraftInput,
    settled: Vec<Outcome>,
}

impl FieldEditor {
    fn new(
        value: &str,
        make: MakeInput,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let input = make(window, cx);
        let prompt = DraftInput::new(
            value,
            input,
            |editor: &mut Self| &mut editor.prompt,
            Self::submit,
            window,
            cx,
        );
        FieldEditor {
            prompt,
            settled: Vec::new(),
        }
    }

    /// Enter in the field.
    fn submit(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        let _ = DraftInput::save(
            self,
            |editor| &mut editor.prompt,
            &OpKey::template("t"),
            |prompt| {
                let path = ApiPath::api(&["templates", "t"])?;
                let body = Some(json!({ "prompt": prompt }));
                Ok(Command::new(
                    Work::Save,
                    Method::Put,
                    path,
                    body,
                    "PUT /api/templates/[template]",
                ))
            },
            window,
            cx,
            |editor, settled, _| editor.settled.push(settled.outcome.clone()),
        );
    }
}

impl Render for FieldEditor {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "field-box".into())
            .w(px(300.))
            .child(TextInput::new(
                "prompt",
                self.prompt.input(),
                "Prompt",
                HoverHelp::tooltip("The template's prompt"),
            ))
    }
}

/// How a test makes its editor's field: a one-line field ([`TextInputState::new`]) or a growing box.
type MakeInput = fn(&mut Window, &mut gpui::App) -> TextInputState;

/// A growing box four rows tall, as the old template prompt started.
fn growing_box(window: &mut Window, cx: &mut gpui::App) -> TextInputState {
    TextInputState::new_growing(4, window, cx)
}

/// A connected app with a window holding a [`FieldEditor`] over `value` in a field `make` builds, its field focused.
fn open_field<'a>(
    value: &str,
    make: MakeInput,
    cx: &'a mut TestAppContext,
) -> (FakeHelm, Entity<FieldEditor>, &'a mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(vec![sessions(&[])]);
    cx.run_until_parked();
    let value = value.to_owned();
    let (editor, cx) = test_support::open_view(cx, move |window, cx| {
        FieldEditor::new(&value, make, window, cx)
    });
    test_support::click(cx, "field-box");
    (fake, editor, cx)
}

/// The field's text now.
fn shown(editor: &Entity<FieldEditor>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| editor.read(cx).prompt.input().text(cx).to_string())
}

/// Hand the field a pushed value, as the editor's store observer does.
fn push(editor: &Entity<FieldEditor>, value: &str, cx: &mut VisualTestContext) {
    let value = value.to_owned();
    cx.update(|window, cx| {
        editor.update(cx, |editor, cx| editor.prompt.follow(&value, window, cx))
    });
    test_support::settle(cx);
}

/// The rule the bound field exists to keep for every editor at once: a refresh never changes what the user is typing
/// (SPEC.md, "Things changing underneath"), while a field the user has not touched shows the current value. Specified:
/// an untouched field shows a pushed value; once the user types, the draft holds the field's text and a later push
/// changes neither.
#[gpui::test]
fn a_bound_field_follows_pushes_until_the_user_types(cx: &mut TestAppContext) {
    let (_fake, editor, cx) = open_field("old", TextInputState::new, cx);
    push(&editor, "pushed", cx);
    assert_eq!(
        shown(&editor, cx),
        "pushed",
        "the untouched field followed the push"
    );
    keys(cx, "end x");
    assert_eq!(shown(&editor, cx), "pushedx", "the premise: the user typed");
    assert!(
        cx.update(|_, cx| editor.read(cx).prompt.draft().is_touched()),
        "and the draft knows it"
    );
    push(&editor, "pushed again", cx);
    assert_eq!(
        shown(&editor, cx),
        "pushedx",
        "the push left the typed text alone"
    );
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).prompt.draft().value().clone()),
        "pushedx"
    );
}

/// While a save is in flight the text on screen must stay the text sent: a push shown then would display a value the
/// helm is not saving, and a keystroke taken then would be lost when the save's success clears the touched mark.
/// Specified: Enter saves the field's text; while it is in flight a typed letter and a push both leave the field as
/// sent; once the save is done the draft counts as saved and a letter typed then lands where the caret was, at the
/// start, which tells the field held read-only (caret untouched) from the backstop that puts the text back (caret moved
/// to the end) and from a field left read-only (the letter refused).
#[gpui::test]
fn a_bound_field_is_read_only_and_shows_no_push_while_saving(cx: &mut TestAppContext) {
    let (mut fake, editor, cx) = open_field("old", TextInputState::new, cx);
    keys(cx, "end x home enter");
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: Enter saved the field");
    assert_eq!(sent[0].body, Some(json!({ "prompt": "oldx" })));
    keys(cx, "y");
    push(&editor, "pushed", cx);
    assert_eq!(
        shown(&editor, cx),
        "oldx",
        "neither the letter nor the push changed the field mid-save"
    );
    fake.reply(sent[0].id, 200, None);
    test_support::settle(cx);
    assert!(
        cx.update(|_, cx| !editor.read(cx).prompt.draft().is_touched()),
        "the save counted as saved"
    );
    // The caret sits at the start. Putting the text back moves a one-line field's caret to the end, so had read-only
    // not kept "y" out, the backstop's put-back would leave this letter at the end instead; and had the save's end
    // not lifted read-only, the letter would not land at all.
    keys(cx, "z");
    assert_eq!(
        shown(&editor, cx),
        "zoldx",
        "once saved, the field takes typing again where the caret was"
    );
}

/// A save the helm refused leaves the draft as the user had it, but an untouched field did not follow pushes while it
/// was saving: saving it again must not send back a value someone else has since replaced. Specified: an untouched
/// field saved, pushed during the save, and refused takes up the pushed value; a touched one refused the same way keeps
/// the user's text.
#[gpui::test]
fn after_a_refused_save_only_an_untouched_field_takes_up_the_last_push(cx: &mut TestAppContext) {
    let (mut fake, editor, cx) = open_field("old", TextInputState::new, cx);
    keys(cx, "enter");
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the untouched field was saved");
    push(&editor, "replaced elsewhere", cx);
    assert_eq!(
        shown(&editor, cx),
        "old",
        "the premise: not shown while saving"
    );
    fake.reply(sent[0].id, 409, Some(json!("changed meanwhile")));
    test_support::settle(cx);
    assert_eq!(
        shown(&editor, cx),
        "replaced elsewhere",
        "the untouched field took up the push after the refusal"
    );

    keys(cx, "end x enter");
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the touched field was saved");
    push(&editor, "replaced again", cx);
    fake.reply(sent[0].id, 409, Some(json!("changed meanwhile")));
    test_support::settle(cx);
    assert_eq!(
        shown(&editor, cx),
        "replaced elsewherex",
        "the touched field kept the user's text"
    );
    let refusal = cx.update(|_, cx| editor.read(cx).prompt.draft().refusal().cloned());
    assert_eq!(
        refusal,
        Some(HostText::from_host("changed meanwhile")),
        "and holds the refusal to show"
    );
}

/// The draft-bound field must work over both kinds of field the editors to come will use: one-line fields and the
/// growing box. Specified, over a growing box: an untouched box follows a push; typing a line break mirrors the
/// two-line text into the draft and saves nothing (Enter, which saves a one-line field, is a line break here); a push
/// after that leaves the typed text alone.
#[gpui::test]
fn a_bound_growing_box_mirrors_line_breaks_and_follows_pushes_until_touched(
    cx: &mut TestAppContext,
) {
    let (mut fake, editor, cx) = open_field("old", growing_box, cx);
    push(&editor, "first line", cx);
    assert_eq!(
        shown(&editor, cx),
        "first line",
        "the untouched box followed the push"
    );
    keys(cx, "end enter x");
    test_support::settle(cx);
    assert_eq!(
        shown(&editor, cx),
        "first line\nx",
        "Enter typed a line break"
    );
    assert_eq!(
        cx.update(|_, cx| editor.read(cx).prompt.draft().value().clone()),
        "first line\nx"
    );
    assert!(fake.commands().is_empty(), "Enter saved nothing");
    push(&editor, "pushed meanwhile", cx);
    assert_eq!(
        shown(&editor, cx),
        "first line\nx",
        "the push left the typed text alone"
    );
}

// ---- Reads ----------------------------------------------------------------------------------------------------------

/// Every read a feature makes (the quick switcher's listing, the launcher's and Settings' lookups) shows a failure in
/// the same words an operation's refusal uses, so the helm's reason reads the same wherever it surfaces; before this
/// helper each read copied that wording. Specified: any 2xx body (a 202's included) decodes as the caller's type; a
/// refusal is the helm's words trimmed, or the JSON it sent, or, with no words, the read as `shown` names it and the
/// status, exactly as the store words a refusal (`Outcome::Refused`); a body that does not decode, a 2xx with no body,
/// and a channel error each say so.
#[test]
fn a_reads_answer_decodes_and_its_refusal_is_worded_as_an_operations() {
    use farhelm_helm_client::CommandError;
    use farhelm_proto::ui_channel::Reply;

    let decoded = |status, body| {
        let reply = Ok(Reply {
            id: 0,
            status,
            body,
        });
        operations::decode::<Vec<String>>(reply, "GET /api/x")
    };
    let words = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!(decoded(200, Some(json!(["a", "b"]))), Ok(words.clone()));
    assert_eq!(decoded(202, Some(json!(["a", "b"]))), Ok(words));
    let refused = |words: &str| Err(HostText::from_host(words));
    assert_eq!(decoded(400, Some(json!(" not here "))), refused("not here"));
    assert_eq!(
        decoded(409, Some(json!({ "error": "busy" }))),
        refused(r#"{"error":"busy"}"#)
    );
    assert_eq!(decoded(404, None), refused("GET /api/x: 404"));
    assert_eq!(decoded(500, Some(json!(""))), refused("GET /api/x: 500"));
    let wrong_shape = decoded(200, Some(json!({ "x": 1 })));
    let prefix = "the helm's answer to GET /api/x could not be read: ";
    assert!(
        matches!(&wrong_shape, Err(words) if words.raw().starts_with(prefix)),
        "an answer of the wrong shape says so: {wrong_shape:?}"
    );
    assert_eq!(
        decoded(204, None),
        refused("the helm sent no answer to GET /api/x")
    );
    assert_eq!(
        operations::decode::<Vec<String>>(Err(CommandError::Reset), "GET /api/x"),
        refused("the connection to the helm is resetting")
    );
}

/// A read is the one way a feature asks the helm for something it does not push, so it must reach the helm as a plain
/// GET on the caller's route and come back to the caller, free to read the store when it does (a callback run inside
/// the UI session's update would panic on that), and a read that a reset made pointless must not come back at all (the
/// reset already reports the lost connection). Specified: with no connection, and with a connection that has no UI
/// session yet, `operations::read` sends nothing and says so; with one, it sends a GET with no body on its path, and
/// its callback receives the decoded answer and can read the store; a read sent before the channel breaks never calls
/// back.
#[gpui::test]
fn a_read_sends_a_get_and_hands_back_the_answer_unless_a_reset_drops_it(cx: &mut TestAppContext) {
    /// Each answer, and whether the store could be read from the callback that received it.
    type Answers = Rc<RefCell<Vec<(Result<Vec<String>, HostText>, bool)>>>;
    let send = |answers: &Answers, cx: &mut TestAppContext| {
        let answers = answers.clone();
        cx.update(move |cx| {
            let path = ApiPath::api(&["example"]).unwrap();
            operations::read(path, "GET /api/example", cx, move |answer, cx| {
                let store_read = session::current_store(cx).is_some_and(|store| {
                    let _ = store.read(cx).collections();
                    true
                });
                answers.borrow_mut().push((answer, store_read))
            })
        })
    };
    let answers = Answers::default();
    assert!(!send(&answers, cx), "with no connection, nothing is sent");
    let (mut fake, _connection) = started(cx);
    assert!(
        !send(&answers, cx),
        "with a connection but no UI session yet, nothing is sent"
    );
    fake.connect(vec![sessions(&[])]);
    cx.run_until_parked();

    assert!(
        send(&answers, cx),
        "the premise: a live UI session sends it"
    );
    cx.run_until_parked();
    let sent = fake.commands();
    let shown: Vec<_> = sent
        .iter()
        .map(|sent| (sent.method, sent.path.as_str(), sent.body.clone()))
        .collect();
    assert_eq!(shown, vec![(Method::Get, "/api/example", None)]);
    fake.reply(sent[0].id, 200, Some(json!(["h1"])));
    cx.run_until_parked();
    assert_eq!(
        *answers.borrow(),
        vec![(Ok(vec!["h1".to_owned()]), true)],
        "the answer arrived, and its callback read the store"
    );

    assert!(send(&answers, cx), "the premise: the second read is sent");
    cx.run_until_parked();
    let second = fake.commands();
    assert_eq!(
        second.len(),
        1,
        "the premise: the second read reached the helm"
    );
    fake.break_channel();
    cx.run_until_parked();
    assert_eq!(
        answers.borrow().len(),
        1,
        "the read the reset dropped never called back"
    );
}

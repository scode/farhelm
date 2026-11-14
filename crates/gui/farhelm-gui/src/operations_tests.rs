//! Tests of the operation store and of saving drafts through it, against the helm client's fake.
//!
//! The fake answers commands in the order they were sent, as the helm does, so "overlapping" here means both commands
//! are in flight at once and each settles on its own reply; which reply comes first is the order the helm admitted
//! them. The helpers come from the UI-session owner's tests, which these build on.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{Collection, CollectionChanges, CollectionState, SessionRow};
use gpui::{AppContext as _, Entity, TestAppContext};
use gpui_component::Root;
use serde_json::json;

use crate::drafts::{self, Draft};
use crate::host_text::HostText;
use crate::main_view::MainView;
use crate::operations::{Command, NotStarted, Outcome, SessionOperations, Settled, Work};
use crate::session::{self, Connection};
use crate::session_tests::{meta, session_entry, sessions, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Every completion delivered, labeled by the test's name for the operation.
type Delivered = Rc<RefCell<Vec<(&'static str, Settled)>>>;

fn operations(cx: &mut TestAppContext) -> Entity<SessionOperations> {
    match cx.update(|cx| session::current_operations(cx)) {
        Some(operations) => operations,
        None => panic!("the premise: a UI session is live"),
    }
}

/// Start `command` on `session`, recording its completion under `label`.
fn start(
    operations: &Entity<SessionOperations>,
    (session, command): (&str, Command),
    label: &'static str,
    delivered: &Delivered,
    cx: &mut TestAppContext,
) -> Result<(), NotStarted> {
    let delivered = delivered.clone();
    cx.update(|cx| {
        operations.update(cx, |operations, cx| {
            operations.start(session, command, cx, move |settled, _| {
                delivered.borrow_mut().push((label, settled.clone()));
            })
        })
    })
}

fn rename(session: &str, title: &str) -> Command {
    Command::rename(session, title).unwrap()
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
    let deleting = (
        "b",
        Command::delete("b", DeleteGuard::NothingAlive).unwrap(),
    );
    start(&operations, deleting, "delete b", &delivered, cx).unwrap();
    let again = (
        "a",
        Command::delete("a", DeleteGuard::Unconditional).unwrap(),
    );
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
            (Method::Delete, "/api/sessions/b?only_if_nothing_alive=true"),
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
        assert_eq!(operations.in_flight("a"), None, "a settled");
        assert_eq!(
            operations.in_flight("b"),
            Some(Work::Delete(DeleteGuard::NothingAlive)),
            "b is still in flight, as the Delete the command sent, guard included"
        );
        let failure = operations
            .failure("a")
            .map(|failure| (failure.work, &failure.message));
        assert_eq!(
            failure,
            Some((
                Work::Rename,
                &HostText::from_host("titles may not be empty")
            ))
        );
        assert_eq!(operations.failure("b"), None, "a's refusal is a's alone");
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
    assert_eq!(cx.update(|cx| operations.read(cx).in_flight("b")), None);
    assert!(
        cx.update(|cx| operations.read(cx).failure("a").is_some()),
        "the premise: a's failure is still on its row"
    );

    let retry = ("a", rename("a", "newer"));
    start(&operations, retry, "rename a again", &delivered, cx).unwrap();
    assert_eq!(
        cx.update(|cx| operations.read(cx).failure("a").cloned()),
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
            .failure("a")
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
/// Specification: for each guard, a started Delete is in flight as `Work::Delete` with the guard its command sent, and
/// the command's path carries that guard's flag.
#[gpui::test]
fn a_delete_in_flight_reports_the_guard_its_command_sent(cx: &mut TestAppContext) {
    let guards = [
        ("a", DeleteGuard::Unconditional, "/api/sessions/a"),
        (
            "b",
            DeleteGuard::AgentEnded,
            "/api/sessions/b?only_if_agent_ended=true",
        ),
        (
            "c",
            DeleteGuard::NothingAlive,
            "/api/sessions/c?only_if_nothing_alive=true",
        ),
    ];
    let (mut fake, _connection) = connected(&["a", "b", "c"], cx);
    let operations = operations(cx);
    let delivered = Delivered::default();
    for (key, guard, _) in guards {
        let deleting = (key, Command::delete(key, guard).unwrap());
        start(&operations, deleting, "delete", &delivered, cx).unwrap();
    }
    cx.run_until_parked();
    let paths: Vec<String> = fake
        .commands()
        .iter()
        .map(|sent| sent.path.as_str().to_owned())
        .collect();
    for (key, guard, path) in guards {
        assert_eq!(
            cx.update(|cx| operations.read(cx).in_flight(key)),
            Some(Work::Delete(guard)),
            "{key} is in flight with its own guard"
        );
        assert!(
            paths.iter().any(|sent| sent == path),
            "{key}'s command carries its guard: {paths:?}"
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
        cx.update(|cx| operations.read(cx).in_flight("a")),
        Some(Work::Rename),
        "the work outlived its view"
    );

    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "the premise: the save reached the helm");
    fake.reply(sent[0].id, 400, Some(json!("refused")));
    cx.run_until_parked();
    cx.update(|cx| {
        let operations = operations.read(cx);
        assert_eq!(operations.in_flight("a"), None, "the store settled it");
        let failure = operations
            .failure("a")
            .map(|failure| (failure.work, failure.message.clone()));
        assert_eq!(
            failure,
            Some((Work::Rename, HostText::from_host("refused")))
        );
    });
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
        cx.update(|cx| first.read(cx).failure("c").is_some()),
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
    let deleting = (
        "b",
        Command::delete("b", DeleteGuard::Unconditional).unwrap(),
    );
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
            first.read(cx).in_flight("b"),
            None,
            "the retired store holds nothing"
        );
        assert_eq!(
            first.read(cx).failure("c"),
            None,
            "its failures went with it"
        );
        assert_eq!(second.read(cx).in_flight("a"), None);
        assert_eq!(second.read(cx).in_flight("b"), None);
        assert_eq!(second.read(cx).failure("c"), None);
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
            &session,
            |title| Command::rename(&session, title),
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

//! Tests of the Rename editor in the real main view over the helm client's fake: what it starts from, what it follows,
//! what Save sends and what becomes of the reply, what a listing without the session does to it, and where focus goes.
//!
//! The editor is opened the way the user opens it, with the shortcut while the open session's terminal holds focus;
//! the tests reach the open editor through [`Opened`], which only test builds record. Every step ends in one of the
//! shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{Availability, CollectionChanges, CollectionState, ListCounts};
use gpui::{
    ClipboardItem, DriverNode, Entity, Global, Modifiers, TestAppContext, VisualTestContext,
    WeakEntity, point, px,
};
use serde_json::{Value, json};

use super::RenameEditor;
use crate::actions::{RENAME_SESSION_CHORD, TERMINAL_CONTEXT};
use crate::layers;
use crate::main_view::MainView;
use crate::operations::OpKey;
use crate::session;
use crate::session_list::tests::{Fixture, listing, preferences, session};
use crate::test_support::started;
use crate::test_support::{self, frame, keys, settle};

/// The editor [`super::open`] opened last, recorded in test builds only, so a test can read its draft and field.
pub(super) struct Opened(pub(super) WeakEntity<RenameEditor>);

impl Global for Opened {}

/// The Rename shortcut on this platform (SPEC.md, "Desktop window chrome").
const RENAME: &str = RENAME_SESSION_CHORD;

/// Kit's select-all and paste in a text field on this platform.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};
const PASTE: &str = if cfg!(target_os = "macos") {
    "cmd-v"
} else {
    "ctrl-v"
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Sessions `a` (titled `title`) and `b`, in that order, with `a` the last visited, so `a` is open at startup.
fn fleet(title: &str) -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a").title(title), session("b")]),
        preferences(&["a"], None),
    ]
}

/// The real main view over the fake helm, connected with `snapshot`, in an active window, checked once settled.
fn open_main(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut helm, _connection) = started(cx);
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    (helm, window)
}

/// A push of the sessions collection as `state` holds it, removing `remove`.
fn push(helm: &mut FakeHelm, state: CollectionState, remove: &[&str], cx: &mut VisualTestContext) {
    helm.push(CollectionChanges {
        collection: state.collection,
        meta: state.meta,
        upsert: state.entries,
        remove: remove.iter().map(|key| key.to_string()).collect(),
    });
    settle(cx);
}

/// The listing of `rows` alone (in that order), with complete counts unless the test changes them.
fn only(rows: &[Fixture]) -> CollectionState {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    listing(&order, rows)
}

/// The rename commands the window sent since the last call, as (session's path, body). Every other command (the
/// selection remembered, a seen mark) is answered at once, so the fake's in-order replies stay possible; a test calls
/// this only while no rename is waiting for its answer.
fn renames(helm: &mut FakeHelm) -> Vec<FakeCommand> {
    let mut renames = Vec::new();
    for command in helm.commands() {
        if command.path.ends_with("/rename") {
            renames.push(command);
        } else {
            helm.reply(command.id, 200, None);
        }
    }
    renames
}

/// The one rename sent since the last call: its title, and its id to answer it by.
fn one_rename(helm: &mut FakeHelm) -> (u64, Value) {
    let sent = renames(helm);
    assert_eq!(sent.len(), 1, "one rename was sent: {sent:?}");
    assert_eq!(
        (sent[0].method, sent[0].path.as_str()),
        (Method::Post, "/api/sessions/a/rename")
    );
    (
        sent[0].id,
        sent[0].body.clone().unwrap_or_default()["title"].clone(),
    )
}

/// The editor opened last, which must still be open.
fn editor(cx: &mut VisualTestContext) -> Entity<RenameEditor> {
    let editor = cx.update(|_, cx| {
        cx.try_global::<Opened>()
            .and_then(|opened| opened.0.upgrade())
    });
    editor.expect("the premise: an editor is open")
}

fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// The editor's field text.
fn field(cx: &mut VisualTestContext) -> String {
    let editor = editor(cx);
    cx.update(|_, cx| editor.read(cx).title.input().text(cx).to_string())
}

/// Whether the editor's field holds focus.
fn field_focused(cx: &mut VisualTestContext) -> bool {
    let editor = editor(cx);
    cx.update(|window, cx| {
        editor
            .read(cx)
            .title
            .input()
            .focus_handle(cx)
            .is_focused(window)
    })
}

/// Whether a session terminal holds focus: the focused element's key context stack is a terminal's.
fn terminal_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| {
        window
            .context_stack()
            .iter()
            .any(|context| context.contains(TERMINAL_CONTEXT))
    })
}

/// The drawn element under `id`, from `nodes`.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

// ---- Opening -------------------------------------------------------------------------------------------------------

/// SPEC.md, "Desktop window chrome": the Rename shortcut opens Rename for the open session from its terminal, and
/// "Closing that shortcut-opened dialog returns focus to the terminal". The draft must start from the helm's raw
/// title, or a hidden character in it would be lost or saved escaped. Specified: with `a` open and its terminal
/// focused, the shortcut opens one editor over `a` whose field holds the raw title, hidden character included, and has
/// focus; the shortcut's handler run again while it is open opens no second editor; Escape closes it with nothing sent
/// and focus back on the terminal.
#[gpui::test]
fn the_shortcut_opens_rename_for_the_open_session_and_escape_returns_to_its_terminal(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = open_main(cx, fleet("fix\u{200B}build"));
    assert!(terminal_focused(cx), "the premise: a's terminal has focus");

    keys(cx, RENAME);
    assert!(any_open(cx), "the shortcut opened the editor");
    let opened = editor(cx);
    let key = cx.update(|_, cx| opened.read(cx).key.clone());
    assert_eq!(key, "a");
    assert_eq!(field(cx), "fix\u{200B}build", "the raw title, unescaped");
    assert!(field_focused(cx), "focus starts in the title field");

    // From the editor's field the chord never reaches the main view's handler; run the handler as a page control
    // with a layer open would (a future popup list that leaves focus in its field, say).
    cx.update(super::open_for_selected);
    test_support::check(cx);
    assert_eq!(editor(cx), opened, "no second editor over the first");

    keys(cx, "escape");
    assert!(!any_open(cx));
    assert!(terminal_focused(cx), "focus went back to the terminal");
    assert_eq!(renames(&mut helm), [], "and nothing was sent");
}

/// SPEC.md, "Desktop window chrome": "Closing that shortcut-opened dialog returns focus to the terminal", also when
/// the shortcut was pressed while a control on the page held focus; returning to that control would leave the user's
/// next keys off the terminal they were working in. Specified: with focus on a's menu button (its menu opened and
/// closed again), the shortcut opens the editor, and Escape closes it with focus on the terminal.
#[gpui::test]
fn closing_a_shortcut_opened_editor_returns_focus_to_the_terminal(cx: &mut TestAppContext) {
    let (_helm, cx) = open_main(cx, fleet("old"));
    test_support::click(cx, "session-menu-a");
    keys(cx, "escape");
    assert!(
        !any_open(cx) && !terminal_focused(cx),
        "the premise: a page control, the menu's button, holds focus"
    );
    keys(cx, RENAME);
    assert!(any_open(cx), "the premise: the editor opened");
    keys(cx, "escape");
    assert!(terminal_focused(cx), "focus went to the terminal");
}

/// SPEC.md: "Close uses the same cancellation and busy-state rules as Cancel". Close goes through the layer manager's
/// dismissal, the way Escape does, so the editor's own rule decides both; the dismissal asks the editor's hook, which
/// reads the editor, so Close must call it from the click and not from inside an update of the editor (GPUI refuses
/// to read an entity it is updating, with a panic). Specified: a click on the header Close closes the editor without
/// that panic, and focus returns to the terminal, as Escape's does.
#[gpui::test]
fn the_header_close_closes_the_editor_like_escape(cx: &mut TestAppContext) {
    let (_helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    assert!(any_open(cx), "the premise: the editor opened");
    test_support::click(cx, "rename-close");
    assert!(!any_open(cx), "Close closed the editor");
    assert!(terminal_focused(cx), "focus went to the terminal");
}

/// The shortcut acts on the open session only; with none there is nothing to rename, and it must still not reach a
/// program or open an empty editor. Specified: over an empty fleet the shortcut opens nothing.
#[gpui::test]
fn the_shortcut_with_no_session_open_does_nothing(cx: &mut TestAppContext) {
    let (_helm, cx) = open_main(cx, vec![listing(&[], &[]), preferences(&[], None)]);
    keys(cx, RENAME);
    assert!(!any_open(cx));
}

/// SPEC.md, "Lifecycle operations": "an open lifecycle confirmation or a pending request refuses another lifecycle
/// action", and the old app refused to open Rename on a session with a request in flight. A Rename sent behind a
/// Delete would race it. Specified: with a Delete of `a` in flight the shortcut opens nothing; with the editor already
/// open when a Delete of `a` starts, Enter sends no rename and leaves the draft as it was, not saving.
#[gpui::test]
fn a_rename_is_refused_while_a_delete_of_that_session_is_in_flight(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("a"));
    let start_delete = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let operations = session::current_operations(cx).expect("a UI session is live");
            let delete =
                crate::session_list::delete::command("a", DeleteGuard::Unconditional).unwrap();
            operations.update(cx, |operations, cx| {
                operations.start(&OpKey::session("a"), delete, cx, |_, _| {})
            })
        })
        .expect("the premise: the Delete started");
    };

    start_delete(cx);
    keys(cx, RENAME);
    assert!(!any_open(cx), "no editor over a session being deleted");
    // Answer the Delete (and the selection remembered at startup) so the session is free again.
    let sent = renames(&mut helm);
    assert_eq!(sent, [], "the premise: no rename was ever sent");
    settle(cx);

    keys(cx, RENAME);
    assert!(
        any_open(cx),
        "the premise: with the Delete settled, the editor opens"
    );
    start_delete(cx);
    keys(cx, "enter");
    assert!(
        helm.commands()
            .iter()
            .all(|command| command.method == Method::Delete),
        "the Save behind the Delete sent nothing"
    );
    let opened = editor(cx);
    let saving = cx.update(|_, cx| opened.read(cx).title.draft().is_saving());
    assert!(!saving, "and the draft is not waiting on a save");
}

// ---- The draft -----------------------------------------------------------------------------------------------------

/// SPEC_impl.md, "Drafts", and SPEC.md's "A refresh never changes what the user is typing": until the user edits the
/// field it follows a rename pushed from elsewhere, so they save from the title as it now is; once they edit it, a push
/// must never overwrite what they typed. Specified: a pushed title replaces the untouched field's text; after a typed
/// letter, a second pushed title leaves the field as typed and still updates the current-title line above it.
#[gpui::test]
fn an_untouched_draft_follows_a_pushed_rename_and_a_touched_one_does_not(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("first"));
    keys(cx, RENAME);

    push(
        &mut helm,
        only(&[session("a").title("pushed"), session("b")]),
        &[],
        cx,
    );
    assert_eq!(field(cx), "pushed", "the untouched field followed");

    keys(cx, "x");
    let typed = field(cx);
    assert_ne!(
        typed, "pushed",
        "the premise: the typed letter reached the field"
    );
    push(
        &mut helm,
        only(&[session("a").title("pushed again"), session("b")]),
        &[],
        cx,
    );
    assert_eq!(field(cx), typed, "the touched field kept what was typed");
    let opened = editor(cx);
    let current = cx.update(|_, cx| opened.read(cx).current.raw().to_owned());
    assert_eq!(current, "pushed again", "the current title still follows");
}

/// SPEC.md, "Things changing underneath": "Text, caret and focus stay as they are". The store notifies the editor on
/// every push of every collection, and putting the field's text back on each one (even to what it already holds)
/// moves Kit's caret to the start and clears its undo history, so typing would jump while another session's status
/// changes. Specified: after End and a typed letter, a push that changes only another session leaves the caret where
/// it was, so the next letter follows the first.
#[gpui::test]
fn typing_after_an_unrelated_push_continues_at_the_caret(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "end x");
    assert_eq!(field(cx), "oldx", "the premise: End then a letter appends");
    push(
        &mut helm,
        only(&[session("a").title("old"), session("b").title("other")]),
        &[],
        cx,
    );
    keys(cx, "y");
    assert_eq!(field(cx), "oldxy", "the push left the caret where it was");
}

/// An untouched draft does not follow pushes while its save is in flight, since the value on screen must stay the one
/// sent. If the helm then refuses, saving again must not send back a title another client has since given the session
/// (SPEC.md: a GUI write must not undo a change an agent or the command line made). Specified: an untouched save,
/// a pushed rename to "agent's" while it is in flight, then the helm's refusal: the field holds "agent's".
#[gpui::test]
fn an_untouched_draft_whose_save_is_refused_takes_up_the_current_title(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "enter");
    let (id, _) = one_rename(&mut helm);
    push(
        &mut helm,
        only(&[session("a").title("agent's"), session("b")]),
        &[],
        cx,
    );
    assert_eq!(
        field(cx),
        "old",
        "the premise: the saving draft did not follow"
    );
    helm.reply(id, 500, Some(json!("try again")));
    settle(cx);
    assert_eq!(field(cx), "agent's");
}

// ---- Saving --------------------------------------------------------------------------------------------------------

/// The draft holds the helm's own bytes, so an untouched save must send them back unchanged, hidden characters
/// included (SPEC_impl.md, "Host-supplied text": an escaped presentation never becomes a saved value). While it is in
/// flight the busy rules hold, and a success closes the editor with focus back where it was (SPEC.md: "A successful
/// rename closes that editor"). Specified: Enter in the untouched field sends the raw title; Escape, and Cancel run by
/// an activation that beat the redraw disabling it, leave the editor open meanwhile; the helm's success closes it and
/// the terminal has focus again.
#[gpui::test]
fn an_untouched_save_sends_the_raw_title_and_success_closes_the_editor(cx: &mut TestAppContext) {
    let raw = "invoice\u{202E}fdp.exe";
    let (mut helm, cx) = open_main(cx, fleet(raw));
    keys(cx, RENAME);
    keys(cx, "enter");
    let (id, title) = one_rename(&mut helm);
    assert_eq!(
        title,
        json!(raw),
        "the raw bytes, not their escaped spelling"
    );

    keys(cx, "escape");
    assert!(
        any_open(cx),
        "Escape does nothing while the save is in flight"
    );
    let opened = editor(cx);
    cx.update(|window, cx| opened.update(cx, |editor, cx| editor.cancel(window, cx)));
    test_support::check(cx);
    assert!(any_open(cx), "nor does Cancel");

    helm.reply(id, 200, None);
    settle(cx);
    assert!(!any_open(cx), "the success closed the editor");
    assert!(terminal_focused(cx), "focus went back to the terminal");
}

/// The supervisor owns every rule about titles, so the client sends what the user typed with no trimming or
/// emptiness check (an empty title is a legal rename). Specified: a typed title is sent as typed; in a second editor, a
/// cleared field sends the empty title.
#[gpui::test]
fn a_typed_title_is_sent_verbatim_and_an_empty_one_too(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, SELECT_ALL);
    keys(cx, "r e n a m e d enter");
    let (id, title) = one_rename(&mut helm);
    assert_eq!(title, json!("renamed"));
    helm.reply(id, 200, None);
    settle(cx);

    keys(cx, RENAME);
    keys(cx, SELECT_ALL);
    keys(cx, "backspace enter");
    let (_, title) = one_rename(&mut helm);
    assert_eq!(
        title,
        json!(""),
        "the empty title is sent, not refused here"
    );
}

/// A click on the title field must leave focus in it. Rename's empty-space rule moves focus to the dialog itself, and
/// until the layer manager told a click into a field apart from one on empty space, every click into the field lost
/// focus that way, so the field could be typed into only from the keyboard. The layer manager's own test covers dialogs
/// in general; this pins it for the editor. Specified: after Tab moves focus off the field, a click on the field
/// focuses it again, and a typed letter reaches it.
#[gpui::test]
fn a_click_on_the_field_focuses_it(cx: &mut TestAppContext) {
    let (_helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "tab");
    assert!(!field_focused(cx), "the premise: focus left the field");
    test_support::click(cx, "rename-field");
    assert!(field_focused(cx), "the click focused the field");
    keys(cx, SELECT_ALL);
    keys(cx, "z");
    assert_eq!(field(cx), "z");
}

/// The old app drew its field read-only for a save's round trip, so the text on screen stayed the text sent and a
/// refusal left the user correcting it where they were. Undoing a stray edit instead would throw the caret to the
/// start of the title in exactly that case. Specified: after End, a letter and Enter, a letter typed while the save is
/// in flight changes nothing; after the helm's refusal, the next letter lands at the end.
#[gpui::test]
fn the_field_is_read_only_while_saving_and_keeps_its_caret(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "end x enter");
    let (id, title) = one_rename(&mut helm);
    assert_eq!(title, json!("oldx"), "the premise: the save is in flight");
    keys(cx, "y");
    assert_eq!(
        field(cx),
        "oldx",
        "the letter typed during the save changed nothing"
    );
    helm.reply(id, 400, Some(json!("no")));
    settle(cx);
    keys(cx, "z");
    assert_eq!(field(cx), "oldxz", "the caret stayed at the end");
}

/// SPEC.md: "Close uses the same cancellation and busy-state rules as Cancel", and Escape is a Close. A click on the
/// dialog's empty space gives the dialog itself focus (Rename's rule, so Enter then saves), and Escape from there must
/// not close the editor mid-save either: the draft and the refusal that may follow would be lost, leaving only the
/// row's copy. Specified: during a save, a click on the body's empty space puts focus on the dialog itself; Escape
/// leaves the editor open; after the helm's refusal the draft is still there, and Escape then closes the editor.
#[gpui::test]
fn escape_on_the_dialog_itself_does_not_close_it_while_saving(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "end x enter");
    let (id, _) = one_rename(&mut helm);
    // Inside the body's padding: empty space.
    let corner = test_support::bounds(cx, "rename-body").origin;
    cx.simulate_click(
        point(corner.x + px(3.), corner.y + px(3.)),
        Modifiers::none(),
    );
    test_support::check(cx);
    let on_dialog = cx.update(|window, cx| {
        layers::top_container(window, cx).is_some_and(|dialog| dialog.is_focused(window))
    });
    assert!(on_dialog, "the premise: the dialog itself has focus");

    keys(cx, "escape");
    assert!(any_open(cx), "Escape did not close the editor mid-save");
    helm.reply(id, 400, Some(json!("no")));
    settle(cx);
    assert_eq!(field(cx), "oldx", "the draft survived");
    keys(cx, "escape");
    assert!(!any_open(cx), "with the save settled, Escape closes it");
}

/// SPEC.md: "a refusal leaves its draft and error visible for correction". The field keeps a pasted line feed so that
/// the supervisor, which refuses one, is the one to say so; and its words may quote the title, so they must show
/// escaped. Specified: a pasted "fix\nbug" is sent with its line feed; the helm's refusal, which quotes a
/// right-to-left override, leaves the editor open with the pasted text in its focused field and shows the refusal with
/// the override as `<U+202E>`.
#[gpui::test]
fn a_refusal_keeps_the_draft_and_shows_the_helms_words_escaped(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("fix\nbug".into())));
    keys(cx, RENAME);
    keys(cx, SELECT_ALL);
    keys(cx, PASTE);
    keys(cx, "enter");
    let (id, title) = one_rename(&mut helm);
    assert_eq!(title, json!("fix\nbug"), "the line feed reached the helm");

    helm.reply(
        id,
        400,
        Some(json!("title \"\u{202E}x\" holds a control character")),
    );
    settle(cx);
    assert!(any_open(cx), "the refusal kept the editor open");
    assert_eq!(field(cx), "fix\nbug");
    assert!(field_focused(cx));
    let nodes = frame(cx);
    let refusal = node(&nodes, "rename-refusal").expect("the refusal is drawn");
    let label = refusal.label.as_deref().unwrap_or_default();
    assert!(
        label.contains("<U+202E>") && !label.contains('\u{202E}'),
        "the refusal is escaped: {label:?}"
    );
}

// ---- The session going away ----------------------------------------------------------------------------------------

/// SPEC.md: "A complete, authoritative listing that no longer contains the source disables submission without
/// discarding the draft". The open session's terminal goes with it, and focus must stay in the field rather than
/// follow it out (the old `modal-focus.spec.ts` case). Specified: after a typed letter, a complete listing without `a`
/// moves the selection on to `b` and shows the unavailable line; the field keeps its text and focus, and typing still
/// reaches it; Enter in the field, a click on Save, and Enter on the dialog itself (where that click leaves focus, as
/// on any empty space) send nothing.
#[gpui::test]
fn a_complete_listing_without_the_session_disables_save_and_keeps_the_draft(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    keys(cx, "x");
    let typed = field(cx);

    push(&mut helm, only(&[session("b")]), &["a"], cx);
    let selected = cx.update(|_, cx| crate::local_state::selected(cx));
    assert_eq!(
        selected.as_deref(),
        Some("b"),
        "the premise: a is proven gone"
    );
    let opened = editor(cx);
    assert!(cx.update(|_, cx| opened.read(cx).unavailable));
    assert!(
        cx.debug_bounds("rename-unavailable").is_some(),
        "it says why"
    );
    assert!(field_focused(cx), "focus stayed in the field");
    assert_eq!(field(cx), typed, "the draft is kept");
    keys(cx, "y");
    assert_ne!(field(cx), typed, "typing still reaches the field");

    // The button draws no accessible disabled state, so its being disabled is shown by what activating it does.
    keys(cx, "enter");
    test_support::click(cx, "rename-save");
    keys(cx, "enter");
    assert_eq!(renames(&mut helm), [], "Save sent nothing");
    assert!(any_open(cx), "and the editor stays open");
}

/// SPEC.md: "a filtered, truncated, failed, or stale listing is not proof that the session disappeared". Disabling
/// Save on such a listing would strand the user's edit behind a passing read error. Specified: after each of a
/// filtered, a truncated and an unavailable listing without `a`, Save stays enabled and Enter sends the rename (whose
/// refusal keeps the editor open for the next case).
#[gpui::test]
fn a_listing_that_proves_nothing_keeps_save_enabled(cx: &mut TestAppContext) {
    let (mut helm, cx) = open_main(cx, fleet("old"));
    keys(cx, RENAME);
    let counts = |matching, truncated| ListCounts {
        total: 2,
        matching,
        truncated,
    };
    let mut filtered = only(&[session("b")]);
    filtered.meta.counts = Some(counts(Some(1), false));
    let mut truncated = only(&[session("b")]);
    truncated.meta.counts = Some(counts(None, true));
    let mut failed = only(&[]);
    failed.meta.availability = Availability::Unavailable {
        error: "the helm could not read the list".into(),
    };
    for (what, state, remove) in [
        ("filtered", filtered, &["a"][..]),
        ("truncated", truncated, &["a"][..]),
        ("unavailable", failed, &["a", "b"][..]),
    ] {
        push(&mut helm, state, remove, cx);
        let opened = editor(cx);
        let unavailable = cx.update(|_, cx| opened.read(cx).unavailable);
        assert!(!unavailable, "{what}: Save stays enabled");
        keys(cx, "enter");
        let (id, _) = one_rename(&mut helm);
        helm.reply(id, 500, Some(json!("try again")));
        settle(cx);
        assert!(any_open(cx), "{what}: the editor stays open");
    }
}

//! Tests of the Add host dialog over the helm client's fake: what Add refuses before contacting anything, the probe it
//! sends and the key it runs under, what each kind of reply does to the dialog, and that closing stays possible and
//! harmless while a probe is out.
//!
//! The dialog is opened the way the host rail's Add host will open it: a button in a root view of the test's own,
//! whose click takes a mark and calls [`super::open`] with its own focus handle. The tests reach the open dialog
//! through [`Opened`], which only test builds record. Every step ends in one of the shared harness's step helpers, so
//! the focus and layer invariants are checked after each. Typed text avoids `-` and spaces, which GPUI's simulated
//! input reads as keystroke syntax rather than characters.
//!
//! The helpers are shared with the setup tests (`hosts/setup/tests.rs`), which go on from a probe's plan; the root
//! view draws the Hosts panel beside Add host for them, since a setup refused after the user moved on shows there.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{CollectionState, HostView};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Global, IntoElement, ParentElement as _, Render,
    Styled as _, TestAppContext, VisualTestContext, WeakEntity, Window, div, px, size,
};
use serde_json::{Value, json};

use super::{AddHostDialog, refused_before_contact};
use crate::components::{Button, HoverHelp, Role};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark};
use crate::hosts::HostsPanel;
use crate::hosts::tests::{aliased, connected, host, hosts};
use crate::operations::Work;
use crate::test_support::{self, click, focused, focused_id, frame, keys, settle, started};
use crate::{layers, root, session};

/// The dialog [`super::open`] opened last, recorded in test builds only, so a test (or a golden scene) can reach its
/// fields and its key.
pub(crate) struct Opened(pub(crate) WeakEntity<AddHostDialog>);

impl Global for Opened {}

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open Add host from: the host rail's Add host as a plain button with a focus handle the test
/// holds, after another control standing in for whatever held focus before, and the Hosts panel below them.
struct Rail {
    root: root::Attachment,
    add: FocusHandle,
    panel: Entity<HostsPanel>,
}

impl Render for Rail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let add = self.add.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .child(Button::new(
                "elsewhere",
                "Elsewhere",
                HoverHelp::tooltip("A control away from Add host"),
                Role::Button,
            ))
            .child(
                Button::new(
                    "rail-add-host",
                    "Add host",
                    HoverHelp::tooltip("Add host: connect a machine"),
                    Role::Button,
                )
                .focus(&self.add)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open(&add, &mark, window, cx);
                }),
            )
            .child(div().w(px(340.)).h(px(400.)).child(self.panel.clone()));
        root::wrap(content, &self.root, window, cx)
    }
}

/// The destination the helm already lists, under the alias `build-box`.
const ADDED: &str = "dev@buildbox";

/// The app over the fake helm listing the local host and `build-box` at [`ADDED`], in an active window, with Add host's
/// focus handle. Nothing is open yet.
fn open_rail(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, FocusHandle) {
    open_rail_with(cx, Vec::new())
}

/// [`open_rail`], with `more` collections in the helm's snapshot beside the hosts (the preference row, say).
pub(crate) fn open_rail_with(
    cx: &mut TestAppContext,
    more: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    let mut snapshot = vec![hosts(&[
        host(1, "local", connected()),
        aliased(host(2, "ssh", connected()), "build-box", ADDED),
    ])];
    snapshot.extend(more);
    helm.connect(snapshot);
    cx.run_until_parked();
    let add = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = add.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| Rail {
        root: root::attach(window, cx),
        add: held,
        panel: cx.new(HostsPanel::new),
    });
    window.simulate_resize(size(px(1600.), px(1000.)));
    settle(window);
    (helm, window, add)
}

/// Click Add host and check that the dialog opened with focus in the destination field, where every test starts
/// typing (the module docs: "Focus starts in the destination field").
pub(crate) fn open_dialog(cx: &mut VisualTestContext) {
    click(cx, "rail-add-host");
    assert!(any_open(cx), "the premise: Add host opened");
    assert!(
        field_focused(cx, |dialog| &dialog.destination),
        "the premise: focus starts in the destination field"
    );
}

pub(crate) fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// Type `text` into the focused field.
pub(crate) fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    test_support::check(cx);
}

/// The one command sent since the last call, which must be a probe: its id and body.
pub(crate) fn one_probe(helm: &mut FakeHelm) -> (u64, Value) {
    one_command(helm, Method::Post, "/api/hosts/probe")
}

/// The one command sent since the last call, which must be `method` on `path`: its id and body.
pub(crate) fn one_command(helm: &mut FakeHelm, method: Method, path: &str) -> (u64, Value) {
    let sent: Vec<FakeCommand> = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!((sent[0].method, sent[0].path.as_str()), (method, path));
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// The open dialog's destination field as it reads now; panics if no dialog was opened.
pub(crate) fn destination(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("a dialog is open");
        dialog.read(cx).destination.input().text(cx).to_string()
    })
}

/// The open dialog's alias field as it reads now; panics if no dialog was opened.
pub(crate) fn alias(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("a dialog is open");
        dialog.read(cx).alias.input().text(cx).to_string()
    })
}

/// The open dialog's host-add key, which its probe runs under and its setup continues under.
pub(crate) fn dialog_key(cx: &mut VisualTestContext) -> crate::operations::OpKey {
    cx.update(|_, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("a dialog is open");
        dialog.read(cx).key.clone()
    })
}

/// The work in flight on the open dialog's key.
pub(crate) fn probe_work(cx: &mut VisualTestContext) -> Option<Work> {
    cx.update(|_, cx| {
        let key = cx.global::<Opened>().0.upgrade()?.read(cx).key.clone();
        session::current_operations(cx)?.read(cx).in_flight(&key)
    })
}

/// The accessible name of what is drawn under `id`, if anything is.
pub(crate) fn label(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// Whether the open dialog's alias field holds focus.
pub(crate) fn alias_focused(cx: &mut VisualTestContext) -> bool {
    field_focused(cx, |dialog| &dialog.alias)
}

/// Whether the open dialog's field that `field` picks holds focus.
fn field_focused(cx: &mut VisualTestContext, field: fn(&AddHostDialog) -> &DraftInput) -> bool {
    cx.update(|window, cx| {
        let dialog = cx.global::<Opened>().0.upgrade().expect("a dialog is open");
        field(dialog.read(cx))
            .input()
            .focus_handle(cx)
            .is_focused(window)
    })
}

/// Kit's select-all in a text field on this platform.
const SELECT_ALL: &str = if cfg!(target_os = "macos") {
    "cmd-a"
} else {
    "ctrl-a"
};

/// The dialog's error line, as assistive technology reads it.
pub(crate) fn error_line(cx: &mut VisualTestContext) -> Option<String> {
    label(cx, "add-host-error")
}

// ---- Before the probe is sent --------------------------------------------------------------------------------------

/// SPEC.md, "Topology": "Adding a destination that is already a host entry is refused, naming that entry, before
/// anything contacts the destination". The helm refuses it too, but only once the probe gets a planning slot; the app's
/// own check answers at once, which is the only reason it exists (the module docs). Specified: with `build-box` listed
/// at `dev@buildbox`, typing that destination and pressing Enter (Add) sends nothing and shows the helm's own words
/// naming `build-box`; the destination stays in its field; a destination that differs only by case is not a duplicate
/// (exact as typed) and is probed.
#[gpui::test]
fn a_destination_already_added_is_refused_naming_it_without_a_probe(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail(cx);
    open_dialog(cx);
    type_text(cx, ADDED);
    // Enter in the field, so focus stays there for the retyping below.
    keys(cx, "enter");
    assert_eq!(helm.commands(), [], "nothing was sent");
    assert_eq!(
        error_line(cx).as_deref(),
        Some(
            "this host is already added as \"build-box\"; set it up again from that host's own row"
        )
    );
    assert_eq!(destination(cx), ADDED, "the destination is kept");

    keys(cx, SELECT_ALL);
    type_text(cx, "dev@BUILDBOX");
    keys(cx, "enter");
    let (_, body) = one_probe(&mut helm);
    assert_eq!(body["target"]["destination"], "dev@BUILDBOX");
    assert_eq!(error_line(cx), None, "starting the probe cleared the line");
}

/// The alias travels as the new entry's alias, and the old app's client-side shape rules refuse a malformed one at
/// once, in the old app's words, before any probe is sent. Blank means no alias, which the helm reads from `null`.
/// Specified: a control character or 65 characters is refused with the old app's sentence and 64 is allowed;
/// in the dialog, an alias of only spaces is sent as `null`, and Enter in the alias field with 65 characters after them
/// sends nothing and says so.
#[gpui::test]
fn a_blank_alias_is_sent_as_none_and_a_malformed_one_is_refused_before_contact(
    cx: &mut TestAppContext,
) {
    let none: [&HostView; 0] = [];
    let refusal = |alias: &str| {
        refused_before_contact("dev@newbox", alias, none)
            .map(|line| line.accessible_name().to_string())
    };
    assert_eq!(
        refusal("bad\u{7}name").as_deref(),
        Some("Host alias must not contain control characters")
    );
    assert_eq!(refusal(&"a".repeat(64)), None);
    assert_eq!(
        refusal(&"a".repeat(65)).as_deref(),
        Some("Host alias must be at most 64 characters")
    );

    let (mut helm, cx, _add) = open_rail(cx);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    keys(cx, "tab");
    assert!(alias_focused(cx), "the premise: Tab reached the alias");
    keys(cx, "space space enter");
    let (id, body) = one_probe(&mut helm);
    assert_eq!(
        body["new_entry"],
        json!({ "alias": null }),
        "spaces alone are no alias"
    );
    helm.reply(
        id,
        200,
        Some(json!({ "result": "manual", "reason": "no systemd" })),
    );
    settle(cx);

    assert!(
        alias_focused(cx),
        "the premise: the alias field kept focus through the probe"
    );
    type_text(cx, &"a".repeat(65));
    keys(cx, "enter");
    assert_eq!(helm.commands(), [], "Enter in the alias sent nothing");
    assert_eq!(
        error_line(cx).as_deref(),
        Some("Host alias must be at most 64 characters")
    );
}

/// What Add sends is the helm's contract for a new entry: the destination exactly as typed and a `new_entry` with the
/// alias, which is what makes the helm refuse an existing entry rather than set it up again. It runs as the operation
/// store's work under the dialog's own key, so a second Add while it is out sends nothing and the helm never holds two
/// one-use plans for one dialog. Specified: with `dev@newbox` and the alias `newbox1`, Add sends exactly `{"target":
/// {"kind": "ssh", "destination": "dev@newbox"}, "new_entry": {"alias": "newbox1"}}`; the dialog's key holds a probe
/// and Add reads "Checking…"; Add and Enter meanwhile send nothing more.
#[gpui::test]
fn add_sends_the_probe_with_its_new_entry_under_the_dialogs_key(cx: &mut TestAppContext) {
    let (mut helm, cx, _add) = open_rail(cx);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    keys(cx, "tab");
    type_text(cx, "newbox1");
    click(cx, "add-host-submit");
    let (_, body) = one_probe(&mut helm);
    assert_eq!(
        body,
        json!({
            "target": { "kind": "ssh", "destination": "dev@newbox" },
            "new_entry": { "alias": "newbox1" },
        })
    );
    assert_eq!(probe_work(cx), Some(Work::Probe));
    assert_eq!(label(cx, "add-host-submit").as_deref(), Some("Checking…"));

    click(cx, "add-host-submit");
    keys(cx, "enter");
    assert_eq!(
        helm.commands(),
        [],
        "a second Add while the probe is out sent nothing"
    );
}

// ---- The outcomes --------------------------------------------------------------------------------------------------

/// SPEC.md, "Topology": a running supervisor is registered as it is. The dialog has nothing more to ask, and the new
/// host shows in the pushed hosts list; leaving the dialog open would invite a second Add that the helm refuses as a
/// duplicate. Run under macOS's click rule, where the click on Add host leaves it unfocused, so focus gets back to it
/// only because the dialog names it as where closing returns focus (nothing focuses it before the dialog opens).
/// Specified: a `discovered` reply closes the dialog and focus goes back to Add host, its opener.
#[gpui::test]
fn a_discovered_host_closes_the_dialog(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    helm.reply(
        id,
        200,
        Some(json!({ "result": "discovered", "host_id": 3, "build_version": "0.16.0", "host_identity": "id-3" })),
    );
    settle(cx);
    assert!(!any_open(cx), "the dialog closed");
    assert!(
        focused(cx, &add),
        "focus went back to Add host, which the click never focused"
    );
    assert_eq!(probe_work(cx), None, "the key is free");
}

/// Every answer short of a registered host leaves the user something to correct or to try again, so it shows in the
/// dialog with the destination still there (the old dialog's error line, and SPEC.md's bounded-host-wait rule that a
/// timeout is an ordinary reply). Specified: for a `manual` reply, a 409 refusal and the 504 that says the check
/// carries on, the dialog stays open with the helm's words in its error line, `dev@newbox` still in the destination
/// field, the key free and Add reading "Add" again; a 200 whose body is no probe reply says the reply could not be
/// read.
#[gpui::test]
fn manual_setup_refusals_and_the_timeout_show_in_the_dialog_with_the_destination_kept(
    cx: &mut TestAppContext,
) {
    let timeout = "the check did not finish within 20 seconds; it carries on, and if it finds a running Farhelm \
                   supervisor the host is added to the hosts list, otherwise try again";
    let replies = [
        (
            200,
            json!({ "result": "manual", "reason": "this host has no systemd user manager" }),
            "this host has no systemd user manager",
        ),
        (
            409,
            json!("a host is already displayed as \"newbox\""),
            "a host is already displayed as \"newbox\"",
        ),
        (504, json!(timeout), timeout),
    ];
    let (mut helm, cx, _add) = open_rail(cx);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    for (status, body, shown) in replies {
        click(cx, "add-host-submit");
        let (id, _) = one_probe(&mut helm);
        helm.reply(id, status, Some(body));
        settle(cx);
        assert!(any_open(cx), "{status}: the dialog stays");
        assert_eq!(error_line(cx).as_deref(), Some(shown), "{status}");
        assert_eq!(
            destination(cx),
            "dev@newbox",
            "{status}: the destination is kept"
        );
        assert_eq!(probe_work(cx), None, "{status}: the key is free");
        assert_eq!(label(cx, "add-host-submit").as_deref(), Some("Add"));
    }

    // A reply the app cannot read may still have registered a host, so it says what happened rather than nothing.
    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    helm.reply(id, 200, Some(json!({ "result": "something newer" })));
    settle(cx);
    let line = error_line(cx).unwrap_or_default();
    assert!(
        line.starts_with("The helm accepted the host probe, but its reply could not be read: "),
        "{line}"
    );
}

// ---- Closing while a probe is out ----------------------------------------------------------------------------------

/// The old dialog never held the user behind its probe (`provisioning.spec.ts`, "the add dialog's cancel stays live
/// while discovery is pending"), and a probe can take the helm's whole host-wait bound. Closing drops only the dialog's
/// wait: the probe stays the operation store's work, and its reply then reaches nobody. Specified: with a probe out,
/// Escape closes the dialog and focus goes back to Add host while the store still holds the probe; in a second
/// opening, Cancel does the same; each probe's `discovered` reply then opens nothing and leaves focus on Add host.
#[gpui::test]
fn escape_and_cancel_close_the_dialog_while_a_probe_is_out(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail(cx);
    for close in ["escape", "add-host-cancel"] {
        open_dialog(cx);
        type_text(cx, "dev@newbox");
        click(cx, "add-host-submit");
        let (id, _) = one_probe(&mut helm);
        let in_flight = probe_work(cx);
        if close == "escape" {
            keys(cx, "escape");
        } else {
            click(cx, "add-host-cancel");
        }
        assert!(!any_open(cx), "{close} closed the dialog");
        assert!(focused(cx, &add), "{close}: focus went back to Add host");
        assert_eq!(
            in_flight,
            Some(Work::Probe),
            "{close}: the premise: the probe was out"
        );

        helm.reply(
            id,
            200,
            Some(json!({ "result": "discovered", "host_id": 3, "build_version": "0.16.0", "host_identity": null })),
        );
        settle(cx);
        assert!(!any_open(cx), "{close}: the reply opened nothing");
        assert!(focused(cx, &add), "{close}: and moved no focus");
    }
}

/// SPEC.md, "Keyboard focus": once the user has moved on, a result moves no focus and opens nothing. A line in the
/// dialog that is still open does neither, so it shows; and a reply after the dialog closed must not bring anything
/// back, which is what the setup confirmation will rely on. Specified: Shift+Tab during a probe moves focus from Add to
/// Cancel; a `manual` reply then shows its line and focus stays on Cancel; a second probe whose dialog was closed with
/// Escape gets a `provisionable` reply, after which no layer is open and Add host keeps focus.
#[gpui::test]
fn an_answer_after_the_user_moved_on_opens_nothing_and_moves_no_focus(cx: &mut TestAppContext) {
    let (mut helm, cx, add) = open_rail(cx);
    open_dialog(cx);
    type_text(cx, "dev@newbox");
    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    keys(cx, "shift-tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("add-host-cancel"),
        "the premise: the user moved on from Add to Cancel"
    );
    helm.reply(
        id,
        200,
        Some(json!({ "result": "manual", "reason": "no systemd" })),
    );
    settle(cx);
    assert_eq!(error_line(cx).as_deref(), Some("no systemd"));
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("add-host-cancel"),
        "the reply moved no focus"
    );

    click(cx, "add-host-submit");
    let (id, _) = one_probe(&mut helm);
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: the dialog closed");
    helm.reply(
        id,
        200,
        Some(json!({
            "result": "provisionable", "probe_id": "p-2", "plan": {}, "confirmation": "the plan",
            "steps": { "lead": "lead", "steps": [], "kept": null },
        })),
    );
    settle(cx);
    assert!(!any_open(cx), "nothing opened for the late plan");
    assert!(focused(cx, &add), "focus stayed on Add host");
}

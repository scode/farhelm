//! Tests of SSH help opened from its own entry point ([`super::open`], the ? menu's): the one read it sends, what each
//! reply shows, what Copy copies, closing, the body's keyboard stop while it holds no Copy, where focus goes as the reply
//! lands, a reply reaching only its own opening, and a long command's wrap. The Add host entry point, where the read
//! carries the typed destination and closing returns to the draft, is tested with Add host (`hosts/add/tests.rs`), whose
//! root view it needs.
//!
//! The dialog is opened the way the ? menu will open it, with the ? button as the opener that closing returns focus to
//! (the menu item goes with its popover, so it cannot be the return target): one button in a root view of the test's
//! own stands in for both the ? button and its menu's item, and its click takes a mark and calls [`super::open`] with
//! its own focus handle. Every step ends in one of the shared harness's step helpers, so the focus and layer invariants
//! are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use gpui::{
    Context, FocusHandle, IntoElement, ParentElement as _, Render, Styled as _, TestAppContext,
    VisualTestContext, Window, div, px, size,
};
use serde_json::{Value, json};

use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::hosts::tests::{connected, host, hosts};
use crate::root;
use crate::test_support::{
    self, any_open, bounds, click, focused, focused_id, frame, keys, label, one_command, settle,
    started,
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The root view the tests open SSH help from: the ? button (standing in for its menu's item too) as a plain button with
/// a focus handle the test holds,
/// after another control standing in for whatever held focus before.
struct HelpBar {
    root: root::Attachment,
    toggle: FocusHandle,
}

impl Render for HelpBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = self.toggle.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .child(Button::new(
                "elsewhere",
                "Elsewhere",
                HoverHelp::tooltip("A control away from the ? button"),
                Role::Button,
            ))
            .child(
                Button::new(
                    "help-toggle",
                    "Help",
                    HoverHelp::tooltip("Help: feedback, docs and SSH help"),
                    Role::Button,
                )
                .focus(&self.toggle)
                .on_click(move |_, window, cx| {
                    let mark = Mark::take(cx);
                    super::open(&toggle, &mark, window, cx);
                }),
            );
        root::wrap(content, &self.root, window, cx)
    }
}

/// The app over the fake helm listing the local host only (the reply, not the store, decides what SSH help lists), in
/// a window big enough for the dialog, with the ? button's focus handle. Nothing is open yet.
fn open_help_bar(cx: &mut TestAppContext) -> (FakeHelm, &mut VisualTestContext, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    helm.connect(vec![hosts(&[host(1, "local", connected())])]);
    cx.run_until_parked();
    let toggle = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let held = toggle.clone();
    let (_, window) = test_support::open_view(cx, move |window, cx| HelpBar {
        root: root::attach(window, cx),
        toggle: held,
    });
    window.simulate_resize(size(px(1600.), px(1200.)));
    settle(window);
    (helm, window, toggle)
}

/// Click the ? button's stand-in and check the read it sends (the route, and no destination, since the menu has no
/// draft), leaving it unanswered: the read's id, for a test that answers it later or never.
fn open_unanswered(helm: &mut FakeHelm, cx: &mut VisualTestContext) -> u64 {
    click(cx, "help-toggle");
    assert!(any_open(cx), "the premise: SSH help opened");
    let (id, sent) = one_command(helm, Method::Post, "/api/ssh-connections");
    assert_eq!(
        sent,
        json!({ "destination": null }),
        "the menu sends no destination"
    );
    id
}

/// [`open_unanswered`], then answer the read with `status` and `body`.
fn open_with_reply(helm: &mut FakeHelm, cx: &mut VisualTestContext, status: u16, body: Value) {
    let id = open_unanswered(helm, cx);
    helm.reply(id, status, Some(body));
    settle(cx);
}

/// A reply listing one host, `build-box`, with `sessions` and `setup` as its two commands (`None`: one the helm could
/// not compose). Each command brings a Copy into the body, which is what takes the body's own stop away.
fn one_host(sessions: Option<&str>, setup: Option<&str>) -> Value {
    json!({
        "unavailable": [],
        "hosts": [{ "name": "build-box", "destination": "build-box", "sessions": sessions, "setup": setup }],
        "extra": null,
    })
}

/// The element id of the body's own stop ([`crate::components::DialogFrame::scroll_stop`]): drawn only while the body
/// holds nothing focusable.
const BODY_STOP: &str = "ssh-help-body-stop";

/// A registered host's commands as the helm composes them, with a raw control character in the destination so the
/// screen's escape and the copied bytes can be told apart.
const SESSIONS: &str = "ssh -N -f -o ControlPath=/state/s.sock -- 'dev@build\u{7}box'";
const SETUP: &str = "ssh -N -f -o ControlPath=/state/p.sock -- 'dev@build\u{7}box'";

// ---- Tests ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Feedback": SSH help "shows the two shell commands for each registered SSH host ... each with a copy
/// button", composed by the helm; what the user pastes must be the helm's exact bytes, while the screen escapes what a
/// host could hide (SPEC_impl.md, "Host-supplied text"). The read goes out as the dialog opens, and closing returns to
/// the control that opened it, also under macOS's click rule, where the click leaves that control unfocused.
/// Specified: opening sends `POST /api/ssh-connections` with `{"destination": null}`, shows "Loading connection
/// commands…" and starts on the footer's Close; the reply's host shows its name, its destination and both commands
/// with the control character escaped; each Copy is named for its connection and copies the raw command; Escape closes
/// the dialog and focus goes back to the ? button.
#[gpui::test]
fn the_menu_entry_shows_each_hosts_two_commands_and_copies_their_exact_bytes(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, toggle) = open_help_bar(cx);
    cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    click(cx, "help-toggle");
    assert_eq!(
        label(cx, "ssh-help-loading").as_deref(),
        Some("Loading connection commands…")
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("ssh-help-done"),
        "focus starts on Close"
    );
    let (id, sent) = one_command(&mut helm, Method::Post, "/api/ssh-connections");
    assert_eq!(sent, json!({ "destination": null }));
    helm.reply(
        id,
        200,
        Some(json!({
            "unavailable": [],
            "hosts": [{
                "name": "build-box", "destination": "dev@build\u{7}box",
                "sessions": SESSIONS, "setup": SETUP,
            }],
            "extra": null,
        })),
    );
    settle(cx);
    assert_eq!(label(cx, "ssh-help-loading"), None, "the loading line went");
    assert_eq!(
        label(cx, "ssh-help-host-0-name").as_deref(),
        Some("build-box")
    );
    assert_eq!(
        label(cx, "ssh-help-host-0-destination").as_deref(),
        Some("dev@build<U+0007>box")
    );
    assert_eq!(
        label(cx, "ssh-help-host-0-sessions").as_deref(),
        Some(SESSIONS.replace('\u{7}', "<U+0007>").as_str())
    );
    assert_eq!(label(cx, "ssh-help-no-hosts"), None, "a host is listed");

    for (copy, name, command) in [
        (
            "ssh-help-host-0-sessions-copy",
            "Copy sessions connection",
            SESSIONS,
        ),
        (
            "ssh-help-host-0-setup-copy",
            "Copy setup and updates connection",
            SETUP,
        ),
    ] {
        assert_eq!(label(cx, copy).as_deref(), Some(name));
        click(cx, copy);
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some(command),
            "{name} copies the raw command"
        );
    }

    keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed SSH help");
    assert!(focused(cx, &toggle), "focus went back to the ? button");
    assert_eq!(
        helm.commands(),
        [],
        "nothing else was sent: SSH help starts nothing"
    );
}

/// The empty and failed states must still leave the user something to read and a way out: SPEC.md has SSH help point
/// at Add host when no SSH host is registered, and the old spec closed it during a failed read
/// (`ssh-help.spec.ts`, "ssh help names the empty state and remains dismissible during a failed read"). Specified: a
/// reply with no hosts says "No SSH hosts are registered. Use Add host in the hosts panel to register one." and its
/// footer Close closes it with focus back on the ? button; a second opening refused with a 500 shows the helm's words
/// in the error line, and the header's close control closes it.
#[gpui::test]
fn with_no_ssh_hosts_it_points_at_add_host_and_a_refusal_shows_in_its_place(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, toggle) = open_help_bar(cx);
    open_with_reply(
        &mut helm,
        cx,
        200,
        json!({ "unavailable": [], "hosts": [], "extra": null }),
    );
    assert_eq!(
        label(cx, "ssh-help-no-hosts").as_deref(),
        Some("No SSH hosts are registered. Use Add host in the hosts panel to register one.")
    );
    assert_eq!(
        label(cx, "ssh-help-no-destination"),
        None,
        "the menu asks for no destination"
    );
    click(cx, "ssh-help-done");
    assert!(!any_open(cx), "Close closed SSH help");
    assert!(focused(cx, &toggle), "focus went back to the ? button");

    open_with_reply(&mut helm, cx, 500, json!("help unavailable"));
    assert_eq!(
        label(cx, "ssh-help-error").as_deref(),
        Some("help unavailable")
    );
    click(cx, "ssh-help-close");
    assert!(!any_open(cx), "the header's close control closed SSH help");
}

/// SPEC.md: when the helm's state path is too long for a socket, holds `${` or is not UTF-8, "the dialog names the
/// affected connection and reason once at the top ... instead of commands for that connection". The helm omits the
/// command and words the reason; the dialog must show the reason once, not per host, and offer no Copy for a command
/// that does not exist. Specified: with one unavailable line and two hosts lacking their setup command, the line shows
/// once (as `ssh-help-unavailable-0`), both hosts show their Sessions command and Copy, and neither has a setup Copy.
#[gpui::test]
fn an_unavailable_connection_is_named_once_at_the_top_and_offers_no_copy(cx: &mut TestAppContext) {
    let reason = "setup and updates connection: the state path /state is longer than 90 bytes";
    let host = |name: &str| json!({ "name": name, "destination": name, "sessions": format!("ssh -- {name}"), "setup": null });
    let (mut helm, cx, _toggle) = open_help_bar(cx);
    open_with_reply(
        &mut helm,
        cx,
        200,
        json!({ "unavailable": [reason], "hosts": [host("one"), host("two")], "extra": null }),
    );
    assert_eq!(label(cx, "ssh-help-unavailable-0").as_deref(), Some(reason));
    assert_eq!(label(cx, "ssh-help-unavailable-1"), None, "named once");
    for host in ["0", "1"] {
        let sessions = format!("ssh-help-host-{host}-sessions-copy");
        let setup = format!("ssh-help-host-{host}-setup-copy");
        assert!(
            label(cx, &sessions).is_some(),
            "host {host} offers its Sessions Copy"
        );
        assert_eq!(label(cx, &setup), None, "host {host} offers no setup Copy");
    }
}

// ---- Keyboard reach and focus as the reply lands -------------------------------------------------------------------

/// The node drawn under `id` in a fresh frame, by the frame's own record: the command sentences and Copy buttons carry
/// an id but no debug selector, so [`bounds`] cannot find them.
fn drawn(cx: &mut VisualTestContext, id: &str) -> gpui::DriverNode {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("{id} is drawn"))
}

/// SPEC.md, "Reaching everything by keyboard": "A region whose content holds nothing focusable gets its own tab stop
/// with an accessible name." SSH help's body holds a control only once the helm's commands arrive, each with its Copy;
/// until then, and whenever the reply brings no command, the body is text that the keyboard could reach no other way.
/// Specified, for each text-only state (a refused read, no SSH host, a host the helm composed no command for, and the
/// read still out): from Close, where focus starts, Tab reaches the header's close control and then the body's stop, a
/// region named "SSH connection help" as the old app's was; the state's line is drawn before and after the keys (the
/// stop scrolls, it does not touch the read); Escape closes SSH help.
#[gpui::test]
fn every_text_only_state_gives_the_body_a_stop_that_tab_reaches(cx: &mut TestAppContext) {
    // The state, the reply to its read (none: the read stays out), and the line only that state draws. The read left
    // out goes last: the fake helm answers reads only in the order they were sent.
    let cases = [
        (
            "error",
            Some((500, json!("help unavailable"))),
            "ssh-help-error",
        ),
        (
            "no hosts",
            Some((
                200,
                json!({ "unavailable": [], "hosts": [], "extra": null }),
            )),
            "ssh-help-no-hosts",
        ),
        (
            "no commands",
            Some((200, one_host(None, None))),
            "ssh-help-host-0-name",
        ),
        ("loading", None, "ssh-help-loading"),
    ];
    let (mut helm, cx, _toggle) = open_help_bar(cx);
    for (state, reply, line) in cases {
        let id = open_unanswered(&mut helm, cx);
        if let Some((status, body)) = reply {
            helm.reply(id, status, Some(body));
            settle(cx);
        }
        assert!(
            label(cx, line).is_some(),
            "{state}: the premise: its line is drawn"
        );
        keys(cx, "tab");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("ssh-help-close"),
            "{state}: Tab wraps from Close to the header's close control"
        );
        keys(cx, "tab");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(BODY_STOP),
            "{state}: Tab reaches the body's stop"
        );
        assert_eq!(
            label(cx, BODY_STOP).as_deref(),
            Some("SSH connection help"),
            "{state}: the stop's name"
        );
        assert!(
            label(cx, line).is_some(),
            "{state}: the keys left the body as it was"
        );
        keys(cx, "escape");
        assert!(!any_open(cx), "{state}: Escape closed SSH help");
    }
}

/// Where the user is in SSH help when the helm's commands arrive ([`the_commands_arriving_leave_focus_usable`]).
#[derive(Clone, Copy, Debug)]
enum Before {
    /// On the body's stop, which the arriving Copy buttons take away.
    BodyStop,
    /// On the header's close control, which stays.
    HeaderClose,
    /// On Close, after a press on the veil outside the dialog.
    VeilPress,
}

/// The old app's "SSH command arrival keeps usable focus from {body, dismiss, backdrop}": the read is answered while the
/// user is already somewhere in the dialog, and the Copy buttons the reply brings take the body's stop away (SPEC.md,
/// "Reaching everything by keyboard", gives the stop only to a region holding nothing focusable). Focus must end on a
/// control the next key acts on, never on the dialog itself or anything behind it, and a control that stays keeps it.
/// From the body's stop, focus goes to the closest control left (SPEC.md, "Opening and closing": "When the focused
/// control goes away, focus moves to the closest remaining control in the same dialog"), here the Sessions Copy, where
/// the old app went to Close; a veil press never takes focus off Close, where the old backdrop click left nothing
/// focused. Specified: each row's place before the reply, the control holding focus once the commands are drawn, and
/// where the next key leaves focus (or that Enter closed SSH help, with focus back on the ? button).
#[gpui::test]
fn the_commands_arriving_leave_focus_usable(cx: &mut TestAppContext) {
    // Where the user is and the control holding focus there, the control holding focus once the commands are drawn,
    // the next key, and where that key leaves focus (`None`: SSH help closed, focus on the ? button).
    let cases = [
        (
            Before::BodyStop,
            BODY_STOP,
            "ssh-help-host-0-sessions-copy",
            "tab",
            Some("ssh-help-host-0-setup-copy"),
        ),
        (
            Before::HeaderClose,
            "ssh-help-close",
            "ssh-help-close",
            "tab",
            Some("ssh-help-host-0-sessions-copy"),
        ),
        (
            Before::VeilPress,
            "ssh-help-done",
            "ssh-help-done",
            "enter",
            None,
        ),
    ];
    let (mut helm, cx, toggle) = open_help_bar(cx);
    for (before, there, after, key, then) in cases {
        let id = open_unanswered(&mut helm, cx);
        match before {
            Before::BodyStop => keys(cx, "tab tab"),
            Before::HeaderClose => keys(cx, "tab"),
            Before::VeilPress => {
                assert!(
                    !bounds(cx, "ssh-help").intersects(&bounds(cx, "elsewhere")),
                    "{before:?}: the premise: the press lands on the veil, beside the dialog"
                );
                click(cx, "elsewhere");
            }
        }
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(there),
            "{before:?}: where the user is as the reply lands"
        );
        helm.reply(id, 200, Some(one_host(Some(SESSIONS), Some(SETUP))));
        settle(cx);
        assert_eq!(
            label(cx, BODY_STOP),
            None,
            "{before:?}: the body's stop went"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some(after),
            "{before:?}: focus once the commands arrived"
        );
        keys(cx, key);
        if let Some(then) = then {
            assert_eq!(
                focused_id(cx).as_deref(),
                Some(then),
                "{before:?}: {key} acted as it does anywhere"
            );
            keys(cx, "escape");
        }
        assert!(!any_open(cx), "{before:?}: SSH help closed");
        assert!(
            focused(cx, &toggle),
            "{before:?}: focus went back to the ? button"
        );
    }
}

/// The module docs promise "its reply reaches only that opening": SSH help reads once per opening, and a reply that
/// arrives after its dialog closed must not fill in, or move focus in, a dialog opened since, which is waiting on its own
/// read (the old app's "SSH help closed before its reply leaves a reopened region independent"). Specified: SSH help
/// opened with its read held and closed; reopened, with its own read held and focus on the header's close control; the
/// first read's reply leaves the reopened dialog loading, with focus where it was, and Tab still reaches its body's
/// stop; the second read's reply then fills the dialog with `ssh opening-1`.
#[gpui::test]
fn a_reply_to_a_closed_opening_leaves_a_reopened_one_alone(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle) = open_help_bar(cx);
    let first = open_unanswered(&mut helm, cx);
    keys(cx, "escape");
    assert!(!any_open(cx), "the premise: the first opening closed");

    let second = open_unanswered(&mut helm, cx);
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("ssh-help-close"),
        "the premise: on the header's close control"
    );
    let opening = |n: u8| {
        one_host(
            Some(&format!("ssh opening-{n}.example.invalid")),
            Some(SETUP),
        )
    };
    helm.reply(first, 200, Some(opening(0)));
    settle(cx);
    assert_eq!(
        label(cx, "ssh-help-loading").as_deref(),
        Some("Loading connection commands…"),
        "the reopened dialog still waits on its own read"
    );
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("ssh-help-close"),
        "focus stayed where it was"
    );
    keys(cx, "tab");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(BODY_STOP),
        "Tab still reaches the stop"
    );

    helm.reply(second, 200, Some(opening(1)));
    settle(cx);
    assert_eq!(
        label(cx, "ssh-help-host-0-sessions").as_deref(),
        Some("ssh opening-1.example.invalid"),
        "the reopened dialog's own reply fills it"
    );
}

/// SPEC.md, "Feedback": SSH help shows the helm's commands with a Copy for each, and a command (the state path twice,
/// quoted options, the destination) is often longer than the dialog is wide. The old app wrapped it inside its well
/// rather than running past the dialog's edge or hiding its end behind a horizontal scroll ("long SSH commands wrap
/// while Copy preserves their exact bytes"; the copied bytes are pinned by
/// `the_menu_entry_shows_each_hosts_two_commands_and_copies_their_exact_bytes`). Specified, for two commands that each
/// hold a run of several hundred characters without a space: each command's text is drawn inside the dialog, left of
/// its Copy, over more than one line of the command's text, and its Copy lies inside the dialog too.
#[gpui::test]
fn a_long_command_wraps_inside_the_dialog(cx: &mut TestAppContext) {
    let sessions = format!(
        "ssh -o ControlPath=/tmp/{} -N 'user@host.example.invalid'",
        "control-path-".repeat(35)
    );
    let setup = format!(
        "ssh -o ControlMaster=yes -o ControlPath=/tmp/{} -N 'user@host.example.invalid'",
        "setup-path-".repeat(35)
    );
    let (mut helm, cx, _toggle) = open_help_bar(cx);
    open_with_reply(&mut helm, cx, 200, one_host(Some(&sessions), Some(&setup)));
    let dialog = bounds(cx, "ssh-help");
    let one_line = px(super::COMMAND_TEXT * 1.5);
    for kind in ["sessions", "setup"] {
        let text = drawn(cx, &format!("ssh-help-host-0-{kind}")).bounds;
        let copy = drawn(cx, &format!("ssh-help-host-0-{kind}-copy")).bounds;
        assert!(
            text.left() >= dialog.left() && text.right() <= copy.left(),
            "{kind}: the command is drawn inside the dialog, left of its Copy: {text:?}, {copy:?}, {dialog:?}"
        );
        assert!(
            copy.right() <= dialog.right(),
            "{kind}: its Copy is inside the dialog: {copy:?}, {dialog:?}"
        );
        assert!(
            text.size.height > one_line,
            "{kind}: the command wraps onto more than one line: {text:?}"
        );
    }
}

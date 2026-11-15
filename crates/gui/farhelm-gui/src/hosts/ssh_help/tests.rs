//! Tests of SSH help opened from its own entry point ([`super::open`], the ? menu's): the one read it sends, what each
//! reply shows, what Copy copies, and closing. The Add host entry point, where the read carries the typed destination
//! and closing returns to the draft, is tested with Add host (`hosts/add/tests.rs`), whose root view it needs.
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
    self, any_open, click, focused, focused_id, keys, label, one_command, settle, started,
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

/// Click the ? button's stand-in and answer the read it sends with `status` and `body`, after checking the read: the route, and
/// no destination, since the menu has no draft.
fn open_with_reply(helm: &mut FakeHelm, cx: &mut VisualTestContext, status: u16, body: Value) {
    click(cx, "help-toggle");
    assert!(any_open(cx), "the premise: SSH help opened");
    let (id, sent) = one_command(helm, Method::Post, "/api/ssh-connections");
    assert_eq!(
        sent,
        json!({ "destination": null }),
        "the menu sends no destination"
    );
    helm.reply(id, status, Some(body));
    settle(cx);
}

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

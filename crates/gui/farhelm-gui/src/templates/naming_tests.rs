//! Tests of naming a quick-launched session: in the real main view over the helm client's fake, a quick launch's
//! success that opened its session puts the Rename editor over it in naming mode, and nothing else does.
//!
//! The window and its templates are the dispatcher tests' (`super::dispatch_tests`): with `go` marked, the ⚡ chord
//! launches a command unseen, and the test answers its create with the new session as the helm lists it, pushed before
//! the reply as the UI channel does. A rename the editor sends is read off the fake helm's commands.

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{ListCounts, Preferences};
use gpui::{TestAppContext, VisualTestContext};
use serde_json::json;

use super::dispatch_tests::{chord, creates, push, window};
use crate::local_state;
use crate::session_list::tests::{listing, preferences_state, session};
use crate::test_support::{self, any_open, keys, settle, terminal_focused};

/// Quick launch `go` with the ⚡ chord and answer its create with `key`'s session, the helm then listing `listed`
/// (`key` among them); returns with the reply taken in. Every other command is answered at once.
fn launch(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    key: &'static str,
    listed: &[&'static str],
) {
    chord(cx);
    let sent = creates(helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    succeed(helm, cx, create, key, listed);
}

/// Answer `create` with `key`'s session, the helm listing `listed` first.
fn succeed(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    create: &FakeCommand,
    key: &'static str,
    listed: &[&'static str],
) {
    let rows: Vec<_> = listed.iter().map(|key| session(key)).collect();
    push(helm, cx, listing(listed, &rows));
    helm.reply(create.id, 200, Some(session(key).entry().value));
    settle(cx);
}

/// The renames the window sent since the last call, every other command answered at once.
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

/// Whether the launcher's "Started" report is up: the sign that a success took the moved-on path and opened nothing.
/// The selection alone is no evidence, since with no session before the sidebar selects the one that appears.
fn started_report(cx: &mut VisualTestContext) -> bool {
    test_support::label(cx, "launch-started-report").is_some()
}

/// The session the window has open.
fn selected(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| local_state::selected(cx))
}

/// SPEC.md, "Quick launch": "When New was never shown and success selects its session, the rename dialog opens over it
/// while its agent is already starting ... The actual created name is focused and fully selected ... a changed name uses
/// ordinary rename. Closing returns to the new terminal, which takes no input while the dialog is open." Naming is
/// the one moment a quick-launched session gets a name the user chose, and a name typed into the terminal instead
/// would reach the agent. Specified: the chord's launch of `n` succeeds; `n` is open with the editor over it and its
/// terminal not focused; typed text replaces the whole name; Enter sends one rename of `n` with exactly that text; the
/// reply closes the editor and focus is on the terminal.
#[gpui::test]
fn an_unseen_quick_launch_opens_naming_and_a_typed_name_renames(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    launch(&mut helm, cx, "n", &["n"]);
    assert_eq!(selected(cx).as_deref(), Some("n"), "the success opened n");
    assert!(any_open(cx), "the naming editor opened over it");
    assert!(!terminal_focused(cx), "the terminal is under the editor");

    keys(cx, "f i x");
    keys(cx, "enter");
    let sent = renames(&mut helm);
    let [rename] = &sent[..] else {
        panic!("one rename, not {sent:?}");
    };
    assert_eq!(rename.path, "/api/sessions/n/rename");
    assert_eq!(
        rename.body.clone().unwrap_or_default(),
        json!({ "title": "fix" }),
        "the typed text replaced the selected name"
    );
    helm.reply(
        rename.id,
        200,
        Some(session("n").title("fix").entry().value),
    );
    settle(cx);
    assert!(!any_open(cx), "the success closed the editor");
    assert!(terminal_focused(cx), "focus went to the new terminal");
}

/// SPEC.md: "Enter with the name unchanged or Escape keeps it without sending a rename", and both return to the new
/// terminal. Ordinary Rename sends an unchanged title; naming must not, or every accepted name would cost a request
/// the supervisor answers for nothing. Specified: `n` launched, Enter on the untouched name closes the editor with no
/// rename and focus on `n`'s terminal; `m` launched next, Escape does the same.
#[gpui::test]
fn unchanged_enter_and_escape_keep_the_created_name(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    for (key, listed, close) in [("n", &["n"][..], "enter"), ("m", &["n", "m"][..], "escape")] {
        launch(&mut helm, cx, key, listed);
        assert!(any_open(cx), "{key}: the premise: naming opened");
        keys(cx, close);
        assert!(!any_open(cx), "{key}: {close} closed the editor");
        assert_eq!(renames(&mut helm), [], "{key}: {close} sent no rename");
        assert_eq!(selected(cx).as_deref(), Some(key));
        assert!(
            terminal_focused(cx),
            "{key}: focus went to the new terminal"
        );
    }
}

/// Settings' "Name sessions on quick launch" is on by default and turns naming off when cleared (SPEC.md). A user who
/// turned it off is not asked again. Specified: with the preference unset, `n`'s launch opens naming (closed with
/// Escape); with it false, `m`'s launch opens `m` and nothing over it.
#[gpui::test]
fn naming_is_off_when_its_preference_is(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    launch(&mut helm, cx, "n", &["n"]);
    assert!(any_open(cx), "unset: naming opened");
    keys(cx, "escape");

    push(
        &mut helm,
        cx,
        preferences_state(Preferences {
            quick_launch_template: Some("go".into()),
            name_session_on_quick_launch: Some(false),
            welcome_seen: Some(true),
            ..Preferences::default()
        }),
    );
    launch(&mut helm, cx, "m", &["n", "m"]);
    assert_eq!(selected(cx).as_deref(), Some("m"), "the success opened m");
    assert!(!started_report(cx), "through the path that opens it");
    assert!(!any_open(cx), "off: no naming");
}

/// SPEC.md: "A launch that revealed New gets no naming dialog, even if a later submission succeeds." Once New was
/// shown the user had its name field in front of them, so a second ask would be noise. Specified: the chord's launch
/// refused by the helm brings New back; Launch there sends a create whose success opens `n` with nothing over it.
#[gpui::test]
fn a_launch_that_revealed_new_is_not_named(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    let [quick] = &sent[..] else {
        panic!("the chord sent one create, not {}", sent.len());
    };
    helm.reply(quick.id, 409, Some(json!("the folder does not exist")));
    settle(cx);
    assert!(any_open(cx), "the premise: the refusal revealed New");
    test_support::click(cx, "launcher-launch");
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("Launch sent one create, not {}", sent.len());
    };
    succeed(&mut helm, cx, create, "n", &["n"]);
    assert_eq!(selected(cx).as_deref(), Some("n"), "the success opened n");
    assert!(!started_report(cx), "through the path that opens it");
    assert!(!any_open(cx), "and named nothing");
}

/// SPEC.md: "A quick launch the user moved on from keeps its generated name without opening the naming dialog". A
/// dialog popping up over whatever the user went on to would take their next keys. Specified: a key pressed while the
/// chord's create is out; its success takes the moved-on path, saying "Started" in the launcher's report instead of
/// opening the session, and no editor opens.
#[gpui::test]
fn a_quick_launch_the_user_moved_on_from_is_not_named(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    keys(cx, "x");
    succeed(&mut helm, cx, &sent[0], "n", &["n"]);
    assert!(started_report(cx), "the success took the moved-on path");
    assert!(!any_open(cx), "and named nothing");
}

/// SPEC.md: "Saving, unchanged Enter, Escape and Cancel all return to the new session's terminal", and Close follows
/// Cancel's rules. Closing back to wherever focus was before the launch would leave the user's next keys off the session
/// they just made. Specified: `n` launched, Cancel closes the editor with no rename and focus on the terminal; `m`
/// launched next, the header Close does the same.
#[gpui::test]
fn cancel_and_close_return_to_the_new_terminal(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    for (key, listed, button) in [
        ("n", &["n"][..], "rename-cancel"),
        ("m", &["n", "m"][..], "rename-close"),
    ] {
        launch(&mut helm, cx, key, listed);
        assert!(any_open(cx), "{key}: the premise: naming opened");
        test_support::click(cx, button);
        assert!(!any_open(cx), "{key}: {button} closed the editor");
        assert_eq!(renames(&mut helm), [], "{key}: {button} sent no rename");
        assert!(
            terminal_focused(cx),
            "{key}: focus went to the new terminal"
        );
    }
}

/// A changed name is the user's draft until they save it: Escape and Cancel throw it away, as they do in ordinary
/// Rename, and must not send it (the old "quick launch owns input and submits once" cases closed a changed name both
/// ways). Specified: `n` launched and `typed` typed over the name, which the editor then shows, Escape closes the
/// editor with no rename and focus on the terminal; `m` launched next, `typed` and Cancel do the same.
#[gpui::test]
fn escape_and_cancel_on_a_changed_name_send_no_rename(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    for (key, listed, close) in [
        ("n", &["n"][..], None),
        ("m", &["n", "m"][..], Some("rename-cancel")),
    ] {
        launch(&mut helm, cx, key, listed);
        assert!(any_open(cx), "{key}: the premise: naming opened");
        cx.simulate_input("typed");
        settle(cx);
        let shown = crate::session_list::tests::painted(&test_support::frame(cx)).join(" ");
        assert!(
            shown.contains("typed"),
            "{key}: the premise: the name changed: {shown}"
        );
        match close {
            None => keys(cx, "escape"),
            Some(button) => test_support::click(cx, button),
        }
        assert!(!any_open(cx), "{key}: the editor closed");
        assert_eq!(renames(&mut helm), [], "{key}: no rename was sent");
        assert!(
            terminal_focused(cx),
            "{key}: focus went to the new terminal"
        );
    }
}

/// Naming keeps the created name only when the saved text is that name byte for byte; anything else is the user's
/// rename and must reach the supervisor, however small. Specified: `n` launched, typing `n` over the selected name
/// (the same bytes) and Enter close the editor with no rename; `m` launched next, End and `2` make `m2`, and Enter
/// sends one rename of `m` to exactly `m2`.
#[gpui::test]
fn a_byte_identical_save_keeps_the_name_and_a_one_character_change_renames(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx) = window(cx, Some("go"));
    launch(&mut helm, cx, "n", &["n"]);
    assert!(any_open(cx), "n: the naming editor opened");
    keys(cx, "n enter");
    assert!(!any_open(cx), "n: the identical save closed the editor");
    assert_eq!(renames(&mut helm), [], "n: and sent nothing");

    launch(&mut helm, cx, "m", &["n", "m"]);
    keys(cx, "end 2 enter");
    let sent = renames(&mut helm);
    let [rename] = &sent[..] else {
        panic!("m: one rename, not {sent:?}");
    };
    assert_eq!(rename.path, "/api/sessions/m/rename");
    assert_eq!(
        rename.body.clone().unwrap_or_default(),
        json!({ "title": "m2" })
    );
}

/// SPEC.md: "When New was never shown and success selects its session, the rename dialog opens over it", whatever the
/// list shows. With a filter or the listing cap leaving the new session out, the store has no row for it at the
/// success (the list asks the helm for the open session only afterwards), so the editor starts from the create's
/// reply, as the old app did. Specified: the chord's create answered while the helm's filtered listing leaves `n` out
/// (it matches nothing); `n` is open with the naming editor over it, whose field holds the reply's name (End, `2` and
/// Enter rename `n` to `n2`).
#[gpui::test]
fn naming_opens_for_a_session_the_listing_leaves_out(cx: &mut TestAppContext) {
    let (mut helm, cx) = window(cx, Some("go"));
    chord(cx);
    let sent = creates(&mut helm, cx);
    let [create] = &sent[..] else {
        panic!("one press sent one create, not {}", sent.len());
    };
    // A status filter that matches nothing: the listing is filtered, so it proves nothing about `n`, which the helm
    // counts but does not list.
    let mut filtered = listing(&[], &[]);
    filtered.meta.counts = Some(ListCounts {
        total: 1,
        matching: Some(0),
        truncated: false,
    });
    push(&mut helm, cx, filtered);
    helm.reply(create.id, 200, Some(session("n").entry().value));
    settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("n"), "the success opened n");
    assert!(any_open(cx), "the naming editor opened over it");
    // An unchanged Enter would pass whatever the field was seeded with, so extend the name instead: the rename's body
    // is the seed plus the typed character, which only the reply's title makes `n2`.
    keys(cx, "end 2 enter");
    let sent = renames(&mut helm);
    let [rename] = &sent[..] else {
        panic!("one rename, not {sent:?}");
    };
    assert_eq!(rename.path, "/api/sessions/n/rename");
    assert_eq!(
        rename.body.clone().unwrap_or_default(),
        json!({ "title": "n2" }),
        "the editor started from the reply's name"
    );
}

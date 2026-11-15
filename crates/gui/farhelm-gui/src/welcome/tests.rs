//! Tests of the Welcome card and of the window-start owner's decisions: when the card opens by itself, what each way of
//! closing it writes, the reopen from Help, the feedback handoff, the refusal of an open the user typed ahead of, and the
//! announcement batch that waits behind the card. The batch's own rules (its order, what its dismissal records) are
//! `announcements::tests`'s, which build on [`open_start`] here.
//!
//! The window is a root view of the tests' own ([`StartBar`]): the window-start owner, as the main view keeps it, plus
//! a stand-in for the selected session's terminal, made home, and a stand-in for the `?` button, whose click reopens the
//! card as the Help menu's Welcome item does (the menu's own test chooses the real item). The helm is the helm client's
//! fake, connected before the window opens as the embedded helm's first snapshot usually is. It never applies a write,
//! so a reset over the same snapshot finds the card unseen again, which is how these tests reach a second window start.
//! Every step ends in one of the shared harness's step helpers, so the focus and layer invariants are checked after each.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{CollectionState, Preferences};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Keystroke, ParentElement as _,
    Render, Styled as _, Subscription, TestAppContext, VisualTestContext, Window, div, px, size,
};
use serde_json::json;

use super::{GET_STARTED_URL, StartDialogs, TITLE};
use crate::announcements::tests::{announcement, queue};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::layers;
use crate::root;
use crate::session;
use crate::session_list::tests::preferences_state;
use crate::test_support::{
    self, any_open, click, focused, focused_id, frame, keys, label, one_command, settle, started,
};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The tests' root view: the window-start owner, a stand-in for the terminal that is home, and a stand-in for the `?`
/// button that reopens the card.
pub(crate) struct StartBar {
    root: root::Attachment,
    home: FocusHandle,
    toggle: FocusHandle,
    _start: Entity<StartDialogs>,
}

impl StartBar {
    /// The view, with `home` made home once the window exists and the owner made after that, as the main view makes it
    /// after the session pane.
    fn new(
        home: FocusHandle,
        toggle: FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let root = root::attach(window, cx);
        let held = home.clone();
        cx.defer_in(window, move |_, window, cx| {
            focus::set_home(Some(&held), window, cx)
        });
        StartBar {
            root,
            home,
            toggle,
            _start: cx.new(|cx| StartDialogs::new(window, cx)),
        }
    }
}

impl Render for StartBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle = self.toggle.clone();
        let content = div()
            .size_full()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                Button::new(
                    "home",
                    "Home",
                    HoverHelp::tooltip("Stands in for the selected session's terminal"),
                    Role::Button,
                )
                .focus(&self.home),
            )
            .child(
                Button::new(
                    "help-toggle",
                    "Help",
                    HoverHelp::tooltip("Help: stands in for the ? button and its Welcome item"),
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

/// The preferences collection with `welcome_seen` as given and nothing else set.
pub(crate) fn seen(welcome_seen: Option<bool>) -> CollectionState {
    preferences_state(Preferences {
        welcome_seen,
        ..Preferences::default()
    })
}

/// The app over the fake helm, connected with `snapshot` before the window opens (`None`: the window opens while the
/// channel is still connecting), in a window big enough for the dialogs, under macOS's click rule (a click leaves a
/// button unfocused, so focus landing on the `?` button or home is the layers' doing). Returns the fake, the window, and
/// the `?` button's and home's focus handles.
pub(crate) fn open_start(
    cx: &mut TestAppContext,
    snapshot: Option<Vec<CollectionState>>,
) -> (FakeHelm, &mut VisualTestContext, FocusHandle, FocusHandle) {
    open_start_overridden(cx, snapshot, None)
}

/// [`open_start`] with `overrides` installed as an agent build's startup overrides before the window opens, the way
/// `agent_startup`'s own tests install them (a global, never the process environment); `None` installs nothing.
fn open_start_overridden(
    cx: &mut TestAppContext,
    snapshot: Option<Vec<CollectionState>>,
    overrides: Option<crate::agent_startup::Overrides>,
) -> (FakeHelm, &mut VisualTestContext, FocusHandle, FocusHandle) {
    let (mut helm, _connection) = started(cx);
    if let Some(overrides) = overrides {
        cx.update(|cx| cx.set_global(overrides));
    }
    if let Some(snapshot) = snapshot {
        helm.connect(snapshot);
        cx.run_until_parked();
    }
    let home = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let toggle = cx.update(|cx| cx.focus_handle().tab_stop(true));
    let (held_home, held_toggle) = (home.clone(), toggle.clone());
    let (_, window) = test_support::open_view(cx, move |window, cx| {
        StartBar::new(held_home, held_toggle, window, cx)
    });
    window.simulate_resize(size(px(1600.), px(1200.)));
    window.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    settle(window);
    (helm, window, toggle, home)
}

/// A reset: the channel breaks, which closes every layer, and comes back with `snapshot`, a new UI session.
pub(crate) fn reset(
    helm: &mut FakeHelm,
    cx: &mut VisualTestContext,
    snapshot: Vec<CollectionState>,
) {
    helm.break_channel();
    settle(cx);
    assert!(!any_open(cx), "premise: the reset closed every layer");
    helm.connect(snapshot);
    settle(cx);
}

/// Whether the dialog drawn under frame id `id` (`welcome`, `announcements`, `feedback`) is on screen.
pub(crate) fn shows(cx: &mut VisualTestContext, id: &str) -> bool {
    frame(cx)
        .iter()
        .any(|node| node.element_id.as_deref() == Some(id))
}

/// Every word the frame painted, in paint order, one space apart: wrapped lines joined again, so a test can look for a
/// sentence whatever its line breaks.
fn words(cx: &mut VisualTestContext) -> String {
    frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Expect the one seen write (`PUT /api/preferences` naming only `welcome_seen`), and answer it with `status`.
fn expect_seen_write(helm: &mut FakeHelm, status: u16) {
    let (id, body) = one_command(helm, Method::Put, "/api/preferences");
    assert_eq!(
        body,
        json!({ "welcome_seen": true }),
        "a sparse patch of welcome_seen alone"
    );
    helm.reply(id, status, None);
}

/// Whether focus is inside the topmost dialog.
fn focus_in_top_dialog(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        layers::top_container(window, cx).is_some_and(|top| top.contains_focused(window, cx))
    })
}

/// Holds a subscription for a test: [`a_press_before_the_open_refuses_the_card_and_the_batch_until_a_reset`]'s press.
struct Probe {
    pressed: bool,
    _watch: Option<Subscription>,
}

// ---- The card at window start --------------------------------------------------------------------------------------

/// SPEC.md, "Welcome card": every helm starts unseen, and the window start decides once after a successful preference
/// read to open "a modal in the app's dialog style, titled **Welcome to Farhelm**, with focus inside". The card is what
/// a new user sees first, and its words carry the two pointers (Get started, feedback) and where to find it again.
/// Specified: over preferences that leave `welcome_seen` unset, the card is open once the window has started, with
/// focus on Got it; it shows the title, the introduction, both pointers with their controls, the footer's note naming
/// the `?` menu, and nothing has been written.
#[gpui::test]
fn an_unseen_helm_opens_the_card_at_window_start_with_focus_on_got_it(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) = open_start(cx, Some(vec![seen(None)]));
    assert!(shows(cx, "welcome"), "the card opened by itself");
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));
    let painted = words(cx);
    for text in [
        TITLE,
        "One window for every coding agent you run",
        "New to it? The Get started guide walks through your first session",
        "Tell us what you think. Send feedback, in the ? menu at the bottom left of the window",
        "This card is in the ? menu as Welcome if you want it again.",
    ] {
        assert!(
            painted.contains(text),
            "{text:?} was not painted: {painted}"
        );
    }
    assert_eq!(
        label(cx, "welcome-get-started").as_deref(),
        Some("Read Get started ↗")
    );
    assert_eq!(
        label(cx, "welcome-feedback").as_deref(),
        Some("Send feedback now")
    );
    assert!(helm.commands().is_empty(), "opening writes nothing");
}

/// SPEC.md: "Escape, the ×, **Got it**, and **Send feedback now** close it and mark it seen in the helm's shared
/// preferences", and "the first-open card has no originating control and restores ordinary initial focus". A seen mark
/// lost on one of the closes would show the card at every launch; a second write would be noise on the helm. A reset
/// decides again, which is also how this test reaches a fresh card for each way of closing. Specified: for Got it, the
/// header's × and Escape in turn, closing the card sends exactly one `PUT /api/preferences` of
/// `{"welcome_seen": true}` and leaves focus home; after a reset over preferences still unseen, the card opens again,
/// and the reset itself wrote nothing. (Send feedback now is its own test.)
#[gpui::test]
fn every_close_of_the_first_card_marks_it_seen_once_and_returns_home(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, home) = open_start(cx, Some(vec![seen(None)]));
    for way in ["welcome-got-it", "welcome-close", "escape"] {
        assert!(
            shows(cx, "welcome"),
            "premise: the card is open before {way}"
        );
        assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));
        match way {
            "escape" => keys(cx, "escape"),
            "welcome-got-it" => click(cx, "welcome-got-it"),
            _ => click(cx, "welcome-close"),
        }
        assert!(!any_open(cx), "{way} closed the card");
        expect_seen_write(&mut helm, 204);
        settle(cx);
        assert!(focused(cx, &home), "{way} left focus home");
        reset(&mut helm, cx, vec![seen(None)]);
        assert!(helm.commands().is_empty(), "the reset wrote nothing");
    }
    assert!(shows(cx, "welcome"), "the last reset opened the card again");
}

/// SPEC.md: "A failed write still closes it and may let it appear again at a later window start or reset." A card that
/// stayed up, or came back at once, would trap the user behind a write they cannot fix. Specified: Got it closes the
/// card; the helm refuses the seen write; the card stays closed, nothing else is sent, and a reset opens it again.
#[gpui::test]
fn a_failed_seen_write_still_closes_the_card(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) = open_start(cx, Some(vec![seen(None)]));
    click(cx, "welcome-got-it");
    expect_seen_write(&mut helm, 500);
    settle(cx);
    assert!(!any_open(cx), "the refused write left the card closed");
    assert!(helm.commands().is_empty(), "nothing retried the write");
    reset(&mut helm, cx, vec![seen(None)]);
    assert!(shows(cx, "welcome"), "a later reset shows the card again");
}

/// SPEC.md: the decision is made "after a successful shared-preferences read; a failed or timed-out read does not open
/// the card", and a seen helm shows nothing; each reset decides again. With the embedded helm the preferences arrive in
/// the first snapshot, so a snapshot without them is a failed read. Specified: over `welcome_seen: true` nothing opens;
/// after a reset whose snapshot holds no preferences entry nothing opens; after a reset over `welcome_seen: false`
/// (which, like unset, means unseen) the card opens; nothing was written throughout.
#[gpui::test]
fn a_seen_helm_or_a_failed_preference_read_opens_no_card_and_each_reset_decides_again(
    cx: &mut TestAppContext,
) {
    let (mut helm, cx, _toggle, _home) = open_start(cx, Some(vec![seen(Some(true))]));
    assert!(!any_open(cx), "a seen helm opens nothing");
    reset(&mut helm, cx, vec![]);
    assert!(!any_open(cx), "no preferences read, no card");
    reset(&mut helm, cx, vec![seen(Some(false))]);
    assert!(
        shows(cx, "welcome"),
        "false is unseen, and the reset decided again"
    );
    assert!(helm.commands().is_empty(), "deciding writes nothing");
}

/// SPEC.md: "**Read Get started** opens `https://farhelm.io/docs/get-started/first-session/` in the user's browser
/// through the same opener as Documentation and leaves the card open." Specified: a click on Read Get started asks the
/// platform to open exactly that address, and the card stays open with nothing written.
#[gpui::test]
fn read_get_started_opens_the_guide_and_leaves_the_card_open(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) = open_start(cx, Some(vec![seen(None)]));
    click(cx, "welcome-get-started");
    assert_eq!(cx.opened_url().as_deref(), Some(GET_STARTED_URL));
    assert!(shows(cx, "welcome"), "the card stays open");
    assert!(
        helm.commands().is_empty(),
        "following the link writes nothing"
    );
}

/// SPEC.md: "**Send feedback now** closes it and opens the feedback dialog, without sending anything", and it is one of
/// the closes that mark the card seen. The old app's spec checked that no feedback request went out. Specified: a click
/// on Send feedback now closes the card and opens the feedback dialog with focus inside it; the one command sent is the
/// seen write; Cancel closes the feedback dialog with focus home, and sends nothing either.
#[gpui::test]
fn send_feedback_now_marks_seen_and_opens_feedback_without_sending(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, home) = open_start(cx, Some(vec![seen(None)]));
    click(cx, "welcome-feedback");
    assert!(!shows(cx, "welcome"), "the card closed");
    assert!(shows(cx, "feedback"), "the feedback dialog opened");
    assert!(
        focus_in_top_dialog(cx),
        "focus is inside the feedback dialog"
    );
    expect_seen_write(&mut helm, 204);
    click(cx, "feedback-cancel");
    assert!(!any_open(cx), "Cancel closed the feedback dialog");
    assert!(
        focused(cx, &home),
        "the feedback dialog returned focus home, as the card would have"
    );
    assert!(helm.commands().is_empty(), "no feedback was sent");
}

/// SPEC.md: "Welcome shows the welcome card again", "Reopening does not reset or rewrite seen. Closing a reopened card
/// returns focus to the help toggle". The feedback handoff from a reopened card carries the card's return target, so
/// it also ends on the `?` button. Specified: over a seen helm, the `?` stand-in's click opens the card with focus on
/// Got it; Escape closes it with focus on the `?` button; reopened, Send feedback now opens the feedback dialog, whose
/// Cancel puts focus on the `?` button; the helm received no command throughout.
#[gpui::test]
fn reopening_from_help_writes_nothing_and_returns_focus_to_the_button(cx: &mut TestAppContext) {
    let (mut helm, cx, toggle, _home) = open_start(cx, Some(vec![seen(Some(true))]));
    click(cx, "help-toggle");
    assert!(shows(cx, "welcome"), "Help reopened the card");
    assert_eq!(focused_id(cx).as_deref(), Some("welcome-got-it"));
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert!(
        focused(cx, &toggle),
        "Escape returned focus to the ? button"
    );

    click(cx, "help-toggle");
    click(cx, "welcome-feedback");
    assert!(shows(cx, "feedback"), "the handoff opened feedback");
    click(cx, "feedback-cancel");
    assert!(!any_open(cx));
    assert!(
        focused(cx, &toggle),
        "feedback returned focus to the ? button"
    );
    assert!(helm.commands().is_empty(), "a reopened card writes nothing");
}

// ---- The window-start mark -----------------------------------------------------------------------------------------

/// The maintainer's decision of 2026-11-14 (SPEC_impl.md, "Overlays: the layer manager"): the window-start dialogs open
/// with a mark taken when the owner first sees the UI session, and a key press or click before the open refuses them;
/// then neither the card nor the batch opens for that UI session, and both stay unseen until the next window start or
/// reset. Without this, a user already typing at launch would have their keys land in a card they never saw (an Enter
/// dismissing it unseen).
///
/// The press has to land between the owner seeing the session and the open, which in the app is the end of the update
/// that brought the snapshot. A connection observer of the test's own, made after the owner and so called after the
/// owner's, presses a key there, the way the platform would deliver one. Specified: with the window open before the
/// first snapshot, over preferences that leave the card unseen and one pending announcement, a key pressed as the
/// snapshot arrives leaves nothing open and nothing written; after a reset with no press, the card opens, and Got it
/// writes seen and then opens the batch.
#[gpui::test]
fn a_press_before_the_open_refuses_the_card_and_the_batch_until_a_reset(cx: &mut TestAppContext) {
    let snapshot = || {
        vec![
            seen(None),
            queue(vec![announcement("1", "Pending news", "Body.")], vec![]),
        ]
    };
    let (mut helm, cx, _toggle, _home) = open_start(cx, None);
    let connection = cx
        .update(|_, cx| session::connection(cx))
        .expect("the app's connection");
    let probe = cx.update(|window, cx| {
        cx.new(|cx: &mut Context<Probe>| {
            let watch = cx.observe_in(
                &connection,
                window,
                |probe: &mut Probe, connection, window, cx| {
                    if probe.pressed || connection.read(cx).session().is_none() {
                        return;
                    }
                    probe.pressed = true;
                    window.dispatch_keystroke(Keystroke::parse("a").expect("a keystroke"), cx);
                },
            );
            Probe {
                pressed: false,
                _watch: Some(watch),
            }
        })
    });
    helm.connect(snapshot());
    settle(cx);
    assert!(
        cx.update(|_, cx| probe.read(cx).pressed),
        "premise: the key was pressed as the session arrived"
    );
    assert!(
        !any_open(cx),
        "the press refused the card, and the batch with it"
    );
    assert!(helm.commands().is_empty(), "the card stays unseen");
    drop(probe);

    reset(&mut helm, cx, snapshot());
    assert!(shows(cx, "welcome"), "the next UI session opens the card");
    click(cx, "welcome-got-it");
    expect_seen_write(&mut helm, 204);
    settle(cx);
    assert!(shows(cx, "announcements"), "the batch followed the card");
}

// ---- The batch behind the card -------------------------------------------------------------------------------------

/// SPEC.md, "Announcements": "Initial Welcome, including its Send feedback now handoff, closes first", and "A batch
/// ready behind another modal waits for it to close". The batch then opens with a mark from the closing press (the
/// migration's orchestrator, 2026-11-14), not the window-start one, which that press would refuse. Specified: over an
/// unseen card and two pending announcements, only the card opens; Got it writes seen and opens the batch, newest
/// first, with focus on its Got it; closing the batch returns focus home.
#[gpui::test]
fn the_batch_waits_for_the_card_and_opens_when_it_closes(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, home) = open_start(
        cx,
        Some(vec![
            seen(None),
            queue(
                vec![
                    announcement("older", "An earlier notice", "Earlier."),
                    announcement("newer", "A newer notice", "Newer."),
                ],
                vec![],
            ),
        ]),
    );
    assert!(
        shows(cx, "welcome") && !shows(cx, "announcements"),
        "the card alone opened"
    );
    click(cx, "welcome-got-it");
    expect_seen_write(&mut helm, 204);
    settle(cx);
    assert!(
        shows(cx, "announcements"),
        "the batch opened once the card closed"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("announcements-got-it"));
    let painted = words(cx);
    let (newer, older) = (
        painted.find("A newer notice"),
        painted.find("An earlier notice"),
    );
    assert!(newer.is_some() && newer < older, "newest first: {painted}");
    click(cx, "announcements-got-it");
    assert!(!any_open(cx));
    assert!(
        focused(cx, &home),
        "closing the automatic batch returns focus home"
    );
}

/// SPEC.md: the batch waits for Welcome "including its Send feedback now handoff". The feedback dialog closes by Cancel,
/// the ×, Escape or its own close after a send; the batch hears all of them through the dialog's close continuation.
/// Specified: over an unseen card and one pending announcement, Send feedback now opens the feedback dialog and no
/// batch; Escape closes the feedback dialog, and the batch opens then with focus on its Got it.
#[gpui::test]
fn the_batch_waits_for_the_feedback_handoff_too(cx: &mut TestAppContext) {
    let (mut helm, cx, _toggle, _home) = open_start(
        cx,
        Some(vec![
            seen(None),
            queue(vec![announcement("1", "Pending news", "Body.")], vec![]),
        ]),
    );
    click(cx, "welcome-feedback");
    expect_seen_write(&mut helm, 204);
    settle(cx);
    assert!(
        shows(cx, "feedback") && !shows(cx, "announcements"),
        "feedback alone is open"
    );
    keys(cx, "escape");
    assert!(!shows(cx, "feedback"), "Escape closed the feedback dialog");
    assert!(
        shows(cx, "announcements"),
        "the batch opened once the handoff closed"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("announcements-got-it"));
}

/// SPEC_impl.md, "One failure path: the reset": a reset closes every layer without asking, and SPEC.md has the card
/// decided again at the next window start or reset. A reset is not a dismissal, so the card it closed is not marked
/// seen and no batch was shown; and the check the card or its handoff held back belongs to the retired UI session, so
/// it must not open anything over the new one, where the new session's own decision opens the card first. Specified,
/// over an unseen card (the fake never applies a write, so it stays unseen) and one pending announcement: a reset while
/// the card is open sends no command, and the next UI session opens the card again with no batch; there, after Send
/// feedback now (whose seen write is answered), a reset while the feedback dialog is open again sends no command, and
/// the next UI session again opens the card with no batch.
#[gpui::test]
fn a_reset_while_the_card_or_its_handoff_is_open_writes_nothing_and_decides_again(
    cx: &mut TestAppContext,
) {
    let snapshot = || {
        vec![
            seen(None),
            queue(vec![announcement("1", "Pending news", "Body.")], vec![]),
        ]
    };
    let (mut helm, cx, _toggle, _home) = open_start(cx, Some(snapshot()));
    assert!(shows(cx, "welcome"), "premise: the card is open");
    reset(&mut helm, cx, snapshot());
    assert!(
        helm.commands().is_empty(),
        "the reset wrote neither seen nor shown"
    );
    assert!(
        shows(cx, "welcome"),
        "the next UI session opened the card again"
    );
    assert!(!shows(cx, "announcements"), "and no batch");

    click(cx, "welcome-feedback");
    expect_seen_write(&mut helm, 204);
    settle(cx);
    assert!(
        shows(cx, "feedback"),
        "premise: the handoff's feedback dialog is open"
    );
    reset(&mut helm, cx, snapshot());
    assert!(
        helm.commands().is_empty(),
        "the reset during the handoff wrote nothing"
    );
    assert!(
        shows(cx, "welcome"),
        "the next UI session opened the card again"
    );
    assert!(!shows(cx, "announcements"), "and no batch");
}

// ---- Agent runs that turn the start dialogs off --------------------------------------------------------------------

/// `agent_startup::start_dialogs_off` exists because every bridge run's helm is fresh, so without it every run would
/// start under the Welcome card and its first click or key would act on the card. It must turn off the automatic
/// decision only: nothing may be written (the helm keeps the card unseen and the announcements pending, as the real
/// start would find them), and a card the user asks for from Help must still open. Specified, with the overrides
/// turning the start dialogs off installed before the window opens, over an unseen card and one pending announcement:
/// nothing opens at the window start, nor after a reset; no command is sent; and the `?` stand-in's click opens the
/// card, which Escape closes, still with nothing written.
#[gpui::test]
fn an_agent_run_with_the_start_dialogs_off_opens_and_writes_nothing_at_start(
    cx: &mut TestAppContext,
) {
    let snapshot = || {
        vec![
            seen(None),
            queue(vec![announcement("1", "Pending news", "Body.")], vec![]),
        ]
    };
    let off = crate::agent_startup::Overrides {
        start_dialogs_off: true,
        ..crate::agent_startup::Overrides::default()
    };
    let (mut helm, cx, _toggle, _home) = open_start_overridden(cx, Some(snapshot()), Some(off));
    assert!(!any_open(cx), "nothing opened at the window start");
    reset(&mut helm, cx, snapshot());
    assert!(!any_open(cx), "nor at a reset");
    assert!(
        helm.commands().is_empty(),
        "the start decision wrote nothing"
    );

    click(cx, "help-toggle");
    assert!(shows(cx, "welcome"), "Help's Welcome still opens the card");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert!(helm.commands().is_empty(), "a reopened card writes nothing");
}

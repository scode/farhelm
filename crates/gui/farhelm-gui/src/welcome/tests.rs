//! Drive the startup decisions over fake snapshots, including the window-start input boundary and the automation
//! override. The same root fixture supports the announcement and upgrade highlights tests.

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::announcements::AnnouncementQueue;
use farhelm_proto::ui_channel::{ANNOUNCEMENTS_KEY, CollectionState, Preferences};
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, Keystroke, ParentElement as _,
    Render, Styled as _, Subscription, TestAppContext, VisualTestContext, Window, div, px, size,
};

use super::StartDialogs;
use crate::announcements::tests::{announcement, queue};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::{self, Mark};
use crate::root;
use crate::session;
use crate::session_list::tests::preferences_state;
use crate::test_support::{self, any_open, click, focused_id, frame, keys, settle, started};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The tests' root view: the window-start owner, a stand-in for the terminal that is home, and a stand-in for the `?`
/// button that opens Welcome on request.
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

/// Holds a subscription for a test: [`a_press_before_the_open_refuses_the_batch_until_a_reset`]'s press.
struct Probe {
    pressed: bool,
    _watch: Option<Subscription>,
}

/// A fresh window over legacy unset preferences starts on its ordinary home. Opening and closing Welcome on request
/// does not write preferences, so older helms can keep their stored field without its value controlling the UI.
#[gpui::test]
fn an_unseen_helm_opens_no_automatic_welcome(cx: &mut TestAppContext) {
    let (mut helm, cx, _, _) = open_start(cx, Some(vec![seen(None), queue(vec![], vec![])]));
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("home"));
    click(cx, "help-toggle");
    assert!(shows(cx, "welcome"));
    keys(cx, "escape");
    assert!(helm.commands().is_empty());
}

/// A key pressed just as the initial snapshot arrives must not land in an unexpected announcement dialog. The
/// later reset can show the batch when no input precedes that check.
#[gpui::test]
fn a_press_before_the_open_refuses_the_batch_until_a_reset(cx: &mut TestAppContext) {
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
    assert!(!any_open(cx), "the press refused the automatic batch");
    assert!(helm.commands().is_empty(), "no dismissal was recorded");
    drop(probe);

    reset(&mut helm, cx, snapshot());
    assert!(
        shows(cx, "announcements"),
        "the next UI session opens the batch"
    );
}

// ---- Staged starts ------------------------------------------------------------------------------------------------

/// A staged brick launch leaves an already-pending announcement for the bell. Looking at the first snapshot must
/// leave the entry pending without showing or recording a dismissal.
#[gpui::test]
fn the_brick_launch_reserves_an_already_pending_batch_for_the_bell(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_global(super::AnnouncementsFromBell));
    let (mut helm, cx, _toggle, _home) = open_start(
        cx,
        Some(vec![
            seen(None),
            queue(vec![announcement("1", "Pending news", "Body.")], vec![]),
        ]),
    );
    cx.update(|_, cx| {
        let store = session::current_store(cx).unwrap();
        let queue = store
            .read(cx)
            .collections()
            .get::<AnnouncementQueue>(ANNOUNCEMENTS_KEY)
            .unwrap();
        assert_eq!(
            queue.pending[0].id, "1",
            "premise: the first snapshot has the entry"
        );
    });
    assert!(!any_open(cx));
    assert!(
        helm.commands().is_empty(),
        "the pending entry was not recorded as shown"
    );
}

/// An automation run can suppress the automatic batch at startup and reset. Help still opens Welcome when the user
/// requests it, without dismissing the pending announcement.
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
    assert!(shows(cx, "welcome"), "Help still opens Welcome");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert!(helm.commands().is_empty(), "opening Welcome writes nothing");
}

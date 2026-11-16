//! The in-process test harness: what every GUI test sets up, and the checks every test step runs.
//!
//! Two kinds of context, because GPUI's test context fakes text. [`TestAppContext`] (with `#[gpui::test]`) gives a
//! deterministic scheduler and simulated input, and is right for behavior: what redraws, where focus goes, which
//! command is sent. But its text system gives every character the same width, so anything about layout (wrapping,
//! truncation, what fits at the minimum window size, the terminal's cell grid) proves nothing there; for those,
//! [`headless`] gives a real text system with the bundled fonts and a real (software, on Linux) renderer, the same
//! setup the screenshot goldens use.
//!
//! [`check_invariants`] is the hook every test step calls: the focus coordinator's rules (with a dialog open, focus is
//! inside the topmost one; with a home set, something holds focus) and the layer manager's (every open popover's anchor
//! was drawn, and a popover that never takes focus does not hold it). GPUI cannot hold two focused elements in one
//! window, so "exactly one" needs only the "at least one" half, which the rules give wherever SPEC.md requires it.
//!
//! For behavior tests, [`open`] is the harness: a window built the way the app builds its own, and step helpers
//! ([`keys`], [`click`], [`redraw`], [`advance`], [`settle`]) that each end with [`check`], so a test written with them
//! runs the invariant checks after every step without having to remember to. A test that drives the window some other
//! way (a raw key event, a mouse move, a scroll, a resize) calls [`settle`] or [`check`] itself after its step. A test
//! that needs a root view [`open`]'s scene cannot give (one with state of its own, or action handlers on the root
//! element, where an action dispatched with nothing else focused still reaches them) builds that view through
//! [`root::attach`] and [`root::wrap`] and opens it with [`open_view`], or [`another_window`] for a second window, so
//! that its window and its checks are the same as everyone else's. The self-tests at the end of this file make every
//! step helper prove that it checks.
//!
//! Between the step helpers and the self-tests sit the fixtures that several test modules share: the focus handles,
//! buttons, dialogs, menus and questions that the focus, layer and prompt tests open, and the fake helm with the app
//! serving it, which the UI-session, store and layer tests use and the operation store's and the areas' tests build on
//! (`started`, `sessions`, `meta`, `session_entry`). A fixture that only one test module uses stays in that module.
//! After them, a section that areas may append to holds the helpers several tests share (`frame`, `focused_id`), with
//! the rule for adding one; the helpers above it change only by request.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use farhelm_helm_client::collections::decode_state;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::{ChannelState, PushedState};
use farhelm_proto::grid::GridOutput;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionMeta, CollectionState, KeyedEntry,
};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Entity, FocusHandle, HeadlessAppContext,
    InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Pixels, Point, Render,
    Styled as _, TestAppContext, VisualTestContext, Window, div, px,
};
use gpui_component::Root;

use crate::components::{Button, HoverHelp, Role, Tier};
use crate::focus::Mark;
use crate::layers::{Anchor, Dialog, LayerId, Popover};
use crate::main_view::MainView;
use crate::prompt::{Answer, Question};
use crate::session::{self, Connection, UiSession};
use crate::{assets, bootstrap, focus, layers, render_count, root};

/// Install the app-wide state a test needs, as the real bootstrap does, on a fake-platform test context.
pub fn install(cx: &mut TestAppContext) {
    cx.update(|cx| {
        if let Err(error) = bootstrap::install(cx) {
            panic!("bootstrap failed in a test: {error:#}");
        }
    });
}

/// A headless app with a real text system that knows only the bundled fonts (so a font installed on the machine can
/// never change a result) and the platform's headless renderer, bootstrapped like the real app.
///
/// A machine without a headless renderer fails at capture time; the screenshot goldens treat that as a failure, not a
/// skip.
pub fn headless() -> anyhow::Result<HeadlessAppContext> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(bootstrap::install)?;
    Ok(cx)
}

/// The checks every test step runs against `window`, called from inside an update of that window. Panics with what
/// was wrong, so the failing step is the one that broke the invariant, not a later assertion that tripped over its
/// consequences.
pub fn check_invariants(window: &mut Window, cx: &mut App) {
    if let Some(problem) =
        focus::invariant_violation(window, cx).or_else(|| layers::invariant_violation(window, cx))
    {
        panic!("invariant broken: {problem}");
    }
}

// ---- The behavior harness --------------------------------------------------------------------------------------------

/// What a test draws in its window, called on every render of the scene.
type Draw = dyn Fn(&mut Window, &mut App) -> AnyElement;

/// The root view [`open`] draws a test's content in, wired through [`root`] exactly as the main view is: Farhelm's key
/// context and focus actions, the layer stack, the focus coordinator and the overlay on top. The content is laid out as
/// a column from the top-left corner, inside 20 pixels of padding, so a point near the window's corner is empty space a
/// test can move or click to. It records its renders as `"scene"` (see [`render_count`]).
struct Scene {
    draw: Box<Draw>,
    root: root::Attachment,
}

impl Render for Scene {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("scene", cx);
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .items_start()
            .p(px(20.))
            .gap(px(8.))
            .child((self.draw)(window, cx));
        root::wrap(content, &self.root, window, cx)
    }
}

/// Open a bootstrapped, active window drawing `draw` in a [`Scene`] inside GPUI Kit's `Root`, as the app does, and
/// check it once its first frame has settled.
///
/// `draw` is called again on every render, so it may read state the test changes later; after a change that notifies
/// nothing (a plain cell), call [`redraw`].
pub fn open(
    cx: &mut TestAppContext,
    draw: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> &mut VisualTestContext {
    install(cx);
    let (_, cx) = open_view(cx, |window, cx| Scene {
        draw: Box::new(draw),
        root: root::attach(window, cx),
    });
    cx
}

/// [`open`] for a root view of the test's own, which `build` makes and which must wire itself through
/// [`root::attach`] and [`root::wrap`] as every root view does: the view goes inside GPUI Kit's `Root`, and the window
/// is activated and checked once its first frame has settled, exactly as [`open`] does. Returns the view, for a test
/// that reads or changes its state, with the window's context.
///
/// Unlike [`open`], this does not bootstrap the app: call [`install`] first, which leaves room for a test that needs
/// app state before its window exists (a UI session its view is made for).
pub fn open_view<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
) -> (Entity<V>, &mut VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let made = cx.new(|cx| build(window, cx));
        view = Some(made.clone());
        Root::new(made, window, cx)
    });
    activate_and_check(cx);
    (built(view), cx)
}

/// A second window in an app [`open`] or [`open_view`] already bootstrapped, for a test about more than one window:
/// `build` makes its root view as for [`open_view`], and the window is activated (so it becomes the active window, and
/// the one before it inactive) and checked the same way. Owned rather than borrowed from `cx`, so the test can keep
/// driving its first window's context beside it.
pub fn another_window<V: Render>(
    cx: &VisualTestContext,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
) -> (Entity<V>, VisualTestContext) {
    let mut app = cx.cx.clone();
    let mut view = None;
    let window = app.add_window(|window, cx| {
        let made = cx.new(|cx| build(window, cx));
        view = Some(made.clone());
        Root::new(made, window, cx)
    });
    let mut other = VisualTestContext::from_window(window.into(), &cx.cx);
    activate_and_check(&mut other);
    (built(view), other)
}

/// The root view a window builder made. GPUI runs the builder before `add_window` returns, so a missing view is a
/// broken harness.
fn built<V>(view: Option<Entity<V>>) -> Entity<V> {
    match view {
        Some(view) => view,
        None => panic!("the window builder did not run"),
    }
}

/// Activate a newly opened window, let its first frame settle, and check it.
///
/// The window is activated because simulated windows start inactive, and an inactive window has no focus path for keys
/// to follow: without it a key test would pass or fail for reasons that have nothing to do with the app.
fn activate_and_check(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.activate_window());
    settle(cx);
}

/// Run the per-step checks ([`check_invariants`]) against the test's window.
pub fn check(cx: &mut VisualTestContext) {
    cx.update(check_invariants);
}

/// Run whatever is queued until the app is idle (output a fake stream delivered, a raw event's handlers, a deferred
/// focus move), and check. Unlike [`redraw`], this asks for no frame of its own, so a test that counts renders sees only
/// the ones its step caused.
pub fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    check(cx);
}

/// Redraw after a change the scene's draw function reads from a plain cell, which notifies nothing, and check.
pub fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    settle(cx);
}

/// Press keys, as the platform would deliver them (GPUI's keystroke syntax, space-separated: `"tab shift-tab"`), and
/// check.
pub fn keys(cx: &mut VisualTestContext, keystrokes: &str) {
    cx.simulate_keystrokes(keystrokes);
    check(cx);
}

/// Let `duration` pass on the executor's clock (a pointer resting for the hover delay, a deadline expiring), run
/// whatever that wakes until the app is idle, and check.
pub fn advance(cx: &mut VisualTestContext, duration: Duration) {
    cx.executor().advance_clock(duration);
    cx.run_until_parked();
    check(cx);
}

/// Click the center of the element drawn under `selector`, and check. Panics if nothing was drawn under it.
pub fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = center(cx, selector);
    cx.simulate_click(at, Modifiers::none());
    check(cx);
}

/// Where the element drawn under `selector` (its `debug_selector`, or a component's id) was last drawn. Panics if
/// nothing was, since every caller's next step would otherwise act on a point that is not the element.
pub fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    match cx.debug_bounds(selector) {
        Some(bounds) => bounds,
        None => panic!("{selector} was not drawn"),
    }
}

/// The center of the element drawn under `selector`; see [`bounds`].
pub fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<Pixels> {
    bounds(cx, selector).center()
}

// ---- Shared fixtures of the focus, layer and prompt tests -----------------------------------------------------------

/// A tab-stop focus handle the test owns, to give a control and then ask whether it is focused.
pub(crate) fn handle(cx: &mut VisualTestContext) -> FocusHandle {
    cx.update(|_, cx| cx.focus_handle().tab_stop(true))
}

/// Whether `handle` holds the window's focus now.
pub(crate) fn focused(cx: &mut VisualTestContext, handle: &FocusHandle) -> bool {
    cx.update(|window, _| handle.is_focused(window))
}

/// A button the test can find (`id` is its selector) and whose focus handle it holds.
pub(crate) fn button(id: &'static str, focus: &FocusHandle) -> Button {
    Button::new(id, id, HoverHelp::tooltip("A test control"), Role::Button).focus(focus)
}

/// A dialog's content view: some buttons, drawn under its own selector.
pub(crate) struct Panel {
    pub(crate) controls: Vec<(&'static str, FocusHandle)>,
    pub(crate) shown: Rc<Cell<bool>>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let shown = self.shown.get();
        div()
            .debug_selector(|| "panel".into())
            .w(px(300.))
            .p(px(20.))
            .flex()
            .gap(px(8.))
            .children(
                self.controls
                    .iter()
                    // The last control can be taken away, to test a focused control disappearing.
                    .enumerate()
                    .filter(|(index, _)| shown || *index + 1 < self.controls.len())
                    .map(|(_, (id, focus))| button(id, focus)),
            )
    }
}

/// Open a dialog of `controls`, starting on `start`, for a mark taken now.
pub(crate) fn open_dialog(
    cx: &mut VisualTestContext,
    controls: Vec<(&'static str, FocusHandle)>,
    start: &FocusHandle,
) -> (LayerId, FocusHandle, Rc<Cell<bool>>) {
    open_dialog_returning(cx, controls, start, focus::ReturnTo::Opener)
}

/// [`open_dialog`], returning focus where `returns_to` says when it closes.
pub(crate) fn open_dialog_returning(
    cx: &mut VisualTestContext,
    controls: Vec<(&'static str, FocusHandle)>,
    start: &FocusHandle,
    returns_to: focus::ReturnTo,
) -> (LayerId, FocusHandle, Rc<Cell<bool>>) {
    let shown = Rc::new(Cell::new(true));
    let container = handle(cx);
    let (start, kept, panel_shown) = (start.clone(), container.clone(), shown.clone());
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Panel {
            controls,
            shown: panel_shown,
        });
        let mark = Mark::take(cx);
        layers::open_dialog(
            Dialog {
                view: view.into(),
                container: kept,
                start,
                subject: None,
                main_action: None,
                keeps_focus_on_empty_click: true,
                returns_to,
                on_dismiss: None,
                placement: layers::DialogPlacement::Centered,
                dismiss_on_outside_press: false,
            },
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    match id {
        Some(id) => (id, container, shown),
        None => panic!("the dialog did not open"),
    }
}

/// Whether the layer `id` is still open.
pub(crate) fn is_open(cx: &mut VisualTestContext, id: LayerId) -> bool {
    cx.update(|window, cx| layers::is_open(id, window, cx))
}

/// Make the coordinator follow `reach` for which controls Tab reaches, as the bootstrap does with the host's answer.
pub(crate) fn follow(cx: &mut VisualTestContext, reach: &Rc<Cell<focus::TabReach>>) {
    let reach = reach.clone();
    cx.update(move |_, cx| focus::follow_tab_reach(Rc::new(move || reach.get()), cx));
}

/// What a test dialog or question is about.
pub(crate) struct Thing {
    pub(crate) name: String,
}

/// A popover menu's own content: its items.
struct Menu {
    items: Vec<(&'static str, FocusHandle)>,
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        layers::panel(cx)
            .debug_selector(|| "menu".into())
            .w(px(160.))
            .children(self.items.iter().map(|(id, focus)| button(id, focus)))
    }
}

/// Draw an anchor element under `selector` and hand its anchor out through `out`, as a row's menu button will.
pub(crate) fn anchor_element(
    selector: &'static str,
    out: &Rc<RefCell<Option<Anchor>>>,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Div {
    let anchor = Anchor::for_element(format!("{selector}-anchor"), window, cx);
    out.replace(Some(anchor.clone()));
    div()
        .relative()
        .debug_selector(move || selector.into())
        .w(px(80.))
        .child(div().h(px(24.)))
        .child(anchor.track())
}

/// Open a menu of `items` below `anchor`, starting on the first, for a mark taken now.
pub(crate) fn open_menu(
    cx: &mut VisualTestContext,
    anchor: Anchor,
    items: Vec<(&'static str, FocusHandle)>,
) -> LayerId {
    let start = items[0].1.clone();
    let id = cx.update(move |window, cx| {
        let view = cx.new(|_| Menu { items });
        let mark = Mark::take(cx);
        layers::open_popover(
            Popover::menu(view, cx.focus_handle(), anchor, start),
            &mark,
            window,
            cx,
        )
    });
    cx.run_until_parked();
    check(cx);
    match id {
        Some(id) => id,
        None => panic!("the menu did not open"),
    }
}

/// A question about a thing, frozen at `name`.
pub(crate) fn question(name: &str) -> Question<String> {
    Question::new(
        "Delete it?",
        Answer::new(
            "Delete",
            HoverHelp::tooltip("Delete it for good"),
            Tier::Danger,
        ),
        name.to_string(),
    )
    .summary(name.to_string())
    .line("Its history is removed.")
}

// ---- The fake helm, and the app serving it --------------------------------------------------------------------------

/// The sessions collection's metadata for a complete, available listing in the order `order`.
pub(crate) fn meta(order: &[&str]) -> CollectionMeta {
    CollectionMeta {
        availability: Availability::Available,
        order: order.iter().map(|key| key.to_string()).collect(),
        counts: None,
        selected_outside_query: None,
        selected_cut: false,
    }
}

/// A session row under `key`: the helm's own pinned list fixture, so the test follows the wire shape the helm serves.
pub(crate) fn session_entry(key: &str) -> KeyedEntry {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../farhelm-helm/http-contract/session-list.json"
    ))
    .unwrap();
    KeyedEntry {
        key: key.into(),
        value: fixture["sessions"][0].clone(),
    }
}

/// The sessions collection holding rows under `keys`, in that order.
pub(crate) fn sessions(keys: &[&str]) -> CollectionState {
    CollectionState {
        collection: Collection::Sessions,
        meta: meta(keys),
        entries: keys.iter().map(|key| session_entry(key)).collect(),
    }
}

/// The app, bootstrapped as for real, serving the fake helm's channel. The channel starts `Connecting`.
pub(crate) fn started(cx: &mut TestAppContext) -> (FakeHelm, Entity<Connection>) {
    install(cx);
    let (fake, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    cx.run_until_parked();
    (fake, connection)
}

/// The connection's channel state now.
pub(crate) fn state(connection: &Entity<Connection>, cx: &mut TestAppContext) -> ChannelState {
    cx.update(|cx| connection.read(cx).state().clone())
}

/// The connection's current UI session; `None` while the window is inert (before the first snapshot, after a reset).
pub(crate) fn current(
    connection: &Entity<Connection>,
    cx: &mut TestAppContext,
) -> Option<Entity<UiSession>> {
    cx.update(|cx| connection.read(cx).session().cloned())
}

/// The quiet-point consistency check (SPEC_impl.md, "What is pushed"): the current session's store must equal a fresh
/// decode of `expected`, the whole state the helm holds now.
pub(crate) fn check_store(
    connection: &Entity<Connection>,
    expected: &[CollectionState],
    cx: &mut TestAppContext,
) {
    let fresh: Vec<PushedState> = expected
        .iter()
        .cloned()
        .map(|state| decode_state(state).unwrap())
        .collect();
    let differences = cx.update(|cx| {
        let session = connection.read(cx).session().unwrap().clone();
        let store = session.read(cx).store().clone();
        store.read(cx).collections().check_against(&fresh)
    });
    assert_eq!(
        differences,
        Vec::<String>::new(),
        "the store matches a fresh snapshot"
    );
}

// ---- Shared helpers, appended on request ------------------------------------------------------------------------------
//
// The extension point for a helper several tests share (docs/gpui/foundation-ownership.md, Test harness row): an area
// appends it here, with a docstring, and its test in `shared_helper_tests` below, adding lines only. A `use` the helper
// needs goes in a `use` line of its own (or the type is named by path), because extending a grouped `use` rewraps it,
// which the ownership check refuses. The helpers above this section and the per-step checks change only by request.

/// One fresh frame's driver record: every element drawn, with its role, accessible name, state, flags and the lines of
/// text it painted, as assistive technology and the agent bridge see it. Turns the window's driver record on (it stays
/// on for the window and changes nothing it draws), redraws, and reads the nodes. Turning the record on and the redraw
/// each render the window, which a test that counts renders must allow for.
pub(crate) fn frame(cx: &mut VisualTestContext) -> Vec<gpui::DriverNode> {
    cx.update(|window, _| window.set_driver_enabled(true));
    redraw(cx);
    cx.update(|window, _| window.driver_nodes().to_vec())
}

/// The id of the first element holding focus, among those with an id, in a fresh [`frame`]. Unlike [`focused`], which
/// asks whether a handle the test holds has focus, this names whatever has it, for a test that holds no handle (a GPUI
/// Kit field's, a row's own).
pub(crate) fn focused_id(cx: &mut VisualTestContext) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.flags.focused && node.element_id.is_some())
        .and_then(|node| node.element_id.map(|id| id.to_string()))
}

/// Whether any dialog or popover is open in the window, as the app's shortcuts that refuse while one is open ask it
/// ([`layers::any_open`]): a popover that never takes focus counts, and the pill does not.
pub(crate) fn any_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// Whether focus is inside a session terminal: the focused element's key context path, in the last drawn frame,
/// includes the terminal's context ([`crate::actions::TERMINAL_CONTEXT`]), which is where the terminal's key bindings
/// apply. Call it after a step helper, which draws; a focus change since the last frame is judged against that frame.
///
/// It asks the keymap's own constant rather than an element id (`terminal-grid`) or a handle: a renamed id would let
/// every "focus is not on the terminal" assertion pass without checking anything, while the constant is the one the
/// terminal sets, and a handle needs the terminal view, which a test of another area does not hold.
pub(crate) fn terminal_focused(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| {
        window
            .context_stack()
            .iter()
            .any(|context| context.contains(crate::actions::TERMINAL_CONTEXT))
    })
}

/// The accessible name of what is drawn under element id `id` in a fresh [`frame`], or `None` when nothing with that id
/// was drawn or it has no name. For a test that reads a line the app wrote (a refusal, a status, a host value drawn
/// through a host sentence, whose name is its escaped text) or a control's label, by the id the view gave it.
pub(crate) fn label(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// The one command the fake helm received since its commands were last taken, which must be `method` on `path`: its
/// envelope id, to reply to, and its body (`null` when it had none). Panics when anything else, or more or less than one
/// command, was sent, so a test that expects one request also proves nothing else went out.
pub(crate) fn one_command(
    helm: &mut FakeHelm,
    method: farhelm_helm_client::Method,
    path: &str,
) -> (u64, serde_json::Value) {
    let sent = helm.commands();
    assert_eq!(sent.len(), 1, "one command was sent: {sent:?}");
    assert_eq!((sent[0].method, sent[0].path.as_str()), (method, path));
    (sent[0].id, sent[0].body.clone().unwrap_or_default())
}

/// The real main view over the fake helm, in an active window, with the helm's side of every terminal stream its pane
/// opened. Made by [`open_main`]; [`MainWindow::terminal_live`] gives it a live, focused terminal, the starting point of
/// a test about a key pressed from the terminal (a chord that must open something and never reach the program).
///
/// A test module adds steps of its own as inherent methods in its own file (`impl MainWindow<'_>`, usually through a
/// local `type Main<'a> = MainWindow<'a>`), so its call sites read `main.step()`. NOTE: Those methods share one
/// namespace across the crate: two modules that pick the same name fail to compile, and the later one renames its own.
pub(crate) struct MainWindow<'a> {
    /// The window's context, for the step helpers.
    pub(crate) cx: &'a mut VisualTestContext,
    /// The helm's side of the channel: the commands the window sent, and the replies and pushes the test makes.
    pub(crate) helm: FakeHelm,
    /// The helm's side of each terminal stream the pane opened, in the order it opened them. The first is the terminal
    /// of the session open at startup.
    pub(crate) terminals: Rc<RefCell<Vec<FakeTerminal>>>,
}

/// The main view connected to the fake helm with `snapshot` (the app bootstrapped as for real, through [`started`]),
/// its connection serving fake terminal streams, open in an active window and checked once settled.
///
/// On return the channel is connected with `snapshot` taken in. If the snapshot's preferences open a session (the last
/// visited) on a host the helm reaches, its pane has opened that session's terminal stream, recorded first in
/// `terminals`, which is still attaching: no socket is open and nothing is replayed; a stale session mounts the
/// unavailable card instead, and opens no stream. Focus is wherever the main view puts it at startup, which with a
/// reachable session open is that terminal. Nothing else is assumed about the snapshot, so a test of the
/// state before the preferences arrive passes a snapshot without them.
pub(crate) fn open_main(cx: &mut TestAppContext, snapshot: Vec<CollectionState>) -> MainWindow<'_> {
    let (mut helm, connection) = started(cx);
    let terminals: Rc<RefCell<Vec<FakeTerminal>>> = Rc::default();
    let record = terminals.clone();
    cx.update(|cx| {
        connection.update(cx, |connection, _| {
            connection.serve_terminals(move |attach| {
                let (terminal, handle, events) = FakeTerminal::open(&attach);
                record.borrow_mut().push(terminal);
                (handle, events)
            })
        })
    });
    helm.connect(snapshot);
    cx.run_until_parked();
    let (_, cx) = open_view(cx, MainView::new);
    MainWindow {
        cx,
        helm,
        terminals,
    }
}

impl MainWindow<'_> {
    /// Bring the first terminal the pane opened live: its socket opens, it replays a [`blank_screen`] at the size its
    /// attach asked for and ends the replay, and the window settles.
    ///
    /// On return the attachment is live, so input the terminal sends is delivered; the terminal holds focus, which is
    /// asserted here because every caller's test rests on it; and [`MainWindow::program_input`] counts from this point
    /// (whatever the terminal sent before is taken). Panics when the pane opened no terminal or focus is elsewhere.
    pub(crate) fn terminal_live(&mut self) {
        {
            let mut terminals = self.terminals.borrow_mut();
            let Some(terminal) = terminals.first_mut() else {
                panic!("the premise: the pane opened a terminal");
            };
            let asked = terminal.attach();
            for message in blank_screen(asked.cols, asked.rows) {
                terminal.output(message);
            }
            terminal.replay_complete();
        }
        settle(self.cx);
        assert!(
            terminal_focused(self.cx),
            "the premise: the open session's terminal has focus"
        );
        self.program_input();
    }

    /// How many input requests the first terminal the pane opened sent its program since the last call (or since
    /// [`MainWindow::terminal_live`]), delivered or not: a key the app should have kept counts even when the stream
    /// dropped it. Takes everything else the terminal sent too (a resize, a take-control), which it does not count.
    pub(crate) fn program_input(&mut self) -> usize {
        self.terminals.borrow_mut()[0]
            .received()
            .into_iter()
            .filter(|request| matches!(request, FakeTerminalRequest::Input { .. }))
            .count()
    }
}

/// A complete first screen of `cols` by `rows` with nothing on it, as the supervisor replays one to a new attachment:
/// the stream's hello (no history), the terminal's modes (no mouse or focus reporting, the main screen, nothing trimmed,
/// no exit) and one frame with a visible, steady block cursor at the top left. A test that needs text on the screen
/// inserts its `rows` message before the frame.
pub(crate) fn blank_screen(cols: u16, rows: u16) -> Vec<GridOutput> {
    [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        serde_json::json!({"t":"frame","rev":1,"cols":cols,"rows":rows,
            "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    ]
    .into_iter()
    .map(|message| serde_json::from_value(message).unwrap())
    .collect()
}

/// Tests of the shared helpers above, one per helper, appended with it.
mod shared_helper_tests {
    use gpui::{IntoElement as _, ParentElement as _, TestAppContext, div};

    use super::{any_open, handle, open_dialog, settle, terminal_focused};
    use super::{button, focused, focused_id, keys, open};
    use gpui::{InteractiveElement as _, Styled as _};
    use std::cell::Cell;
    use std::rc::Rc;

    /// Why this matters: tests that hold no focus handle (a GPUI Kit field's, a list row's own) name the focused
    /// element by its id, and a helper that named nothing, or an element other than the focused one, would let a focus
    /// bug pass. Specified: with nothing focused there is no id; when Tab has put focus on the first of two buttons
    /// (checked through its handle) its id is named, and after one more Tab the second's.
    #[gpui::test]
    fn focused_id_names_the_element_holding_focus(cx: &mut TestAppContext) {
        let first = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let second = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let (drawn_first, drawn_second) = (first.clone(), second.clone());
        let cx = open(cx, move |_, _| {
            div()
                .child(button("first", &drawn_first))
                .child(button("second", &drawn_second))
                .into_any_element()
        });
        assert!(
            !focused(cx, &first) && !focused(cx, &second),
            "the premise: neither button has focus yet"
        );
        assert_eq!(focused_id(cx), None, "nothing is named");
        keys(cx, "tab");
        assert!(
            focused(cx, &first),
            "the premise: Tab reached the first button"
        );
        assert_eq!(focused_id(cx).as_deref(), Some("first"));
        keys(cx, "tab");
        assert!(
            focused(cx, &second),
            "the premise: Tab reached the second button"
        );
        assert_eq!(focused_id(cx).as_deref(), Some("second"));
    }

    /// Why this matters: the tests of Rename, Delete, Settings, Add host and the quick switcher ask whether anything is
    /// open to check that a shortcut opened, or refused to open, its dialog, and a helper that always said no (or yes)
    /// would let either bug pass. Specified: with nothing open it says no; with a dialog open (checked by its id) it
    /// says yes; once Escape has closed the dialog it says no again.
    #[gpui::test]
    fn any_open_says_whether_a_dialog_is_open(cx: &mut TestAppContext) {
        let opener = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let drawn = opener.clone();
        let cx = open(cx, move |_, _| button("opener", &drawn).into_any_element());
        assert!(!any_open(cx), "nothing is open yet");
        let start = handle(cx);
        let (id, _, _) = open_dialog(cx, vec![("inside", start.clone())], &start);
        assert!(super::is_open(cx, id), "the premise: the dialog is open");
        assert!(any_open(cx), "the open dialog counts");
        keys(cx, "escape");
        assert!(
            !super::is_open(cx, id),
            "the premise: Escape closed the dialog"
        );
        assert!(!any_open(cx), "nothing is open once it closed");
    }

    /// Why this matters: tests across areas check that a closed dialog or a switched session hands focus back to the
    /// terminal, and that a shortcut leaves it there; a helper that answered from anything but the focused element's
    /// place would let focus land elsewhere unnoticed. Specified, with a stand-in in the terminal's key context beside an
    /// ordinary button: with nothing focused it says no; with the button focused (checked through its handle) no; once
    /// the stand-in is chosen as a session view chooses its terminal (checked the same way) yes.
    #[gpui::test]
    fn terminal_focused_says_whether_focus_is_in_a_terminal(cx: &mut TestAppContext) {
        let other = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let terminal = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let (drawn_other, drawn_terminal) = (other.clone(), terminal.clone());
        let cx = open(cx, move |_, _| {
            let stand_in = div()
                .key_context(crate::actions::TERMINAL_CONTEXT)
                .size(gpui::px(40.));
            div()
                .child(button("other", &drawn_other))
                .child(crate::focus::track(
                    stand_in,
                    &drawn_terminal,
                    crate::focus::OnPress::Focus,
                ))
                .into_any_element()
        });
        assert!(
            !focused(cx, &other) && !focused(cx, &terminal),
            "the premise: nothing is focused yet"
        );
        assert!(!terminal_focused(cx), "nothing focused is not the terminal");
        keys(cx, "tab");
        assert!(focused(cx, &other), "the premise: Tab reached the button");
        assert!(
            !terminal_focused(cx),
            "a button outside it is not the terminal"
        );
        cx.update(|window, cx| {
            let mark = crate::focus::Mark::take(cx);
            crate::focus::choose_terminal(&terminal, &mark, window, cx);
        });
        settle(cx);
        assert!(
            focused(cx, &terminal),
            "the premise: the stand-in took focus"
        );
        assert!(terminal_focused(cx), "the terminal's own element is");
    }

    /// Why this matters: tests read the app's lines and controls' names by id, and a helper that read another element's
    /// name, or answered for an id that was not drawn, would let a wrong or missing line pass. Specified: a button drawn
    /// under `named` with the label "Shown words" reads as that label; an id nothing drew reads as `None`.
    #[gpui::test]
    fn label_reads_the_name_drawn_under_an_id(cx: &mut TestAppContext) {
        let focus = cx.update(|cx| cx.focus_handle().tab_stop(true));
        let cx = open(cx, move |_, _| {
            div()
                .child(
                    crate::components::Button::new(
                        "named",
                        "Shown words",
                        crate::components::HoverHelp::tooltip("Help"),
                        crate::components::Role::Button,
                    )
                    .focus(&focus),
                )
                .into_any_element()
        });
        assert_eq!(super::label(cx, "named").as_deref(), Some("Shown words"));
        assert_eq!(super::label(cx, "never-drawn"), None);
    }

    /// Why this matters: tests that expect exactly one request use this to read its id and body, and a helper that
    /// returned a stale command, or let a second one pass unseen, would hide a duplicate or missing request.
    /// Specified: after one keyless send of `{"a": 1}` on `POST /api/example` through a live UI session, it returns that
    /// command's body and an id whose reply reaches the send's callback, and the fake's command list is empty
    /// afterwards.
    #[gpui::test]
    fn one_command_takes_the_single_command_sent(cx: &mut TestAppContext) {
        let (mut helm, _connection) = super::started(cx);
        helm.connect(Vec::new());
        cx.run_until_parked();
        let answered = Rc::new(Cell::new(false));
        let noted = answered.clone();
        let sent = cx.update(|cx| {
            let path = farhelm_helm_client::ApiPath::api(&["example"]).unwrap();
            crate::operations::send(
                farhelm_helm_client::Method::Post,
                path,
                Some(serde_json::json!({ "a": 1 })),
                "POST /api/example",
                cx,
                move |_, _| noted.set(true),
            )
        });
        assert!(sent, "the premise: a live UI session sent it");
        cx.run_until_parked();
        let (id, body) =
            super::one_command(&mut helm, farhelm_helm_client::Method::Post, "/api/example");
        assert_eq!(body, serde_json::json!({ "a": 1 }));
        assert!(helm.commands().is_empty(), "the command was taken");
        helm.reply(id, 200, None);
        cx.run_until_parked();
        assert!(
            answered.get(),
            "the id is the command's: its reply reached the send"
        );
    }

    /// Why this matters: the tests of the chords that work from a focused terminal (the quick switcher's, the launch
    /// sequence's prefix) start here and read "never reached the program" off the input count. A terminal that is not
    /// live sends its program nothing, so a helper that left it attaching would make every zero vacuous, and one that
    /// left it unfocused, or miscounted, would let a leaking chord pass. Specified: over sessions a and b with a the
    /// last visited, the pane opens one terminal, focused while it attaches; once live it still has focus and nothing
    /// is counted; a typed letter is delivered as one input request, counted once, and the next count is zero.
    #[gpui::test]
    fn open_main_and_terminal_live_give_a_focused_terminal_that_takes_input(
        cx: &mut TestAppContext,
    ) {
        use farhelm_helm_client::fake::terminal::FakeTerminalRequest;
        use farhelm_proto::ui_channel::{
            Collection, CollectionState, KeyedEntry, ListCounts, PREFERENCES_KEY, Preferences,
        };

        // Built from this file's own fixtures, so the harness's test depends on no area's: the helm's session rows
        // under ids a and b with complete counts, and preferences with a last visited, which opens a at startup.
        let mut listing = super::sessions(&["a", "b"]);
        for entry in &mut listing.entries {
            entry.value["id"] = serde_json::json!(entry.key);
            // No terminal tabs, whatever the helm fixture lists: each tab of the open session attaches a terminal of
            // its own, and this test counts a's agent terminal alone.
            entry.value["tabs"] = serde_json::json!([]);
            // On a host the helm reaches: the fixture's row is on one it cannot, which opens no terminal.
            entry.value["stale"] = serde_json::json!(false);
        }
        listing.meta.counts = Some(ListCounts {
            total: 2,
            matching: None,
            truncated: false,
        });
        let preferences = Preferences {
            visit_history: vec!["a".to_owned()],
            // A helm that has shown the Welcome card, which would otherwise open over the main view and could take
            // focus from the terminal this test needs focused (`crate::welcome`).
            welcome_seen: Some(true),
            ..Preferences::default()
        };
        let preferences = CollectionState {
            collection: Collection::Preferences,
            meta: super::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::to_value(preferences).unwrap(),
            }],
        };
        let mut main = super::open_main(cx, vec![listing, preferences]);
        assert_eq!(main.terminals.borrow().len(), 1, "a's terminal alone");
        assert!(terminal_focused(main.cx), "focused while it attaches");
        main.terminal_live();
        assert!(terminal_focused(main.cx), "focused once live");
        assert_eq!(main.program_input(), 0, "nothing counted yet");
        keys(main.cx, "x");
        let sent = main.terminals.borrow_mut()[0].received();
        assert!(
            matches!(
                sent.as_slice(),
                [FakeTerminalRequest::Input {
                    delivered: true,
                    ..
                }]
            ),
            "the letter was delivered: {sent:?}"
        );
        keys(main.cx, "y");
        assert_eq!(main.program_input(), 1, "one letter, one request");
        assert_eq!(main.program_input(), 0, "counted since the last call");
    }
}

// ---- Self-tests: every step helper checks ----------------------------------------------------------------------------

/// Proof that each step helper runs the invariant checks. Nothing else would notice a helper losing its [`check`]: the
/// checks fail only when the code under test breaks an invariant, and in a passing suite nothing does. So each test
/// breaks one on purpose, with a popover whose anchor never draws its tracker (which [`layers::invariant_violation`]
/// reports), at a moment when only the step under test can see it: the window is clean when [`open`] checks it, and the
/// popover is opened without a check, or by the step itself. Each test therefore panics in that step's check or not at
/// all.
mod self_tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use gpui::{
        AnyElement, App, AppContext as _, Context, IntoElement, Render, TestAppContext,
        VisualTestContext, Window, div,
    };

    use super::{Scene, advance, another_window, click, keys, open, redraw, settle};
    use crate::components::{Button, HoverHelp, Role};
    use crate::focus::Mark;
    use crate::layers::{self, Anchor, Popover};
    use crate::root;

    /// The anchor the scene drew last, shared with the test and the scene's button.
    type Shared = Rc<RefCell<Option<Anchor>>>;

    /// A popover's content: nothing, since only its anchor matters here.
    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    /// Open the harness on a scene drawn by [`breaking_draw`].
    fn scene(cx: &mut TestAppContext, at_first_draw: bool) -> (&mut VisualTestContext, Shared) {
        let (draw, anchor) = breaking_draw(at_first_draw);
        (open(cx, draw), anchor)
    }

    /// A scene's draw function holding an anchor whose tracker it never draws, and a button, `opener`, that opens a
    /// popover on that anchor when clicked. With `at_first_draw`, the first frame also opens one (deferred, as no view
    /// may open a layer while it draws), so the window is already broken when it is first checked.
    fn breaking_draw(
        at_first_draw: bool,
    ) -> (
        impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        Shared,
    ) {
        let anchor: Shared = Rc::new(RefCell::new(None));
        let (drawn, first) = (anchor.clone(), Cell::new(at_first_draw));
        let draw = move |window: &mut Window, cx: &mut App| {
            drawn.replace(Some(Anchor::for_element("untracked-anchor", window, cx)));
            if first.replace(false) {
                let deferred = drawn.clone();
                window.defer(cx, move |window, cx| open_popover_on(&deferred, window, cx));
            }
            let clicked = drawn.clone();
            Button::new(
                "opener",
                "Open",
                HoverHelp::tooltip("Opens the test popover"),
                Role::Button,
            )
            .on_click(move |_, window, cx| open_popover_on(&clicked, window, cx))
            .into_any_element()
        };
        (draw, anchor)
    }

    /// Open a popover on the scene's untracked anchor. Its premises panic with their own messages, which never match a
    /// test's expected one, so a broken fixture fails its test instead of passing it.
    fn open_popover_on(anchor: &Shared, window: &mut Window, cx: &mut App) {
        let Some(anchor) = anchor.borrow().clone() else {
            panic!("the premise: the scene drew its anchor");
        };
        let view = cx.new(|_| Empty);
        let mark = Mark::take(cx);
        let popover = Popover::menu(view, cx.focus_handle(), anchor, cx.focus_handle());
        if layers::open_popover(popover, &mark, window, cx).is_none() {
            panic!("the premise: the popover opened");
        }
    }

    /// Break the invariant without checking: what a step the harness does not wrap would do.
    fn break_unchecked(cx: &mut VisualTestContext, anchor: &Shared) {
        cx.update(|window, cx| open_popover_on(anchor, window, cx));
    }

    /// [`open`] checks the window once its first frame has settled, so a scene broken from the start fails there
    /// rather than at some later step that tripped over it.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn open_checks_the_first_frame(cx: &mut TestAppContext) {
        scene(cx, true);
    }

    /// [`redraw`] checks after the frame it asked for.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn redraw_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        redraw(cx);
    }

    /// [`keys`] checks after the keys are handled.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn keys_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        keys(cx, "a");
    }

    /// [`click`] checks after the click is handled: here the click itself breaks the invariant.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn click_checks(cx: &mut TestAppContext) {
        let (cx, _) = scene(cx, false);
        click(cx, "opener");
    }

    /// [`advance`] checks after what the clock woke has run.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn advance_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        advance(cx, Duration::from_millis(1));
    }

    /// [`settle`] checks once what was queued has run.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn settle_checks(cx: &mut TestAppContext) {
        let (cx, anchor) = scene(cx, false);
        break_unchecked(cx, &anchor);
        settle(cx);
    }

    /// [`another_window`] checks the window it opens once its first frame has settled, as [`open`] does for the first:
    /// here the second window is the broken one, and the first is clean.
    #[gpui::test]
    #[should_panic(expected = "invariant broken")]
    fn another_window_checks_its_first_frame(cx: &mut TestAppContext) {
        let (cx, _) = scene(cx, false);
        let (draw, _) = breaking_draw(true);
        another_window(cx, |window, cx| Scene {
            draw: Box::new(draw),
            root: root::attach(window, cx),
        });
    }
}

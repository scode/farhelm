//! Tests of the sidebar: the app header, the width the helm stores, and the divider's drag, double-click and arrows.
//!
//! Everything is tested in the real main view over the helm client's fake, which hands the test every command the
//! window sent and lets it push preference changes and answer commands as the helm would. The fleet is empty, so the
//! only commands the window sends are the sidebar's own (with sessions listed, the selection rules write their own
//! preference too), except in the tests of Tab's order and of a press beside the divider, which need a session open so
//! the pane holds a terminal. Widths are read from GPUI's driver record of a fresh frame, where the sidebar's column is
//! the element drawn under `sidebar`. The one layout test, the main pane's floor, uses the real text system, as every
//! test whose outcome is layout does.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, DriverNode, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, Point, Role, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext,
    point, px, size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use farhelm_proto::BUILD_VERSION;

use super::default_width;
use super::header::{version_help, wordmark};
use crate::main_view::MainView;
use crate::session_list::tests::{listing, session};
use crate::test_support::{self, frame, meta, started};
use crate::{assets, bootstrap, focus, layers, theme};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The preferences entry with `width` as the stored sidebar width.
fn preferences_entry(width: Option<u32>) -> KeyedEntry {
    let preferences = Preferences {
        sidebar_width: width,
        ..Preferences::default()
    };
    KeyedEntry {
        key: PREFERENCES_KEY.into(),
        value: serde_json::to_value(preferences).unwrap(),
    }
}

/// An empty fleet with the helm storing `width` as the sidebar's width (`None`: unset).
fn snapshot(width: Option<u32>) -> Vec<CollectionState> {
    fleet(width, &[])
}

/// A fleet of `sessions`, the first of them open, with the helm storing `width` as the sidebar's width.
fn fleet(width: Option<u32>, sessions: &[&'static str]) -> Vec<CollectionState> {
    let rows: Vec<_> = sessions.iter().map(|key| session(key)).collect();
    vec![
        listing(sessions, &rows),
        CollectionState {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            entries: vec![preferences_entry(width)],
        },
    ]
}

/// The helm pushing `width` as the stored sidebar width.
fn push_width(fake: &mut FakeHelm, width: Option<u32>) {
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![preferences_entry(width)],
        remove: vec![],
    });
}

/// The real main view over the fake helm, connected with an empty fleet storing `width`, in an active window.
fn open(cx: &mut TestAppContext, width: Option<u32>) -> (FakeHelm, &mut VisualTestContext) {
    open_over(cx, snapshot(width))
}

/// The real main view over the fake helm, connected with `snapshot`, in an active window.
fn open_over(
    cx: &mut TestAppContext,
    snapshot: Vec<CollectionState>,
) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot);
    cx.run_until_parked();
    let (_, window) = test_support::open_view(cx, MainView::new);
    (fake, window)
}

/// The widths the window wrote since the last call, in order, failing on any other command. The commands stay
/// unanswered, as a helm that has not replied yet.
fn written(fake: &mut FakeHelm) -> Vec<u64> {
    widths(&fake.commands())
}

/// The widths `commands` write, in order, failing on a command that is not a write of the sidebar's width alone.
fn widths(commands: &[FakeCommand]) -> Vec<u64> {
    commands
        .iter()
        .map(|command| {
            assert_eq!(command.path, "/api/preferences", "only the sidebar writes");
            let body = command.body.clone().unwrap_or_default();
            match body.get("sidebar_width").and_then(Value::as_u64) {
                Some(width) if body.as_object().is_some_and(|fields| fields.len() == 1) => width,
                _ => panic!("not a sidebar width write: {body}"),
            }
        })
        .collect()
}

/// The node drawn under `id` in `nodes`.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> &'a DriverNode {
    match nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
    {
        Some(node) => node,
        None => panic!("{id} was not drawn"),
    }
}

/// The sidebar column's width in a fresh frame.
fn width(cx: &mut VisualTestContext) -> Pixels {
    node(&frame(cx), "sidebar").bounds.size.width
}

/// Where a press on the divider lands: its middle, partway down.
fn on_divider(cx: &mut VisualTestContext) -> Point<Pixels> {
    test_support::center(cx, "sidebar-divider")
}

/// Press the primary button at `at`, as the first press of a click (`click_count` 1) or the second of a double-click.
fn press(cx: &mut VisualTestContext, at: Point<Pixels>, click_count: usize) {
    cx.simulate_event(MouseDownEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count,
        first_mouse: false,
    });
    test_support::check(cx);
}

/// Move the pointer to `at` with the primary button still held (`held`) or not.
fn move_to(cx: &mut VisualTestContext, at: Point<Pixels>, held: bool) {
    cx.simulate_mouse_move(at, held.then_some(MouseButton::Left), Modifiers::none());
    test_support::check(cx);
}

/// Release the primary button at `at`.
fn release(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_event(MouseUpEvent {
        position: at,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 1,
    });
    test_support::check(cx);
}

/// `at` moved `dx` pixels sideways.
fn shifted(at: Point<Pixels>, dx: f32) -> Point<Pixels> {
    point(at.x + px(dx), at.y)
}

// ---- The app header ------------------------------------------------------------------------------------------------

/// The header is the window's identity: the wordmark says which app this is, and the version says which build, which is
/// the first thing a bug report needs. Specified: the header draws an image named "farhelm" and a readout named and
/// painted as this build's version, whose hover help is what [`version_help`] says of it; for a development build that
/// is "This is a development build of Farhelm, not a release.", and for a release "This is Farhelm <version>.".
#[gpui::test]
fn the_header_shows_the_wordmark_and_the_build_with_its_hover_help(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, None);
    let nodes = frame(cx);
    let mark = node(&nodes, "app-wordmark");
    assert_eq!(
        (mark.role, mark.label.as_deref()),
        (Some(Role::Image), Some("farhelm"))
    );
    assert_eq!(
        node(&nodes, "app-version").label.as_deref(),
        Some(BUILD_VERSION)
    );
    assert!(
        crate::session_list::tests::painted(&nodes)
            .iter()
            .any(|text| text == BUILD_VERSION),
        "the version is painted"
    );

    let readout = test_support::center(cx, "app-version");
    cx.simulate_mouse_move(readout, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    let help = cx
        .update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string());
    assert_eq!(help, Some(version_help(BUILD_VERSION)));
    assert_eq!(
        version_help("0.0.0-unreleased"),
        "This is a development build of Farhelm, not a release."
    );
    assert_eq!(version_help("1.4.2"), "This is Farhelm 1.4.2.");
}

/// The two wordmark files differ only in their colors, so drawing the wrong one for the ground leaves a mark that
/// vanishes into it. Specified: the dark palette draws the dark ground's file and the light palette the light one's.
#[test]
fn the_wordmark_is_the_file_for_the_appearances_ground() {
    assert_eq!(wordmark(&theme::DARK), assets::WORDMARK_DARK);
    assert_eq!(wordmark(&theme::LIGHT), assets::WORDMARK_LIGHT);
}

// ---- The stored width ----------------------------------------------------------------------------------------------

/// The width is a helm preference shared by every client (SPEC.md, "Session list"), so the sidebar must draw what the
/// helm stores and follow it when another client changes it, and fall back to the default where nothing is stored.
/// Specified: with nothing stored the sidebar is 396 pixels wide; a pushed 500 redraws it at 500; a pushed unset
/// returns it to 396; a pushed 9000, outside the range (which no helm sends, since it reads such a stored value back as
/// unset), draws 396 too; and drawing writes nothing. (A window opened over a stored width is the next test.)
#[gpui::test]
fn the_sidebar_draws_the_stored_width_and_follows_a_pushed_change(cx: &mut TestAppContext) {
    let (mut fake, window) = open(cx, None);
    assert_eq!(width(window), px(default_width() as f32));
    push_width(&mut fake, Some(500));
    window.run_until_parked();
    assert_eq!(width(window), px(500.));
    push_width(&mut fake, None);
    window.run_until_parked();
    assert_eq!(width(window), px(396.));
    push_width(&mut fake, Some(9000));
    window.run_until_parked();
    assert_eq!(
        width(window),
        px(396.),
        "a width outside the range is no width"
    );
    assert_eq!(
        written(&mut fake),
        Vec::<u64>::new(),
        "drawing writes nothing"
    );
}

/// A stored width is where the sidebar starts, not only where a pushed change takes it: the reset and every new window
/// draw from the snapshot alone. Specified: a window opened over a stored 600 draws the sidebar 600 pixels wide.
#[gpui::test]
fn a_window_opens_at_the_stored_width(cx: &mut TestAppContext) {
    let (_fake, window) = open(cx, Some(600));
    assert_eq!(width(window), px(600.));
}

// ---- The divider ---------------------------------------------------------------------------------------------------

/// Assistive technology and the test bridge find the divider by its role and name, and hear the width through its
/// value (the old handle's `role="separator"` with `aria-valuenow`). Specified: the divider is a splitter (AccessKit's
/// focusable separator) named "Sidebar width" whose value is the stored width, 520, and after a step 530.
#[gpui::test]
fn the_divider_is_a_named_separator_whose_value_is_the_width(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, Some(520));
    let nodes = frame(cx);
    let divider = node(&nodes, "sidebar-divider");
    assert_eq!(divider.role, Some(Role::Splitter));
    assert_eq!(divider.label.as_deref(), Some("Sidebar width"));
    assert_eq!(divider.value.as_deref(), Some("520"));
    let at = on_divider(cx);
    press(cx, at, 1);
    release(cx, at);
    test_support::keys(cx, "right");
    assert_eq!(
        node(&frame(cx), "sidebar-divider").value.as_deref(),
        Some("530")
    );
}

/// A drag must follow the pointer on every move (the sidebar is what the user is looking at while dragging) and tell
/// the helm only the width it ended at: writing every pointer sample would flood the channel and every other client
/// with widths nobody chose. The width stays within the helm's range, which refuses anything outside it. Specified:
/// from the default, moves of +30 and +90 draw 426 and 486 with nothing written; the release writes 486 once and the
/// sidebar stays at 486; a second drag of +500 draws 656 and writes 656; a third of -500 draws 296 and writes 296.
#[gpui::test]
fn a_drag_follows_every_move_writes_once_and_clamps_at_both_ends(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    let start = on_divider(cx);
    press(cx, start, 1);
    move_to(cx, shifted(start, 30.), true);
    assert_eq!(width(cx), px(426.));
    move_to(cx, shifted(start, 90.), true);
    assert_eq!(width(cx), px(486.));
    assert_eq!(
        written(&mut fake),
        Vec::<u64>::new(),
        "nothing during the drag"
    );
    release(cx, shifted(start, 90.));
    assert_eq!(written(&mut fake), [486]);
    assert_eq!(width(cx), px(486.), "held until the helm answers");

    for (dx, clamped) in [(500., 656), (-500., 296)] {
        let start = on_divider(cx);
        press(cx, start, 1);
        move_to(cx, shifted(start, dx), true);
        assert_eq!(width(cx), px(clamped as f32));
        release(cx, shifted(start, dx));
        assert_eq!(written(&mut fake), [clamped]);
    }
}

/// A release the window never sees (made outside it, or taken by another app) must not leave the sidebar following
/// the pointer, and must still save what the user saw, as the old handle's cancelled capture kept its last width.
/// Specified: after a press and a move of +50 (446), a move with the button no longer held ends the drag: the sidebar
/// stays at 446, 446 is written once, and further moves change nothing.
#[gpui::test]
fn a_drag_whose_release_was_lost_keeps_its_last_width(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    let start = on_divider(cx);
    press(cx, start, 1);
    move_to(cx, shifted(start, 50.), true);
    assert_eq!(
        width(cx),
        px(446.),
        "premise: the drag is following the pointer"
    );
    move_to(cx, shifted(start, 120.), false);
    assert_eq!(width(cx), px(446.));
    assert_eq!(written(&mut fake), [446]);
    move_to(cx, shifted(start, 200.), true);
    assert_eq!(width(cx), px(446.), "the drag is over");
    assert_eq!(written(&mut fake), Vec::<u64>::new());
}

/// Double-click is the way back to the default (SPEC.md), and its first click must not count as a drag of its own.
/// Specified: over a stored 500, a press and release with no move writes nothing; the second press of a double-click
/// then draws 396 and writes 396, once.
#[gpui::test]
fn a_double_click_resets_to_the_default_and_writes_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, Some(500));
    let at = on_divider(cx);
    press(cx, at, 1);
    release(cx, at);
    assert_eq!(
        written(&mut fake),
        Vec::<u64>::new(),
        "a click that moved nothing"
    );
    press(cx, at, 2);
    release(cx, at);
    assert_eq!(width(cx), px(396.));
    assert_eq!(written(&mut fake), [396]);
}

/// The arrows are the divider's keyboard path (SPEC.md: "focus the separator and use Left and Right to step by
/// 10px"). Each press writes, so a second press before the helm answers must step from what the first one showed, not
/// from the store's older width; and the divider keeps focus, or the next press would go elsewhere. Specified: with
/// the divider focused over a stored 300, Right writes 310 and Right again 320 before any answer, the sidebar showing
/// 320; the first reply (with the helm's push of 310) leaves 320 shown; Left Left Left from there write 310, 300 and
/// 296 (clamped); a stored 656 then Right writes 656; and focus is on the divider throughout.
#[gpui::test]
fn the_arrows_step_clamp_write_each_press_and_keep_focus(cx: &mut TestAppContext) {
    let (mut fake, window) = open(cx, Some(300));
    let at = on_divider(window);
    press(window, at, 1);
    release(window, at);
    assert!(
        divider_focused(window),
        "premise: the press focused the divider"
    );

    test_support::keys(window, "right");
    test_support::keys(window, "right");
    let commands = fake.commands();
    assert_eq!(widths(&commands), [310, 320]);
    assert_eq!(width(window), px(320.));
    push_width(&mut fake, Some(310));
    fake.reply(commands[0].id, 204, None);
    window.run_until_parked();
    assert_eq!(
        width(window),
        px(320.),
        "an earlier reply leaves the latest held"
    );
    push_width(&mut fake, Some(320));
    fake.reply(commands[1].id, 204, None);
    window.run_until_parked();

    test_support::keys(window, "left left left");
    let commands = fake.commands();
    assert_eq!(widths(&commands), [310, 300, 296]);
    assert!(divider_focused(window));

    push_width(&mut fake, Some(656));
    for command in &commands {
        fake.reply(command.id, 204, None);
    }
    window.run_until_parked();
    assert_eq!(
        width(window),
        px(656.),
        "premise: the helm stores 656 and nothing is held"
    );
    test_support::keys(window, "right");
    assert_eq!(written(&mut fake), [656]);
    assert!(divider_focused(window));
}

/// A refused write must not leave the sidebar at a width the helm does not hold. Specified: Right over a stored 400
/// draws 410; the helm refusing it puts the sidebar back at 400.
#[gpui::test]
fn a_refused_write_shows_what_the_helm_stores(cx: &mut TestAppContext) {
    let (mut fake, window) = open(cx, Some(400));
    let at = on_divider(window);
    press(window, at, 1);
    release(window, at);
    test_support::keys(window, "right");
    assert_eq!(width(window), px(410.));
    let commands = fake.commands();
    assert_eq!(commands.len(), 1, "premise: one write");
    fake.reply(commands[0].id, 400, Some(json!("refused")));
    window.run_until_parked();
    assert_eq!(width(window), px(400.));
}

/// The divider is split in two so that Tab meets it between the sidebar and the pane, as it met the old handle, and it
/// is a separator rather than a button, so Tab reaches it wherever Tab reaches anything: on a Mac with Keyboard
/// navigation off (the system default), plain Tab skips buttons but not the divider. Specified, with a session open
/// (its terminal in the pane), Tab reaching every control, and the divider focused: Shift+Tab goes to a control inside
/// the sidebar's column; then, with Tab reaching text fields and pop-ups only, Tab comes back to the divider (the
/// sidebar holds no text field, so a divider counted as a button would be skipped for the terminal); and Tab again goes
/// into the pane.
#[gpui::test]
fn tab_meets_the_divider_between_the_list_and_the_pane(cx: &mut TestAppContext) {
    let (_fake, cx) = open_over(cx, fleet(None, &["a"]));
    let reach = Rc::new(Cell::new(focus::TabReach::EveryControl));
    let source = reach.clone();
    cx.update(|_, cx| focus::follow_tab_reach(Rc::new(move || source.get()), cx));
    let at = on_divider(cx);
    press(cx, at, 1);
    release(cx, at);
    assert!(
        divider_focused(cx),
        "premise: the press focused the divider"
    );
    let column = node(&frame(cx), "sidebar").bounds;

    test_support::keys(cx, "shift-tab");
    let before = focused_node(cx);
    assert!(
        before.bounds.right() <= column.right()
            && before.element_id.as_deref() != Some("sidebar-divider"),
        "Shift+Tab went to the sidebar's last control: {:?}",
        before.element_id
    );
    reach.set(focus::TabReach::TextFieldsAndPopups);
    test_support::keys(cx, "tab");
    assert!(
        divider_focused(cx),
        "Tab, skipping buttons, still lands on the divider"
    );
    test_support::keys(cx, "tab");
    let after = focused_node(cx);
    assert!(
        after.bounds.left() >= column.right(),
        "Tab went on into the pane: {:?}",
        after.element_id
    );
}

/// The old handle focused itself on every press, on every platform, and a separator is no button that a Mac click
/// leaves unfocused; under the Mac rule a press on a button focuses nothing, so the divider must not depend on that
/// rule. Specified: under each click rule (a click focusing controls, as on Linux, and not, as on a Mac), with focus
/// moved off the divider by a press on the header's empty space, a press on the grip focuses the divider.
#[gpui::test]
fn a_press_on_the_grip_focuses_the_divider_under_either_click_rule(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, None);
    for click_focuses_controls in [false, true] {
        cx.update(|_, cx| focus::set_click_focuses_controls(click_focuses_controls, cx));
        let mark = test_support::bounds(cx, "app-version");
        let empty = point(mark.left() - px(40.), mark.center().y);
        press(cx, empty, 1);
        release(cx, empty);
        assert!(!divider_focused(cx), "premise: focus is off the divider");
        let at = on_divider(cx);
        press(cx, at, 1);
        release(cx, at);
        assert!(
            divider_focused(cx),
            "click_focuses_controls: {click_focuses_controls}"
        );
    }
}

/// The divider reaches 4 pixels into the pane, into the terminal's padding, and must take no more: the old acceptance
/// case checked that the terminal's first column still receives clicks, for selection, links and programs that read the
/// mouse, which a divider taking nothing past its 5 pixels implies. Specified, with a session open and the divider
/// focused: a press one pixel right of the grip (in the terminal's padding), then a move of
/// 50 pixels with the button held and the release, focuses the terminal's grid, leaves the sidebar at 396 and writes no
/// width.
#[gpui::test]
fn a_press_just_right_of_the_grip_reaches_the_terminal(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_over(cx, fleet(None, &["a"]));
    // The strip's box, which the grip is drawn over.
    let strip = test_support::bounds(cx, "sidebar-divider");
    press(cx, strip.center(), 1);
    release(cx, strip.center());
    assert!(divider_focused(cx), "premise: the divider holds focus");
    let beside = point(strip.right() + px(1.), strip.center().y);
    press(cx, beside, 1);
    move_to(cx, shifted(beside, 50.), true);
    release(cx, shifted(beside, 50.));
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("terminal-grid")
    );
    assert_eq!(width(cx), px(396.));
    let wrote_width = fake.commands().iter().any(|command| {
        command
            .body
            .as_ref()
            .is_some_and(|body| body.get("sidebar_width").is_some())
    });
    assert!(!wrote_width, "the press was the terminal's, not a drag");
}

/// SPEC.md: "The main pane keeps its 320px floor; a narrower window scrolls horizontally." Without the sideways scroll,
/// the part of the pane past the window's edge, most of the terminal, could never be shown. Specified: in a 720 by 560
/// window over a stored 656, a vertical wheel over the header moves nothing (only a sideways wheel scrolls the row, so
/// a wheel over the list or the terminal scrolls only those), and a sideways wheel of 200 pixels there moves the pane
/// and the divider 200 pixels to the left, the pane keeping its 320.
#[gpui::test]
fn a_window_too_narrow_for_both_panes_scrolls_sideways(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, Some(656));
    cx.simulate_resize(size(px(720.), px(560.)));
    test_support::settle(cx);
    let nodes = frame(cx);
    let (pane, divider) = (
        node(&nodes, "main-pane").bounds,
        node(&nodes, "sidebar-divider").bounds,
    );
    assert_eq!(
        pane.size.width,
        px(320.),
        "premise: the pane is at its floor"
    );
    let over = test_support::center(cx, "app-version");
    cx.simulate_event(ScrollEvent {
        position: over,
        delta: ScrollDelta::Pixels(point(px(0.), px(-200.))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    test_support::settle(cx);
    assert_eq!(
        node(&frame(cx), "main-pane").bounds.left(),
        pane.left(),
        "a vertical wheel scrolls the row nowhere"
    );
    cx.simulate_event(ScrollEvent {
        position: over,
        delta: ScrollDelta::Pixels(point(px(-200.), px(0.))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    test_support::settle(cx);
    let nodes = frame(cx);
    let (moved, moved_divider) = (
        node(&nodes, "main-pane").bounds,
        node(&nodes, "sidebar-divider").bounds,
    );
    assert_eq!(moved.left(), pane.left() - px(200.));
    assert_eq!(moved_divider.left(), divider.left() - px(200.));
    assert_eq!(moved.size.width, px(320.));
}

/// The element holding focus in a fresh frame, among those with an id.
fn focused_node(cx: &mut VisualTestContext) -> DriverNode {
    match frame(cx)
        .into_iter()
        .find(|node| node.flags.focused && node.element_id.is_some())
    {
        Some(node) => node,
        None => panic!("nothing with an id holds focus"),
    }
}

/// Whether the divider holds focus now, by the id of the element holding it in a fresh frame.
fn divider_focused(window: &mut VisualTestContext) -> bool {
    test_support::focused_id(window).as_deref() == Some("sidebar-divider")
}

// ---- The main pane's floor, with the real text system --------------------------------------------------------------

/// SPEC.md: "The main pane keeps its 320px floor; a narrower window scrolls horizontally." At the window's 720-pixel
/// minimum the widest sidebar (656) leaves the pane 64 pixels, so without the floor the terminal would be crushed to a
/// handful of columns. Only real layout shows it, so this lays the main view out with the bundled fonts in a headless
/// app. Specified: in a 720 by 560 window over a stored 656, the sidebar is 656 wide and the pane beside it starts at
/// 656 and is 320 wide, past the window's edge (`a_window_too_narrow_for_both_panes_scrolls_sideways` shows the row
/// scrolling to it); over nothing stored the pane takes the remaining 324.
#[test]
fn the_main_pane_keeps_its_floor_at_the_minimum_window_with_the_widest_sidebar() {
    for (stored, sidebar, pane) in [(Some(656), 656., 320.), (None, 396., 324.)] {
        let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            assets::UI_FONT,
        ));
        let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
        cx.update(bootstrap::install).unwrap();
        let (mut fake, handle, events) = FakeHelm::new();
        let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
        fake.connect(snapshot(stored));
        cx.run_until_parked();
        let window = cx
            .open_window(size(px(720.), px(560.)), |window, cx| {
                let main = cx.new(|cx| MainView::new(window, cx));
                cx.new(|cx| Root::new(main, window, cx))
            })
            .unwrap();
        let nodes = cx
            .update_window(window.into(), |_, window, cx| {
                window.set_driver_enabled(true);
                window.draw(cx).clear(cx);
                window.draw(cx).clear(cx);
                window.driver_nodes().to_vec()
            })
            .unwrap();
        assert_eq!(node(&nodes, "sidebar").bounds.size.width, px(sidebar));
        let main = node(&nodes, "main-pane").bounds;
        assert_eq!(
            (main.left(), main.size.width),
            (px(sidebar), px(pane)),
            "stored {stored:?}"
        );
    }
}

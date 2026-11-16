//! Tests of an avatar's menu: a secondary click on a host's avatar, or Shift+F10 on a focused one, opens that host's
//! actions menu beside the avatar's icon box, never choosing the host, and Escape returns focus to the avatar, in the
//! collapsed and the expanded rail.
//!
//! The menu itself (what it offers, what its commands do) is `hosts::menu`'s, tested there from the Hosts panel's row
//! toggle; these tests pin only the rail's wiring: the gesture, the host, the anchor and the opener. They run the real
//! main view over the helm client's fake with the rail's fixtures (`host_rail/tests.rs`), and read the menu from GPUI's
//! driver record by its ids (`host-menu`, `host-menu-items`, `host-menu-retry`). A right press is what GPUI delivers
//! for a macOS Ctrl+click too (its macOS platform layer turns one into the other before any element sees it), so the
//! right press stands for both.

use farhelm_proto::ui_channel::{CollectionState, HostView, Preferences};
use gpui::{
    Bounds, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
    size,
};

use crate::hosts::tests::{connected, host, hosts};
use crate::session_list::host_rail::tests::{answer_everything, label, node, pressed};
use crate::session_list::tests::{listing, open_main, preferences_state, session};
use crate::test_support::{self, any_open, focused_id, frame, keys};

/// The local host (1) and a connected SSH host at `dev@build-box.invalid` (2), with sessions `a` and `b`, `a` visited,
/// and the rail stored `expanded` or not, in a 1200 by 700 window with every command sent at startup answered.
fn open(
    cx: &mut TestAppContext,
    expanded: bool,
) -> (farhelm_helm_client::fake::FakeHelm, &mut VisualTestContext) {
    let listed: [HostView; 2] = [host(1, "local", connected()), host(2, "ssh", connected())];
    let snapshot: Vec<CollectionState> = vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences_state(Preferences {
            visit_history: vec!["a".to_owned()],
            welcome_seen: Some(true),
            host_rail_open: Some(expanded),
            ..Preferences::default()
        }),
        hosts(&listed),
    ];
    let (mut fake, cx) = open_main(cx, snapshot);
    cx.simulate_resize(size(px(1200.), px(700.)));
    answer_everything(&mut fake);
    test_support::settle(cx);
    (fake, cx)
}

/// A right press and release at `at`, as a secondary click delivers them, and the per-step checks.
fn secondary_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    test_support::settle(cx);
}

/// Where the open host menu is drawn; fails if none is.
fn menu(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    test_support::bounds(cx, "host-menu")
}

/// SPEC.md: "Host actions open on demand from the row menu or the same menu on its rail avatar", and the old avatar
/// opened it on a secondary click (the old `oncontextmenu`). Opening a host's actions must never choose it, or a look
/// at a host's menu would narrow the list under the user. The menu opens beside the avatar's icon box, where the old one
/// was anchored, and the avatar is its opener, so Escape returns focus there on every platform, which under macOS's
/// click rule (a click focuses no button) only the explicit hand-over achieves. Specified, under that rule: a right
/// press and release on the SSH host's avatar opens the host menu named "Host actions for dev@build-box.invalid" with
/// focus on Retry, its left edge 10 pixels past the avatar's icon box (the beside placement's gap); nothing is sent and
/// All stays pressed; Escape closes it with focus on the avatar.
#[gpui::test]
fn a_secondary_click_on_an_avatar_opens_its_hosts_menu_beside_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, false);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::check(cx);
    let avatar = test_support::bounds(cx, "host-rail-host-2");

    secondary_click(cx, avatar.center());
    let nodes = frame(cx);
    assert!(node(&nodes, "host-menu").is_some(), "the menu opened");
    assert_eq!(
        label(&nodes, "host-menu-items"),
        "Host actions for dev@build-box.invalid"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    assert_eq!(menu(cx).left(), avatar.left() + px(40. + 10.));
    assert_eq!(fake.commands(), [], "opening the menu sent nothing");
    assert!(pressed(&nodes, "host-rail-all"), "the host was not chosen");

    keys(cx, "escape");
    assert!(!any_open(cx), "Escape closed the menu");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-2"));
}

/// The old avatar also opened its menu from the keyboard with Shift+F10, the platforms' context-menu chord, so a
/// keyboard user reaches a host's actions from the rail as well as from Hosts. Only that chord: a plain F10 or another
/// modifier is some other shortcut's. Specified (Linux's rule): a click on the SSH host's avatar chooses it and leaves
/// focus on it; F10 alone opens nothing; Shift+F10 opens its host menu with focus on Retry and sends nothing; Escape
/// closes it with focus back on the avatar, the host still chosen.
#[gpui::test]
fn shift_f10_on_a_focused_avatar_opens_its_menu(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, false);
    test_support::click(cx, "host-rail-host-2");
    answer_everything(&mut fake);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("host-rail-host-2"),
        "the premise: the click focused the avatar"
    );
    keys(cx, "f10");
    assert!(!any_open(cx), "F10 alone opens nothing");
    keys(cx, "shift-f10");
    assert!(
        node(&frame(cx), "host-menu").is_some(),
        "Shift+F10 opened the menu"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("host-menu-retry"));
    assert_eq!(fake.commands(), [], "opening the menu sent nothing");
    keys(cx, "escape");
    assert!(!any_open(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-2"));
    assert!(pressed(&frame(cx), "host-rail-host-2"), "still chosen");
}

/// SPEC.md: expanded, "Badges, menus and actions retain their behavior", and the old sheet anchored the menu to the
/// inner icon box ("Only the inner icon box anchors corner marks and host menus"), so the menu stays beside the mark
/// rather than moving out past the 205-pixel row. Specified: with the rail stored expanded, a right press near the far
/// end of the SSH host's row (beyond its names) opens its host menu, its left edge 10 pixels past the row's 40-pixel icon
/// box, well inside the row; Escape returns focus to the avatar.
#[gpui::test]
fn the_expanded_rows_menu_opens_beside_its_icon_box(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, true);
    let row = test_support::bounds(cx, "host-rail-host-2");
    assert_eq!(
        row.size.width,
        px(205.),
        "the premise: the rail is expanded"
    );
    secondary_click(cx, point(row.right() - px(10.), row.center().y));
    assert_eq!(
        label(&frame(cx), "host-menu-items"),
        "Host actions for dev@build-box.invalid"
    );
    assert_eq!(menu(cx).left(), row.left() + px(40. + 10.));
    keys(cx, "escape");
    assert_eq!(focused_id(cx).as_deref(), Some("host-rail-host-2"));
}

/// A second secondary click on the same avatar while its menu is open must leave one menu, not stack a second over the
/// first: the layer manager closes a popover on a press outside it but not on a press on its own anchor, which here is
/// the avatar's icon box, so the rail closes the open menu itself before it opens the new one. Specified: with the SSH
/// host's menu open from a right press on its avatar, a second right press there leaves exactly one host menu drawn,
/// named for that host and beside its avatar.
#[gpui::test]
fn a_second_secondary_click_on_the_same_avatar_leaves_one_menu(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, false);
    let avatar = test_support::bounds(cx, "host-rail-host-2");
    secondary_click(cx, avatar.center());
    assert!(any_open(cx), "the premise: the host's menu is open");
    secondary_click(cx, avatar.center());
    let nodes = frame(cx);
    let menus = nodes
        .iter()
        .filter(|node| node.element_id.as_deref() == Some("host-menu"))
        .count();
    assert_eq!(menus, 1, "one host menu");
    assert_eq!(
        label(&nodes, "host-menu-items"),
        "Host actions for dev@build-box.invalid"
    );
    assert_eq!(menu(cx).left(), avatar.left() + px(40. + 10.));
}

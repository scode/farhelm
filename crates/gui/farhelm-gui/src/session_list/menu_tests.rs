//! Tests of a session row's actions menu, its Rename and its Delete: the menu's placement, names, descriptions, keys,
//! focus and one highlight, another row's button and New pressed while it is open, how an open menu follows its button
//! through what the helm pushes, Rename opening the editor for its row, the question and the precondition each request
//! carries, what a pending or refused Delete shows, the selection while a Delete runs, and the Delete shortcut.
//!
//! Everything runs in the real main view over the helm client's fake, as the list's own tests do, and reads what the
//! window sent from the fake and what it drew from GPUI's driver record of the frame. Controls are found by their ids:
//! a row's menu button is `session-menu-<key>`, the menu's Rename and Delete are `session-menu-rename` and
//! `session-menu-delete`, the Rename editor is `rename`, and the question's answers are `prompt-confirm` and
//! `prompt-cancel`.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionState, HostView, KeyedEntry,
    PREFERENCES_KEY, Preferences,
};
use gpui::{
    Background, Bounds, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, Role, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point,
    px, size,
};
use serde_json::{Value, json};

use super::host_rail::tests::{node, texts_within};
use super::lifecycle_tests::node_bounds;
use super::pins_tests::away;
use super::tests::{
    Fixture, change, launcher_drawn, listing_meta, open_main, painted, row_order, selected, sent,
    session,
};
use crate::actions::{DELETE_SESSION_CHORD as DELETE_CHORD, RENAME_SESSION_SHORTCUT_NAME};
use crate::hosts::tests::{connected, host, hosts};
use crate::launcher::view_tests::{search_focused, search_text};
use crate::operations::OpKey;
use crate::test_support::{focused_id, frame, meta};
use crate::{layers, session, test_support, theme};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// `key` as an ended session: its agent exited.
pub(crate) fn ended(key: &'static str) -> Fixture {
    session(key).status(json!({"state": "exited", "exit_code": 0}))
}

/// `fixture`'s entry listing `tabs` terminal tabs. The helm's pinned fixture lists one, so every test says how many
/// it means.
pub(crate) fn entry(fixture: &Fixture, tabs: usize) -> KeyedEntry {
    let mut entry = fixture.entry();
    entry.value["tabs"] = Value::Array(
        (0..tabs)
            .map(|tab| json!({ "id": format!("tab-{tab}") }))
            .collect(),
    );
    entry
}

/// A snapshot of `rows` (each with its tab count) in that order, with the first one open, and Skip session
/// confirmations as `skip` says.
pub(crate) fn snapshot(rows: &[(Fixture, usize)], skip: bool) -> Vec<CollectionState> {
    let order: Vec<&str> = rows.iter().map(|(fixture, _)| fixture.key).collect();
    let preferences = Preferences {
        visit_history: order.iter().take(1).map(|key| key.to_string()).collect(),
        skip_session_confirmations: Some(skip),
        // A helm that has shown the Welcome card, which would otherwise open over the main view (`crate::welcome`).
        welcome_seen: Some(true),
        ..Preferences::default()
    };
    vec![
        CollectionState {
            collection: Collection::Sessions,
            meta: listing_meta(&order, rows.len() as u64, false),
            entries: rows
                .iter()
                .map(|(fixture, tabs)| entry(fixture, *tabs))
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::to_value(preferences).unwrap(),
            }],
        },
    ]
}

/// Push `rows` as changed, under the complete listing `order`.
pub(crate) fn push(
    fake: &mut FakeHelm,
    order: &[&str],
    rows: &[(Fixture, usize)],
    cx: &mut VisualTestContext,
) {
    let mut changes = change(listing_meta(order, order.len() as u64, false), &[], &[]);
    changes.upsert = rows
        .iter()
        .map(|(fixture, tabs)| entry(fixture, *tabs))
        .collect();
    fake.push(changes);
    test_support::settle(cx);
}

/// The Deletes the window sent since the last call, as (id, path). Every other command is answered with success at
/// once, as the helm answers a preference write or a seen mark, so a test can answer its Deletes in the order the
/// fake requires; a test that has a Delete waiting reads later commands with `sent` instead.
fn deletes(fake: &mut FakeHelm) -> Vec<(u64, String)> {
    let mut found = Vec::new();
    for FakeCommand {
        id, method, path, ..
    } in fake.commands()
    {
        if method == Method::Delete {
            found.push((id, path));
        } else {
            fake.reply(id, 200, None);
        }
    }
    found
}

/// The paths of `deletes`.
fn paths(deletes: &[(u64, String)]) -> Vec<&str> {
    deletes.iter().map(|(_, path)| path.as_str()).collect()
}

/// Whether anything was drawn under `selector` in the last frame.
pub(crate) fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_some()
}

/// Whether any dialog, menu or question is open.
pub(crate) fn any_layer(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| layers::any_open(window, cx))
}

/// The label of the node drawn under the element id `id`.
pub(crate) fn label_of(cx: &mut VisualTestContext, id: &str) -> Option<String> {
    frame(cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .and_then(|node| node.label.map(|label| label.to_string()))
}

/// Open the menu of the row whose menu button is `button`, and choose Delete, by clicks.
fn choose_delete(cx: &mut VisualTestContext, button: &'static str) {
    test_support::click(cx, button);
    test_support::click(cx, "session-menu-delete");
    test_support::settle(cx);
}

/// A left click at `at`, delivered inside the caller's window update, so several can land with no frame drawn between
/// them.
pub(super) fn click_at(at: Point<Pixels>) -> [PlatformInput; 2] {
    [
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: 1,
        }),
    ]
}

/// The menu items painted with a fill inside the element drawn under `menu`, by id, from a fresh frame's painted quads:
/// the menu's one highlight, which [`crate::components::MenuItem`] paints from focus alone and only on an available
/// item. Paint, not focus, because the old `menu-highlight.spec.ts` bug was a second fill under the resting pointer, so
/// any opaque fill exactly on an item counts, whatever its color (a hover style's included), and an accent fill on no
/// item comes back as its bounds. The panel's own surface and the focus ring's inset border match no item, and fills
/// outside the menu (the open session's row) do not count.
pub(crate) fn lit(cx: &mut VisualTestContext, menu: &'static str) -> Vec<String> {
    let nodes = frame(cx);
    let menu = test_support::bounds(cx, menu);
    let accent = cx.update(|_, cx| Background::from(theme::color(theme::palette(cx).accent_fill)));
    let (scale, quads) = cx.update(|window, _| (window.scale_factor(), window.painted_quads()));
    let menu = menu.scale(scale);
    quads
        .iter()
        .filter(|quad| menu.contains(&quad.bounds.origin) && !quad.background.is_transparent())
        .filter_map(|quad| {
            let item = nodes.iter().find(|node| {
                node.role == Some(Role::MenuItem) && node.bounds.scale(scale) == quad.bounds
            });
            match item {
                Some(item) => item.element_id.as_ref().map(ToString::to_string),
                None => (quad.background == accent).then(|| format!("{:?}", quad.bounds)),
            }
        })
        .collect()
}

// ---- The menu ------------------------------------------------------------------------------------------------------

/// SPEC.md, "Opening and closing" and "Inside a dialog or menu, focus stays inside": a menu the user opened starts on
/// its first command, keeps Tab inside, and Escape gives focus back to the button that opened it, on every platform.
/// Opening a row's menu is not opening its session, which would switch what the user is looking at. Run under macOS's
/// click rule, where the click leaves the button unfocused, so only the menu naming its button as the return target
/// brings focus back there. Specified: clicking b's menu button opens the menu beside the button, the layer manager's
/// beside placement (its left edge the button's right edge plus 10 and its top the button's top less 4, each rounded
/// to a whole pixel; `layers_tests.rs` pins that placement and its clamp for any anchor), with focus on its first
/// command, Rename; its header names b and says "Codex · Running", a is still the open session and nothing is sent;
/// Tab moves through Mark unread (b reads seen), Pin to top, Clone, Replace with…, Replace, Stop and Delete, in the old
/// menu's order (the separators between their groups take no focus), and then wraps to Rename, staying in the menu;
/// Escape closes the menu and focus is on b's button.
#[gpui::test]
fn the_menu_opens_beside_its_button_holds_focus_and_leaves_the_selection(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    assert_ne!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: b's button is unfocused before the click"
    );

    test_support::click(cx, "session-menu-b");
    let button = test_support::bounds(cx, "session-menu-b");
    let menu = test_support::bounds(cx, "session-menu");
    assert_eq!(
        menu.origin,
        gpui::point(
            (button.right() + px(10.)).round(),
            (button.top() - px(4.)).round()
        ),
        "the menu is beside its button: {menu:?} beside {button:?}"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert_eq!(
        label_of(cx, "session-menu-summary-b").as_deref(),
        Some("Codex · Running")
    );
    assert!(painted(&frame(cx)).iter().any(|text| text == "b"));
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(sent(&mut fake), []);

    for next in [
        "session-menu-seen",
        "session-menu-pin",
        "session-menu-clone",
        "session-menu-replace-with",
        "session-menu-replace",
        "session-menu-stop",
        "session-menu-delete",
        "session-menu-rename",
    ] {
        test_support::keys(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(next));
    }
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));
}

/// SPEC.md: a menu goes away "when the thing it is about is gone (a deleted session's open menu, say)", and focus
/// never stays inside something that is no longer drawn. This is the popover-movement acceptance case the layer
/// manager left to the first real menu. Run under Linux's click rule, where the click focuses b's button, so the button
/// the menu names as its return target is also what held focus when it opened, and both are gone with the row; the
/// frame on screen at the close no longer draws the button, so GPUI would never report focus on it as lost.
///
/// The row coming back is the old `sidebar.spec.ts` case "a host-filtered-out row's open menu stays closed when it
/// returns" (reference commit `24bc0580`): a menu that reappeared with its row would offer commands for a session that
/// may have changed while it was gone, with nobody having asked. Choosing a host on the rail is itself a press outside
/// the menu, which closes it before any filter applies, so a push is the one way the row leaves under an open menu.
/// Specified: with b's menu open, a pushed listing without b closes the menu, and focus is held by something still
/// alive; a later listing with b again draws b's row with no menu or other layer open.
#[gpui::test]
fn a_pushed_removal_of_the_row_closes_its_menu(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    test_support::click(cx, "session-menu-b");
    assert!(any_layer(cx), "the premise: the menu is open");

    fake.push(change(listing_meta(&["a"], 1, false), &[], &["b"]));
    test_support::settle(cx);
    test_support::redraw(cx);
    assert!(!any_layer(cx));
    assert!(!drawn(cx, "session-menu"));
    assert!(
        cx.update(|window, cx| window.focused(cx).is_some()),
        "focus recovered rather than being left on b's button, which went with its row"
    );

    push(&mut fake, &["a", "b"], &[(session("b"), 0)], cx);
    assert!(drawn(cx, "session-menu-b"), "the premise: b's row is back");
    assert!(!any_layer(cx), "b's menu did not come back with its row");
    assert!(!drawn(cx, "session-menu"));
}

/// Every row has a menu button and the same eight commands, so a screen reader user can only tell which session a menu
/// acts on from its name, and each command needs its words to say what it does. The old `sidebar.spec.ts` pinned this
/// in "the actions menu exposes a real menu-button relationship" and "the session flyout sits beside its row with a
/// titled, described menu" (reference commit `24bc0580`), together with opening the menu not scrolling its row. What
/// GPUI's driver record cannot show is left out: the button's expanded state, `aria-haspopup`, and the description as
/// an accessible property apart from the painted line (`components/menu_item.rs` makes the painted line the item's
/// accessible description). Specified, with a open: b's button is named "Session actions for b"; opening its menu
/// leaves b's row where it was; the commands sit in a menu, apart from the header, named "Session actions for b"; each
/// of the eight is a menu item named by its label that paints the old item's description inside itself (the table
/// below); choosing Delete closes the menu and asks in a dialog that quotes b's title.
#[gpui::test]
fn the_menu_and_its_commands_say_whose_they_are_and_what_each_does(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    assert_eq!(
        label_of(cx, "session-menu-b").as_deref(),
        Some("Session actions for b")
    );
    let row = test_support::bounds(cx, "session-row-b");

    test_support::click(cx, "session-menu-b");
    assert_eq!(
        test_support::bounds(cx, "session-row-b"),
        row,
        "opening the menu moved no row"
    );
    let nodes = frame(cx);
    let items = node(&nodes, "session-menu-items").expect("the commands are drawn");
    assert_eq!(items.role, Some(Role::Menu));
    assert_eq!(items.label.as_deref(), Some("Session actions for b"));
    for (id, name, description) in [
        (
            "session-menu-rename",
            "Rename",
            "Give this session a new title",
        ),
        ("session-menu-seen", "Mark unread", "Show new output again"),
        (
            "session-menu-pin",
            "Pin to top",
            "Keep this session above unpinned ones",
        ),
        ("session-menu-clone", "Clone", "New session, keep this one"),
        (
            "session-menu-replace-with",
            "Replace with…",
            "Edit settings, then swap",
        ),
        (
            "session-menu-replace",
            "Replace",
            "Fresh conversation, same settings",
        ),
        (
            "session-menu-stop",
            "Stop",
            "Ends the agent and its processes, and closes terminal tabs",
        ),
        (
            "session-menu-delete",
            "Delete",
            "Removes the session and its state",
        ),
    ] {
        let item = node(&nodes, id).unwrap_or_else(|| panic!("{id} was not drawn"));
        assert_eq!(item.role, Some(Role::MenuItem), "{id}");
        assert_eq!(item.label.as_deref(), Some(name), "{id}");
        let painted: Vec<String> = texts_within(&nodes, id)
            .into_iter()
            .map(|text| text.text.to_string())
            .collect();
        assert!(
            painted.iter().any(|text| text == description),
            "{id} describes itself: {painted:?}"
        );
    }

    test_support::click(cx, "session-menu-delete");
    let nodes = frame(cx);
    assert!(node(&nodes, "session-menu").is_none(), "the menu closed");
    assert_eq!(
        node(&nodes, "prompt").and_then(|prompt| prompt.role),
        Some(Role::Dialog),
        "the question is a dialog, not a menu with no commands"
    );
    assert_eq!(
        label_of(cx, "session-delete-title-b").as_deref(),
        Some("\"b\"")
    );
}

/// The menu opens beside its button, past the sidebar's edge, so it covers none of the rows' buttons, and a press on
/// another row's ⋯ must reach that button rather than only closing the open menu. The old `sidebar.spec.ts` pinned
/// this with a click at the button's coordinates ("an open menu leaves the other rows' toggles clickable", reference
/// commit `24bc0580`), since a covering panel would have taken the press, and a covered Delete or Stop could have
/// started something. Specified, under the Linux click rule: with a's menu open, which does not cover b's button, a
/// click on b's button leaves exactly one menu open, b's, with its header naming b and focus on its Rename; no
/// question opens and nothing is sent.
#[gpui::test]
fn another_rows_menu_button_takes_a_click_while_a_menu_is_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));
    test_support::click(cx, "session-menu-a");
    assert!(
        label_of(cx, "session-menu-summary-a").is_some(),
        "the premise: a's menu is open"
    );
    let (menu, button) = (
        test_support::bounds(cx, "session-menu"),
        test_support::bounds(cx, "session-menu-b"),
    );
    assert!(
        !menu.intersects(&button),
        "the premise: a's menu {menu:?} leaves b's button {button:?} uncovered"
    );

    test_support::click(cx, "session-menu-b");
    let nodes = frame(cx);
    let menus = nodes
        .iter()
        .filter(|node| node.element_id.as_deref() == Some("session-menu"))
        .count();
    assert_eq!(menus, 1, "one menu");
    assert!(
        label_of(cx, "session-menu-summary-b").is_some(),
        "b's menu opened"
    );
    assert!(
        label_of(cx, "session-menu-summary-a").is_none(),
        "a's closed"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert!(!drawn(cx, "prompt"));
    assert_eq!(sent(&mut fake), []);
}

/// The old `sidebar.spec.ts` cases "opening the create-session form closes an open row menu" and the row-menu half of
/// "opening New focuses search and immediate typing lands there, with or without a prior menu" (reference commit
/// `24bc0580`). The hazard they guarded: the press on New closes the row menu, whose close returns focus to its button,
/// and a return landing after the launcher took focus leaves the user typing into the list. Both click rules run: under
/// Linux's the press focuses New first, the old case's path, so the closing menu finds focus already outside it; under
/// macOS's the press focuses nothing, so the menu's return always runs. Specified, for each rule: with b's menu open
/// and New not covered by it, a click on New closes the menu and opens the launcher with focus in its search box, and
/// "xyz" typed at once lands there; Escape then closes every layer.
#[gpui::test]
fn new_with_a_row_menu_open_closes_it_and_typing_reaches_the_search_box(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    for focuses in [true, false] {
        cx.update(|_, cx| crate::focus::set_click_focuses_controls(focuses, cx));
        test_support::click(cx, "session-menu-b");
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("session-menu-rename"),
            "the premise ({focuses}): b's menu is open and holds focus"
        );
        let (menu, new) = (
            test_support::bounds(cx, "session-menu"),
            test_support::bounds(cx, super::NEW_ID),
        );
        assert!(
            !menu.intersects(&new),
            "the premise: {menu:?} leaves New {new:?} uncovered"
        );

        test_support::click(cx, super::NEW_ID);
        assert!(!drawn(cx, "session-menu"), "{focuses}: the menu closed");
        assert!(launcher_drawn(cx), "{focuses}: New opened the launcher");
        assert!(search_focused(cx), "{focuses}: the search box holds focus");
        test_support::keys(cx, "x y z");
        assert_eq!(search_text(cx), "xyz", "{focuses}");
        // The first Escape may only close the search's results; bounded, so a launcher that never closes fails.
        for _ in 0..3 {
            if any_layer(cx) {
                test_support::keys(cx, "escape");
            }
        }
        assert!(!any_layer(cx), "{focuses}: Escape closed the launcher");
    }
}

// ---- The menu through changes underneath ---------------------------------------------------------------------------

/// What one step of [`an_open_menu_follows_its_button_through_what_the_helm_pushes`] pushes while b's menu is open.
enum Push {
    /// The listing in this order, metadata only.
    Order(&'static [&'static str]),
    /// A complete listing without d, the row below b.
    WithoutD,
    /// The host registry in the other order. Neither host needs attention, so nothing above the rows changes.
    HostsReversed,
    /// The listing as one the helm could not read: the rows stay, under a line that says so.
    Failed,
    /// The listing read again.
    Read,
}

/// The host registry's ids in the order the store holds them.
fn registry(cx: &mut VisualTestContext) -> Vec<i64> {
    cx.update(|_, cx| {
        let Some(store) = session::current_store(cx) else {
            return Vec::new();
        };
        let hosts = store.read(cx).collections().ordered::<HostView>();
        hosts.iter().map(|host| host.id).collect()
    })
}

/// SPEC.md, "Things changing underneath": "When the session list reorders, a focused row, or its open menu, moves with
/// its session." The old app closed an open row menu whenever something moved its button (`popover-movement.spec.ts`,
/// `popover-header-movement.spec.ts` and `popover-host-movement.spec.ts` at reference commit `24bc0580`, superseded by
/// that rule). `delete_notice_tests.rs` pins content growing above the rows; this pins a reorder, which puts the row
/// at another index of the virtualized list, another row's removal, a host registry push, and a failed listing, whose
/// rows stay drawn where the old app replaced them. The menu is opened by its keys with the pointer off the rows: a
/// pointer-opened menu keeps the order hold, and a reorder then moves nothing (`pins_tests.rs`
/// `an_open_row_menu_keeps_the_hold`). Specified: with a, b, c and d listed (a open) and b's menu opened by Down on
/// its button, each push leaves the menu open with focus on Rename and moves it by as much as b's button: a reorder to
/// a, c, b, d moves it down; a and c swapping above b moves nothing; the order back to a, b, c, d moves it up; a
/// complete listing without d moves nothing; the host registry reordered moves nothing; a failed listing draws
/// "Failed to load sessions: …" above the rows and moves it down; the next listing read takes the line away and moves
/// it back up.
#[gpui::test]
fn an_open_menu_follows_its_button_through_what_the_helm_pushes(cx: &mut TestAppContext) {
    use std::cmp::Ordering::{Equal, Greater, Less};

    let rows = ["a", "b", "c", "d"].map(|key| (session(key), 0));
    let mut state = snapshot(&rows, false);
    state.push(hosts(&[
        host(2, "ssh", connected()),
        host(3, "ssh", connected()),
    ]));
    let (mut fake, cx) = open_main(cx, state);
    // Tall enough that the menu never meets the window's bottom edge, which would hold it there rather than beside b.
    cx.simulate_resize(size(px(900.), px(1100.)));
    test_support::settle(cx);
    // The click and Escape leave focus on b's button; moving away then lets go of the hold the click took.
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    away(cx);
    test_support::keys(cx, "down");
    assert!(
        label_of(cx, "session-menu-summary-b").is_some(),
        "the premise: b's menu is open"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));

    let mut order: &[&str] = &["a", "b", "c", "d"];
    for (step, push, way) in [
        ("b moves down", Push::Order(&["a", "c", "b", "d"]), Greater),
        ("a and c swap", Push::Order(&["c", "a", "b", "d"]), Equal),
        ("b moves up", Push::Order(&["a", "b", "c", "d"]), Less),
        ("d, below b, goes", Push::WithoutD, Equal),
        ("the host registry reorders", Push::HostsReversed, Equal),
        ("the listing fails", Push::Failed, Greater),
        ("the listing is read again", Push::Read, Less),
    ] {
        let button = test_support::bounds(cx, "session-menu-b").top();
        let menu = test_support::bounds(cx, "session-menu").top();
        let three = listing_meta(&["a", "b", "c"], 3, false);
        match push {
            Push::Order(next) => {
                order = next;
                fake.push(change(listing_meta(next, 4, false), &[], &[]));
            }
            Push::WithoutD => {
                order = &["a", "b", "c"];
                fake.push(change(three, &[], &["d"]));
            }
            Push::HostsReversed => {
                let reversed = hosts(&[host(3, "ssh", connected()), host(2, "ssh", connected())]);
                fake.push(CollectionChanges {
                    collection: Collection::Hosts,
                    meta: reversed.meta,
                    upsert: reversed.entries,
                    remove: Vec::new(),
                });
            }
            Push::Failed => {
                let mut failed = three;
                failed.availability = Availability::Unavailable {
                    error: "down".into(),
                };
                fake.push(change(failed, &[], &[]));
            }
            Push::Read => fake.push(change(three, &[], &[])),
        }
        test_support::settle(cx);
        assert_eq!(row_order(&frame(cx)), order, "{step}: the premise: no hold");
        assert_eq!(
            label_of(cx, "session-list-error").is_some(),
            matches!(push, Push::Failed),
            "{step}: the failure line shows over the failed listing alone"
        );
        if matches!(push, Push::HostsReversed) {
            assert_eq!(registry(cx), [3, 2], "{step}: the premise");
        }

        assert!(
            label_of(cx, "session-menu-summary-b").is_some(),
            "{step}: b's menu is still open"
        );
        assert_eq!(
            focused_id(cx).as_deref(),
            Some("session-menu-rename"),
            "{step}"
        );
        let moved = test_support::bounds(cx, "session-menu-b").top() - button;
        let followed = test_support::bounds(cx, "session-menu").top() - menu;
        assert!(
            (followed - moved).abs() < px(1.),
            "{step}: the menu moved {followed:?} with its button's {moved:?}"
        );
        // Rounded to a whole pixel, so layout rounding does not read as a move.
        let went = moved.round().partial_cmp(&px(0.));
        assert_eq!(went, Some(way), "{step}: b's button moved {moved:?}");
    }
}

// ---- The menu's keys and its one highlight --------------------------------------------------------------------------

/// SPEC.md has menus "use their own arrow keys", and the old row menu's closed toggle opened on Down at its first
/// command and on Up at its last (`closed_toggle_key_intent` in `menu_panel.rs` at reference commit `24bc0580`), so a
/// keyboard user never opens the menu and then walks to its far end. Home and End are the old menu's too, and the layer
/// manager's menu context does not bind them. Run under macOS's click rule, so focus is on the button only because the
/// closing menu returned it there. Specified: with a open and b's button focused (its menu opened by a click and closed
/// by Escape), Down opens b's menu with focus on Rename; End moves to Delete and Home back to Rename; Escape closes it
/// with focus on b's button; Up opens it with focus on Delete; Escape again returns focus to the button; a is still
/// the open session and nothing is sent. (A modified or held arrow opening nothing is pinned once, for every toggle, by
/// `components/tests/menu_keys.rs`.)
#[gpui::test]
fn down_and_up_on_the_closed_button_open_the_menu_on_its_first_and_last_command(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx), "the premise: the menu is closed");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-b"),
        "the premise: b's button holds focus"
    );

    test_support::keys(cx, "down");
    assert!(drawn(cx, "session-menu"), "Down opened the menu");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    test_support::keys(cx, "end");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-delete"));
    test_support::keys(cx, "home");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    test_support::keys(cx, "up");
    assert!(drawn(cx, "session-menu"), "Up opened the menu");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-delete"));
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert_eq!(sent(&mut fake), []);
}

/// SPEC.md, "Things changing underneath": "A menu has one highlight. Whichever moved last, the pointer or the keys,
/// sets it, and Enter acts on the highlighted item." The old app once painted two fills at once, the first command's
/// for focus and the hovered one's for hover, and Enter then ran the focused one; `menu-highlight.spec.ts` at reference
/// commit `24bc0580` pinned the fix ("session menu shares one highlight between hover and arrow keys"). Here a
/// command's fill is drawn from focus alone ([`crate::components::MenuItem`]), so one focused command is one highlight,
/// and this pins that the pointer and the keys move that one focus. Specified: with b's menu open on Rename, the
/// pointer entering Clone moves focus to Clone; with the pointer resting there, Down moves it to Replace with…, Up back
/// to Clone and Up again to Pin to top, and a redraw leaves it there; the pointer leaving the menu leaves the highlight
/// where it is, and entering Clone again takes it there; Enter then runs Clone, opening the launcher with its "Launch"
/// action, never Rename's editor.
#[gpui::test]
fn the_pointer_and_the_arrows_move_one_highlight_and_enter_runs_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    test_support::click(cx, "session-menu-b");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: the menu starts on Rename"
    );
    let clone = test_support::center(cx, "session-menu-clone");
    // Off the menu, over the list to its left, level with Clone.
    let menu = test_support::bounds(cx, "session-menu");
    let away = gpui::point(menu.left() - px(40.), clone.y);
    assert!(
        !menu.contains(&away),
        "the premise: the parking point is outside the menu"
    );

    cx.simulate_mouse_move(clone, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-clone"));
    for (key, expected) in [
        ("down", "session-menu-replace-with"),
        ("up", "session-menu-clone"),
        ("up", "session-menu-pin"),
    ] {
        test_support::keys(cx, key);
        assert_eq!(focused_id(cx).as_deref(), Some(expected), "after {key}");
    }
    test_support::redraw(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-pin"),
        "the resting pointer does not take the highlight back"
    );

    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-pin"),
        "leaving the menu leaves the highlight where it is"
    );
    cx.simulate_mouse_move(clone, None, Modifiers::none());
    test_support::settle(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-clone"));

    test_support::keys(cx, "enter");
    assert!(!drawn(cx, "session-menu"), "the menu closed");
    assert!(!drawn(cx, "rename"), "Rename did not run");
    let action = launch_action(cx);
    assert!(action.starts_with("Launch"), "Clone ran: {action}");
}

// ---- Clone and Replace with -----------------------------------------------------------------------------------------

/// The accessible name of the launcher's main action ("Launch …", "Replace …"), which says what the open launcher
/// would do.
fn launch_action(cx: &mut VisualTestContext) -> String {
    label_of(cx, "launcher-launch").expect("the launcher is drawn")
}

/// The row menu is where SPEC.md offers Clone and Replace with for any session, and each must open the launcher on
/// its own row, not on the open session, and give focus back to the row's button when the launcher closes. Their words
/// are the old menu items' (labels and descriptions), with hover help from the old header's Clone and the interrupted
/// card's Replace with, since the old items had none. Specified, with a open: b's menu offers Clone, described "New
/// session, keep this one", with the help "Clone: open the new-session form with these settings; this session keeps
/// running", and Replace with…, described "Edit settings, then swap", with the help "Replace with: edit the settings,
/// then swap this session for a new one"; choosing Clone closes the menu and opens the launcher with b's title in its
/// Name field and a "Launch" action; Escape closes it with focus on b's button; choosing Replace with… opens it with a
/// "Replace" action. Opening either sends no Replace and nothing to b's routes.
#[gpui::test]
fn the_menus_clone_and_replace_with_open_the_launcher_on_their_row(cx: &mut TestAppContext) {
    use super::host_rail::tests::hover_help;
    // b is an ordinary folder session: Clone of one in a managed checkout names its new checkout `<title>-clone`
    // instead (`launcher::checkout_tests` covers that), and this test is about which row the launcher opens on.
    let b = session("b").unmanaged().not_launched_as_checkout();
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (b, 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(true, cx));

    test_support::click(cx, "session-menu-b");
    assert_eq!(label_of(cx, "session-menu-clone").as_deref(), Some("Clone"));
    assert_eq!(
        label_of(cx, "session-menu-replace-with").as_deref(),
        Some("Replace with…")
    );
    let painted = painted(&frame(cx));
    for description in ["New session, keep this one", "Edit settings, then swap"] {
        assert!(
            painted.iter().any(|text| text == description),
            "{description}"
        );
    }
    assert_eq!(
        hover_help(cx, "session-menu-clone").as_deref(),
        Some("Clone: open the new-session form with these settings; this session keeps running")
    );
    assert_eq!(
        hover_help(cx, "session-menu-replace-with").as_deref(),
        Some("Replace with: edit the settings, then swap this session for a new one")
    );

    test_support::click(cx, "session-menu-clone");
    assert!(!drawn(cx, "session-menu"), "the menu closed");
    assert!(drawn(cx, "launcher"), "Clone opened the launcher");
    let action = launch_action(cx);
    assert!(action.starts_with("Launch"), "{action}");
    let name: Vec<String> = texts_within(&frame(cx), "launcher-name")
        .into_iter()
        .map(|text| text.text.to_string())
        .collect();
    assert_eq!(name, ["b"], "b's title, not the open session's");
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-replace-with");
    let action = launch_action(cx);
    assert!(action.starts_with("Replace"), "{action}");
    test_support::keys(cx, "escape");
    assert!(
        sent(&mut fake)
            .iter()
            .all(|(path, _)| !path.ends_with("/replace") && !path.starts_with("/api/sessions/b"))
    );
}

/// Clone and Replace with open the launcher, which SPEC.md keeps from opening while a host change runs (or a launch is
/// pending), so they are drawn unavailable then, as New's button is, and say why, as the menu's plain Replace does.
/// Specified: with Add host's probe in flight, b's menu draws both with "Wait for the host change to finish" as their
/// help and painted description, and a click on either leaves the menu open with no launcher.
#[gpui::test]
fn the_menus_launcher_commands_wait_while_a_host_change_runs(cx: &mut TestAppContext) {
    use super::host_rail::tests::hover_help;
    use crate::launcher::launch_tests::{HostChange, start_host_change};
    const WAIT: &str = "Wait for the host change to finish";
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    let path = start_host_change(cx, HostChange::Probe);
    assert!(
        fake.commands().iter().any(|command| command.path == path),
        "the premise: the probe is in flight"
    );
    test_support::click(cx, "session-menu-b");
    let painted = painted(&frame(cx));
    assert!(
        painted.iter().filter(|text| *text == WAIT).count() == 2,
        "exactly Clone and Replace with… describe the wait: {painted:?}"
    );
    for item in ["session-menu-clone", "session-menu-replace-with"] {
        assert_eq!(hover_help(cx, item).as_deref(), Some(WAIT), "{item}");
        test_support::click(cx, item);
        assert!(
            drawn(cx, "session-menu") && !drawn(cx, "launcher"),
            "{item} is unavailable: the menu is still open and the launcher closed"
        );
    }
}

// ---- Rename --------------------------------------------------------------------------------------------------------

/// The row menu is one of Rename's two openers (SPEC.md, "Lifecycle operations": "A list rename opens in a modal editor
/// owned by the list rather than by the row-actions popup"). It must rename the row it was opened from, not the open
/// session, and give focus back to the menu's button when the editor closes. Specified: with a open, Enter on b's
/// Rename closes the menu and opens the editor; Escape closes it with focus on b's button; opened again, Enter saves,
/// sending b's own title to b's rename route.
#[gpui::test]
fn the_menus_rename_opens_the_editor_for_its_row(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);

    test_support::click(cx, "session-menu-b");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: the menu starts on Rename"
    );
    test_support::keys(cx, "enter");
    assert!(drawn(cx, "rename"), "the editor opened");
    assert!(!drawn(cx, "session-menu"), "and the menu closed");
    test_support::keys(cx, "escape");
    assert!(!any_layer(cx));
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-b"));

    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, "enter enter");
    assert_eq!(
        sent(&mut fake),
        [("/api/sessions/b/rename".to_owned(), json!({ "title": "b" }))]
    );
}

/// SPEC.md: a pending request "refuses another lifecycle action" on its session, and a Rename sent behind a Delete
/// would race it. Specified: with Skip session confirmations on, b's Delete is sent and left unanswered; b's menu then
/// draws Rename unavailable, and choosing it opens no editor and sends nothing.
#[gpui::test]
fn the_menus_rename_is_refused_while_a_delete_of_its_row_is_in_flight(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], true));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    assert_eq!(
        paths(&deletes(&mut fake)),
        ["/api/sessions/b"],
        "the premise: b's Delete is in flight"
    );

    test_support::click(cx, "session-menu-b");
    test_support::click(cx, "session-menu-rename");
    // What tells an unavailable item from an enabled one whose open was refused: the enabled one closes the menu.
    assert!(
        drawn(cx, "session-menu"),
        "the unavailable Rename ignored the click; the menu is still open"
    );
    assert!(!drawn(cx, "rename"));
    assert_eq!(sent(&mut fake), []);
}

/// SPEC.md, "Desktop window chrome": the Rename shortcut appears "only on the open session's row-menu item", the one
/// row the shortcut acts on; naming it on another row would suggest it renames that one. Specified: with a open, the
/// help of Rename in a's menu, shown after the hover delay, holds the shortcut's name, and in b's menu it does not.
#[gpui::test]
fn only_the_open_rows_rename_names_the_shortcut(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    let mut help_in_menu_of = |button: &'static str| {
        test_support::click(cx, button);
        let rename = test_support::center(cx, "session-menu-rename");
        cx.simulate_mouse_move(rename, None, Modifiers::none());
        test_support::advance(cx, layers::HOVER_DELAY);
        let help = cx
            .update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string());
        test_support::keys(cx, "escape escape");
        help.expect("the premise: Rename's help is showing")
    };
    let open = help_in_menu_of("session-menu-a");
    let other = help_in_menu_of("session-menu-b");
    assert!(open.contains(RENAME_SESSION_SHORTCUT_NAME), "{open}");
    assert!(!other.contains(RENAME_SESSION_SHORTCUT_NAME), "{other}");
}

// ---- The question and the request ----------------------------------------------------------------------------------

/// SPEC.md, "Opening and closing": "from a row's menu, Delete then Enter deletes", and a live session's question says
/// it kills the agent, quotes the title on a line of its own and, for a managed checkout, says what happens to it. A
/// click on the question's empty space keeps focus on Confirm (SPEC.md gives the row Delete question that rule), so the
/// Enter after it still answers what the user was looking at. Specified, by keys after the menu opens: Up to Delete
/// (wrapping from Rename to the last command) and Enter asks with focus on Confirm and sends nothing, the question
/// standing beside the row (right of its menu button, as the old app turned the menu into the question); its lines are
/// the consequence, the quoted title and the checkout line; a click on the question's empty space leaves focus on
/// Confirm; Enter then sends the Delete with no precondition flag and closes the question.
#[gpui::test]
fn delete_then_enter_asks_about_a_live_session_and_enter_deletes_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    test_support::click(cx, "session-menu-a");
    test_support::keys(cx, "up");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-delete"),
        "the premise: Up from the first command reaches Delete"
    );
    test_support::keys(cx, "enter");
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    let (question, button) = (
        test_support::bounds(cx, "prompt"),
        test_support::bounds(cx, "session-menu-a"),
    );
    assert!(
        question.left() > button.right(),
        "asked beside the row, from the anchor its menu opened on: {question:?} right of {button:?}"
    );
    let texts = painted(&frame(cx));
    for line in [
        "Still running — deleting kills the agent:",
        "The managed checkout stays while another session uses it. When its last session is deleted, its folder \
         moves to the trash; no files are deleted.",
    ] {
        assert!(texts.iter().any(|text| text == line), "{line}: {texts:?}");
    }
    assert_eq!(
        label_of(cx, "session-delete-title-a").as_deref(),
        Some("\"a\"")
    );
    assert_eq!(deletes(&mut fake), []);

    // Inside the question's 8-pixel padding: empty space.
    let corner = test_support::bounds(cx, "prompt").origin;
    cx.simulate_click(
        gpui::point(corner.x + gpui::px(3.), corner.y + gpui::px(3.)),
        Modifiers::none(),
    );
    test_support::check(cx);
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    test_support::keys(cx, "enter");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
    assert!(!any_layer(cx));
}

/// A session title is host text with no length limit, and the row's Delete question quotes it whole, wrapping rather
/// than cutting it (a recorded deviation in `prompt.rs`). The question must still say what Delete does and still be
/// answerable by pointer, or a long enough title would hide Cancel and Confirm below the window. The old
/// `terminal.spec.ts` case "a legal multi-KB, unbroken title keeps the consequence text intact and clips only the
/// title, without disturbing the confirm/cancel buttons" (reference commit `24bc0580`) pinned it. The prompt's own
/// height cap and its scrolling are pinned with a question built by hand, by
/// `prompt_tests::an_anchored_question_quoting_a_huge_title_keeps_its_answers_in_the_window`; this pins the real row's
/// question on that cap. Specified, in a window of the app's minimum size, for a running session whose title is 20,000
/// characters with no spaces: Delete chosen from its menu by keys asks with a quoted title taller than the window and
/// drawn clipped inside the question; the question, Cancel and Confirm lie inside the window; the consequence line and
/// the checkout's line are painted whole; nothing is sent until a click on Confirm sends the one Delete. Delete is
/// chosen by keys because, without the row menu's height fix (#4025), a row menu at the minimum window size runs past
/// the window's bottom, where Delete takes no click; the keys reach it either way.
#[gpui::test]
fn a_huge_title_leaves_the_delete_questions_words_and_answers_whole(cx: &mut TestAppContext) {
    use crate::host_text::tests::painted_words;

    /// The text painted inside the question whose words (isolates aside) hold `words`, which must be there. Only
    /// the question's own texts count: the row, the header and the details bar draw the same title too.
    fn painted_text(nodes: &[gpui::DriverNode], words: &str) -> gpui::DriverText {
        let texts = texts_within(nodes, "prompt");
        match texts
            .into_iter()
            .find(|painted| painted_words(&painted.text).contains(words))
        {
            Some(painted) => painted,
            None => panic!(
                "{}… was not painted in the question",
                words.chars().take(40).collect::<String>()
            ),
        }
    }

    let title = "x".repeat(20_000);
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a").title(&title), 0)], false));
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    test_support::settle(cx);
    deletes(&mut fake);
    // Delete is chosen by keys, as `delete_then_enter_asks_about_a_live_session_and_enter_deletes_it` chooses it: this
    // test is about the question, and what the pointer can reach inside the menu is the menu's tests' concern.
    test_support::click(cx, "session-menu-a");
    test_support::keys(cx, "up");
    assert_eq!(
        focused_id(cx).as_deref(),
        Some("session-menu-delete"),
        "the premise: Up from the first command reaches Delete"
    );
    test_support::keys(cx, "enter");
    assert!(drawn(cx, "prompt"), "the premise: a's question is open");

    let shown = gpui::Bounds::new(
        gpui::point(px(0.), px(0.)),
        cx.update(|window, _| window.viewport_size()),
    );
    let question = test_support::bounds(cx, "prompt");
    let nodes = frame(cx);
    let quoted = painted_text(&nodes, &title);
    assert!(
        quoted.bounds.size.height > shown.size.height,
        "the premise: the quoted title is taller than the window"
    );
    assert!(
        quoted.visible_bounds.size.height < quoted.bounds.size.height
            && quoted.visible_bounds.is_contained_within(&question),
        "the title is clipped inside the question: {:?} in {question:?}",
        quoted.visible_bounds
    );
    for control in ["prompt", "prompt-cancel", "prompt-confirm"] {
        let at = test_support::bounds(cx, control);
        assert!(
            at.is_contained_within(&shown),
            "{control} lies inside the window: {at:?}"
        );
    }
    for whole in [
        "Still running — deleting kills the agent:",
        super::delete::MANAGED_CHECKOUT,
    ] {
        let painted = painted_text(&nodes, whole);
        assert_eq!(
            painted.visible_bounds, painted.bounds,
            "{whole:?} is painted whole"
        );
    }
    assert_eq!(deletes(&mut fake), [], "nothing is sent before the answer");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
}

/// The precondition is what makes a stale row safe: SPEC.md, "Confirmed 2026-10-01", has the supervisor refuse a
/// Delete when the session has more alive than the click showed. The old app's `delete-precondition.spec.ts` pinned
/// three cases, and its `terminal.spec.ts` three more (reference commit `24bc0580`): an unknown status confirms first
/// with words that admit the uncertainty, since the agent may be running after all, while an interrupted session and
/// one whose agent never started have nothing alive and go at once. Specified: a running session asks and sends no
/// flag; an ended session with two tabs asks about the tabs and sends `only_if_agent_ended`; an ended session with no
/// tabs asks nothing and sends `only_if_nothing_alive` at once; a session whose status is unknown asks "Status unknown
/// — the agent may still be running and will be killed:" above its quoted title, sends nothing until Confirm and then
/// no flag; an interrupted session and an errored one with no tabs ask nothing and send `only_if_nothing_alive` at
/// once.
#[gpui::test]
fn each_delete_carries_the_precondition_of_what_its_row_showed(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(
        cx,
        snapshot(
            &[
                (session("a"), 0),
                (ended("b"), 2),
                (ended("c"), 0),
                (session("d").status(json!({"state": "unknown"})), 0),
                (session("e").status(json!({"state": "interrupted"})), 0),
                (
                    session("f").status(json!({"state": "error", "detail": "cannot launch"})),
                    0,
                ),
            ],
            false,
        ),
    );
    deletes(&mut fake);

    choose_delete(cx, "session-menu-a");
    assert!(any_layer(cx), "a running session asks");
    test_support::click(cx, "prompt-confirm");
    choose_delete(cx, "session-menu-b");
    assert!(
        painted(&frame(cx)).iter().any(|text| text
            == "2 terminal tabs are still open, and deleting closes them with everything running in them; delete \
                anyway:"),
        "an ended session's tabs are what it asks about"
    );
    test_support::click(cx, "prompt-confirm");
    choose_delete(cx, "session-menu-c");
    assert!(!any_layer(cx), "nothing alive asks nothing");
    assert_eq!(
        paths(&deletes(&mut fake)),
        [
            "/api/sessions/a",
            "/api/sessions/b?only_if_agent_ended=true",
            "/api/sessions/c?only_if_nothing_alive=true",
        ]
    );
    choose_delete(cx, "session-menu-d");
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text
                == "Status unknown — the agent may still be running and will be killed:"),
        "an unknown status admits the agent may be running"
    );
    assert_eq!(
        label_of(cx, "session-delete-title-d").as_deref(),
        Some("\"d\"")
    );
    assert_eq!(deletes(&mut fake), [], "d's Delete waits for Confirm");
    test_support::click(cx, "prompt-confirm");
    for (button, what) in [
        ("session-menu-e", "an interrupted session"),
        ("session-menu-f", "a session whose agent never started"),
    ] {
        choose_delete(cx, button);
        assert!(!any_layer(cx), "{what} asks nothing");
    }
    assert_eq!(
        paths(&deletes(&mut fake)),
        [
            "/api/sessions/d",
            "/api/sessions/e?only_if_nothing_alive=true",
            "/api/sessions/f?only_if_nothing_alive=true",
        ]
    );
}

/// A listing the helm could not read is no evidence that anything went, so the list keeps its rows
/// (`tests::a_failed_listing_keeps_its_rows_and_says_so_until_the_next_listing`), and a question about one of them
/// must stay too, or the user's answer would be lost to a transient read failure. The old `terminal.spec.ts` case "a
/// failed listing read while confirming does not clear the confirming state" (reference commit `24bc0580`) pinned it;
/// `an_open_menu_follows_its_button_through_what_the_helm_pushes` pins the same for an open menu. Specified: with b's
/// Delete question open, a pushed listing the helm could not read draws the failure line and leaves the question open;
/// the next listing read takes the line away and leaves it open too; Confirm then sends b's Delete and closes it.
#[gpui::test]
fn a_question_stays_open_and_answers_across_a_failed_listing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    assert!(drawn(cx, "prompt"), "the premise: b's question is open");

    let mut failed = listing_meta(&["a", "b"], 2, false);
    failed.availability = Availability::Unavailable {
        error: "down".into(),
    };
    for (step, meta, fails) in [
        ("the listing fails", failed, true),
        (
            "the listing is read again",
            listing_meta(&["a", "b"], 2, false),
            false,
        ),
    ] {
        fake.push(change(meta, &[], &[]));
        test_support::settle(cx);
        assert_eq!(
            label_of(cx, "session-list-error").is_some(),
            fails,
            "{step}: the premise: the failure line shows over the failed listing alone"
        );
        assert!(drawn(cx, "prompt"), "{step}: the question is still open");
    }
    assert_eq!(deletes(&mut fake), [], "nothing was sent before the answer");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/b"]);
    assert!(!any_layer(cx));
}

/// SPEC.md: with **Skip session confirmations** on, a row Delete "runs on the first click through the ordinary action
/// path, with consent from the state shown by that render". Specified: with the preference on, Delete on a running
/// session with a tab opens no question and sends the Delete with no flag.
#[gpui::test]
fn skipped_confirmations_delete_on_the_click_with_the_rows_guard(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 1)], true));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert!(!any_layer(cx));
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
}

/// The prompt's guarantee at the real question (SPEC_impl.md, "Questions: the prompt"): the answer acts on what the
/// question showed, and only the first answer runs, even when the session changes under the open question and both
/// answers land before a redraw. Specified: with a's question open, a pushed change ends a's agent, and Cancel then
/// Confirm delivered with no frame between them sends nothing; with b's question open and b ended the same way,
/// Confirm then Cancel sends b's Delete with no flag, the guard of the running row the question was drawn from.
///
/// At this real question the second click finds the question closed and its view already released, which is what
/// stops it here; the prompt's own first-answer guard, which matters when both answers reach a prompt that is still
/// alive, is tested by `prompt_tests::a_confirmation_acts_only_on_what_it_showed` (a hand mutant that removed the
/// guard failed only that test).
#[gpui::test]
fn a_question_answers_once_for_what_it_showed_even_after_the_session_changed(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);

    for (key, button, order, expected) in [
        (
            "a",
            "session-menu-a",
            ["prompt-cancel", "prompt-confirm"],
            vec![],
        ),
        (
            "b",
            "session-menu-b",
            ["prompt-confirm", "prompt-cancel"],
            vec!["/api/sessions/b"],
        ),
    ] {
        choose_delete(cx, button);
        assert!(any_layer(cx), "the premise: {key}'s question is open");
        push(&mut fake, &["a", "b"], &[(ended(key), 0)], cx);
        assert!(any_layer(cx), "{key}: an ended session is not gone");
        let points = [
            test_support::center(cx, order[0]),
            test_support::center(cx, order[1]),
        ];
        cx.update(|window, cx| {
            for at in points {
                for event in click_at(at) {
                    window.dispatch_event(event, cx);
                }
            }
        });
        test_support::settle(cx);
        assert_eq!(paths(&deletes(&mut fake)), expected, "{key}: {order:?}");
        assert!(!any_layer(cx), "{key}: the question closed");
    }
}

/// SPEC.md: "an open lifecycle confirmation … refuses another lifecycle action … on that session", and the row's
/// button must not reopen a menu over its own question. Specified: with a's question open, clicking a's menu button
/// leaves the question open, opens no menu and sends nothing.
#[gpui::test]
fn the_menu_button_is_inert_while_its_question_is_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    assert!(drawn(cx, "prompt"), "the premise: the question is open");

    test_support::click(cx, "session-menu-a");
    test_support::redraw(cx);
    assert!(drawn(cx, "prompt"));
    assert!(!drawn(cx, "session-menu"));
    assert_eq!(sent(&mut fake), []);
}

// ---- A pending and a refused Delete --------------------------------------------------------------------------------

/// SPEC.md, "Lifecycle operations": while a Delete runs the row shows "a spinner and what is happening in place of its
/// title", the session's own competing actions are refused, other sessions act normally, and "Opening the session
/// remains possible, including while its Delete is pending". The old row hid its title meanwhile and kept it for
/// assistive technology. Specified: with a's Delete sent and unanswered, a's row paints "Stopping agent…" and not its
/// title, which stays its open control's accessible name; a's menu Delete does nothing, not even close the menu; b's
/// menu still asks; clicking a's row opens a, and the Delete shortcut over it asks nothing; no second Delete is sent.
#[gpui::test]
fn a_pending_delete_shows_on_its_row_refuses_another_and_leaves_the_rest_alone(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("b"), 0), (session("a"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);

    let texts = painted(&frame(cx));
    assert!(texts.iter().any(|text| text == "Stopping agent…"));
    assert!(!texts.iter().any(|text| text == "a"), "the title gives way");
    assert_eq!(label_of(cx, "session-row-a").as_deref(), Some("a"));
    choose_delete(cx, "session-menu-a");
    assert!(!drawn(cx, "prompt"), "a's own Delete is refused");
    assert!(
        drawn(cx, "session-menu"),
        "and is not even taken: its menu stays open"
    );
    test_support::keys(cx, "escape");

    choose_delete(cx, "session-menu-b");
    assert!(drawn(cx, "prompt"), "b's Delete still asks");
    test_support::click(cx, "prompt-cancel");

    test_support::click(cx, "session-row-a");
    assert_eq!(selected(cx).as_deref(), Some("a"));
    test_support::keys(cx, DELETE_CHORD);
    assert!(!drawn(cx, "prompt"), "nor does the shortcut ask about it");
    // The writes for opening a are behind a's unanswered Delete, so they are read, not answered.
    assert!(
        sent(&mut fake)
            .iter()
            .all(|(path, _)| !path.starts_with("/api/sessions/a?") && path != "/api/sessions/a"),
        "no second Delete"
    );
}

/// SPEC.md: "a pending request refuses another lifecycle action", whatever the request: a session being renamed is not
/// deleted under it, which would also make the Delete's Confirm send nothing (the operation store refuses a second
/// request). Specified: with a Rename of a sent and unanswered, a's menu Delete opens nothing and leaves the menu open,
/// and the shortcut over the open session a asks nothing; no Delete is sent.
#[gpui::test]
fn any_pending_request_refuses_the_rows_delete(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0)], false));
    deletes(&mut fake);
    cx.update(|_, cx| {
        let operations = session::current_operations(cx).unwrap();
        let rename = super::rename::command("a", "renamed").unwrap();
        operations.update(cx, |operations, cx| {
            operations
                .start(&OpKey::session("a"), rename, cx, |_, _| {})
                .unwrap()
        });
    });
    assert!(
        sent(&mut fake)
            .iter()
            .any(|(path, _)| path == "/api/sessions/a/rename"),
        "the premise: a's Rename is in flight"
    );

    choose_delete(cx, "session-menu-a");
    assert!(!drawn(cx, "prompt"));
    assert!(
        drawn(cx, "session-menu"),
        "Delete is unavailable in the menu"
    );
    test_support::keys(cx, "escape");
    test_support::keys(cx, DELETE_CHORD);
    assert!(!drawn(cx, "prompt"));
    assert_eq!(sent(&mut fake), []);
}

/// SPEC.md: "A failure or partial result appears on the source session's row", and the operation store keeps it "until
/// the next operation on that session starts". Specified: a refused Delete paints the helm's words on the row and
/// takes the spinner away; they stay through an unrelated pushed change; a new Delete of the row asks again and, once
/// confirmed, the words are gone and the spinner is back.
#[gpui::test]
fn a_refusal_shows_on_the_row_until_the_next_delete_starts(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };
    fake.reply(id, 409, Some(json!("the agent is busy")));
    test_support::settle(cx);
    let texts = painted(&frame(cx));
    assert!(texts.iter().any(|text| text == "the agent is busy"));
    assert!(!texts.iter().any(|text| text == "Stopping agent…"));

    push(
        &mut fake,
        &["a", "b"],
        &[(session("b").title("renamed"), 0)],
        cx,
    );
    assert!(
        painted(&frame(cx))
            .iter()
            .any(|text| text == "the agent is busy")
    );

    choose_delete(cx, "session-menu-a");
    assert!(drawn(cx, "prompt"), "a refused Delete asks again");
    test_support::click(cx, "prompt-confirm");
    let texts = painted(&frame(cx));
    assert!(!texts.iter().any(|text| text == "the agent is busy"));
    assert!(texts.iter().any(|text| text == "Stopping agent…"));
}

/// A row's menu shows the session's last refusal uncut, under its commands, and the helm's words can make it taller
/// than a small window; placed by the layer manager's clamp alone, such a menu sat at the window's top and ran past its
/// bottom, out of reach. The old panel was capped to the viewport and scrolled, with the refusal inside its scroll
/// (`tests/sidebar.spec.ts`, "a retained refusal stays inside a bottom-row menu's scrolling panel", at reference commit
/// `24bc0580`, whose 120-word refusal this reuses). Specified: in a window of the app's least size, with b's Delete
/// refused in 120 words, b's reopened menu lies inside the window with focus on Rename, drawn wholly inside the menu,
/// and the refusal runs past the menu's bottom; End reaches Delete, drawn wholly inside the menu (the commands fit
/// unscrolled, so this checks reach, not the reveal); a wheel turn over the menu brings the refusal's end inside it, and
/// the menu stays where it was; with Rename scrolled out above the menu, Home focuses it and scrolls it wholly inside.
#[gpui::test]
fn a_long_refusal_scrolls_inside_a_menu_that_stays_in_a_small_window(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    let height = crate::bootstrap::MIN_WINDOW_SIZE.height;
    cx.simulate_resize(crate::bootstrap::MIN_WINDOW_SIZE);
    test_support::settle(cx);
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };
    let refusal: Vec<String> = (0..120).map(|n| format!("refusal-sentinel-{n}")).collect();
    fake.reply(id, 409, Some(json!(refusal.join(" "))));
    test_support::settle(cx);

    test_support::click(cx, "session-menu-b");
    let menu = test_support::bounds(cx, "session-menu");
    assert!(
        menu.top() >= px(0.) && menu.bottom() <= height,
        "inside the window: {menu:?}"
    );
    let inside = |item: Bounds<Pixels>| menu.top() <= item.top() && item.bottom() <= menu.bottom();
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert!(inside(node_bounds(cx, "session-menu-rename")));
    let line = node_bounds(cx, "session-menu-outcome-b");
    assert!(
        line.bottom() > menu.bottom(),
        "the premise: the refusal runs past the menu: {line:?} in {menu:?}"
    );
    test_support::keys(cx, "end");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-delete"));
    assert!(inside(node_bounds(cx, "session-menu-delete")));

    cx.simulate_mouse_move(menu.center(), None, Modifiers::none());
    cx.simulate_event(ScrollEvent {
        position: menu.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
        ..Default::default()
    });
    test_support::settle(cx);
    assert_eq!(test_support::bounds(cx, "session-menu"), menu);
    // The refusal alone is taller than the menu, as in the old case, so only its end can come inside.
    let line = node_bounds(cx, "session-menu-outcome-b");
    assert!(
        menu.top() < line.bottom() && line.bottom() <= menu.bottom(),
        "the refusal's end scrolled into the menu: {line:?} in {menu:?}"
    );

    let rename = node_bounds(cx, "session-menu-rename");
    assert!(
        rename.top() < menu.top(),
        "the premise: Rename scrolled out of the menu: {rename:?} in {menu:?}"
    );
    test_support::keys(cx, "home");
    assert_eq!(focused_id(cx).as_deref(), Some("session-menu-rename"));
    assert!(inside(node_bounds(cx, "session-menu-rename")));
}

/// SPEC.md, "Session list": "Automatic selection after Delete waits until every admitted Delete has settled … and
/// never opens a session that is still being deleted". The helm pushes the removal before it answers, so a list that
/// picked the next session on the removal would open it while the Delete was still on its way. Specified: with a open
/// and its Delete sent, a pushed listing without a leaves nothing open; once the Delete is answered, b opens and is
/// remembered.
#[gpui::test]
fn the_next_session_opens_only_once_the_delete_has_settled(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise: a is open");
    deletes(&mut fake);
    choose_delete(cx, "session-menu-a");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };

    fake.push(change(listing_meta(&["b"], 1, false), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(selected(cx), None, "nothing opens while the Delete runs");

    fake.reply(id, 200, None);
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("b"));
    assert!(
        sent(&mut fake)
            .iter()
            .any(|(path, body)| path == "/api/preferences" && body["last_selected"] == "b")
    );
}

/// The old `sidebar.spec.ts` case "visit history chooses and remembers the next session after ${mode} deletion": SPEC.md
/// has the automatic choice take "the most recent visit ... still listed" before the newest-created session, so a Delete
/// takes the user back to where they were rather than to whatever was made last, and an ended session counts, since
/// eligibility is about being listed, not alive. The rule is unit-tested
/// (`the_automatic_choice_prefers_history_then_the_last_selection_then_the_newest`); the test above has a history of
/// one entry. Specified: with b open, an ended a made first, c made last, and the visit history b then a, b's Delete
/// from its row, once the helm removes b and answers, opens a, not c, and remembers a.
#[gpui::test]
fn after_a_delete_the_visit_history_beats_a_newer_session_and_an_ended_one_counts(
    cx: &mut TestAppContext,
) {
    let rows = [
        (session("b").created(200), 0),
        (ended("a").created(0), 0),
        (session("c").created(300), 0),
    ];
    let mut state = snapshot(&rows, false);
    state[1].entries[0].value["visit_history"] = json!(["b", "a"]);
    let (mut fake, cx) = open_main(cx, state);
    assert_eq!(selected(cx).as_deref(), Some("b"), "the premise: b is open");
    deletes(&mut fake);
    choose_delete(cx, "session-menu-b");
    test_support::click(cx, "prompt-confirm");
    let [(id, _)] = deletes(&mut fake)[..] else {
        panic!("one Delete was sent");
    };

    fake.push(change(listing_meta(&["a", "c"], 2, false), &[], &["b"]));
    fake.reply(id, 200, None);
    test_support::settle(cx);
    assert_eq!(selected(cx).as_deref(), Some("a"));
    assert!(
        sent(&mut fake)
            .iter()
            .any(|(path, body)| path == "/api/preferences" && body["last_selected"] == "a")
    );
}

// ---- The Delete shortcut -------------------------------------------------------------------------------------------

/// SPEC.md, "Desktop window chrome": the shortcut "invokes the open session's header Delete action with its ordinary
/// confirmation and busy rules. With no session open it does nothing", and "A visible modal keeps keyboard ownership".
/// So it asks the header's question, hung under the header's Delete button, not the row's. Specified: the chord asks
/// about the open session a, below the header's Delete, with focus on Confirm and nothing sent; the chord again while
/// the question is open changes nothing; Enter deletes a.
#[gpui::test]
fn the_shortcut_asks_about_the_open_session_and_waits_while_a_question_is_open(
    cx: &mut TestAppContext,
) {
    let (mut fake, cx) = open_main(cx, snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    deletes(&mut fake);
    test_support::keys(cx, DELETE_CHORD);
    let button = test_support::bounds(cx, "session-header-delete");
    let question = test_support::bounds(cx, "prompt");
    assert!(
        question.top() >= button.bottom(),
        "asked below the header's Delete"
    );
    assert_eq!(focused_id(cx).as_deref(), Some("prompt-confirm"));
    assert_eq!(deletes(&mut fake), []);

    test_support::keys(cx, DELETE_CHORD);
    assert!(drawn(cx, "prompt"));
    assert_eq!(deletes(&mut fake), []);

    test_support::keys(cx, "enter");
    assert_eq!(paths(&deletes(&mut fake)), ["/api/sessions/a"]);
}

/// The shortcut's two quiet cases: with no session open it does nothing (and the key must still be taken, as a
/// once-per-press shortcut's must), and a menu that is open keeps the keyboard. Specified: over an empty fleet the
/// chord opens nothing and sends nothing; with b's menu open, the chord leaves the menu as the only layer and sends
/// nothing.
#[gpui::test]
fn the_shortcut_does_nothing_with_no_session_open_or_a_menu_open(cx: &mut TestAppContext) {
    let (mut fake, cx) = open_main(cx, snapshot(&[], false));
    test_support::keys(cx, DELETE_CHORD);
    assert!(!any_layer(cx));
    assert_eq!(sent(&mut fake), []);

    fake.connect(snapshot(&[(session("a"), 0), (session("b"), 0)], false));
    test_support::settle(cx);
    deletes(&mut fake);
    test_support::click(cx, "session-menu-b");
    test_support::keys(cx, DELETE_CHORD);
    assert!(drawn(cx, "session-menu"));
    assert!(!drawn(cx, "prompt"));
    assert_eq!(deletes(&mut fake), []);
}

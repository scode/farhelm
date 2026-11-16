//! Tests of the host rail's expansion: the Expand control at the foot (its place, names, hover help and keys), the
//! `host_rail_open` preference it writes and the pushed preference it follows, the expanded rail's rows and labels,
//! choosing and badges in the expanded rail, and the sidebar's shown width (the 164-pixel rule and the main pane's
//! floor).
//!
//! Like the rail's other tests (`host_rail/tests.rs`, whose fixtures these share), they run the real main view over the
//! helm client's fake, which hands the test every command the window sent and lets it push preference changes and
//! answer commands as the helm would, and read controls from GPUI's driver record of a fresh frame. GPUI's record has
//! no expanded state, so the control's `aria-expanded` is not read back here; its name, which says the same thing in
//! words, is. The tests whose outcome is real text layout (a name cut short, the list's width beside a cut name) use
//! the real text system, since the test context's fake one gives every character the same width.

use std::sync::Arc;

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::{FakeCommand, FakeHelm};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, HostView, KeyedEntry, PREFERENCES_KEY,
    Preferences,
};
use gpui::{
    AppContext as _, Bounds, DriverNode, HeadlessAppContext, Modifiers, MouseButton, Pixels, Role,
    TestAppContext, VisualTestContext, point, px, size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::EXTRA;
use crate::hosts::tests::{connected, host as status_host, hosts as status_hosts};
use crate::main_view::MainView;
use crate::session_list::host_rail::tests::{
    answer_everything, fleet_hosts, hosts, hover_help, label, node, old_build, pressed,
    texts_within,
};
use crate::session_list::host_rail::{COLLAPSE_NAME, EXPAND_NAME, TOGGLE_ID};
use crate::session_list::tests::{listing, preferences_state, session};
use crate::sidebar::shown_width;
use crate::test_support::{self, click, focused_id, frame, keys, meta};
use crate::{assets, bootstrap};

/// The foot's other controls, by their element ids.
const BELL: &str = crate::rail_notices::BELL_ID;
const HOSTS: &str = "hosts-panel-toggle";
const GEAR: &str = "app-settings-toggle";
const HELP: &str = "app-help-toggle";

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// The preference row with session `a` visited, the Welcome card seen, `open` as the stored expansion and `width` as
/// the stored sidebar width.
fn stored(open: Option<bool>, width: Option<u32>) -> Preferences {
    Preferences {
        visit_history: vec!["a".to_owned()],
        welcome_seen: Some(true),
        host_rail_open: open,
        sidebar_width: width,
        ..Preferences::default()
    }
}

/// Sessions `a` and `b` over the rail's usual fleet ([`fleet_hosts`]), with the helm storing `open` as the rail's
/// expansion and `width` as the sidebar's width.
fn fleet(open: Option<bool>, width: Option<u32>) -> Vec<CollectionState> {
    vec![
        listing(&["a", "b"], &[session("a"), session("b")]),
        preferences_state(stored(open, width)),
        hosts(&fleet_hosts()),
    ]
}

/// The real main view over the fake helm, connected with [`fleet`] storing `open`, in a 1200 by 700 window, with
/// every command sent at startup (the selection remembered, a seen mark) answered, so a test sees only its own.
fn open(cx: &mut TestAppContext, open: Option<bool>) -> (FakeHelm, &mut VisualTestContext) {
    let (mut fake, cx) = crate::session_list::tests::open_main(cx, fleet(open, None));
    cx.simulate_resize(size(px(1200.), px(700.)));
    answer_everything(&mut fake);
    test_support::settle(cx);
    (fake, cx)
}

/// The helm pushing `open` as the stored expansion, as another client's or an agent's write would reach this window.
fn push_open(fake: &mut FakeHelm, open: Option<bool>, cx: &mut VisualTestContext) {
    fake.push(CollectionChanges {
        collection: Collection::Preferences,
        meta: meta(&[PREFERENCES_KEY]),
        upsert: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::to_value(stored(open, None)).unwrap(),
        }],
        remove: vec![],
    });
    test_support::settle(cx);
}

/// The one command the window sent since the last look, which must be a `PUT /api/preferences` naming only
/// `host_rail_open`: the command (left unanswered, as a helm that has not replied yet) and that field's value.
fn one_write(fake: &mut FakeHelm) -> (FakeCommand, Value) {
    let mut sent = fake.commands();
    assert_eq!(sent.len(), 1, "one command: {sent:?}");
    let command = sent.remove(0);
    assert_eq!(
        (command.method, command.path.as_str()),
        (Method::Put, "/api/preferences")
    );
    let body = command.body.clone().unwrap_or_default();
    let fields = body.as_object().map_or(0, |fields| fields.len());
    assert_eq!(fields, 1, "a sparse write: {body}");
    let value = body["host_rail_open"].clone();
    (command, value)
}

/// The bounds of what was drawn under `id` in `nodes`; fails if nothing was.
fn at(nodes: &[DriverNode], id: &str) -> Bounds<Pixels> {
    match node(nodes, id) {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    }
}

/// The rail's width in a fresh frame (its scrolling part spans it).
fn rail_width(cx: &mut VisualTestContext) -> Pixels {
    test_support::bounds(cx, "host-rail").size.width
}

/// The lines of text painted under `id` in a fresh frame, as strings.
fn painted(cx: &mut VisualTestContext, id: &str) -> Vec<String> {
    texts_within(&frame(cx), id)
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

// ---- The Expand control ------------------------------------------------------------------------------------------

/// SPEC.md: "An ordinary keyboard-reachable button below Hosts, Settings and Help shows » and "Expand navigation rail"
/// while collapsed". It is the foot's last control, as the old `#host-rail-toggle` was, and a control that drifted or
/// lost its words would leave the rail's names out of reach. A helm that stores nothing starts collapsed. Specified: in a
/// 1200 by 700 window over a helm storing no expansion, the rail is 56 pixels wide; the control is a button named
/// "Expand navigation rail", not a choice (no pressed state), a 40-pixel square 7.5 pixels from the window's left edge,
/// directly below Help, its bottom 8 pixels above the window's (the foot's padding); it paints "»" alone; and its hover
/// help is "Expand navigation rail".
#[gpui::test]
fn the_expand_control_sits_below_help_with_the_old_toggles_words(cx: &mut TestAppContext) {
    let (_fake, cx) = open(cx, None);
    assert_eq!(rail_width(cx), px(56.));
    let nodes = frame(cx);
    let toggle = node(&nodes, TOGGLE_ID).expect("the Expand control is drawn");
    assert_eq!(toggle.role, Some(Role::Button));
    assert_eq!(toggle.label.as_deref(), Some(EXPAND_NAME));
    assert_eq!(EXPAND_NAME, "Expand navigation rail");
    assert_eq!(toggle.toggled, None, "not a choice");
    let bounds = toggle.bounds;
    assert_eq!(
        (bounds.size.width, bounds.size.height, bounds.left()),
        (px(40.), px(40.), px(7.5))
    );
    assert_eq!(bounds.bottom(), px(700. - 8.));
    assert_eq!(
        at(&nodes, HELP).bottom(),
        bounds.top(),
        "directly below Help"
    );
    assert_eq!(painted(cx, TOGGLE_ID), ["»"]);
    assert_eq!(
        hover_help(cx, TOGGLE_ID).as_deref(),
        Some("Expand navigation rail")
    );
}

/// The expansion is a helm preference written the way Compact is (SPEC.md: "Expansion is a helm preference that every
/// client shares"): a sparse write, so it never puts back a stale copy of another field, and shown at once while it is
/// in flight, as the old toggle widened the rail on the click. Collapsing must store `false` rather than leave the
/// field. Specified: a click on the control sends exactly one command, `PUT /api/preferences` with
/// `{"host_rail_open": true}`, and while it is unanswered the rail is 220 pixels wide and the control is named and
/// helped "Collapse navigation rail" and paints "«" and "Collapse"; once the helm pushes the stored preference and
/// answers, the rail stays expanded; a second click sends `{"host_rail_open": false}` and the rail is 56 pixels again,
/// and stays so once the helm stores it.
#[gpui::test]
fn expanding_writes_the_preference_and_shows_the_choice_in_flight(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    click(cx, TOGGLE_ID);
    let (write, value) = one_write(&mut fake);
    assert_eq!(value, json!(true));
    assert_eq!(rail_width(cx), px(220.), "while the write is in flight");
    assert_eq!(label(&frame(cx), TOGGLE_ID), COLLAPSE_NAME);
    assert_eq!(COLLAPSE_NAME, "Collapse navigation rail");
    assert_eq!(painted(cx, TOGGLE_ID), ["«", "Collapse"]);
    assert_eq!(
        hover_help(cx, TOGGLE_ID).as_deref(),
        Some("Collapse navigation rail")
    );

    push_open(&mut fake, Some(true), cx);
    fake.reply(write.id, 200, None);
    test_support::settle(cx);
    assert_eq!(rail_width(cx), px(220.), "the stored expansion");

    click(cx, TOGGLE_ID);
    let (write, value) = one_write(&mut fake);
    assert_eq!(value, json!(false));
    assert_eq!(rail_width(cx), px(56.), "collapsed while in flight");
    assert_eq!(label(&frame(cx), TOGGLE_ID), EXPAND_NAME);
    push_open(&mut fake, Some(false), cx);
    fake.reply(write.id, 200, None);
    test_support::settle(cx);
    assert_eq!(rail_width(cx), px(56.));
    assert_eq!(fake.commands(), [], "nothing else was sent");
}

/// The rail is drawn from the preference the helm pushes, so a choice made in another window, by the command line or
/// by an agent reaches this window as it happens, and a window opens with the rail as it was last left (SPEC.md). The
/// window keeps no copy. Specified: a window opened over a helm storing the rail expanded draws it 220 pixels wide; a
/// push of the preference unset collapses it to 56, and a push of `true` expands it again, the control's name
/// following each time; nothing is sent.
#[gpui::test]
fn the_rail_follows_the_stored_preference_and_its_pushes(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, Some(true));
    assert_eq!(rail_width(cx), px(220.), "opened as it was left");
    assert_eq!(label(&frame(cx), TOGGLE_ID), COLLAPSE_NAME);
    push_open(&mut fake, None, cx);
    assert_eq!(rail_width(cx), px(56.));
    assert_eq!(label(&frame(cx), TOGGLE_ID), EXPAND_NAME);
    push_open(&mut fake, Some(true), cx);
    assert_eq!(rail_width(cx), px(220.));
    assert_eq!(label(&frame(cx), TOGGLE_ID), COLLAPSE_NAME);
    assert_eq!(fake.commands(), [], "following a push sends nothing");
}

/// SPEC.md, "Errors and diagnostics": a helm-side preference's failed save is logged and silent, "the choice stays in
/// effect only until the next read from the helm and is never resent". Under the channel the next read is at once, so
/// a refused expansion leaves the rail as the helm stores it, as a refused Compact leaves the rows. Specified: a click
/// sends `{"host_rail_open": true}`; once the helm refuses it with words of its own, the rail is 56 pixels wide again,
/// the frame paints none of the helm's words, and nothing is sent again; a second click still sends its write.
#[gpui::test]
fn a_refused_expansion_leaves_the_rail_as_stored_and_shows_nothing(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    click(cx, TOGGLE_ID);
    let (write, value) = one_write(&mut fake);
    assert_eq!(value, json!(true));
    fake.reply(write.id, 400, Some(json!("no wide rails on this helm")));
    test_support::settle(cx);
    assert_eq!(rail_width(cx), px(56.));
    let shown: Vec<String> = frame(cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(
        !shown.iter().any(|text| text.contains("wide rails")),
        "the refusal shows nothing: {shown:?}"
    );
    assert_eq!(fake.commands(), [], "a refused write is not resent");
    click(cx, TOGGLE_ID);
    let (_, value) = one_write(&mut fake);
    assert_eq!(value, json!(true), "a later press still sends");
}

/// The rail's write shares the preference row's one request at a time with the list's order, so a press while an order
/// write is out must wait for it rather than be lost (SPEC.md, "Losing the connection to the helm": an action is never
/// lost silently), and it must widen the rail at once, as a press does when nothing is in flight. This exercises the
/// queue's wait on a write it did not send. Specified: with `{"list_sort": "title"}` sent from the order menu and
/// unanswered, a click on the control sends nothing yet while the rail is 220 pixels wide and the control is named
/// "Collapse navigation rail"; once the helm answers the order write, exactly `{"host_rail_open": true}` is sent and
/// the rail stays expanded.
#[gpui::test]
fn a_press_during_an_order_write_is_shown_at_once_and_sent_after_it(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    click(cx, crate::session_list::order::TITLE_ID);
    click(cx, "session-order-title");
    let (order, body) = test_support::one_command(&mut fake, Method::Put, "/api/preferences");
    assert_eq!(
        body,
        json!({ "list_sort": "title" }),
        "the premise: the order write is in flight"
    );

    click(cx, TOGGLE_ID);
    assert_eq!(
        fake.commands(),
        [],
        "nothing is sent while the order's is out"
    );
    assert_eq!(rail_width(cx), px(220.), "shown at once");
    assert_eq!(label(&frame(cx), TOGGLE_ID), COLLAPSE_NAME);

    fake.reply(order, 200, None);
    test_support::settle(cx);
    let (_, value) = one_write(&mut fake);
    assert_eq!(value, json!(true));
    assert_eq!(rail_width(cx), px(220.));
}

/// The old toggle was an ordinary button: Tab reaches it after Help, and Enter and Space activate it without moving
/// focus (the old `keyboard activation expands and collapses the rail`). Focus staying on it matters more here than for
/// most buttons, since its name, look and width change under it. Specified (Linux's rule): with focus on Hosts, three
/// Tabs reach the gear, Help and the control; Enter sends `{"host_rail_open": true}` and expands the rail with focus
/// still on the control; once the helm has stored it, Space sends `{"host_rail_open": false}` and collapses it, focus
/// still on the control.
#[gpui::test]
fn tab_reaches_the_control_and_enter_and_space_toggle_it_keeping_focus(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, None);
    // Hosts keeps focus whatever the click rule; a second press brings Sessions back.
    click(cx, HOSTS);
    click(cx, HOSTS);
    assert_eq!(
        focused_id(cx).as_deref(),
        Some(HOSTS),
        "the premise: Hosts has focus"
    );
    for expected in [GEAR, HELP, TOGGLE_ID] {
        keys(cx, "tab");
        assert_eq!(focused_id(cx).as_deref(), Some(expected));
    }
    keys(cx, "enter");
    let (write, value) = one_write(&mut fake);
    assert_eq!(value, json!(true));
    assert_eq!(rail_width(cx), px(220.));
    assert_eq!(focused_id(cx).as_deref(), Some(TOGGLE_ID), "after Enter");
    push_open(&mut fake, Some(true), cx);
    fake.reply(write.id, 200, None);
    test_support::settle(cx);

    keys(cx, "space");
    let (_, value) = one_write(&mut fake);
    assert_eq!(value, json!(false));
    assert_eq!(rail_width(cx), px(56.));
    assert_eq!(focused_id(cx).as_deref(), Some(TOGGLE_ID), "after Space");
}

// ---- The expanded rail ---------------------------------------------------------------------------------------------

/// SPEC.md: expanded, "host rows show the display name and a smaller destination underneath when different ...
/// Captions disappear, All hosts, Add host and the foot controls show labels, and each whole row highlights and selects
/// as one click target". The old test checked the same, and that the icons kept their column and size: a mark that
/// moved sideways when the rail expanded would make every click after it miss. Names and labels are pictures of what
/// the buttons are already named, so the names stay as they were. Specified: over the usual fleet, once the helm pushes
/// the rail expanded, the rail is 220 pixels wide and every entry a row 205 pixels wide (the rail less its padding and
/// edge) from the same left edge; no caption is drawn; All paints "All" and "All hosts", Add host "Add host", the local
/// avatar its display name "Local (this machine)" alone, the unaliased remote its destination alone (its name is the
/// destination), and the aliased remote two lines, its alias's start over its destination's start, both starting 48
/// pixels into the row's content (inside its 1-pixel edge); the foot's controls are rows at the same left edge painting
/// "Notifications", "Hosts", "Settings", "Help" and "«", "Collapse"; every avatar's connection dot keeps its left edge
/// and size and every foot control its place (the rows close up as the captions under them go, which the old test
/// allowed too: it compared the icons' x and size only); every avatar keeps its name; and with one unread notice the
/// bell is still named "Notifications: 1 unread" (the notices bell's owner asked that its count stay in its name in the
/// expanded rail, where the label beside the glyph is only a picture of that name).
#[gpui::test]
fn the_expanded_rail_names_its_rows_and_keeps_every_mark_in_its_column(cx: &mut TestAppContext) {
    // One unread notice about host 2's trash in the first snapshot, as the helm serves it, so the bell has a count to
    // name.
    let mut snapshot = fleet(None, None);
    snapshot.push(CollectionState {
        collection: Collection::Notices,
        meta: meta(&["1"]),
        entries: vec![KeyedEntry {
            key: "1".into(),
            value: json!({
                "id": 1, "host": 2, "raised_at": 1_794_591_000, "read": false, "resolved": false,
                "content": {
                    "kind": "stuck_trash", "checkout_id": "c1", "name": "c1-20261113T174055Z",
                    "repository": "acme/web", "bytes": 4096, "reason": "Permission denied", "at": 1_794_591_900,
                },
            }),
        }],
    });
    let (mut fake, cx) = crate::session_list::tests::open_main(cx, snapshot);
    cx.simulate_resize(size(px(1200.), px(700.)));
    answer_everything(&mut fake);
    test_support::settle(cx);
    let entries = ["host-rail-host-1", "host-rail-host-3", "host-rail-host-2"];
    let dots = ["host-rail-dot-1", "host-rail-dot-3", "host-rail-dot-2"];
    let foot = [BELL, HOSTS, GEAR, HELP, TOGGLE_ID];
    let before = frame(cx);
    let names: Vec<String> = entries.iter().map(|id| label(&before, id)).collect();
    let dots_before: Vec<_> = dots
        .iter()
        .map(|dot| test_support::bounds(cx, dot))
        .collect();

    push_open(&mut fake, Some(true), cx);
    assert_eq!(rail_width(cx), px(220.));
    let nodes = frame(cx);
    for id in ["host-rail-all", "host-rail-add"].iter().chain(&entries) {
        let row = at(&nodes, id);
        assert_eq!(
            (row.left(), row.size.width, row.size.height),
            (px(7.), px(205.), px(40.)),
            "{id} is a row"
        );
    }
    for id in [1, 3, 2] {
        assert!(
            node(&nodes, &format!("host-rail-caption-{id}")).is_none(),
            "host {id}'s caption is gone"
        );
    }
    assert_eq!(painted(cx, "host-rail-all"), ["All", "All hosts"]);
    assert_eq!(painted(cx, "host-rail-add"), ["Add host"]);
    assert_eq!(painted(cx, "host-rail-host-1"), ["Local (this machine)"]);
    // The test context's text system gives every character one width, so a long destination is cut here; the real
    // text system's widths are `the_sidebar_widens_by_164_pixels_down_to_the_main_panes_floor`'s.
    let unaliased = painted(cx, "host-rail-host-2");
    assert_eq!(unaliased.len(), 1, "the destination alone: {unaliased:?}");
    assert!(unaliased[0].starts_with("ops@render-west"), "{unaliased:?}");
    let aliased = texts_within(&nodes, "host-rail-host-3");
    assert_eq!(
        aliased.len(),
        2,
        "the alias over the destination: {aliased:?}"
    );
    assert!(
        aliased[0].text.starts_with("Build"),
        "{:?}",
        aliased[0].text
    );
    assert!(
        aliased[1].text.starts_with("deploy@"),
        "{:?}",
        aliased[1].text
    );
    assert!(aliased[0].bounds.top() < aliased[1].bounds.top());
    let row = at(&nodes, "host-rail-host-3");
    for line in &aliased {
        assert_eq!(line.bounds.left(), row.left() + px(1. + 48.), "{line:?}");
    }

    let bell = painted(cx, BELL);
    assert!(bell.iter().any(|text| text == "Notifications"), "{bell:?}");
    assert_eq!(label(&nodes, BELL), "Notifications: 1 unread");
    for (id, label) in [
        (HOSTS, vec!["Hosts"]),
        (GEAR, vec!["Settings"]),
        (HELP, vec!["Help"]),
        (TOGGLE_ID, vec!["«", "Collapse"]),
    ] {
        assert_eq!(painted(cx, id), label, "{id}");
    }
    for (id, was) in foot.iter().map(|id| (*id, at(&before, id))) {
        let now = at(&nodes, id);
        assert_eq!(
            (now.left(), now.top(), now.size.height),
            (was.left(), was.top(), was.size.height),
            "{id} stayed in place"
        );
        assert!(now.size.width > px(150.), "{id} is a row: {now:?}");
    }
    for (dot, was) in dots.iter().zip(dots_before) {
        let now = test_support::bounds(cx, dot);
        assert_eq!(
            (now.left(), now.size),
            (was.left(), was.size),
            "{dot} kept its column"
        );
    }
    for (id, name) in entries.iter().zip(names) {
        assert_eq!(label(&nodes, id), name, "{id} keeps its name");
    }
}

/// SPEC.md: "Picking a host keeps the rail expanded. Badges, menus and actions retain their behavior", and the old
/// test clicked a host's expanded name, beyond its icon, to show the whole row is the click target. Specified: over a
/// local host and a remote on an older build, with the rail stored expanded, a click on the remote's row 60 pixels past
/// its icon box sends exactly one query, `{"host": 2, "selected": "a"}` (no write of the expansion), presses the avatar
/// and leaves the rail 220 pixels wide; the remote's update badge is drawn inside its icon box, its dot too, and Hosts
/// is named for the one update and draws its `↑`.
#[gpui::test]
fn choosing_and_badges_work_in_the_expanded_rail(cx: &mut TestAppContext) {
    let listed: [HostView; 2] = [
        status_host(1, "local", connected()),
        status_host(2, "ssh", old_build()),
    ];
    let (mut fake, cx) = crate::session_list::tests::open_main(
        cx,
        vec![
            listing(&["a", "b"], &[session("a"), session("b")]),
            preferences_state(stored(Some(true), None)),
            status_hosts(&listed),
        ],
    );
    answer_everything(&mut fake);
    test_support::settle(cx);
    assert_eq!(rail_width(cx), px(220.), "the premise: expanded");
    let row = test_support::bounds(cx, "host-rail-host-2");
    let past_icon = point(row.left() + px(100.), row.center().y);
    cx.simulate_click(past_icon, Modifiers::none());
    test_support::check(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "one command: {sent:?}");
    assert_eq!(
        sent[0].path,
        farhelm_proto::ui_channel::SESSION_QUERY_PATH,
        "a query, not a write of the expansion"
    );
    assert_eq!(
        sent[0].body.clone().unwrap_or_default(),
        json!({ "host": 2, "selected": "a" })
    );
    assert!(pressed(&frame(cx), "host-rail-host-2"));
    assert_eq!(rail_width(cx), px(220.), "choosing kept the rail expanded");

    let icon_box_right = row.left() + px(40.);
    for mark in ["host-rail-update-2", "host-rail-dot-2"] {
        let drawn = test_support::bounds(cx, mark);
        assert!(
            drawn.right() <= icon_box_right + px(1.),
            "{mark} {drawn:?} sits on the icon box, left of {icon_box_right:?}"
        );
    }
    assert_eq!(label(&frame(cx), HOSTS), "Hosts: 1 host has an update");
    assert!(cx.debug_bounds("hosts-panel-toggle-update").is_some());
}

// ---- The shown width -----------------------------------------------------------------------------------------------

/// The old script's `shownWidth`, as a pure rule: SPEC.md, "Expanding the rail adds 164px to the sidebar's shown width
/// and takes that space from the main pane ... In narrower windows the sidebar grows only as far as the window allows
/// while leaving the main pane its minimum width ... The stored resize width and its range keep their meaning". The two
/// pixels past the pane's 320-pixel floor keep the macOS header from floating because of expansion alone. Specified:
/// the extra is 164 (220 less 56); collapsed, the stored width whatever the window; expanded in a roomy window, the
/// stored width plus 164 (396 → 560, 296 → 460, 656 → 820); at exactly the room for that (an 882-pixel window and 396)
/// still 560, and a pixel narrower 559; at the 720-pixel minimum window, 398 for a stored 396, and the stored width
/// itself for 656, which already leaves the pane less than its floor; and never less than the stored width.
#[test]
fn the_shown_width_adds_164_pixels_within_the_main_panes_floor() {
    assert_eq!(EXTRA, 164);
    for viewport in [500., 720., 1400.] {
        assert_eq!(shown_width(396, false, px(viewport)), 396);
    }
    for (stored, shown) in [(396, 560), (296, 460), (656, 820)] {
        assert_eq!(shown_width(stored, true, px(2000.)), shown, "{stored}");
    }
    assert_eq!(shown_width(396, true, px(882.)), 560);
    assert_eq!(shown_width(396, true, px(881.)), 559);
    assert_eq!(shown_width(396, true, px(720.)), 398);
    assert_eq!(shown_width(656, true, px(720.)), 656);
    assert_eq!(shown_width(296, true, px(300.)), 296);
}

/// The 164-pixel rule laid out for real, as the old test measured it: in a roomy window the list beside the rail keeps
/// its width and the main pane gives up 164 pixels; at the minimum window the sidebar takes only what leaves the pane its
/// floor, and the session list gives up the rest; the widest stored sidebar keeps its width and the pane its floor. An
/// expanded name too long for its row is cut short rather than widening the rail. Only real text measures a name, so
/// this lays the main view out with the bundled fonts in a headless app. Specified, over a helm storing the rail
/// expanded: in a 1400 by 700 window over the default width, the sidebar is 560 wide, the rail 220, the list's column
/// from 220 to 559 (the collapsed list's 339 pixels) and the pane from 560 on, 840 wide; the long alias's name is cut,
/// ending in "…", inside its 148-pixel names box; in a 720 by 560 window the sidebar is 398, the list's column 177
/// wide and the pane 322, and each status segment's words ("All", "Running", "Waiting", "Idle") are painted whole inside
/// their segment, inside the column (the old expanded rail's segment rules, without which "Running" does not fit a
/// quarter of the column), and the heading's order title, count, Trash, Templates and New lie inside the column with
/// no two overlapping (the old heading wrapped its actions below the title there); and over a stored 656 the sidebar
/// is 656 and the pane 320 from 656.
#[test]
fn the_sidebar_widens_by_164_pixels_down_to_the_main_panes_floor() {
    for (window, width, sidebar, list, pane) in [
        (1400., None, 560., 339., 840.),
        (720., None, 398., 177., 322.),
        (720., Some(656), 656., 435., 320.),
    ] {
        let nodes = laid_out(px(window), fleet(Some(true), width));
        let column = at(&nodes, "sidebar");
        assert_eq!(column.size.width, px(sidebar), "{window} {width:?}");
        let rail = at(&nodes, "host-rail");
        assert_eq!((rail.left(), rail.size.width), (px(0.), px(220.)));
        let listed = at(&nodes, "session-list");
        assert_eq!(
            (listed.left(), listed.size.width),
            (px(220.), px(list)),
            "{window} {width:?}"
        );
        let main = at(&nodes, "main-pane");
        assert_eq!(
            (main.left(), main.size.width),
            (px(sidebar), px(pane)),
            "{window} {width:?}"
        );
        if window < 1000. && width.is_none() {
            // The narrowest list column beside the expanded rail: every status segment's words inside the segment and
            // every segment inside the column, which four equal cells could not do for "Running" (the old
            // `html.host-rail-open .session-status-filter` rules).
            for status in ["All", "Running", "Waiting", "Idle"] {
                let id = format!("session-status-{status}");
                let segment = at(&nodes, &id);
                let words = texts_within(&nodes, &id);
                let words = words.first().expect("the segment paints its words");
                assert_eq!(words.text.as_ref(), status, "{id} is not cut");
                assert!(
                    words.bounds.left() >= segment.left()
                        && words.bounds.right() <= segment.right(),
                    "{id}'s words {:?} fit its segment {segment:?}",
                    words.bounds
                );
                assert!(
                    segment.right() <= listed.right(),
                    "{id} {segment:?} fits the list's column {listed:?}"
                );
            }
            // The heading in that column: the order's title, the count and each action apart from one another and
            // inside the column (the old `.session-heading { flex-wrap: wrap }`).
            let parts: Vec<(&str, Bounds<Pixels>)> = [
                crate::session_list::order::TITLE_ID,
                crate::session_list::COUNT_ID,
                crate::trash::BUTTON_ID,
                "templates-button",
                crate::session_list::NEW_ID,
            ]
            .into_iter()
            .map(|id| (id, at(&nodes, id)))
            .collect();
            for (index, (id, part)) in parts.iter().enumerate() {
                assert!(
                    part.left() >= listed.left() && part.right() <= listed.right(),
                    "{id} {part:?} fits the list's column {listed:?}"
                );
                for (other, bounds) in &parts[index + 1..] {
                    assert!(
                        !part.intersects(bounds),
                        "{id} {part:?} and {other} {bounds:?} do not overlap"
                    );
                }
            }
        }
        if window > 1000. {
            let name = texts_within(&nodes, "host-rail-host-3");
            let cut = name.first().expect("the alias's name is painted");
            assert!(cut.text.ends_with('…'), "cut short: {:?}", cut.text);
            assert!(
                cut.bounds.size.width <= px(148.),
                "inside the names box: {:?}",
                cut.bounds
            );
        }
    }
}

/// The main view over `snapshot`, laid out with the real text system in a headless app `window` wide and 700 high (560
/// for the minimum window), and the driver's record of its second frame (the list asks for one more to give its rows
/// their height estimates).
fn laid_out(window: Pixels, snapshot: Vec<CollectionState>) -> Vec<DriverNode> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    fake.connect(snapshot);
    cx.run_until_parked();
    let height = if window < px(1000.) { 560. } else { 700. };
    let opened = cx
        .open_window(size(window, px(height)), |window, cx| {
            let main = cx.new(|cx| MainView::new(window, cx));
            cx.new(|cx| Root::new(main, window, cx))
        })
        .unwrap();
    cx.update_window(opened.into(), |_, window, cx| {
        window.set_driver_enabled(true);
        window.draw(cx).clear(cx);
        window.draw(cx).clear(cx);
        window.driver_nodes().to_vec()
    })
    .unwrap()
}

// ---- The divider while expanded ------------------------------------------------------------------------------------

/// The divider is drawn at the edge the user sees, and it still sets the stored width: SPEC.md, "The stored resize
/// width and its range keep their meaning, independent of rail expansion". A grip left at the stored width would sit
/// 164 pixels inside the session list, taking presses meant for the list's controls while the visible edge would not
/// resize. Specified: in a 1200 by 700 window with the rail stored expanded over the default width, the sidebar's column
/// is 560 wide, the divider's grip starts a pixel inside its edge (559), and the divider reports the stored 396 as its
/// value; a drag from the grip 20 pixels right draws the column 580 wide during the drag, and its release writes
/// exactly `{"sidebar_width": 416}`, the stored width plus the drag, with the column staying 580 while the write is
/// unanswered.
#[gpui::test]
fn the_divider_sits_at_the_shown_edge_and_drags_the_stored_width(cx: &mut TestAppContext) {
    let (mut fake, cx) = open(cx, Some(true));
    let nodes = frame(cx);
    assert_eq!(
        at(&nodes, "sidebar").size.width,
        px(560.),
        "the premise: expanded"
    );
    let grip = at(&nodes, "sidebar-divider-grip");
    assert_eq!(grip.left(), px(559.));
    assert_eq!(
        node(&nodes, "sidebar-divider")
            .and_then(|divider| divider.value.clone())
            .as_deref(),
        Some("396")
    );

    let start = grip.center();
    let to = point(start.x + px(20.), start.y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    test_support::check(cx);
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    test_support::check(cx);
    assert_eq!(
        at(&frame(cx), "sidebar").size.width,
        px(580.),
        "during the drag"
    );
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    test_support::settle(cx);
    let sent = fake.commands();
    assert_eq!(sent.len(), 1, "one write: {sent:?}");
    assert_eq!(sent[0].path, "/api/preferences");
    assert_eq!(
        sent[0].body.clone().unwrap_or_default(),
        json!({ "sidebar_width": 416 })
    );
    assert_eq!(
        at(&frame(cx), "sidebar").size.width,
        px(580.),
        "held until the helm answers"
    );
}

//! Tests of what a session row says beyond its title and status mark: its age and timestamp, its last line (host and
//! directory), its ended wording, the cause a stale row's status is only last-known, the inks of the selected and stale
//! rows, that all of it stays inside the row at the narrowest sidebar, and which technical values are drawn in the
//! monospace face: the row's, the details bar's beside it, and the program in the row menu's summary.
//!
//! Like the list's other tests, these run the real main view over the helm client's fake and read the frame from GPUI's
//! driver record, finding a row's parts by their ids (`session-item-<key>` for the row, `session-row-<key>-age` and
//! `session-row-<key>-cause` for its marks). Ages are counted from GPUI's test clock, which starts at [`TEST_NOW`] and
//! moves only when a test advances it, so every age a test reads is exact. Rules that are pure functions (the causes,
//! the inks) are tested as such, and the tests whose outcome is layout or a font face use the real text system.

use std::sync::Arc;
use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{
    Availability, CollectionState, HostView, Preferences, SIDEBAR_WIDTH_RANGE,
};
use gpui::{
    App, AppContext as _, DriverNode, DriverText, Entity, HeadlessAppContext, Modifiers, Pixels,
    Point, Role, TestAppContext, TextStyle, VisualTestContext, Window, WindowTextSystem, point, px,
    size,
};
use gpui_component::Root;
use serde_json::{Value, json};

use super::SessionList;
use super::host_rail::tests::{hosts, local, remote};
use super::row::{
    AGE_ID_SUFFIX, AGE_TRACK_ID_SUFFIX, CAUSE_ID_SUFFIX, ITEM_ID_PREFIX, Inks, ROW_ID_PREFIX,
    meta_parts,
};
use super::status::{self, HostLookup};
use super::tests::{
    TEST_NOW, collections, listing, open_main, painted, preferences_state, session,
};
use crate::main_view::MainView;
use crate::session_pane::details::{AGE_ID as DETAILS_AGE_ID, COMMAND_ID as DETAILS_COMMAND_ID};
use crate::test_support::{self, frame};
use crate::{assets, bootstrap, layers, local_time, render_count, theme, ticker};

// ---- Helpers --------------------------------------------------------------------------------------------------------

/// The node drawn under `id`, if any.
pub(super) fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The lines of text painted under `id`, at any depth, in paint order: where a node's text is, since text is painted
/// by the text elements inside it.
pub(super) fn texts_within(nodes: &[DriverNode], id: &str) -> Vec<DriverText> {
    let Some(top) = nodes
        .iter()
        .position(|node| node.element_id.as_deref() == Some(id))
    else {
        panic!("{id} was not drawn");
    };
    let inside = |mut index: usize| loop {
        if index == top {
            return true;
        }
        match nodes[index].parent {
            Some(parent) => index = parent,
            None => return false,
        }
    };
    (0..nodes.len())
        .filter(|&index| inside(index))
        .flat_map(|index| nodes[index].texts.iter().cloned())
        .collect()
}

/// The strings painted under `key`'s row, top to bottom and left to right within a line.
fn row_texts(nodes: &[DriverNode], key: &str) -> Vec<String> {
    texts_within(nodes, &format!("{ITEM_ID_PREFIX}{key}"))
        .into_iter()
        .map(|text| text.text.to_string())
        .collect()
}

/// `key`'s age mark's id.
fn age_id(key: &str) -> String {
    format!("{ROW_ID_PREFIX}{key}{AGE_ID_SUFFIX}")
}

/// `key`'s stale cause mark's id.
fn cause_id(key: &str) -> String {
    format!("{ROW_ID_PREFIX}{key}{CAUSE_ID_SUFFIX}")
}

/// What `key`'s age mark painted, or `None` when the row drew no age.
fn age_text(nodes: &[DriverNode], key: &str) -> Option<String> {
    node(nodes, &age_id(key))?;
    let texts = texts_within(nodes, &age_id(key));
    Some(texts.iter().map(|text| text.text.to_string()).collect())
}

/// The hover help showing once the pointer has moved from a corner of `row` (inside its padding, clear of every mark)
/// onto the middle of the mark drawn under `selector` and rested for the hover delay: the way a pointer reaches a mark
/// inside a row, so the mark's own hover is the latest.
pub(super) fn help_after_crossing(
    cx: &mut VisualTestContext,
    row: &str,
    selector: &str,
) -> Option<String> {
    let nodes = frame(cx);
    let Some(mark) = node(&nodes, selector) else {
        panic!("{selector} was not drawn");
    };
    let at = mark.bounds.center();
    help_at(cx, row, at)
}

/// The hover help showing once the pointer has moved from a corner of `row` onto `at` and rested for the hover delay;
/// for a point that is not one node's centre, such as a picture inside a mark.
pub(super) fn help_at(cx: &mut VisualTestContext, row: &str, at: Point<Pixels>) -> Option<String> {
    let nodes = frame(cx);
    let Some(row) = node(&nodes, row) else {
        panic!("{row} was not drawn");
    };
    let corner = row.bounds.origin + point(px(2.), px(2.));
    for at in [corner, at] {
        cx.simulate_mouse_move(at, None, Modifiers::none());
        test_support::settle(cx);
    }
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// The hosts collection over the local host and remotes with these registry ids, each remote's connection state
/// replaced by `state` (the helm's wire form of `HostStateView`).
fn hosts_in(states: &[(i64, Value)]) -> CollectionState {
    let mut fleet = vec![local()];
    fleet.extend(
        states
            .iter()
            .map(|(id, _)| remote(*id, &format!("ops@host{id}.example.invalid"))),
    );
    let mut collection = hosts(&fleet);
    for (entry, (_, state)) in collection.entries.iter_mut().skip(1).zip(states) {
        entry.value["state"] = state.clone();
    }
    collection
}

/// A host's connection state as the helm's pinned host list spells it: the state of its `index`th host.
fn pinned_state(index: usize) -> Value {
    let list: Value = serde_json::from_str(include_str!(
        "../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    list["hosts"][index]["state"].clone()
}

/// A registered host in `state`, decoded as the GUI holds it.
fn host_in(state: Value) -> HostView {
    let collection = hosts_in(&[(2, state)]);
    serde_json::from_value(collection.entries[1].value.clone()).unwrap()
}

// ---- Ages ------------------------------------------------------------------------------------------------------------

/// Every age a test or golden shows is stamped relative to [`TEST_NOW`], so it holds only while GPUI's test clock starts
/// there; a GPUI upgrade that moved the start would otherwise turn every age assertion into a confusing mismatch.
/// Specified: in a fresh test app, the wall clock the ticker reads is `TEST_NOW` in whole seconds.
#[gpui::test]
fn the_test_clock_starts_where_the_fixtures_expect(cx: &mut TestAppContext) {
    test_support::install(cx);
    let now = cx.update(|cx| super::age::seconds(ticker::wall_now(cx)));
    assert_eq!(now, Some(TEST_NOW));
}

/// SPEC.md, "Status": a row shows how long ago the session was last active as a short age "with the full timestamp
/// available on the row", falls back to the creation time when the helm sends no activity stamp, and shows "no age at
/// all" when neither exists. Specified: with the app's zone pinned two hours east of UTC, a session last active two
/// minutes ago (23:57:58 UTC) paints `2m`, and is named and helped by its local time, `Last activity 2025-07-02
/// 01:57:58`, which names no zone; one with no activity stamp created three hours ago paints `3h`; one with neither
/// stamp draws no age mark.
#[gpui::test]
fn a_rows_age_shows_its_bucket_and_its_timestamp_as_hover_help(cx: &mut TestAppContext) {
    let rows = [
        session("a").activity(TEST_NOW - 120),
        session("b").activity(0).created(TEST_NOW - 3 * 3_600),
        session("c").activity(0).created(0).seen(None),
    ];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b", "c"], &rows)]);
    cx.update(|_, cx| local_time::pin(2 * 3_600, cx));
    test_support::redraw(cx);
    let nodes = frame(cx);
    assert_eq!(age_text(&nodes, "a").as_deref(), Some("2m"));
    let stamp = "Last activity 2025-07-02 01:57:58";
    let mark = node(&nodes, &age_id("a")).unwrap();
    assert_eq!(mark.label.as_deref(), Some(stamp));
    assert_eq!(mark.role, Some(Role::Label));
    assert_eq!(age_text(&nodes, "b").as_deref(), Some("3h"));
    assert_eq!(age_text(&nodes, "c"), None, "no stamp, no age");
    assert_eq!(
        help_after_crossing(cx, "session-row-a", &age_id("a")).as_deref(),
        Some(stamp)
    );
}

/// An age is only right if it moves on by itself, and SPEC_impl.md, "Animation time: the ticker", gives that job to the
/// ticker's one 30-second clock, in an inactive window too, since a window behind others is still on screen. Without
/// it the list would show a stale age until something else redrew it. Specified: in an inactive window, with an idle
/// row (no pulse) 59 minutes 40 seconds old, 29 seconds redraw the list not once; the 30th second redraws it, and it
/// then paints `1h`.
#[gpui::test]
fn an_age_moves_on_when_the_clock_ticks_even_in_an_inactive_window(cx: &mut TestAppContext) {
    let row = session("a")
        .status(json!({"state": "idle"}))
        .activity(TEST_NOW - (59 * 60 + 40))
        .seen(Some(TEST_NOW - (59 * 60 + 40)));
    let (_fake, cx) = open_main(cx, vec![listing(&["a"], &[row])]);
    assert_eq!(age_text(&frame(cx), "a").as_deref(), Some("59m"), "premise");
    cx.deactivate_window();
    test_support::settle(cx);
    assert!(
        !cx.update(|window, _| window.is_window_active()),
        "premise: the window is inactive"
    );

    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(29));
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-list", cx)),
        0,
        "premise: nothing else redraws the list"
    );
    test_support::advance(cx, Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| render_count::count("session-list", cx)) > 0,
        "the clock redrew the list"
    );
    assert_eq!(age_text(&frame(cx), "a").as_deref(), Some("1h"));
}

// ---- The last line -----------------------------------------------------------------------------------------------

/// SPEC.md, "Session list": the last line is the host's name, `:`, then the working directory, and "without either a
/// registry match or a supplied host name it shows only the directory, rather than inventing a local identity or a
/// dangling separator". A row with no directory is the mirror case, and one with neither has nothing to say. With no
/// host list read, the host's name is the one the helm sent. Specified: a row with both paints `buildbox`, `:` and its
/// directory with the home folded (`~/src/a`); a row with no directory paints `buildbox` and no `:`; a row whose helm
/// sent no host name paints its directory (`/srv/ci`) and no `:`; a row with neither paints only its title and age.
#[gpui::test]
fn the_last_line_names_the_host_and_directory_without_a_dangling_separator(
    cx: &mut TestAppContext,
) {
    let stamped = |key| session(key).activity(TEST_NOW - 60);
    let rows = [
        stamped("both"),
        stamped("host").cwd(""),
        stamped("dir").host_name("").cwd("/srv/ci"),
        stamped("none").host_name("").cwd(""),
    ];
    let order = ["both", "host", "dir", "none"];
    let (_fake, cx) = open_main(cx, vec![listing(&order, &rows)]);
    let nodes = frame(cx);
    assert_eq!(
        row_texts(&nodes, "both"),
        ["both", "1m", "buildbox", ":", "~/src/both"]
    );
    assert_eq!(row_texts(&nodes, "host"), ["host", "1m", "buildbox"]);
    assert_eq!(row_texts(&nodes, "dir"), ["dir", "1m", "/srv/ci"]);
    assert_eq!(row_texts(&nodes, "none"), ["none", "1m"]);
}

/// The last line's parts are the one place a row's host name and directory are chosen, so they are pinned apart from
/// the drawing too. Specified: with no registry row, the helm's name and the folded home; an empty directory and an
/// empty name are each `None`, never an empty part.
#[test]
fn the_last_lines_parts_are_none_when_empty() {
    let row = session("a").row();
    assert_eq!(
        meta_parts(&row, None),
        (Some("buildbox".to_owned()), Some("~/src/a".to_owned()))
    );
    let bare = session("a").host_name("").cwd("").row();
    assert_eq!(meta_parts(&bare, None), (None, None));
}

// ---- Ended rows ------------------------------------------------------------------------------------------------------

/// SPEC.md, "Status": an exit the supervisor did not witness shows its code only when the terminal kept it, "and an
/// explicit unknown otherwise — never a guess", and the noncompact row keeps the complete ended wording visible. The old
/// app said so on the sidebar alone, keeping plain `Exited` elsewhere. Specified: an exit with no code paints
/// `Exited (code unknown)`; with a stop annotation its sentence is named `Exited (code unknown) — stopped by user`; an
/// exit with code 17 paints `Exited (code 17)`; and `status::badge`, which the quick switcher and the menu header draw
/// from, still says plain `Exited` for the exit with no code (which surfaces call it is not checked here).
#[gpui::test]
fn an_exit_without_a_code_says_the_code_is_unknown_on_the_row(cx: &mut TestAppContext) {
    let exited = |code: Value| json!({"state": "exited", "exit_code": code});
    let rows = [
        session("a").status(exited(Value::Null)),
        session("b")
            .status(exited(Value::Null))
            .annotation("stopped by user"),
        session("c").status(exited(json!(17))),
    ];
    let (_fake, cx) = open_main(cx, vec![listing(&["a", "b", "c"], &rows)]);
    let nodes = frame(cx);
    assert!(row_texts(&nodes, "a").contains(&"Exited (code unknown)".to_owned()));
    assert_eq!(
        node(&nodes, "session-row-b-status").and_then(|node| node.label.as_deref()),
        Some("Exited (code unknown) — stopped by user")
    );
    assert!(row_texts(&nodes, "c").contains(&"Exited (code 17)".to_owned()));

    let elsewhere = status::badge(&rows[0].row(), "elsewhere".into());
    match elsewhere {
        Some(status::Badge::Ended { line, .. }) => assert_eq!(line.accessible_text(), "Exited"),
        _ => panic!("an exit is an ended badge"),
    }
}

// ---- Stale rows -------------------------------------------------------------------------------------------------------

/// The cause is the one thing a stale row says about why its status is only last-known, and the old table worded each
/// host state so the user knows whether to wait, update, or fix the host. Retry counts and error text must not reach
/// it, or the headline would flicker with every attempt. Specified, for each host state (the helm's pinned host list
/// where it has one): connecting and unreachable read `Waiting for supervisor`; connected `Refreshing session`; an older
/// host refused at the handshake `Host needs update` and a newer one `Host is too new`; identity mismatch and unverified,
/// duplicate, retired, suspended, stopped, suspending, resuming and deleting each their own words; no host list yet
/// `Checking host`; a failed host list `Host state unavailable`; a host the list lacks `Host removed`. The hover help
/// adds that this is the session's last-known state.
#[test]
fn each_host_state_names_its_cause() {
    let known = |state: Value| status::cause(HostLookup::Known(&host_in(state)));
    assert_eq!(known(pinned_state(1)), "Waiting for supervisor");
    assert_eq!(known(pinned_state(2)), "Waiting for supervisor");
    assert_eq!(known(pinned_state(0)), "Refreshing session");
    assert_eq!(known(pinned_state(3)), "Host needs update");
    let mut newer = pinned_state(3);
    newer["peer_protocol"] = json!(30);
    assert_eq!(known(newer), "Host is too new");
    assert_eq!(known(pinned_state(4)), "Host identity mismatch");
    assert_eq!(known(pinned_state(5)), "Host identity unverified");
    assert_eq!(known(pinned_state(6)), "Duplicate host");
    assert_eq!(known(pinned_state(7)), "Host connection stopped");
    assert_eq!(known(pinned_state(11)), "Host suspended");
    assert_eq!(
        known(json!({"phase": "stopped", "start_failed": null, "waiting_for_place": false})),
        "Environment stopped"
    );
    for (direction, words) in [
        ("suspending", "Host suspending"),
        ("resuming", "Host resuming"),
    ] {
        assert_eq!(
            known(json!({
                "phase": "tensorlake-changing",
                "starting": false,
                "direction": direction,
                "requested_at": 1_700_000_000_000_i64,
            })),
            words
        );
    }
    assert_eq!(
        known(json!({"phase": "deleting", "failure": null})),
        "Host being deleted"
    );

    let none = collections(vec![listing(&[], &[])]);
    assert_eq!(status::cause(HostLookup::of(&none, 2)), "Checking host");
    let mut failed = hosts_in(&[(2, pinned_state(1))]);
    failed.meta.availability = Availability::Unavailable {
        error: "down".into(),
    };
    let failed = collections(vec![failed]);
    assert_eq!(
        status::cause(HostLookup::of(&failed, 2)),
        "Host state unavailable"
    );
    let read = collections(vec![hosts_in(&[(2, pinned_state(1))])]);
    assert_eq!(
        status::cause(HostLookup::of(&read, 2)),
        "Waiting for supervisor"
    );
    assert_eq!(status::cause(HostLookup::of(&read, 9)), "Host removed");
    assert_eq!(
        status::cause_help("Host removed"),
        "Host removed: this is the session's last-known state"
    );
}

/// SPEC.md, "Session list": sessions on an unreachable host stay in the list "with a neutral cause mark such as
/// `Waiting for supervisor` or `Host needs update`, rather than vanishing". The cause belongs to the stale row alone: a
/// row on the same host that the helm does not mark stale names none. Specified: a stale running row on an unreachable
/// host draws a status mark `Waiting for supervisor` on its line under the title, with hover help `Waiting for
/// supervisor: this is the session's last-known state`; a stale exited row on a host the registry lacks paints its ended
/// words and `Host removed` on that same line; a row on the unreachable host that is not stale draws no cause and
/// no line between its title and its last line.
#[gpui::test]
fn a_stale_row_names_its_cause_with_hover_help(cx: &mut TestAppContext) {
    let rows = [
        session("a").host(3).stale().activity(TEST_NOW - 60),
        session("b")
            .host(9)
            .stale()
            .status(json!({"state": "exited", "exit_code": 0}))
            .activity(TEST_NOW - 60),
        session("c").host(3).activity(TEST_NOW - 60),
    ];
    let (_fake, cx) = open_main(
        cx,
        vec![
            listing(&["a", "b", "c"], &rows),
            hosts_in(&[(3, pinned_state(2))]),
        ],
    );
    let nodes = frame(cx);
    let cause = node(&nodes, &cause_id("a")).expect("the stale row draws its cause");
    assert_eq!(cause.role, Some(Role::Status));
    assert_eq!(cause.label.as_deref(), Some("Waiting for supervisor"));
    assert_eq!(
        row_texts(&nodes, "a"),
        ["a", "1m", "Waiting for supervisor", "host3", ":", "~/src/a"]
    );
    assert_eq!(
        row_texts(&nodes, "b"),
        [
            "b",
            "1m",
            "Exited (code 0)",
            "Host removed",
            "buildbox",
            ":",
            "~/src/b"
        ]
    );
    assert!(
        node(&nodes, &cause_id("c")).is_none(),
        "a live row names no cause"
    );
    assert_eq!(row_texts(&nodes, "c"), ["c", "1m", "host3", ":", "~/src/c"]);
    assert_eq!(
        help_after_crossing(cx, "session-row-a", &cause_id("a")).as_deref(),
        Some("Waiting for supervisor: this is the session's last-known state")
    );
}

/// The selected row is a solid accent fill, on which the ordinary foregrounds lose their contrast, and a stale row's
/// title is quieter because its facts are last-known (the old `.session-row.stale` and `.session-row.selected`
/// rules). Every part this piece adds (the age, the cause) takes the row's secondary ink. Specified, in both palettes: an
/// ordinary row's title is `--fg-0` and its secondary text `--fg-1`; a stale row's title is `--fg-1`; the selected row's
/// title is `--on-fill` and its secondary text `--on-accent-dim`; the selected stale row's title is `--on-accent-dim`.
/// This pins the ink table only: GPUI's driver record carries no colors, so that the row draws with these inks is
/// pinned by the `session-list` goldens (a stale row) and the `session-list-selected-stale` goldens (a selected one).
#[test]
fn the_selected_and_stale_rows_use_their_own_inks() {
    for palette in [&theme::DARK, &theme::LIGHT] {
        let inks = |selected, stale| Inks::of(palette, selected, stale);
        assert_eq!(inks(false, false).title, palette.fg_0);
        assert_eq!(inks(false, false).secondary, palette.fg_1);
        assert_eq!(inks(false, true).title, palette.fg_1);
        assert_eq!(inks(false, true).secondary, palette.fg_1);
        assert_eq!(inks(true, false).title, palette.on_fill);
        assert_eq!(inks(true, false).secondary, palette.on_accent_dim);
        assert_eq!(inks(true, true).title, palette.on_accent_dim);
        assert_eq!(inks(true, true).secondary, palette.on_accent_dim);
    }
}

// ---- Layout, with the real text system -------------------------------------------------------------------------------

/// The main view laid out with the real text system and the bundled fonts in a headless app (no renderer, which layout
/// does not need), over a helm holding `rows` in that order and the sidebar at its narrowest width: the driver record
/// of its second frame (the list asks for one to give its rows their height estimates).
fn narrowest_layout(rows: &[super::tests::Fixture]) -> Vec<DriverNode> {
    narrowest_layout_choosing(rows, Vec::new(), None)
}

/// [`narrowest_layout`] with `extra` collections in the helm's snapshot (a host registry, say) and, when `host` names
/// one, that host chosen on the rail before the first frame, through the same call an avatar's click makes.
fn narrowest_layout_choosing(
    rows: &[super::tests::Fixture],
    extra: Vec<CollectionState>,
    host: Option<i64>,
) -> Vec<DriverNode> {
    narrowest_layout_then(rows, extra, host, None)
}

/// A step a real-layout test takes on the session list once the first frames are drawn, inside a window update.
type Step<'a> = &'a dyn Fn(&Entity<SessionList>, &mut Window, &mut App);

/// [`narrowest_layout_choosing`], with `then` taken once its two frames are drawn and the record read from the frame
/// after it, when the test names one. A step that needs something only a drawn frame records, such as opening a row's
/// menu ([`SessionList::toggle_menu`], which needs the button's anchor), goes here; so does an empty step, for a test
/// whose frame must come after the app has run what the first frames asked for (the pane's details bar for the session
/// the list chose).
fn narrowest_layout_then(
    rows: &[super::tests::Fixture],
    extra: Vec<CollectionState>,
    host: Option<i64>,
    then: Option<Step<'_>>,
) -> Vec<DriverNode> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    let preferences = preferences_state(Preferences {
        sidebar_width: Some(*SIDEBAR_WIDTH_RANGE.start()),
        ..Preferences::default()
    });
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let mut snapshot = vec![listing(&order, rows), preferences];
    snapshot.extend(extra);
    fake.connect(snapshot);
    cx.run_until_parked();
    let opened = std::rc::Rc::new(std::cell::RefCell::new(None));
    let keep = opened.clone();
    let window = cx
        .open_window(size(px(1000.), px(600.)), move |window, cx| {
            let main = cx.new(|cx| MainView::new(window, cx));
            *keep.borrow_mut() = Some(main.clone());
            cx.new(|cx| Root::new(main, window, cx))
        })
        .unwrap();
    if let Some(host) = host {
        let main = opened.borrow().clone().expect("the main view was made");
        cx.update(|cx| {
            let list = main.read(cx).session_list().clone();
            list.update(cx, |list, cx| list.choose_host(Some(host), cx));
        });
        cx.run_until_parked();
    }
    let draw = |cx: &mut HeadlessAppContext| {
        cx.update_window(window.into(), |_, window, cx| {
            window.set_driver_enabled(true);
            window.draw(cx).clear(cx);
            window.draw(cx).clear(cx);
            window.driver_nodes().to_vec()
        })
        .unwrap()
    };
    let nodes = draw(&mut cx);
    let Some(then) = then else {
        return nodes;
    };
    let list = cx.update(|cx| {
        let main = opened.borrow().clone().expect("the main view was made");
        main.read(cx).session_list().clone()
    });
    cx.update_window(window.into(), |_, window, cx| then(&list, window, cx))
        .unwrap();
    cx.run_until_parked();
    draw(&mut cx)
}

/// `text`'s width at `size` pixels in the interface face and in the monospace face, in that order, shaped by the real
/// text system with the bundled fonts. A font-role test's premise is that the two differ for the text it measures,
/// since a width alone cannot tell the faces apart when they happen to draw a string alike.
fn face_widths(text: &str, size: f32) -> (f32, f32) {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx = HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
    cx.update(bootstrap::install).unwrap();
    cx.update(|cx| {
        let shaper = WindowTextSystem::new(cx.text_system().clone());
        let width = |family: &'static str| {
            let run = TextStyle {
                font_family: family.into(),
                ..TextStyle::default()
            }
            .to_run(text.len());
            let line = shaper.shape_line(text.to_owned().into(), px(size), &[run], None);
            f32::from(line.width)
        };
        (width(assets::UI_FONT), width(assets::TERMINAL_FONT))
    })
}

/// The bounds of the node drawn under `id`; fails if nothing was.
fn bounds_of(nodes: &[DriverNode], id: &str) -> gpui::Bounds<Pixels> {
    match node(nodes, id) {
        Some(node) => node.bounds,
        None => panic!("{id} was not drawn"),
    }
}

/// The sidebar's narrowest width leaves the list 239 pixels (240 less the sidebar's edge), and a row's parts must
/// still fit inside it: a long title cut short rather than pushing the age out, a long directory cut from the left,
/// and an ended message that wraps rather than running past the edge (the old `narrow rows align fixed facts` and `a
/// row with unbroken oversized fields stays contained` cases). The age column keeps a floor of four characters (the
/// old `minmax(4ch, max-content)`), so a short age (`2m`) and a long one (`400d`) leave their titles the same room:
/// without the floor, titles beside short ages would run further right than titles beside long ones. Only the real
/// text system measures text. Specified, at the sidebar's narrowest width, over two rows with long titles aged `400d`
/// and `2m`: every line painted in either row lies inside its row's item, horizontally; each title ends before its age
/// track begins; the two age tracks start at the same x and end at the same x, as do the two ages; the long error's
/// wording paints more than one line; and the long directory's line ends with the directory's last segment.
#[test]
fn a_long_title_directory_and_error_stay_inside_the_row_at_the_narrowest_sidebar() {
    let long_title =
        "A session title long enough to run well past the edge of the narrowest sidebar";
    let long = session("long")
        .title(long_title)
        .cwd("/srv/builds/a/very/deep/tree/of/folders/that/keeps/going/to/the/last-segment")
        .status(json!({
            "state": "error",
            "detail": "the agent could not be started because the command named in its launch was not found on the host",
        }))
        .stale()
        .activity(TEST_NOW - 400 * 86_400);
    let short = session("short").title(long_title).activity(TEST_NOW - 120);
    let nodes = narrowest_layout(&[long, short]);
    let list = bounds_of(&nodes, "session-list");
    // The narrowest sidebar less its 56-pixel rail and its 1-pixel edge.
    assert_eq!(list.size.width, px(239.), "premise: the narrowest list");
    let edge = |x: Pixels| f32::from(x);
    let near = |a: Pixels, b: Pixels| (edge(a) - edge(b)).abs() < 0.5;

    for key in ["long", "short"] {
        let item = bounds_of(&nodes, &format!("{ITEM_ID_PREFIX}{key}"));
        for text in texts_within(&nodes, &format!("{ITEM_ID_PREFIX}{key}")) {
            assert!(
                edge(text.visible_bounds.left()) >= edge(item.left()) - 0.5
                    && edge(text.visible_bounds.right()) <= edge(item.right()) + 0.5,
                "{key}: {:?} at {:?} lies inside the row {item:?}",
                text.text,
                text.visible_bounds
            );
        }
        let track = bounds_of(
            &nodes,
            &format!("{ROW_ID_PREFIX}{key}{AGE_TRACK_ID_SUFFIX}"),
        );
        let title = texts_within(&nodes, &format!("{ROW_ID_PREFIX}{key}"))
            .into_iter()
            .find(|text| text.text.starts_with("A session"))
            .expect("the long title painted");
        assert!(
            title.visible_bounds.right() <= track.left(),
            "{key}: the title {:?} ends before the age track {track:?}",
            title.visible_bounds
        );
    }
    let track = |key: &str| {
        bounds_of(
            &nodes,
            &format!("{ROW_ID_PREFIX}{key}{AGE_TRACK_ID_SUFFIX}"),
        )
    };
    assert!(
        near(track("long").left(), track("short").left())
            && near(track("long").right(), track("short").right()),
        "the age tracks share one column whatever the age: {:?} and {:?}",
        track("long"),
        track("short")
    );
    let age = |key: &str| bounds_of(&nodes, &age_id(key));
    assert!(
        near(age("long").right(), age("short").right()),
        "the ages share one right edge: {:?} and {:?}",
        age("long"),
        age("short")
    );
    // A sentence paints its wrapped lines as one text, so the error wraps when what it paints is at least two of the
    // ended line's 16-pixel lines tall.
    let error_lines = texts_within(&nodes, "session-row-long-status");
    assert!(
        error_lines
            .iter()
            .any(|text| text.bounds.size.height >= px(32.)),
        "the error wraps: {:?}",
        error_lines
            .iter()
            .map(|text| &text.text)
            .collect::<Vec<_>>()
    );
    let texts = painted(&nodes);
    assert!(
        texts.iter().any(|text| text.ends_with("last-segment")),
        "the directory keeps its end: {texts:?}"
    );
}

/// The chosen host's chip shares the heading's filter row with the status segments, and a long alias must be cut inside
/// the chip rather than push the chip, or its ✕, out of the list (the old `filters.spec.ts`, "the host chip truncates a
/// long alias at the minimum sidebar width"). Only the real text system measures the name. Specified, at the narrowest
/// width with a remote host aliased far longer than the list is wide and chosen on the rail: the status segments and the
/// chip are each drawn horizontally inside the list's column; the chip paints a name cut short with "…" and its ✕, both
/// inside the chip.
#[test]
fn a_chosen_hosts_long_alias_is_cut_inside_its_chip_at_the_narrowest_sidebar() {
    let long =
        "Build farm for the release verification sessions with a deliberately long host alias";
    let build = remote(3, "ops@build.example").alias(long);
    let nodes = narrowest_layout_choosing(&[session("a")], vec![hosts(&[local(), build])], Some(3));
    let list = bounds_of(&nodes, "session-list");
    for id in ["session-status", "host-filter-chip"] {
        let part = bounds_of(&nodes, id);
        assert!(
            part.left() >= list.left() && part.right() <= list.right(),
            "{id} {part:?} lies inside the list {list:?}"
        );
    }
    let chip = bounds_of(&nodes, "host-filter-chip");
    let painted = texts_within(&nodes, "host-filter-chip");
    let name = painted
        .iter()
        .find(|text| text.text.starts_with("Build farm"))
        .expect("the chip painted the alias");
    assert!(name.text.ends_with('…'), "cut short: {:?}", name.text);
    for text in painted
        .iter()
        .filter(|text| text.text.starts_with("Build") || text.text == "✕")
    {
        assert!(
            text.bounds.right() <= chip.right() + px(0.5),
            "{:?} at {:?} lies inside the chip {chip:?}",
            text.text,
            text.bounds
        );
    }
    assert!(
        painted.iter().any(|text| text.text == "✕"),
        "the chip kept its ✕"
    );
}

/// At the sidebar's narrowest width the list's heading must still show the order's title and its three icon buttons,
/// Trash, Templates and New, each whole: the old `sidebar-resize.spec.ts` checked the heading's actions inside the
/// sidebar at 296, 396 and 656 pixels, and the narrowest is the width that can run out of room (the heading wraps its
/// actions below the title rather than covering it). Only the real text system measures the title. Specified, at the
/// narrowest width over one row: the title, Trash, Templates and New are each drawn horizontally inside the list's
/// column, and no two of them overlap.
#[test]
fn the_headings_title_and_actions_stay_whole_at_the_narrowest_sidebar() {
    let nodes = narrowest_layout(&[session("a")]);
    let list = bounds_of(&nodes, "session-list");
    let parts = [
        "session-order",
        crate::trash::BUTTON_ID,
        "templates-button",
        super::NEW_ID,
    ]
    .map(|id| (id, bounds_of(&nodes, id)));
    for (id, part) in parts {
        assert!(
            part.left() >= list.left() && part.right() <= list.right(),
            "{id} {part:?} lies inside the list {list:?}"
        );
    }
    for (index, (one, first)) in parts.iter().enumerate() {
        for (other, second) in &parts[index + 1..] {
            assert!(
                !first.intersects(second),
                "{one} {first:?} and {other} {second:?} overlap"
            );
        }
    }
}

/// The old stylesheet drew a row's working directory in the monospace face (`.session-cwd` in its technical-value
/// typography), so paths read as technical values and their characters line up; the host's name kept the interface
/// face. The test text system gives every face the same widths, so only the real one can tell the faces apart.
/// Specified, with the real text system: a directory of nine narrow characters (`/iiiiiiii`) and one of nine wide ones
/// (`/MMMMMMMM`), on rows with no host name, each paint nine advances of the monospace face at 12 pixels; and the rows'
/// titles, six `i`s and six `M`s in the interface face (short enough to stay whole at this width), differ in width
/// (the premise: that face is proportional, so the comparison can tell the faces apart).
#[test]
fn the_directory_is_drawn_in_the_monospace_face() {
    let rows = [
        session("narrow")
            .title("iiiiii")
            .activity(TEST_NOW - 60)
            .host_name("")
            .cwd("/iiiiiiii"),
        session("wide")
            .title("MMMMMM")
            .activity(TEST_NOW - 60)
            .host_name("")
            .cwd("/MMMMMMMM"),
    ];
    let nodes = narrowest_layout(&rows);
    let width = |key: &str, text: &str| -> f32 {
        let line = texts_within(&nodes, &format!("{ITEM_ID_PREFIX}{key}"))
            .into_iter()
            .find(|line| line.text.as_ref() == text)
            .unwrap_or_else(|| {
                let painted: Vec<_> = texts_within(&nodes, &format!("{ITEM_ID_PREFIX}{key}"))
                    .into_iter()
                    .map(|line| line.text)
                    .collect();
                panic!("{key} paints {text}; it painted {painted:?}")
            });
        f32::from(line.bounds.size.width)
    };
    assert!(
        width("wide", "MMMMMM") - width("narrow", "iiiiii") > 10.,
        "premise: the title, in the interface face, is proportional"
    );
    // JetBrains Mono advances every character 0.6 em.
    let nine = 9. * 0.6 * 12.;
    for (key, text) in [("narrow", "/iiiiiiii"), ("wide", "/MMMMMMMM")] {
        let drawn = width(key, text);
        assert!(
            (drawn - nine).abs() < 0.5,
            "{key}'s directory is {drawn} wide, not nine monospace advances ({nine})"
        );
    }
}

/// The old `interface-fonts.spec.ts` case "interface and technical values load their own bundled font roles", for the
/// values the sidebar and the details bar show beside names: the old stylesheet drew the row's age (`.status-time`) and
/// the details bar's command line in the monospace face, as technical values. The test text system gives every face
/// the same widths, so only the real one can tell the faces apart. The old case's pin glyph is not measured: no bundled
/// face but the monospace one has it, so the text system draws it from that face whichever face asks, and no width can
/// say which did. Specified, with the real text system, over one running session (open, so the details bar shows it)
/// last active two minutes ago and launched with `iiiiiiii`: the row's age `2m` and the bar's age `2m`, status marks at
/// 12 pixels, and the bar's command `iiiiiiii`, a copy field at 13.333 pixels, each paint one line as wide as its text
/// in the monospace face at that size, where the interface face would draw it at least a pixel wider or narrower (the
/// premise).
#[test]
fn the_ages_and_the_command_line_are_drawn_in_the_monospace_face() {
    let rows = [session("a").activity(TEST_NOW - 120).invocation("iiiiiiii")];
    // An empty step, so the frame read comes after the app ran what the first frames asked for (the details bar of the
    // session the list opened among it).
    let settle: Step<'_> = &|_: &Entity<SessionList>, _: &mut Window, _: &mut App| {};
    let nodes = narrowest_layout_then(&rows, Vec::new(), None, Some(settle));
    // The copy field's text size is its own (`components/copy_field.rs`); a status mark's is the theme's 12 pixels.
    let parts = [
        ("the row's age", age_id("a"), "2m", 12.),
        (
            "the details bar's age",
            DETAILS_AGE_ID.to_owned(),
            "2m",
            12.,
        ),
        (
            "the details bar's command",
            DETAILS_COMMAND_ID.to_owned(),
            "iiiiiiii",
            13.333,
        ),
    ];
    for (part, id, text, size) in parts {
        let painted = texts_within(&nodes, &id);
        let [line] = &painted[..] else {
            panic!("{part} paints one line: {painted:?}");
        };
        assert_eq!(line.text.as_ref(), text, "{part}'s text");
        let (interface, monospace) = face_widths(text, size);
        assert!(
            (interface - monospace).abs() >= 1.,
            "premise: the two faces draw {text:?} apart ({interface} and {monospace})"
        );
        let drawn = f32::from(line.bounds.size.width);
        assert!(
            (drawn - monospace).abs() < 0.5,
            "{part} is {drawn} wide; the monospace face draws it {monospace}, the interface face {interface}"
        );
    }
}

/// The old `interface-fonts.spec.ts` case "interface and technical values load their own bundled font roles", its row
/// menu half: the line under the menu's title names a command session by the program it runs, and the old menu drew
/// those command bytes in the monospace face beside status words in the interface face. Only the real text system can
/// tell the faces apart. Specified, with the real text system, over two running sessions recorded as plain commands
/// (agent kind `generic`) that run `iiiiiiii` and `MMMMMMMM`: with each one's menu open in turn, its summary paints one
/// line that names its program, and the two lines are as wide as each other (within half a pixel), where the two
/// programs differ by more than 10 pixels in the interface face (the premise, at 12 pixels).
#[test]
fn the_menu_summary_draws_a_commands_program_in_the_monospace_face() {
    let rows = [
        session("narrow")
            .agent_kind("generic")
            .invocation("iiiiiiii"),
        session("wide").agent_kind("generic").invocation("MMMMMMMM"),
    ];
    let ((narrow_face, _), (wide_face, _)) =
        (face_widths("iiiiiiii", 12.), face_widths("MMMMMMMM", 12.));
    assert!(
        wide_face - narrow_face > 10.,
        "premise: the interface face draws the two programs apart ({narrow_face} and {wide_face})"
    );
    let summary_width = |key: &'static str, program: &str| {
        let open: Step<'_> = &|list: &Entity<SessionList>, window: &mut Window, cx: &mut App| {
            list.update(cx, |list, cx| list.toggle_menu(key, window, cx));
        };
        let nodes = narrowest_layout_then(&rows, Vec::new(), None, Some(open));
        let painted = texts_within(&nodes, &format!("session-menu-summary-{key}"));
        let [line] = &painted[..] else {
            panic!("{key}'s summary paints one line: {painted:?}");
        };
        assert!(
            line.text.contains(program),
            "{key}'s summary names its program: {:?}",
            line.text
        );
        f32::from(line.bounds.size.width)
    };
    let (narrow, wide) = (
        summary_width("narrow", "iiiiiiii"),
        summary_width("wide", "MMMMMMMM"),
    );
    assert!(
        (wide - narrow).abs() < 0.5,
        "the summaries are {narrow} and {wide} wide, so the program is not in the monospace face"
    );
}

// ---- The menu button's reveal ---------------------------------------------------------------------------------------

/// Whether b's menu button shows, as its wrapper's debug selector names the reveal (`super::menu`, "The button's
/// reveal"); the frame's record carries no opacity. Fails unless exactly one of the two names was drawn, so a button
/// drawn twice or not at all cannot pass for either.
fn b_button_shown(cx: &mut VisualTestContext) -> bool {
    test_support::redraw(cx);
    let shown = cx.debug_bounds("session-menu-b:shown").is_some();
    let hidden = cx.debug_bounds("session-menu-b:hidden").is_some();
    assert_ne!(shown, hidden, "b's button is drawn once");
    shown
}

/// Move the pointer onto the middle of what is drawn under element id `id` and let the window settle.
fn pointer_to(cx: &mut VisualTestContext, id: &str) {
    let at = match node(&frame(cx), id) {
        Some(drawn) => drawn.bounds.center(),
        None => panic!("{id} was not drawn"),
    };
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::settle(cx);
}

/// The old `buttons.spec.ts` case "the row kebab is hidden at rest and revealed by hover, focus, selection, or its own
/// open menu": every row has a menu button, which would crowd the list if all showed, so it shows only while there is a
/// reason to, and each reason must go away again, or a button would stay stuck on a row the user left. An open menu
/// must keep its button with the pointer gone, since the pointer leaves the row to reach the menu's commands. Run under
/// macOS's click rule, so a click focuses no control. Specified, over rows a (open) and b: at rest b's button is
/// hidden; with the pointer over b's row it shows, and still shows after a key that moves no focus (F12) is pressed
/// with the pointer resting there; with the pointer on the list's title again it is hidden; a click on it opens its
/// menu, and with the pointer on the title it still shows; Escape closes the menu and focus returns to the button,
/// which shows; Shift+Tab moves focus to b's open control, and the button still shows (focus anywhere in the row);
/// Shift+Tab again moves focus to a's menu button, out of b's row, and b's button is hidden; a click on b's row opens
/// b, and with the pointer on the title and focus outside b's row its button shows.
#[gpui::test]
fn the_menu_button_shows_for_hover_focus_its_open_menu_and_selection_and_none_latches(
    cx: &mut TestAppContext,
) {
    use super::menu_tests::snapshot;
    let (_fake, cx) = super::tests::open_main(
        cx,
        snapshot(
            &[
                (super::tests::session("a"), 0),
                (super::tests::session("b"), 0),
            ],
            false,
        ),
    );
    cx.update(|_, cx| crate::focus::set_click_focuses_controls(false, cx));
    pointer_to(cx, "session-order");
    assert_eq!(
        super::tests::selected(cx).as_deref(),
        Some("a"),
        "the premise: a is open"
    );
    assert!(!b_button_shown(cx), "hidden at rest");

    pointer_to(cx, "session-item-b");
    assert!(b_button_shown(cx), "the pointer over the row shows it");
    // A key that moves no focus: GPUI's own hover tracking stands down after keyboard input, the old `:hover` did not.
    let focused = test_support::focused_id(cx);
    test_support::keys(cx, "f12");
    test_support::redraw(cx);
    assert_eq!(
        test_support::focused_id(cx),
        focused,
        "the premise: the key moved no focus"
    );
    assert!(
        b_button_shown(cx),
        "a key pressed with the pointer resting on the row keeps it"
    );
    pointer_to(cx, "session-order");
    assert!(!b_button_shown(cx), "hover does not latch");

    test_support::click(cx, "session-menu-b");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("session-menu-rename"),
        "the premise: b's menu is open"
    );
    pointer_to(cx, "session-order");
    assert!(
        b_button_shown(cx),
        "its open menu keeps it, the pointer away"
    );

    test_support::keys(cx, "escape");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("session-menu-b")
    );
    assert!(b_button_shown(cx), "its own focus shows it");
    test_support::keys(cx, "shift-tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("session-row-b")
    );
    assert!(
        b_button_shown(cx),
        "focus on another control of the row shows it"
    );
    test_support::keys(cx, "shift-tab");
    assert_eq!(
        test_support::focused_id(cx).as_deref(),
        Some("session-menu-a"),
        "the premise: focus left b's row"
    );
    assert!(
        !b_button_shown(cx),
        "neither focus nor the closed menu latches"
    );

    test_support::click(cx, "session-row-b");
    pointer_to(cx, "session-order");
    assert_eq!(super::tests::selected(cx).as_deref(), Some("b"));
    let focused = test_support::focused_id(cx).unwrap_or_default();
    assert!(
        !focused.starts_with("session-row-b") && focused != "session-menu-b",
        "the premise: focus is outside b's row: {focused}"
    );
    assert!(b_button_shown(cx), "the open session's button shows");
}

/// The menu button's hover help is the old toggle's, which says what the menu holds rather than only that it opens.
/// Specified: hovering b's button shows `Session actions: rename, clone, replace, stop, delete and more`.
#[gpui::test]
fn the_menu_buttons_help_names_what_its_menu_holds(cx: &mut TestAppContext) {
    use super::menu_tests::snapshot;
    let (_fake, cx) = super::tests::open_main(
        cx,
        snapshot(
            &[
                (super::tests::session("a"), 0),
                (super::tests::session("b"), 0),
            ],
            false,
        ),
    );
    assert_eq!(
        help_after_crossing(cx, "session-row-b", "session-menu-b").as_deref(),
        Some("Session actions: rename, clone, replace, stop, delete and more")
    );
}

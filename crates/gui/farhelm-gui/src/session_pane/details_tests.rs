//! Tests of the open session's details bar: which parts it draws and in what order for each kind of status, what it
//! leaves out (a stale session's badge and age, a host nobody named, a mark for a host nobody placed), the words it
//! uses for the local host, how its parts give way in a narrow pane, and its age moving on with the shared clock. The
//! bar of an open session the list does not show is `carried_tests.rs`'s.
//!
//! The decisions are a pure function ([`Details::of`]), tested without a window where only the decision matters.
//! Everything else runs in a window over the helm client's fake and reads GPUI's driver record of the frame: the bar is
//! `session-details`, its parts `session-details-status`, `-title`, `-host` (the mark), `-host-name` and `-age`. The
//! first session of every snapshot is the open one, since it is the last visited. The layout test uses the real text
//! system, because the test context's gives every character the same width and cannot show what is cut.

use std::sync::Arc;
use std::time::Duration;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::{CollectionState, SessionRow};
use gpui::{
    AppContext as _, Context, DriverNode, Entity, HeadlessAppContext, IntoElement, Modifiers,
    ParentElement as _, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px,
    size,
};
use gpui_component::Root;
use serde_json::json;

use super::SessionPane;
use super::details::{
    AGE_ID, BAR_ID, Details, HOST_MARK_ID, HOST_NAME_ID, STATUS_ID, Status, TITLE_ID,
};
use crate::session_list::SessionList;
use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::status::Live;
use crate::session_list::tests::{
    Fixture, TEST_NOW, change, collections, listing, listing_meta, open_main, preferences,
    selected, session,
};
use crate::test_support::{self, frame, label, started};
use crate::{assets, bootstrap, layers, render_count, root, ticker};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// `rows` listed in that order, the first one last visited and so open at startup, with `extra` collections beside them
/// (a host list, say).
fn snapshot(rows: &[Fixture], extra: Vec<CollectionState>) -> Vec<CollectionState> {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let mut snapshot = vec![listing(&order, rows), preferences(&order[..1], None)];
    snapshot.extend(extra);
    snapshot
}

/// The node drawn under `id` in `nodes`, if any.
fn node<'a>(nodes: &'a [DriverNode], id: &str) -> Option<&'a DriverNode> {
    nodes
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
}

/// The strings painted under `id`, at any depth, in paint order. Text is painted by the text elements inside a part, so
/// this follows the record's parent links up from every node.
fn painted_within(nodes: &[DriverNode], id: &str) -> Vec<String> {
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
        .flat_map(|index| nodes[index].texts.iter().map(|text| text.text.to_string()))
        .collect()
}

/// The ids of the bar's parts that were drawn, left to right.
fn parts_in_order(nodes: &[DriverNode]) -> Vec<&'static str> {
    let mut drawn: Vec<(f32, &'static str)> =
        [STATUS_ID, TITLE_ID, HOST_MARK_ID, HOST_NAME_ID, AGE_ID]
            .into_iter()
            .filter_map(|id| node(nodes, id).map(|node| (f32::from(node.bounds.origin.x), id)))
            .collect();
    drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
    drawn.into_iter().map(|(_, id)| id).collect()
}

/// The hover help shown once the pointer has rested on the element drawn under `selector` for the hover delay. The
/// pointer goes to the top row's empty left region first, so a tooltip already showing is put away and the next one is
/// shown afresh.
fn hover(cx: &mut VisualTestContext, selector: &'static str) -> Option<String> {
    let away = test_support::center(cx, super::header::TABS_SLOT);
    cx.simulate_mouse_move(away, None, Modifiers::none());
    test_support::settle(cx);
    let at = test_support::center(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    test_support::advance(cx, layers::HOVER_DELAY);
    cx.update(|window, cx| layers::shown_tooltip(window, cx))
        .map(|shown| shown.text.to_string())
}

/// `key` last active `ago` seconds before the test clock's start, and seen then (so an idle one is plainly idle).
fn active(key: &'static str, ago: i64) -> Fixture {
    session(key)
        .activity(TEST_NOW - ago)
        .seen(Some(TEST_NOW - ago))
}

/// A root view drawing the pane alone across the window, wired as every root view is, with a session list kept but not
/// drawn: the list owns the selection rules, so without one nothing opens a session. Undrawn, the list holds no clock
/// or pulse of its own, so whatever redraws the pane is the pane's.
struct PaneOnly {
    pane: Entity<SessionPane>,
    _list: Entity<SessionList>,
    root: root::Attachment,
}

impl PaneOnly {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        PaneOnly {
            pane: cx.new(|cx| SessionPane::new(window, cx)),
            _list: cx.new(SessionList::new),
            root: root::attach(window, cx),
        }
    }
}

impl Render for PaneOnly {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        root::wrap(
            div().size_full().child(self.pane.clone()),
            &self.root,
            window,
            cx,
        )
    }
}

// ---- What the bar shows --------------------------------------------------------------------------------------------

/// SPEC.md, "Terminal experience": the bar "shows, in order, status, session name, host, age", below the terminal, and
/// the status is the list's own (a mark for a live status, the words for an ended one), so the bar and the row cannot
/// describe a session differently. An unclassified session "shows no status and still shows its age" (SPEC.md,
/// "Status"). Specified, opening each session in turn, on the local host with the host list read: a running session
/// draws its mark named `Running`, its title, the host mark named `Local`, `Local (this machine)` and `2m`, left to
/// right in that order; an idle one the same with its mark named `Idle`; an exited one the same with its badge named
/// and painting `Exited (code 0) — stopped by user`; an unclassified one no badge, but its title, host and age in
/// order. The bar sits below the terminal.
#[gpui::test]
fn the_bar_draws_status_title_host_and_age_in_order_for_each_kind_of_status(
    cx: &mut TestAppContext,
) {
    let on_local = |fixture: Fixture| fixture.host(1).host_name("this machine");
    let rows = [
        on_local(active("a", 120).title("Refactor the API client")),
        on_local(
            active("b", 120)
                .title("Update dependencies")
                .status(json!({"state": "idle"})),
        ),
        on_local(
            active("c", 120)
                .title("Release notes")
                .status(json!({"state": "exited", "exit_code": 0}))
                .annotation("stopped by user"),
        ),
        on_local(
            active("d", 120)
                .title("Just created")
                .status(json!({"state": "unknown"})),
        ),
    ];
    let (_fake, cx) = open_main(cx, snapshot(&rows, vec![hosts(&[local()])]));
    let full = [STATUS_ID, TITLE_ID, HOST_MARK_ID, HOST_NAME_ID, AGE_ID];
    for (key, row, status, title, parts) in [
        (
            "a",
            "session-row-a",
            Some("Running"),
            "Refactor the API client",
            &full[..],
        ),
        (
            "b",
            "session-row-b",
            Some("Idle"),
            "Update dependencies",
            &full[..],
        ),
        (
            "c",
            "session-row-c",
            Some("Exited (code 0) — stopped by user"),
            "Release notes",
            &full[..],
        ),
        ("d", "session-row-d", None, "Just created", &full[1..]),
    ] {
        test_support::click(cx, row);
        assert_eq!(
            selected(cx).as_deref(),
            Some(key),
            "the premise: {key} is open"
        );
        let nodes = frame(cx);
        assert_eq!(
            parts_in_order(&nodes),
            parts,
            "{key}'s parts, left to right"
        );
        assert_eq!(
            node(&nodes, STATUS_ID).and_then(|node| node.label.as_deref().map(str::to_owned)),
            status.map(str::to_owned),
            "{key}'s badge"
        );
        if let Some(status) = status.filter(|_| key == "c") {
            // The words and the annotation are two text elements (the host-text rule), so the painted line is their
            // concatenation.
            assert_eq!(
                painted_within(&nodes, STATUS_ID).concat(),
                status,
                "the ended words are painted"
            );
        }
        assert_eq!(painted_within(&nodes, TITLE_ID), [title], "{key}'s title");
        assert_eq!(
            node(&nodes, HOST_MARK_ID).and_then(|node| node.label.as_deref().map(str::to_owned)),
            Some("Local".to_owned())
        );
        assert_eq!(
            painted_within(&nodes, HOST_NAME_ID),
            ["Local (this machine)"]
        );
        assert_eq!(painted_within(&nodes, AGE_ID), ["2m"], "{key}'s age");
        let bar = node(&nodes, BAR_ID).expect("the bar was drawn");
        let terminal = node(&nodes, "terminal-grid").expect("the terminal was drawn");
        assert!(
            bar.bounds.top() >= terminal.bounds.bottom(),
            "the bar is below the terminal: {:?} {:?}",
            bar.bounds,
            terminal.bounds
        );
    }
}

/// The bar names an exit the way the quick switcher and the menu header do, with plain `Exited` when the code is not
/// known, as the old app's `status_badge` did; only the sidebar row spells out `Exited (code unknown)`, where the
/// complete ended wording lives (SPEC.md, "Status"). The two forms are a deliberate split, and a bar that borrowed the
/// row's would say more than the old bar. Specified: as a decision and drawn, an exit with no code and no annotation
/// has the badge `Exited`, while its row says `Exited (code unknown)`.
#[gpui::test]
fn an_exit_without_a_code_reads_plain_exited_in_the_bar(cx: &mut TestAppContext) {
    let exited = active("a", 120).status(json!({"state": "exited", "exit_code": null}));
    let collections = collections(snapshot(std::slice::from_ref(&exited), Vec::new()));
    let row: &SessionRow = collections.get::<SessionRow>("a").unwrap();
    match Details::of(row, &collections, Some(TEST_NOW)).status {
        Some(Status::Ended(ended)) => {
            assert_eq!((ended.words.as_str(), ended.host), ("Exited", None))
        }
        other => panic!("an ended badge: {other:?}"),
    }

    let (_fake, cx) = open_main(cx, snapshot(&[exited], Vec::new()));
    let nodes = frame(cx);
    assert_eq!(label(cx, STATUS_ID).as_deref(), Some("Exited"));
    assert_eq!(painted_within(&nodes, STATUS_ID), ["Exited"]);
    assert!(
        painted_within(&nodes, "session-item-a")
            .iter()
            .any(|text| text == "Exited (code unknown)"),
        "the premise: the row says the code is unknown"
    );
}

/// Each part's hover help is the whole of what it may cut short or abbreviate: the old bar's tooltips, the title's
/// through the host-text element, the host's the name as shown, the age's the stamp, and an ended badge's its complete
/// wording, since the words may end in an ellipsis. Specified, on the exited session: the badge says `Exited (code 0) —
/// stopped by user`, the title its title, the host name `buildbox`, and the age `Last activity 2025-07-01 23:57:58 UTC`.
#[gpui::test]
fn each_parts_hover_help_is_its_whole_value(cx: &mut TestAppContext) {
    let row = active("a", 120)
        .title("Release notes for 0.40")
        .status(json!({"state": "exited", "exit_code": 0}))
        .annotation("stopped by user");
    let (_fake, cx) = open_main(cx, snapshot(&[row], Vec::new()));
    assert_eq!(
        hover(cx, STATUS_ID).as_deref(),
        Some("Exited (code 0) — stopped by user")
    );
    assert_eq!(
        hover(cx, TITLE_ID).as_deref(),
        Some("Release notes for 0.40")
    );
    assert_eq!(hover(cx, HOST_NAME_ID).as_deref(), Some("buildbox"));
    assert_eq!(
        hover(cx, AGE_ID).as_deref(),
        Some("Last activity 2025-07-01 23:57:58 UTC")
    );
}

/// SPEC.md: "a stale session's status appears only in its stale notice, not again in the bottom bar", and its age
/// belongs with that last-known status in one band (a later piece); the session's name and host stay in the bar, since
/// they are not last-known claims about the agent. Specified, as a decision and drawn: a stale running session has no
/// badge and no age, and draws its title and host name; the same session on a connected host has both.
#[gpui::test]
fn a_stale_session_draws_no_badge_and_no_age(cx: &mut TestAppContext) {
    let stale = active("a", 120).stale();
    let collections_of =
        |fixture: &Fixture| collections(snapshot(std::slice::from_ref(fixture), Vec::new()));
    let decided = |fixture: &Fixture| {
        let collections = collections_of(fixture);
        let row: &SessionRow = collections.get::<SessionRow>(fixture.key).unwrap();
        Details::of(row, &collections, Some(TEST_NOW))
    };
    let fresh = decided(&active("a", 120));
    assert_eq!(
        fresh.status,
        Some(Status::Live(Live::Running)),
        "the premise"
    );
    assert!(fresh.age.is_some(), "the premise");
    let shown = decided(&stale);
    assert_eq!(shown.status, None);
    assert_eq!(shown.age, None);

    let (_fake, cx) = open_main(cx, snapshot(&[stale], Vec::new()));
    let nodes = frame(cx);
    assert!(node(&nodes, STATUS_ID).is_none(), "no badge");
    assert!(node(&nodes, AGE_ID).is_none(), "no age");
    assert_eq!(parts_in_order(&nodes), [TITLE_ID, HOST_NAME_ID]);
}

/// SPEC.md: "A missing host name omits the host rather than inventing one", and "the host's icon follows the session
/// list's rules", under which a host that cannot be placed gets no mark while its name still shows. Specified: a
/// session whose helm sent no host name draws no host mark and no host name, its age right after its title; with no
/// host list read, `buildbox` is drawn with no mark.
#[gpui::test]
fn no_host_name_draws_no_host_and_an_unplaced_host_no_mark(cx: &mut TestAppContext) {
    let rows = [active("a", 120).host_name(""), active("b", 120)];
    let (_fake, cx) = open_main(cx, snapshot(&rows, Vec::new()));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise");
    let nodes = frame(cx);
    assert!(node(&nodes, HOST_MARK_ID).is_none());
    assert!(node(&nodes, HOST_NAME_ID).is_none());
    assert_eq!(parts_in_order(&nodes), [STATUS_ID, TITLE_ID, AGE_ID]);

    test_support::click(cx, "session-row-b");
    let nodes = frame(cx);
    assert!(
        node(&nodes, HOST_MARK_ID).is_none(),
        "no host list read: no mark"
    );
    assert_eq!(painted_within(&nodes, HOST_NAME_ID), ["buildbox"]);
}

/// SPEC.md: the host's "name is the full display name", never the row's short form, and only the local host reads
/// `Local (this machine)`: a remote host the user happened to call `this machine` must not pass for the helm's own.
/// Specified: on a listed remote host the bar paints the helm's whole name, `ops@render-west.example.invalid`, beside
/// a mark named `Remote`. The wording rule itself, `this machine` read as `Local (this machine)` only for a host known
/// to be local, is pinned in `hosts/identity/tests.rs`.
#[gpui::test]
fn a_remote_host_is_named_in_full_and_only_the_local_host_reads_local(cx: &mut TestAppContext) {
    let row = active("a", 120)
        .host(2)
        .host_name("ops@render-west.example.invalid");
    let registry = hosts(&[local(), remote(2, "ops@render-west.example.invalid")]);
    let (_fake, cx) = open_main(cx, snapshot(&[row], vec![registry]));
    let nodes = frame(cx);
    assert_eq!(
        painted_within(&nodes, HOST_NAME_ID),
        ["ops@render-west.example.invalid"]
    );
    assert_eq!(label(cx, HOST_MARK_ID).as_deref(), Some("Remote"));
}

// ---- The clock -----------------------------------------------------------------------------------------------------

/// The bar's age moves on by itself, as the rows' do, through the ticker's one 30-second clock (SPEC_impl.md, "Animation
/// time: the ticker"), in an inactive window too, since a window behind others is still on screen. The list is not
/// drawn here, so only the pane's own clock can redraw it. Specified: in an inactive window, with an idle session 59
/// minutes 40 seconds old, the bar paints `59m`; 29 seconds redraw the pane not once; the 30th redraws it, and the bar
/// then paints `1h`.
#[gpui::test]
fn the_age_moves_on_with_the_shared_clock(cx: &mut TestAppContext) {
    let row = active("a", 59 * 60 + 40).status(json!({"state": "idle"}));
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot(&[row], Vec::new()));
    cx.run_until_parked();
    let (_view, cx) = test_support::open_view(cx, PaneOnly::new);
    assert_eq!(painted_within(&frame(cx), AGE_ID), ["59m"], "the premise");
    cx.deactivate_window();
    test_support::settle(cx);

    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(29));
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-pane", cx)),
        0,
        "the premise: nothing else redraws the pane"
    );
    test_support::advance(cx, Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| render_count::count("session-pane", cx)) > 0,
        "the clock redrew the pane"
    );
    assert_eq!(painted_within(&frame(cx), AGE_ID), ["1h"]);
}

/// The pane takes the bar's pulse and clock handles only while the bar needs them, and lets both go when no session is
/// open: a handle kept past that would go on redrawing the whole window ten times a second, or every 30 seconds, for a
/// bar that is not there (SPEC_impl.md, "Animation time: the ticker"). The list is not drawn, so only the pane's own
/// handles can redraw it. Specified: in an active window with a running session open, a second of ticks redraws the
/// pane (the pulse); once the helm's complete listing drops the session and the pane shows no session, neither a
/// second (the pulse's ticks) nor 30 seconds (the clock's period) redraws it.
#[gpui::test]
fn the_pulse_and_clock_handles_go_when_no_session_is_open(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot(&[active("a", 120)], Vec::new()));
    cx.run_until_parked();
    let (_view, cx) = test_support::open_view(cx, PaneOnly::new);
    assert!(node(&frame(cx), BAR_ID).is_some(), "the premise: a's bar");
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    assert!(
        cx.update(|_, cx| render_count::count("session-pane", cx)) > 0,
        "the premise: the running mark's pulse redraws the pane"
    );

    fake.push(change(listing_meta(&[], 0, false), &[], &["a"]));
    test_support::settle(cx);
    assert_eq!(selected(cx), None, "the premise: no session is open");
    assert!(node(&frame(cx), BAR_ID).is_none(), "the premise: no bar");
    cx.update(|_, cx| render_count::reset(cx));
    test_support::advance(cx, Duration::from_secs(1));
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-pane", cx)),
        0,
        "no pulse"
    );
    test_support::advance(cx, ticker::CLOCK_PERIOD);
    assert_eq!(
        cx.update(|_, cx| render_count::count("session-pane", cx)),
        0,
        "no clock"
    );
}

// ---- Layout, with the real text system -----------------------------------------------------------------------------

/// The old bar's oversized-title case (`header.spec.ts`, "oversized title and copy fields overflow their boxes"): the
/// bar never wraps, a title too long for it is cut with an ellipsis rather than pushing the rest out of the pane, and
/// the live mark and the age never shrink, since a cut mark or age says nothing. The title and the host shrink in
/// proportion to their widths, as the old bar's did, so nothing is asserted of the host beyond being on the line. Text
/// width decides this, so it runs with the real text system and the bundled fonts. Specified: in a 420-pixel pane, a
/// running session with a long title draws its live mark whole, its title cut short with `…`, and its age `2m` whole,
/// all on the bar's one line and inside the pane.
#[test]
fn an_oversized_title_is_clipped_and_the_mark_and_age_stay_whole() {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx =
        HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    cx.run_until_parked();
    let row = active("a", 120).title(
        "Refactor the API client so that every request retries with backoff and logs each attempt",
    );
    fake.connect(snapshot(&[row], Vec::new()));
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(420.), px(300.)), |window, cx| {
            let view = cx.new(|cx| PaneOnly::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap();
    let mut nodes = Vec::new();
    // Three frames, as the header's layout test draws: the pane mounts its terminal only after the window's first frame
    // (a deferred sync), and icons decode on the background executor, so the first frame can lack both.
    for _ in 0..3 {
        cx.run_until_parked();
        nodes = cx
            .update_window(window.into(), |_, window, cx| {
                window.set_driver_enabled(true);
                window.draw(cx).clear(cx);
                window.driver_nodes().to_vec()
            })
            .unwrap();
    }
    let bounds = |id: &str| node(&nodes, id).unwrap_or_else(|| panic!("{id} was drawn"));
    let status = bounds(STATUS_ID);
    let title = painted_within(&nodes, TITLE_ID);
    assert!(
        title.len() == 1 && title[0].starts_with("Refactor") && title[0].ends_with('…'),
        "the title is cut short: {title:?}"
    );
    assert_eq!(painted_within(&nodes, AGE_ID), ["2m"]);
    let (title, host, age) = (bounds(TITLE_ID), bounds(HOST_NAME_ID), bounds(AGE_ID));
    for part in [status, age] {
        assert_eq!(part.visible_bounds, part.bounds, "drawn whole: {part:?}");
        assert!(
            part.bounds.right() <= px(420.),
            "inside the pane: {:?}",
            part.bounds
        );
    }
    let line = status.bounds.center().y;
    for part in [title, host, age] {
        assert!(
            part.bounds.top() <= line && line <= part.bounds.bottom(),
            "one line: {:?}",
            part.bounds
        );
    }
}

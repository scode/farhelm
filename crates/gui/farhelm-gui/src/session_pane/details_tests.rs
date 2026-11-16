//! Tests of the open session's details bar: which parts it draws and in what order for each kind of status, what it
//! leaves out (a stale session's badge and age, a host nobody named, a mark for a host nobody placed), the words it
//! uses for the local host, how its parts give way in a narrow pane, its age moving on with the shared clock, the
//! managed-checkout mark, and the directory and command copy fields with their hidden-characters warning. The bar of
//! an open session the list does not show is `carried_tests.rs`'s. How a copy field confirms a click is its own
//! component's (`components/tests/copy_field.rs`); here the bar's wiring of it is.
//!
//! The decisions are a pure function ([`Details::of`]), tested without a window where only the decision matters.
//! Everything else runs in a window over the helm client's fake and reads GPUI's driver record of the frame: the bar is
//! `session-details`, its parts `session-details-status`, `-title`, `-host` (the mark), `-host-name`, `-age`,
//! `-checkout`, `-directory` and `-command`, and the warning above it `session-details-copy-warning`. The
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
    AGE_ID, BAR_ID, CHECKOUT_ID, COMMAND_ID, DIRECTORY_ID, Details, HOST_MARK_ID, HOST_NAME_ID,
    STATUS_ID, Status, TITLE_ID, WARNING_ID, directory_label,
};
use crate::actions::{FocusNextControl, FocusPreviousControl};
use crate::components::{CONFIRMATION, COPIED};
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
pub(super) fn painted_within(nodes: &[DriverNode], id: &str) -> Vec<String> {
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
/// or pulse of its own, so whatever redraws the pane is the pane's. The unavailable card's tests use it too.
pub(super) struct PaneOnly {
    pane: Entity<SessionPane>,
    _list: Entity<SessionList>,
    root: root::Attachment,
}

impl PaneOnly {
    /// The harness's root view. The pane gets the kept list's New handle, as the main view passes it, but the list is
    /// not drawn, so neither is New: over an empty fleet the focus coordinator counts that undrawn home as none, and
    /// focus rests on the window's root, not on a handle that takes no keys.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let list = cx.new(SessionList::new);
        let new = list.read(cx).new_focus().clone();
        PaneOnly {
            pane: cx.new(|cx| SessionPane::new(new, window, cx)),
            _list: list,
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

/// The driver record of the pane drawn alone in a window `width` pixels wide with `row` open, laid out by the real text
/// system with the bundled fonts, since what is cut depends on real glyph widths.
fn real_layout(width: f32, row: Fixture) -> Vec<DriverNode> {
    let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
        assets::UI_FONT,
    ));
    let mut cx =
        HeadlessAppContext::with_platform(text_system, Arc::new(assets::Assets), || Ok(None));
    cx.update(bootstrap::install).unwrap();
    let (mut fake, handle, events) = FakeHelm::new();
    let _connection = cx.update(|cx| crate::session::start(handle, events, None, cx));
    cx.run_until_parked();
    fake.connect(snapshot(&[row], Vec::new()));
    cx.run_until_parked();
    let window = cx
        .open_window(size(px(width), px(300.)), |window, cx| {
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
    nodes
}

/// The old bar's oversized-title case (`header.spec.ts`, "oversized title and copy fields overflow their boxes"): the
/// bar never wraps, a title too long for it is cut with an ellipsis rather than pushing the rest out of the pane, and
/// the live mark and the age never shrink, since a cut mark or age says nothing. The title and the host shrink in
/// proportion to their widths, as the old bar's did, so nothing is asserted of the host beyond being on the line (the
/// copy fields have given up all their width by then; `the_copy_fields_give_way_before_the_title_and_host` is their
/// case). Text width decides this, so it runs with the real text system and the bundled fonts. Specified: in a
/// 420-pixel pane, a running session with a long title draws its live mark whole, its title cut short with `…`, and
/// its age `2m` whole, all on the bar's one line and inside the pane.
#[test]
fn an_oversized_title_is_clipped_and_the_mark_and_age_stay_whole() {
    let row = active("a", 120).title(
        "Refactor the API client so that every request retries with backoff and logs each attempt",
    );
    let nodes = real_layout(420., row);
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

/// SPEC.md, "Terminal experience": the copy fields "ellipsize before the session name or host when the bar runs out of
/// room". The old bar's `flex: 0 3 auto` did not quite: shrinking is weighted by width, so beside a long command its
/// title was cut too, and this bar follows SPEC.md there instead (the parity doc records it). Text width decides
/// this, so it runs with the real text system. Specified: in a 700-pixel pane, a running session titled `Refactor the
/// API client` on `buildbox`, launched with a command far too long for the bar, draws its title and host whole and its
/// command cut short with `…`.
#[test]
fn the_copy_fields_give_way_before_the_title_and_host() {
    let command = "/home/user/.cargo/bin/farhelm-fixtures fake-agent --script basic --transcript \
                   /home/user/farhelm-gpui-worker/target/debug/build/fixtures/transcript-basic.txt";
    let row = active("a", 120)
        .title("Refactor the API client")
        .invocation(command);
    let nodes = real_layout(700., row);
    assert_eq!(
        painted_within(&nodes, TITLE_ID),
        ["Refactor the API client"]
    );
    assert_eq!(painted_within(&nodes, HOST_NAME_ID), ["buildbox"]);
    let shown = painted_within(&nodes, COMMAND_ID);
    assert!(
        shown.len() == 1 && shown[0].starts_with("/home") && shown[0].ends_with('…'),
        "the command is cut short: {shown:?}"
    );
}

// ---- The managed checkout and the copy fields -----------------------------------------------------------------------

/// What the test clipboard holds, as text.
fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

/// The hidden-characters warning's wording for the command line, exactly as the old bar wrote it.
const COMMAND_WARNING: &str = "⚠ The copied command contains hidden characters, displayed as <U+…>";

/// A command line with a right-to-left override and a line break in it, the old bar's hidden-characters case
/// (`header.spec.ts`, "details copy shows escaped peer text, copies raw bytes, and warns upward").
const HIDDEN_COMMAND: &str = "codex --prompt \u{202E}abc\n# tail";

/// SPEC.md: "Directory display still abbreviates the home directory to `~`", the session list's fold. The old bar added
/// one guard: the fold must not remove a presentation-unsafe character, since the copy holds it and the shortened label
/// would then hide its escape. Specified: `/home/alice/src` shows as `~/src`, and so does a hidden character after the
/// account, which the fold keeps; one inside the account segment keeps the whole path; a path outside a home is
/// unchanged.
#[test]
fn the_directory_folds_its_home_unless_the_fold_hides_a_character() {
    assert_eq!(directory_label("/home/alice/src"), "~/src");
    assert_eq!(directory_label("/home/alice/s\u{202E}rc"), "~/s\u{202E}rc");
    assert_eq!(
        directory_label("/home/al\u{202E}ice/src"),
        "/home/al\u{202E}ice/src"
    );
    assert_eq!(directory_label("/tmp/work"), "/tmp/work");
}

/// SPEC.md: the bar ends with "the managed-checkout branch mark when present, directory, and command line", the two
/// last as muted click-to-copy buttons (their order is the `session-details-bar` golden's). Each field shows its value
/// (the directory folded), is named by it, and its hover help is the whole recorded value with "— click to copy", as
/// the old tooltip and accessible description were. A click copies the recorded bytes, not what the field shows (the
/// old `header.spec.ts` "copy full values" case), and an ordinary value has nothing hidden to warn about. Specified,
/// for a session in `/home/user/src/a` launched as `codex --model gpt-5`: the directory paints and is named `~/src/a`,
/// with the help `/home/user/src/a — click to copy`; the command paints and is named `codex --model gpt-5`, with the
/// help `codex --model gpt-5 — click to copy`; a click on the directory puts `/home/user/src/a` on the clipboard and
/// draws no warning.
#[gpui::test]
fn the_copy_fields_show_name_help_and_copy_their_values(cx: &mut TestAppContext) {
    let row = active("a", 120).invocation("codex --model gpt-5");
    let (_fake, cx) = open_main(cx, snapshot(&[row], Vec::new()));
    let nodes = frame(cx);
    assert_eq!(painted_within(&nodes, DIRECTORY_ID), ["~/src/a"]);
    assert_eq!(painted_within(&nodes, COMMAND_ID), ["codex --model gpt-5"]);
    assert_eq!(label(cx, DIRECTORY_ID).as_deref(), Some("~/src/a"));
    assert_eq!(
        label(cx, COMMAND_ID).as_deref(),
        Some("codex --model gpt-5")
    );
    assert_eq!(
        hover(cx, DIRECTORY_ID).as_deref(),
        Some("/home/user/src/a — click to copy")
    );
    assert_eq!(
        hover(cx, COMMAND_ID).as_deref(),
        Some("codex --model gpt-5 — click to copy")
    );
    test_support::click(cx, DIRECTORY_ID);
    assert_eq!(clipboard(cx).as_deref(), Some("/home/user/src/a"));
    assert!(
        node(&frame(cx), WARNING_ID).is_none(),
        "nothing hidden: no warning"
    );
}

/// The old bar's hidden-characters case: the field shows a command's hidden characters escaped and copies them raw,
/// and since a command pasted into a shell runs the clipboard's version, a warning opens upward from the bar, inside
/// the window, for as long as the click's confirmation shows. Specified: before the click the command paints
/// `codex --prompt <U+202E>abc<U+000A># tail` and nothing painted holds a raw U+202E; a click copies the raw command;
/// the warning, a status named with the old wording exactly, sits above the bar, unclipped and inside the window; 1.5
/// seconds later it is gone.
#[gpui::test]
fn copying_hidden_characters_warns_upward_while_the_click_is_confirmed(cx: &mut TestAppContext) {
    let row = active("a", 120).invocation(HIDDEN_COMMAND);
    let (_fake, cx) = open_main(cx, snapshot(&[row], Vec::new()));
    let nodes = frame(cx);
    assert_eq!(
        painted_within(&nodes, COMMAND_ID),
        ["codex --prompt <U+202E>abc<U+000A># tail"]
    );
    assert!(
        nodes
            .iter()
            .flat_map(|node| &node.texts)
            .all(|text| !text.text.contains('\u{202E}')),
        "no raw override reaches the screen"
    );
    test_support::click(cx, COMMAND_ID);
    assert_eq!(clipboard(cx).as_deref(), Some(HIDDEN_COMMAND));
    let nodes = frame(cx);
    let warning = node(&nodes, WARNING_ID).expect("the warning is drawn");
    assert_eq!(warning.role, Some(gpui::Role::Status));
    assert_eq!(warning.label.as_deref(), Some(COMMAND_WARNING));
    let bar = node(&nodes, BAR_ID).expect("the bar is drawn");
    let window = cx.update(|window, _| window.viewport_size());
    assert!(
        warning.bounds.bottom() <= bar.bounds.top(),
        "above the bar: {:?} {:?}",
        warning.bounds,
        bar.bounds
    );
    assert!(
        warning.bounds.left() >= px(0.)
            && warning.bounds.top() >= px(0.)
            && warning.bounds.right() <= window.width,
        "inside the window: {:?}",
        warning.bounds
    );
    assert_eq!(warning.visible_bounds, warning.bounds, "not clipped");
    test_support::advance(cx, CONFIRMATION);
    assert!(
        node(&frame(cx), WARNING_ID).is_none(),
        "gone with the confirmation"
    );
}

/// The warning names what was copied: copying a directory holding a hidden character warns about the folder (the
/// command's wording is the case above). Specified: a click on `/home/al<U+202E>ice/src` copies the raw path and the
/// warning reads `⚠ The copied folder contains hidden characters, displayed as <U+…>`.
#[gpui::test]
fn copying_a_hidden_character_in_the_directory_warns_for_the_folder(cx: &mut TestAppContext) {
    let row = active("a", 120).cwd("/home/al\u{202E}ice/src");
    let (_fake, cx) = open_main(cx, snapshot(&[row], Vec::new()));
    test_support::click(cx, DIRECTORY_ID);
    assert_eq!(clipboard(cx).as_deref(), Some("/home/al\u{202E}ice/src"));
    assert_eq!(
        label(cx, WARNING_ID).as_deref(),
        Some("⚠ The copied folder contains hidden characters, displayed as <U+…>")
    );
}

/// The old `header.spec.ts` case "copied command warning survives a safe metadata refresh": the warning describes the
/// bytes the click copied, so a refresh that makes the value plain during the confirmation must neither take the
/// warning away nor touch the clipboard, while the help already names the new value. Specified: after copying the hidden
/// command, the helm pushes the session with `codex --model gpt-5`; the field still paints `✓ Copied`, the warning is
/// still drawn, the clipboard still holds the hidden command, and the help names the new command; 1.5 seconds after the
/// click the warning is gone and the field shows the new command.
#[gpui::test]
fn the_warning_survives_a_refresh_that_makes_the_value_plain(cx: &mut TestAppContext) {
    let row = |invocation: &str| active("a", 120).invocation(invocation);
    let (mut fake, cx) = open_main(cx, snapshot(&[row(HIDDEN_COMMAND)], Vec::new()));
    test_support::click(cx, COMMAND_ID);
    assert!(
        node(&frame(cx), WARNING_ID).is_some(),
        "the premise: a warning"
    );
    fake.push(change(
        listing_meta(&["a"], 1, false),
        &[row("codex --model gpt-5")],
        &[],
    ));
    test_support::settle(cx);
    let nodes = frame(cx);
    assert_eq!(painted_within(&nodes, COMMAND_ID), [COPIED]);
    assert!(node(&nodes, WARNING_ID).is_some(), "the warning holds");
    assert_eq!(clipboard(cx).as_deref(), Some(HIDDEN_COMMAND));
    assert_eq!(
        hover(cx, COMMAND_ID).as_deref(),
        Some("codex --model gpt-5 — click to copy")
    );
    test_support::advance(cx, CONFIRMATION);
    let nodes = frame(cx);
    assert!(node(&nodes, WARNING_ID).is_none());
    assert_eq!(painted_within(&nodes, COMMAND_ID), ["codex --model gpt-5"]);
}

/// The old managed-checkout mark (`ManagedCheckoutMark` in `icons.rs` at the reference commit) is named for the
/// repository and explains the checkout in its hover: where it is, the session's own folder when that is not the
/// checkout's (so a subdirectory's location stays known), and that the checkout goes to the trash with its last
/// session. A session without a checkout shows no mark (`github-checkouts.spec.ts`: "an unmanaged session shows its
/// directory and no mark"). Specified: the helm fixture's checkout of `octo/widgets` at `/home/user/src/widgets`, for
/// a session in `/home/user/src/a`, names the mark `Managed checkout of octo/widgets` and its help is exactly the old
/// tooltip with `Session folder: /home/user/src/a.`; for a session in the checkout's own folder the help has no
/// session folder; an unmanaged session draws no mark and still draws its directory.
#[gpui::test]
fn the_checkout_mark_names_the_repository_and_explains_the_checkout(cx: &mut TestAppContext) {
    let rows = [
        active("a", 120),
        active("b", 120).cwd("/home/user/src/widgets"),
        active("c", 120).unmanaged(),
    ];
    let (_fake, cx) = open_main(cx, snapshot(&rows, Vec::new()));
    assert_eq!(selected(cx).as_deref(), Some("a"), "the premise");
    assert_eq!(
        label(cx, CHECKOUT_ID).as_deref(),
        Some("Managed checkout of octo/widgets")
    );
    assert_eq!(
        hover(cx, CHECKOUT_ID).as_deref(),
        Some(
            "Managed checkout of octo/widgets at /home/user/src/widgets. Session folder: /home/user/src/a. \
             Farhelm made the checkout folder for its first session. When the last session using it is deleted, the \
             checkout moves to the trash."
        )
    );
    test_support::click(cx, "session-row-b");
    assert_eq!(
        hover(cx, CHECKOUT_ID).as_deref(),
        Some(
            "Managed checkout of octo/widgets at /home/user/src/widgets. Farhelm made the checkout folder for its \
             first session. When the last session using it is deleted, the checkout moves to the trash."
        )
    );
    test_support::click(cx, "session-row-c");
    let nodes = frame(cx);
    assert!(node(&nodes, CHECKOUT_ID).is_none(), "no checkout, no mark");
    assert_eq!(painted_within(&nodes, DIRECTORY_ID), ["~/src/c"]);
}

/// SPEC.md: "Keyboard order follows the top row, terminal, then bottom bar." The bar is drawn after the terminal, so
/// its copy fields come after it in Tab's order, the directory first. (The terminal keeps Tab itself for its program,
/// so the walk is driven by the focus coordinator's own step actions, which Tab and Option-Tab dispatch elsewhere.)
/// Specified: with the open session's terminal focused, the next control is the directory field, the one after it the
/// command field, and stepping back twice returns to the terminal.
#[gpui::test]
fn the_copy_fields_follow_the_terminal_in_tab_order(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(cx, snapshot(&[active("a", 120)], Vec::new()));
    assert!(
        test_support::terminal_focused(cx),
        "the premise: the terminal is home"
    );
    cx.dispatch_action(FocusNextControl);
    assert_eq!(test_support::focused_id(cx).as_deref(), Some(DIRECTORY_ID));
    cx.dispatch_action(FocusNextControl);
    assert_eq!(test_support::focused_id(cx).as_deref(), Some(COMMAND_ID));
    cx.dispatch_action(FocusPreviousControl);
    cx.dispatch_action(FocusPreviousControl);
    let _ = frame(cx);
    assert!(test_support::terminal_focused(cx), "back on the terminal");
}

/// A confirmation belongs to the open session: the old view was keyed by the session, so choosing another one started
/// its bar afresh, and a "✓ Copied" carried over would claim a copy the user never made of that session's value.
/// Specified: after a click on a's directory, choosing b shows b's directory, not `✓ Copied`, and choosing a again
/// within the 1.5 seconds shows a's directory too.
#[gpui::test]
fn a_confirmation_does_not_follow_another_selection(cx: &mut TestAppContext) {
    let (_fake, cx) = open_main(
        cx,
        snapshot(&[active("a", 120), active("b", 120)], Vec::new()),
    );
    test_support::click(cx, DIRECTORY_ID);
    assert_eq!(
        painted_within(&frame(cx), DIRECTORY_ID),
        [COPIED],
        "the premise"
    );
    test_support::click(cx, "session-row-b");
    assert_eq!(painted_within(&frame(cx), DIRECTORY_ID), ["~/src/b"]);
    test_support::click(cx, "session-row-a");
    assert_eq!(painted_within(&frame(cx), DIRECTORY_ID), ["~/src/a"]);
}

/// The old bar's directory field showed `(empty)` for an empty path and `(whitespace only: N characters)` for a blank
/// one (its `display_peer` markers), so a field never shows nothing; an empty field would be a button a few pixels wide
/// with no name. The command line takes the same markers here (the old one showed nothing). The copy keeps the raw
/// value. Specified: a session recorded with an empty directory and a command of three spaces paints and names
/// `(empty)` and `(whitespace only: 3 characters)`, and a click on the command copies the three spaces.
#[gpui::test]
fn an_empty_or_blank_value_shows_the_old_marker_and_copies_the_raw_bytes(cx: &mut TestAppContext) {
    let row = active("a", 120).cwd("").invocation("   ");
    let (_fake, cx) = open_main(cx, snapshot(&[row], Vec::new()));
    let nodes = frame(cx);
    assert_eq!(painted_within(&nodes, DIRECTORY_ID), ["(empty)"]);
    assert_eq!(
        painted_within(&nodes, COMMAND_ID),
        ["(whitespace only: 3 characters)"]
    );
    assert_eq!(label(cx, DIRECTORY_ID).as_deref(), Some("(empty)"));
    assert_eq!(
        label(cx, COMMAND_ID).as_deref(),
        Some("(whitespace only: 3 characters)")
    );
    test_support::click(cx, COMMAND_ID);
    assert_eq!(clipboard(cx).as_deref(), Some("   "));
}

/// The old managed-checkout mark explained a checkout in a disposable environment differently: Delete discards it
/// rather than moving it to the trash, and the environment's end removes it with every unpushed change. A user reading
/// the ordinary wording there would expect the trash to keep their work. Specified: for a session on a host that is
/// session-owned and that the helm describes as an environment, the mark's help is exactly the old environment
/// wording; once the helm pushes the same host, still session-owned, without the environment description, the help is
/// the ordinary wording, so both halves of the rule are needed.
#[gpui::test]
fn the_checkout_mark_explains_an_environments_checkout(cx: &mut TestAppContext) {
    use farhelm_proto::ui_channel::{Collection, CollectionChanges, HostView, KeyedEntry};
    use farhelm_proto::{EnvironmentHostView, LaunchTarget, Place};
    let mut registry = hosts(&[local(), remote(2, "buildbox")]);
    let entry = &mut registry.entries[1];
    let mut view: HostView = serde_json::from_value(entry.value.clone()).unwrap();
    assert_eq!(view.id, 2, "the premise: the session's host");
    view.is_session_owned = true;
    view.environment = Some(EnvironmentHostView {
        name: "sandbox-1".into(),
        place_name: None,
        target: LaunchTarget::Environment {
            recipe: "default".into(),
            place: Place::TensorlakeAccount,
        },
        tunnels: None,
        credential_warning: None,
    });
    entry.value = serde_json::to_value(&view).unwrap();
    let meta = registry.meta.clone();
    let row = active("a", 120).cwd("/home/user/src/widgets");
    let (mut fake, cx) = open_main(cx, snapshot(&[row], vec![registry]));
    assert_eq!(
        hover(cx, CHECKOUT_ID).as_deref(),
        Some(
            "Managed checkout of octo/widgets at /home/user/src/widgets. Delete discards this checkout with its \
             session, without moving it to the trash, unless another session still uses it. Deleting the \
             environment's last session, or the whole environment, permanently removes every checkout and unpushed \
             change in it."
        )
    );

    view.environment = None;
    fake.push(CollectionChanges {
        collection: Collection::Hosts,
        meta,
        upsert: vec![KeyedEntry {
            key: HostView::key(2),
            value: serde_json::to_value(&view).unwrap(),
        }],
        remove: Vec::new(),
    });
    test_support::settle(cx);
    assert_eq!(
        hover(cx, CHECKOUT_ID).as_deref(),
        Some(
            "Managed checkout of octo/widgets at /home/user/src/widgets. Farhelm made the checkout folder for its \
             first session. When the last session using it is deleted, the checkout moves to the trash."
        )
    );
}

/// The warning opens upward from the bar inside the bar's own width, where the old one was capped at 80% of the
/// window (a recorded deviation): in a narrow pane it must wrap onto more lines rather than run past the window's
/// edge, since it is the one thing such a copy must not leave unsaid. The pane is drawn alone, so it takes the whole
/// window. Specified: after copying a command with hidden characters in a wide window, the warning is drawn; once the
/// window is 220 pixels wide, the warning is still drawn, inside the window and unclipped, narrower and taller than it
/// was, its words wrapped onto more lines.
#[gpui::test]
fn the_warning_wraps_inside_a_narrow_window(cx: &mut TestAppContext) {
    let (mut fake, _connection) = started(cx);
    fake.connect(snapshot(
        &[active("a", 120).invocation(HIDDEN_COMMAND)],
        Vec::new(),
    ));
    cx.run_until_parked();
    let (_view, cx) = test_support::open_view(cx, PaneOnly::new);
    test_support::click(cx, COMMAND_ID);
    let wide = node(&frame(cx), WARNING_ID)
        .expect("the premise: the warning is drawn")
        .bounds;
    cx.simulate_resize(size(px(220.), px(500.)));
    test_support::settle(cx);
    let nodes = frame(cx);
    let warning = node(&nodes, WARNING_ID).expect("the warning is still drawn");
    assert!(
        warning.bounds.left() >= px(0.) && warning.bounds.right() <= px(220.),
        "inside the window: {:?}",
        warning.bounds
    );
    assert_eq!(warning.visible_bounds, warning.bounds, "not clipped");
    assert!(
        warning.bounds.size.width < wide.size.width
            && warning.bounds.size.height > wide.size.height,
        "wrapped: {:?} against {wide:?}",
        warning.bounds
    );
}

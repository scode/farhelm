//! Golden scenes for the session list and the sidebar around it, a row's overlays (hover help, the summary card, the
//! menus and the Delete question) and the Rename dialog. The scene list itself, `SCENES` with each scene's name and
//! size, stays in `mod.rs`.

use std::rc::Rc;

use crate::focus::Mark;
use crate::layers;
use crate::operations::OpKey;
use crate::session_list::SessionList;
use crate::session_list::filters::Status;
use crate::session_list::marking::Gesture;
use crate::session_list::tests::{TEST_NOW, listing, preferences, preferences_state, session};
use crate::theme;
use crate::{local_state, session, ticker};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, ListCounts, Preferences,
    SessionGroup,
};
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Pixels, Styled, Window,
    div, point, px,
};

use super::{
    AfterDrawn, SceneState, defer_after, fleet, rest_pointer_at, session_at_age,
    session_list_scene, session_list_scene_opening, session_list_scene_preferring,
    session_list_scene_with_listing, session_list_then, session_lists_at,
};

/// A fleet with every mark and line a row draws differently: running (selected, so its inverse colors are pinned),
/// waiting, idle with new output, idle with nothing new, running on a host that is not connected (the grey mark, the
/// dimmed title and the cause), and exited with no code and a stop annotation on a line of its own. Their ages run from
/// `now` to weeks, counted from the test clock's start, so every bucket's width is in the picture.
pub(super) fn session_list_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_scene(state, &fleet(), cx);
}

/// Show the real inline question on the Codex row. The sidebar width pins
/// the wrapping and order of the new guarded answer beside the YOLO answers.
pub(super) fn session_list_guarded_replace_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::session_list::replace_yolo::{Surface, ask};

    session_list_then(
        state,
        |_, _, window, cx| {
            let mark = Mark::take(cx);
            let window = window.window_handle();
            // The refusal callback reopens the window once this draw's update
            // has returned it to the app.
            cx.defer(move |cx| {
                assert!(ask(
                    "b",
                    DeleteGuard::Unconditional,
                    Surface::Row,
                    mark,
                    window,
                    cx
                ));
            });
        },
        cx,
    );
}

/// Keep the usual sidebar width so the answers wrap. The taller list leaves room for the full question even when
/// the fleet's host notice takes up the top of the sidebar.
pub(super) fn session_list_guarded_replace(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    defer_after(state, window, cx);
    session_lists_at(state, px(596.), cx)
}

/// Rows on a host the helm cannot reach, the open one among them, so the selected row's inverse inks are pinned for a
/// stale row too: its title in `--on-accent-dim` rather than `--on-fill`, and its cause in the selected row's secondary
/// ink. Beside it, a stale exited row shows its ended words and its cause on one line, and an ordinary row on a
/// connected host stays as it is.
pub(super) fn session_list_selected_stale_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let rows = [
        session_at_age("g", "Refactor the API client", 2 * 60).cwd("/home/user/src/api"),
        session_at_age("f", "Benchmark on the build box", 3 * 3_600)
            .host(3)
            .stale()
            .cwd("/home/user/bench"),
        session_at_age("h", "Profile the release build", 2 * 86_400)
            .host(3)
            .stale()
            .status(serde_json::json!({ "state": "exited", "exit_code": 1 }))
            .cwd("/home/user/bench"),
    ];
    session_list_scene_opening(state, &rows, Some("f"), cx);
}

/// The host rail with every host status it draws, which no reference screen shows, so it is checked against the old
/// stylesheet's `.host-rail-state`, `.host-rail-update`, `.host-rail-attention`, progress-ring and
/// `.hosts-toggle-update` rules: the local host connected (a green dot); a remote on an older build (the amber `↑`); a
/// remote refused for a lower protocol (a red dot and the `↑`, which turns the Hosts control's `↑` red); an unreachable
/// remote (its mark dimmed, an amber dot); a remote retrying a failed Update, chosen as the host filter so its ring
/// is drawn in the pressed avatar's inverse ink while its attention row keeps the amber edge; and a remote whose
/// Update failed while Hosts was closed (the warning mark at the lower left).
pub(super) fn host_rail_status_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::progress::tests::step;
    use crate::hosts::tests::{aliased, connected, host, hosts as registry, progress, run, skew};
    use crate::session_list::host_rail::tests::{old_build, unreachable};
    use farhelm_proto::provisioning::ProvisioningOperation::Update;
    use farhelm_proto::provisioning::{
        RunStatus::{Failed, Running},
        StepStatus,
    };
    let remote = |id, alias: &str, state| {
        aliased(
            host(id, "ssh", state),
            alias,
            &format!("ops@{alias}.example.invalid"),
        )
    };
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(&[], &[]),
        preferences(&[], None),
        registry(&[
            host(1, "local", connected()),
            remote(2, "older", old_build()),
            remote(3, "skewed", skew(28, 29)),
            remote(4, "down", unreachable()),
            remote(5, "busy", connected()),
            remote(6, "failed", connected()),
        ]),
        progress(&[run(5, Update, Failed), run(6, Update, Failed)]),
    ]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    // Chosen once the list has drawn under its UI session (a choice before that has no session to belong to); it shows
    // pressed at once, and the helm's answer to its query is not needed for the rail.
    state.after.set(Some(|list, state, _, cx| {
        // Allow the failed run to latch its warning before the new progress takes over the row.
        let mut retry = run(5, Update, Running);
        retry.run_id = Some("run-5-retry".into());
        retry.steps = vec![
            step("Read the installed version", StepStatus::Completed),
            step("Install the update", StepStatus::Running),
        ];
        let changed = progress(&[retry, run(6, Update, Failed)]);
        state
            .keep
            .borrow_mut()
            .iter_mut()
            .find_map(|item| item.downcast_mut::<FakeHelm>())
            .expect("the scene's helm")
            .push(CollectionChanges {
                collection: changed.collection,
                meta: changed.meta,
                upsert: changed.entries,
                remove: Vec::new(),
            });
        list.update(cx, |list, cx| list.choose_host(Some(5), cx));
    }));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// The list in compact density (reference `session-list-compact`): every row one line, with no last line and no ended
/// line. The open row runs (its inverse inks pinned); then a row on a host the helm cannot reach, its clock after the
/// title; then each of the four ended marks in the status column, in its ink: a user's stop (the filled square), an
/// exit with a code, an interruption and a launch error; and last a stale exited row, whose ended mark and clock show
/// together.
pub(super) fn session_list_compact_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let exited =
        |code: serde_json::Value| serde_json::json!({ "state": "exited", "exit_code": code });
    let rows = [
        session_at_age("a", "Refactor the API client", 5).host(1),
        session_at_age("f", "Benchmark on the build box", 3_600)
            .host(3)
            .stale(),
        session_at_age("e", "Release notes for 0.40", 2 * 3_600)
            .status(exited(serde_json::Value::Null))
            .annotation("stopped by user"),
        session_at_age("g", "Run the migration", 3 * 3_600).status(exited(serde_json::json!(17))),
        session_at_age("h", "Profile the release build", 86_400)
            .status(serde_json::json!({ "state": "interrupted" })),
        session_at_age("i", "Bisect the flaky test", 2 * 86_400)
            .status(serde_json::json!({ "state": "error", "detail": "cannot launch" })),
        session_at_age("j", "Check the bench numbers", 3 * 86_400)
            .host(3)
            .stale()
            .status(exited(serde_json::json!(1))),
    ];
    let preferences = preferences_state(Preferences {
        compact: Some(true),
        visit_history: vec!["a".to_owned()],
        welcome_seen: Some(true),
        ..Preferences::default()
    });
    session_list_scene_preferring(state, &rows, preferences, Vec::new(), cx);
}

/// A list with a pinned run: two pins in their shared order ahead of the unpinned rows, which keep the helm's order,
/// the open session among the pins so the glyph's inverse ink on the selected fill is pinned too, and the heavier
/// separator with its gap between the last pin and the first unpinned row. The helm serves the rows unpinned first, so
/// the picture shows the list doing the grouping.
pub(super) fn session_list_pinned_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let row = |key, title, ago| {
        session_at_age(key, title, ago).status(serde_json::json!({ "state": "idle" }))
    };
    let rows = [
        row("u", "Fix the flaky upload test", 14 * 60).cwd("/srv/ci/uploads"),
        row("p", "Refactor the API client", 2 * 60)
            .cwd("/home/user/src/api")
            .pinned(2),
        row("v", "Benchmark the cache", 3 * 3_600).cwd("/home/user/bench"),
        row("q", "Review the install docs", 5 * 60)
            .cwd("/home/user/src/docs")
            .pinned(1),
    ];
    session_list_scene_opening(state, &rows, Some("p"), cx);
}

/// Show the guide through an expanded group's pin divider and the open-session stripe on a different, collapsed group.
pub(super) fn session_list_groups_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let row = |key, title, cwd, ago| {
        session_at_age(key, title, ago)
            .cwd(cwd)
            .status(serde_json::json!({ "state": "idle" }))
    };
    let rows = [
        row("a", "Refactor the API client", "/home/user/src/api", 120).pinned(1),
        row("b", "Review the install docs", "/home/user/src/docs", 300)
            .grouped(10)
            .pinned(2),
        row("c", "Update the examples", "/home/user/src/docs", 420).grouped(10),
        row("d", "Fix the flaky upload test", "/srv/ci/uploads", 840).grouped(20),
        row("e", "Benchmark the cache", "/home/user/bench", 3 * 3_600),
    ];
    let mut sessions = listing(&["a", "b", "c", "d", "e"], &rows);
    sessions.meta.session_groups = vec![
        SessionGroup {
            id: 10,
            name: "Documentation".to_owned(),
            position: 1,
            collapsed: false,
            members: 2,
        },
        SessionGroup {
            id: 20,
            name: "CI".to_owned(),
            position: 2,
            collapsed: true,
            members: 1,
        },
    ];
    session_list_scene_with_listing(state, sessions, preferences(&["d"], None), Vec::new(), cx);
}

/// The fleet with two rows marked as a Cmd-click marks them: the open row, which keeps its accent fill under a marker
/// in the fill's ink, and an unselected row, tinted under an accent marker. The heading's band is the marked bar, "2
/// marked" with Stop, Delete, the marked menu's ⋯ and Clear, in place of the title, the count, Trash, Templates and
/// New.
pub(super) fn session_list_marked_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, _, cx| {
            list.update(cx, |list, cx| {
                for key in ["a", "c"] {
                    list.row_clicked(key, Gesture::Toggle, cx);
                }
            })
        },
        cx,
    );
}

// ---- Help beside the rail and sidebar -------------------------------------------------------------------------------

/// The selected All hosts entry has its own notch and inverse ring. Leaving the session rows in the picture
/// makes it clear the card follows the rail edge inside the wider sidebar.
pub(super) fn host_rail_hover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    session_list_then(state, rest_on_all_hosts, cx);
}

/// Warm up hover help once the All hosts entry has drawn.
fn rest_on_all_hosts(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    if rest_pointer_on(
        state,
        "host-rail-all",
        gpui::Bounds::center,
        rest_on_all_hosts,
        window,
        cx,
    ) {
        state.after.set(Some(|_, _, window, cx| {
            let shown = layers::shown_tooltip(window, cx).expect("All hosts shows help");
            assert_eq!(
                shown.text.as_ref(),
                "All hosts: show sessions from every host"
            );
            assert_eq!(shown.side(), layers::TooltipSide::Beside);
            assert!(shown.ring().is_some(), "the rail entry carries its ring");
        }));
        window.refresh();
    }
}

/// The width of the session-row hover scenes' content: the sidebar's column, then room for the 310-pixel summary card 6
/// pixels past the column's edge and 4 inside the window's, which beside placement needs or it falls back above the row.
const BESIDE_SCENE_WIDTH: f32 = 776.;

/// [`session_lists_then`] in a sidebar column the way the sidebar draws it for hover help: the list on the sidebar's
/// surface with its 1-pixel edge, and the edge that beside placement sits against ([`layers::beside_edge`]) drawn on a
/// box around that border, with room to the column's right for the help.
pub(super) fn session_lists_beside(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    defer_after(state, window, cx);
    let palette = theme::palette(cx);
    let column = div()
        .w(theme::metrics::SIDEBAR_WIDTH)
        .h_full()
        .bg(theme::color(palette.bg_1))
        .border_r_1()
        .border_color(theme::color(palette.border_sidebar_edge))
        .text_color(theme::color(palette.fg_0))
        .children(state.lists.borrow().iter().cloned());
    div()
        .w(px(BESIDE_SCENE_WIDTH))
        .h(px(394.))
        .flex()
        .child(
            div()
                .relative()
                .h_full()
                .child(column)
                .child(layers::beside_edge("golden-sidebar-help-edge", window, cx)),
        )
        .into_any_element()
}

/// The idle row's age, its hover help beside the sidebar with a notch pointing to the age (reference
/// `tooltip-last-activity`). The setup turns on GPUI's driver record, which [`rest_pointer_on`] finds the age in.
pub(super) fn session_row_hover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    session_list_then(state, rest_on_idle_age, cx);
}

/// A muted waiting dot has its own beside help without a ring, and the Unmute bell stays after the title.
pub(super) fn muted_waiting_dot_hover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    let [selected, waiting, ..] = fleet();
    session_list_scene(state, &[selected, waiting.muted(None)], cx);
    state.after.set(Some(rest_on_muted_waiting_dot));
}

/// Pick the noninteractive status dot so the capture proves which target owns this help.
fn rest_on_muted_waiting_dot(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    rest_pointer_on(
        state,
        "session-seen-b",
        gpui::Bounds::center,
        rest_on_muted_waiting_dot,
        window,
        cx,
    );
}

/// [`rest_pointer_on`] row `c`'s age.
fn rest_on_idle_age(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    rest_pointer_on(
        state,
        "session-row-c-age",
        gpui::Bounds::center,
        rest_on_idle_age,
        window,
        cx,
    );
}

/// The summary card of the idle local row `Update dependencies`, beside the sidebar with the notch and the accent ring
/// around the row's open control (reference `tooltip-session-summary`): its status, its Cursor launch, the local host,
/// the managed checkout, the directory and the two times. Shown from the row's last line, an unmarked part of the row,
/// as the old e2e case reached it from the open area's lower part. The row was created a day before it was last active,
/// so its Started is a real time: the shared fleet's rows keep the helm fixture's creation stamp of 100 seconds, which
/// the card would show as a moment in 1970.
pub(super) fn session_summary_card_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    let rows = fleet().map(|row| {
        if row.key == "d" {
            row.created(TEST_NOW - 3 * 3_600 - 86_400)
        } else {
            row
        }
    });
    session_list_scene(state, &rows, cx);
    state.after.set(Some(rest_on_idle_row));
}

/// [`rest_pointer_on`] row `d`'s open control, on its last line.
fn rest_on_idle_row(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    rest_pointer_on(
        state,
        "session-row-d",
        |bounds| point(bounds.center().x, bounds.bottom() - px(9.)),
        rest_on_idle_row,
        window,
        cx,
    );
}

/// [`rest_pointer_at`] `at` of the bounds of the element the last frame drew under `id`, so its hover help shows
/// through the row's own wiring. Returns whether it dispatched the hover. If the store has yet to draw the
/// element, the next frame calls `retry` for it.
fn rest_pointer_on(
    state: &SceneState,
    id: &str,
    at: fn(&gpui::Bounds<Pixels>) -> gpui::Point<Pixels>,
    retry: AfterDrawn,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some(target) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| at(&node.bounds))
    else {
        state.after.set(Some(retry));
        return false;
    };
    rest_pointer_at(state, target, window, cx);
    true
}

/// The waiting row's actions menu, open beside its button with Rename, its first command, highlighted (reference
/// `session-row-menu`).
pub(super) fn session_row_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| list.update(cx, |list, cx| list.toggle_menu("b", window, cx)),
        cx,
    );
}

/// The open running session's menu fits both hints at the standard panel width, with Rename highlighted and Delete
/// in danger ink. On Linux this pictures the full Ctrl+Shift+Backspace label.
pub(super) fn open_session_row_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| {
            assert_eq!(local_state::selected(cx).as_deref(), Some("a"));
            list.update(cx, |list, cx| list.toggle_menu("a", window, cx));
        },
        cx,
    );
}

/// The list's order menu, open below its title as a click opens it (reference `sort-menu`): the title in its open look,
/// the shared header ("Session list"), the three orders with Recently active, the stored default, checked and
/// highlighted, and Compact, unchecked, after a separator.
pub(super) fn session_order_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| list.update(cx, |list, cx| list.toggle_order_menu(window, cx)),
        cx,
    );
}

/// The waiting row's Delete question, asked from its row as the menu's Delete asks it (reference
/// `row-delete-confirmation`): its tab, its running agent, its quoted title and its managed checkout. The
/// `anchored-prompt` golden pins the prompt's own look with made-up wording; this one pins the row's wording and where
/// it sits beside the list.
pub(super) fn row_delete_confirmation_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| {
            let mark = Mark::take(cx);
            if let Some(target) = crate::session_list::menu::actionable("b", cx) {
                list.update(cx, |list, cx| list.delete(&target, &mark, window, cx));
            }
        },
        cx,
    );
}

/// A connected sandbox can keep another session. The selected shell's Delete still warns that its processes and
/// tabs close before it explains when the entire sandbox goes away.
pub(super) fn environment_delete_confirmation_setup(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) {
    use crate::session_list::environment_tests::{
        Provider, bash_session, connected, environment_host, registry,
    };
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(
            &["a", "b"],
            &[
                bash_session("a")
                    .host(2)
                    .tabs(1)
                    .activity(TEST_NOW - 60)
                    .title("Check the release build"),
                session("b")
                    .host(2)
                    .activity(TEST_NOW - 120)
                    .title("Watch integration tests"),
            ],
        ),
        preferences(&["a"], None),
        registry(&environment_host(Provider::Sandbox, connected())),
    ]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    state.after.set(Some(|list, _, window, cx| {
        let mark = Mark::take(cx);
        if let Some(target) = crate::session_list::menu::actionable("a", cx) {
            list.update(cx, |list, cx| list.delete(&target, &mark, window, cx));
        }
    }));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// An exited shell's Stop still closes its extra tab. Hover confirms that the answer describes only that remaining tab.
pub(super) fn tab_only_stop_confirmation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::session_list::environment_tests::bash_session;
    window.set_driver_enabled(true);
    let fixture = bash_session("a")
        .host(1)
        .tabs(1)
        .activity(TEST_NOW - 60)
        .title("Build console")
        .status(serde_json::json!({"state": "exited", "exit_code": 0}));
    let row = serde_json::from_value(fixture.entry().value).expect("the shell's listed row");
    session_list_scene(state, &[fixture], cx);
    let question = crate::session_list::stop::question(
        "a",
        &row,
        crate::session_list::stop::guard(&row),
        None,
    );
    let mark = Mark::take(cx);
    assert!(crate::prompt::ask(question, None, &mark, |_, _, _| {}, window, cx).is_some());
    state.after.set(Some(rest_on_terminal_tabs_stop));
}

/// Wait for Stop to produce its answer control before moving the pointer onto the actual tooltip target.
fn rest_on_terminal_tabs_stop(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    rest_pointer_on(
        state,
        "prompt-confirm",
        |bounds| bounds.center(),
        rest_on_terminal_tabs_stop,
        window,
        cx,
    );
}

/// Show the extra answer for an exited shell in its own container. The note and the answer's hover both describe
/// the tabs still open and the remaining environment processes.
pub(super) fn environment_tab_only_stop_confirmation_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::session_list::environment_tests::{
        Provider, bash_session, connected, environment_host, registry,
    };
    let host = environment_host(Provider::Container, connected());
    let fixture = bash_session("a")
        .host(host.id)
        .tabs(1)
        .activity(TEST_NOW - 60)
        .title("Build console")
        .status(serde_json::json!({"state": "exited", "exit_code": 0}));
    let row = serde_json::from_value(fixture.entry().value).expect("the shell's listed row");
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(&["a"], &[fixture]),
        preferences(&["a"], None),
        registry(&host),
    ]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    window.set_driver_enabled(true);
    // Use the same frozen Stop question as the row menu, including the extra answer for its current host.
    let question = crate::session_list::stop::question(
        "a",
        &row,
        crate::session_list::stop::guard(&row),
        Some((
            crate::environment_ui::ProviderUi(farhelm_proto::ProviderKind::Podman),
            host.id,
        )),
    );
    let mark = Mark::take(cx);
    assert!(crate::prompt::ask(question, None, &mark, |_, _, _| {}, window, cx).is_some());
    // The golden runner waits for visible bounds before hovering the alternate answer.
    state.hover.set(Some("prompt-alternative-0"));
}

/// Show the Stop confirmation while a create still owns this window's destination. Its alternate answer needs to
/// explain the wait without making ordinary Stop look unavailable.
pub(super) fn stop_while_launching_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::session_list::environment_tests::{Provider, connected, environment_host, registry};

    window.set_driver_enabled(true);
    let host = environment_host(Provider::Sandbox, connected());
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(&["a"], &[stop_while_launching_session()]),
        preferences(&["a"], None),
        registry(&host),
    ]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    state.after.set(Some(open_launch_for_stop));
    state.after_capture.set(Some(disconnect_pending_launch));
}

/// Let the launcher and operation store release the pending create after the screenshot. Its reply must stay out
/// until then so the launch still owns the host in the captured frame.
pub(super) fn disconnect_pending_launch(state: &SceneState) {
    state
        .keep
        .borrow_mut()
        .iter_mut()
        .find_map(|item| item.downcast_mut::<FakeHelm>())
        .expect("the scene keeps its fake helm")
        .break_channel();
}

/// Show the deadline notice in the open session and a separate Stop failure in the list. The sandbox still
/// appears connected, which is the part of Stop and suspend in which there would otherwise be no feedback.
pub(super) fn stop_and_suspend_outcomes_setup(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::host_text::HostText;
    use crate::operations::Work;
    use crate::session_list::environment_tests::{
        Provider, STOP_AND_SUSPEND_NOTICE, bash_session, connected, environment_host, registry,
    };
    use crate::session_pane::SessionPane;

    window.set_driver_enabled(true);
    let rows = [
        bash_session("a")
            .host(2)
            .title("Build console")
            .activity(TEST_NOW - 30)
            .invocation("bash"),
        bash_session("b")
            .host(2)
            .title("Package release")
            .activity(TEST_NOW - 60)
            .invocation("bash"),
    ];
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let keep = Rc::downgrade(&state.keep);
    connection.update(cx, |connection, _| {
        connection.serve_terminals(move |attach| {
            super::terminal::replayed_stream(None, keep.upgrade().unwrap(), |_| {})(attach)
        });
    });
    helm.connect(vec![
        listing(&["a", "b"], &rows),
        preferences(&["a"], None),
        registry(&environment_host(Provider::Sandbox, connected())),
    ]);
    let list = cx.new(SessionList::new);
    let new = list.read(cx).new_focus().clone();
    let pane = cx.new(|cx| SessionPane::new(new, window, cx));
    state.lists.borrow_mut().push(list);
    state.keep.borrow_mut().push(Box::new(pane));
    state.after.set(Some(await_stop_and_suspend_outcomes));
    super::once_ui_session(state, helm, connection, cx, |cx| {
        session::current_operations(cx)
            .expect("the operation store is live")
            .update(cx, |operations, cx| {
                operations.notice(
                    &OpKey::session("a"),
                    2,
                    HostText::from_host(STOP_AND_SUSPEND_NOTICE),
                    cx,
                );
                operations.note(
                    &OpKey::session("b"),
                    Work::StopAndSuspend,
                    HostText::from_host("The supervisor did not finish stopping the session."),
                    cx,
                );
            });
    });
}

/// A settled background alone could pass the pixel check before the fake helm's snapshot. Wait for the actual
/// notice and failure labels in the sidebar and open pane.
fn await_stop_and_suspend_outcomes(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    _: &mut App,
) {
    let nodes = window.driver_nodes();
    if ![
        "session-outcome-a",
        "session-outcome-b",
        "session-header-outcome-a",
    ]
    .into_iter()
    .all(|id| {
        nodes
            .iter()
            .any(|node| node.element_id.as_deref() == Some(id) && !node.visible_bounds.is_empty())
    }) {
        state.after.set(Some(await_stop_and_suspend_outcomes));
    }
}

/// Keep a sidebar-width column beside the open pane to exercise the same wrap as the desktop.
pub(super) fn stop_and_suspend_outcomes(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use crate::session_pane::SessionPane;

    let Some(pane) = state
        .keep
        .borrow()
        .iter()
        .find_map(|item| item.downcast_ref::<Entity<SessionPane>>().cloned())
    else {
        // The first frame precedes the scene setup.
        return div().into_any_element();
    };
    defer_after(state, window, cx);
    div()
        .flex()
        .h(px(370.))
        .child(super::session_lists_at(state, px(370.), cx))
        .child(div().w(px(660.)).min_w_0().h_full().child(pane))
        .into_any_element()
}

/// The stopped session and the pending create share the same host.
fn stop_while_launching_session() -> crate::session_list::tests::Fixture {
    crate::session_list::environment_tests::bash_session("a")
        .host(2)
        .tabs(1)
        .title("Build console")
}

/// Open the normal launcher after the fake helm's host list is readable.
fn open_launch_for_stop(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::launcher::{launch_tests::command_draft, view};
    use farhelm_proto::ui_channel::HostView;
    if !session::current_store(cx).is_some_and(|store| {
        store
            .read(cx)
            .collections()
            .get::<HostView>(&HostView::key(2))
            .is_some_and(|host| {
                host.is_session_owned
                    && host.environment.is_some()
                    && matches!(
                        host.state,
                        farhelm_proto::ui_channel::HostStateView::Connected { .. }
                    )
            })
    }) {
        state.after.set(Some(open_launch_for_stop));
        return;
    }
    let mut draft = command_draft();
    draft.choose_host(2, "~".into());
    draft.edit_folder("/home/user/work".into());
    let mark = Mark::take(cx);
    assert!(view::open(draft, &mark, window, cx).is_some());
    state.after.set(Some(press_launch_for_stop));
}

/// Begin a create and leave its reply outstanding, so the question sees the real launch guard.
fn press_launch_for_stop(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(center) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some("launcher-launch") && node.flags.click)
        .map(|node| node.bounds.center())
    else {
        state.after.set(Some(press_launch_for_stop));
        return;
    };
    crate::test_support::click_before_redraw(center, window, cx);
    state.after.set(Some(show_stop_while_launching));
}

/// Draw the actual Stop question and rest on the held answer once its button exists.
fn show_stop_while_launching(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    assert!(crate::launcher::launch::host_lifecycle_refusal(2, cx).is_some());
    let row = serde_json::from_value(stop_while_launching_session().entry().value)
        .expect("the listed shell session");
    let question = crate::session_list::stop::question(
        "a",
        &row,
        crate::session_list::stop::guard(&row),
        Some((
            crate::environment_ui::ProviderUi(farhelm_proto::ProviderKind::Tensorlake),
            2,
        )),
    );
    let mark = Mark::take(cx);
    assert!(crate::prompt::ask(question, None, &mark, |_, _, _| {}, window, cx).is_some());
    state.after.set(Some(rest_on_held_stop_and_suspend));
}

/// Let the same disabled button that users hover supply its tooltip in the capture.
fn rest_on_held_stop_and_suspend(
    _: &Entity<SessionList>,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) {
    rest_pointer_on(
        state,
        "prompt-alternative-0",
        |bounds| bounds.center(),
        rest_on_held_stop_and_suspend,
        window,
        cx,
    );
}

/// A pending Delete on the selected row (its spinner and "Stopping agent…" in place of the title, in the fill's ink)
/// and a refused one on the idle row with new output (the helm's words under the row, in red). Motion is reduced, so
/// the spinner holds its first step and the picture can settle. The refusal is answered before the pending Delete is
/// sent, because the fake answers in order.
pub(super) fn session_list_deleting_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
    session_list_then(state, stage_deleting_rows, cx);
}

/// The preference change can redraw the list before the fake helm's snapshot arrives. Keep this step pending
/// until the snapshot's operation store is live, so the capture cannot settle on a list with neither Delete.
fn stage_deleting_rows(_: &Entity<SessionList>, state: &SceneState, _: &mut Window, cx: &mut App) {
    let Some(operations) = session::current_operations(cx) else {
        state.after.set(Some(stage_deleting_rows));
        return;
    };
    let start = |key: &str, cx: &mut App| {
        let command = crate::session_list::delete::command(key, DeleteGuard::Unconditional)
            .expect("the fixture can build the Delete command");
        operations
            .update(cx, |operations, cx| {
                operations.start(&OpKey::session(key), command, cx, |_, _| {})
            })
            .expect("the fixture can start the Delete");
    };
    start("c", cx);
    let mut keep = state.keep.borrow_mut();
    let helm = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
        .expect("the scene keeps its connected fake helm");
    let mut refused = false;
    for command in helm.commands() {
        let (status, body) = if command.path == "/api/sessions/c" {
            refused = true;
            (
                409,
                Some(serde_json::json!("the session's host is not connected")),
            )
        } else {
            (200, None)
        };
        helm.reply(command.id, status, body);
    }
    assert!(refused, "the first Delete reached the fake helm");
    drop(keep);
    start("a", cx);
}

/// A host chosen over an empty listing (reference `session-list-filter-empty`): the rail with `buildbox` pressed, its
/// chip under the heading, and the sentence naming it with "Show all hosts" under it. The fleet's rows go: the fake
/// pushes the host query's empty listing (0 of 6) and then accepts the query, answering the list's other commands
/// before it in order, as the helm does.
pub(super) fn host_filter_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| {
            list.update(cx, |list, cx| list.choose_host(Some(2), cx));
            let open = local_state::selected(cx);
            let mut keep = state.keep.borrow_mut();
            let Some(helm) = keep
                .iter_mut()
                .find_map(|kept| kept.downcast_mut::<FakeHelm>())
            else {
                return;
            };
            helm.push(listing_carrying_the_open_session(&[], open));
            for command in helm.commands() {
                helm.reply(command.id, 204, None);
            }
        },
        cx,
    );
}

/// The listing the helm pushes for a filtered query over the scenes' fleet: `shown` of its 6 in order, complete, and
/// the rest removed, except the open session, which the helm carries outside the order because the query names it.
/// Leaving the open session out would tell the list it was deleted elsewhere, and the list would move the selection
/// on, which is not what these scenes picture.
fn listing_carrying_the_open_session(shown: &[&str], open: Option<String>) -> CollectionChanges {
    let carried = open.filter(|key| !shown.contains(&key.as_str()));
    let remove = fleet()
        .iter()
        .map(|row| row.key.to_owned())
        .filter(|key| !shown.contains(&key.as_str()) && carried.as_ref() != Some(key))
        .collect();
    CollectionChanges {
        collection: Collection::Sessions,
        meta: CollectionMeta {
            availability: Availability::Available,
            order: shown.iter().map(|key| key.to_string()).collect(),
            counts: Some(ListCounts {
                total: 6,
                matching: Some(shown.len() as u64),
                truncated: false,
            }),
            selected_outside_query: carried,
            selected_cut: false,
            session_groups: Vec::new(),
        },
        upsert: Vec::new(),
        remove,
    }
}

/// The fleet narrowed to `buildbox` and Waiting, the scenes' host and status: the rail chooses the host and the
/// segments the status, the fake pushes the query's listing (`shown` of the fleet's 6, which matched as many) and then
/// accepts both queries, answering the list's other commands before them in order, as the helm does.
fn filtered_to_waiting_on_buildbox(
    list: &Entity<SessionList>,
    state: &SceneState,
    shown: &[&str],
    cx: &mut App,
) {
    list.update(cx, |list, cx| {
        list.choose_host(Some(2), cx);
        list.choose_status(Status::Waiting, cx);
    });
    let open = local_state::selected(cx);
    let mut keep = state.keep.borrow_mut();
    let Some(helm) = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
    else {
        return;
    };
    helm.push(listing_carrying_the_open_session(shown, open));
    for command in helm.commands() {
        helm.reply(command.id, 204, None);
    }
}

/// The status segments with Waiting chosen and `buildbox`'s chip beside them (reference
/// `session-list-filter-waiting`), over the fleet narrowed to its one waiting row: the small segments in their well
/// across the column, the chosen segment in the accent fill, and the chip after them in the same wrapping row.
pub(super) fn session_status_segments_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| filtered_to_waiting_on_buildbox(list, state, &["b"], cx),
        cx,
    );
}

/// An empty answer to a host and a status (reference `session-list-filter-empty`): "No waiting sessions on" the host's
/// mark and name, and both links under it, Show all hosts first.
pub(super) fn filter_empty_both_links_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| filtered_to_waiting_on_buildbox(list, state, &[], cx),
        cx,
    );
}

/// No sessions at all: the list says so.
pub(super) fn session_list_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_scene(state, &[], cx);
}

/// The Rename editor over a session the fake helm lists, as the shortcut or a row's menu opens it (reference
/// `rename-dialog`): the current title, the field holding it with focus, and Cancel and Save. The fake and the
/// connection are kept for the scene's life, like the session list's.
pub(super) fn rename_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        listing(&["a"], &[session("a").title("bench tokenizer")]),
        preferences(&["a"], None),
    ]);
    // The snapshot reaches the store from the connection's pump, on the executor, after this setup returns; the
    // editor needs it to seed its draft, so it opens once the connection has a UI session, and only once.
    let handle = window.window_handle();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::session_list::rename::open("a", &mark, window, cx);
        });
    });
}

/// [`session_lists_then`] in a column tall enough for the host status scene's six avatars above the rail's foot,
/// which the ordinary column would cover from the third avatar down.
pub(super) fn host_rail_lists(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    defer_after(state, window, cx);
    session_lists_at(state, px(676.), cx)
}

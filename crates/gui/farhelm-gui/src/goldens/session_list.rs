//! Golden scenes for the session list and the sidebar around it, a row's overlays (hover help, the summary card, the
//! menus and the Delete question) and the Rename dialog. The scene list itself, `SCENES` with each scene's name and
//! size, stays in `mod.rs`.

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
};
use gpui::{
    AnyElement, App, AppContext as _, Entity, IntoElement, ParentElement, Pixels, Styled, Window,
    div, point, px,
};

use super::{
    AfterDrawn, SceneState, defer_after, fleet, session_list_scene, session_list_scene_opening,
    session_list_scene_preferring, session_list_then, session_lists_at,
};

/// A fleet with every mark and line a row draws differently: running (selected, so its inverse colors are pinned),
/// waiting, idle with new output, idle with nothing new, running on a host that is not connected (the grey mark, the
/// dimmed title and the cause), and exited with no code and a stop annotation on a line of its own. Their ages run from
/// `now` to weeks, counted from the test clock's start, so every bucket's width is in the picture.
pub(super) fn session_list_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_scene(state, &fleet(), cx);
}

/// Rows on a host the helm cannot reach, the open one among them, so the selected row's inverse inks are pinned for a
/// stale row too: its title in `--on-accent-dim` rather than `--on-fill`, and its cause in the selected row's secondary
/// ink. Beside it, a stale exited row shows its ended words and its cause on one line, and an ordinary row on a
/// connected host stays as it is.
pub(super) fn session_list_selected_stale_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let row = |key, title, ago: i64| {
        session(key)
            .title(title)
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
    };
    let rows = [
        row("g", "Refactor the API client", 2 * 60).cwd("/home/user/src/api"),
        row("f", "Benchmark on the build box", 3 * 3_600)
            .host(3)
            .stale()
            .cwd("/home/user/bench"),
        row("h", "Profile the release build", 2 * 86_400)
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
/// remote (its mark dimmed, an amber dot); a remote with an Update run going on (the dashed ring), chosen as the
/// host filter so its ring is drawn in the pressed avatar's inverse ink; and a remote whose Update failed while Hosts
/// was closed (the warning mark at the lower left).
pub(super) fn host_rail_status_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{aliased, connected, host, hosts as registry, progress, run, skew};
    use crate::session_list::host_rail::tests::{old_build, unreachable};
    use farhelm_proto::provisioning::ProvisioningOperation::Update;
    use farhelm_proto::provisioning::RunStatus::{Failed, Running};
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
        progress(&[run(5, Update, Running), run(6, Update, Failed)]),
    ]);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    // Chosen once the list has drawn under its UI session (a choice before that has no session to belong to); it shows
    // pressed at once, and the helm's answer to its query is not needed for the rail.
    state.after.set(Some(|list, _, _, cx| {
        list.update(cx, |list, cx| list.choose_host(Some(5), cx))
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
    let row = |key, title, ago: i64| {
        session(key)
            .title(title)
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
    };
    let exited =
        |code: serde_json::Value| serde_json::json!({ "state": "exited", "exit_code": code });
    let rows = [
        row("a", "Refactor the API client", 5).host(1),
        row("f", "Benchmark on the build box", 3_600)
            .host(3)
            .stale(),
        row("e", "Release notes for 0.40", 2 * 3_600)
            .status(exited(serde_json::Value::Null))
            .annotation("stopped by user"),
        row("g", "Run the migration", 3 * 3_600).status(exited(serde_json::json!(17))),
        row("h", "Profile the release build", 86_400)
            .status(serde_json::json!({ "state": "interrupted" })),
        row("i", "Bisect the flaky test", 2 * 86_400)
            .status(serde_json::json!({ "state": "error", "detail": "cannot launch" })),
        row("j", "Check the bench numbers", 3 * 86_400)
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
    let row = |key, title, ago: i64| {
        session(key)
            .title(title)
            .status(serde_json::json!({ "state": "idle" }))
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
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

/// The fleet with two rows marked as a Cmd-click marks them: the open row, which keeps its accent fill under a marker
/// in the fill's ink, and an unselected row, tinted under an accent marker. The heading's band is the marked bar, "2
/// marked" with the marked menu's ⋯ and Clear, in place of the title, the count, Trash, Templates and New.
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

// ---- Session-row hovers beside the sidebar --------------------------------------------------------------------------

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
                .child(layers::beside_edge(window, cx)),
        )
        .into_any_element()
}

/// The idle row's age, its hover help beside the sidebar with the notch and the accent ring on the age (reference
/// `tooltip-last-activity`). The setup turns on GPUI's driver record, which [`rest_pointer_on`] finds the age in.
pub(super) fn session_row_hover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    session_list_then(state, rest_on_idle_age, cx);
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

/// Move the pointer to `at` of the bounds of the element the last frame drew under `id`, as a pointer coming to rest
/// there, so its hover help shows through the row's own wiring rather than a copy of it. A scene's frames do not wait
/// out the hover delay, so a tooltip of the scene's own is shown first: help shows at once while another shows, and
/// the control's then replaces it. Until a frame has drawn the element (the store's first snapshot reaches the list a
/// frame or more after the list first draws, see [`press_bell_b`]), this waits for the next frame as `retry`.
fn rest_pointer_on(
    state: &SceneState,
    id: &str,
    at: fn(&gpui::Bounds<Pixels>) -> gpui::Point<Pixels>,
    retry: AfterDrawn,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(target) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| at(&node.bounds))
    else {
        state.after.set(Some(retry));
        return;
    };
    layers::show_tooltip_at(
        &state.tooltip_owner,
        "Warming".into(),
        point(px(0.), px(0.)),
        window,
        cx,
    );
    window.dispatch_event(
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: target,
            pressed_button: None,
            modifiers: gpui::Modifiers::default(),
        }),
        cx,
    );
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
            list.update(cx, |list, cx| list.delete("b", &mark, window, cx));
        },
        cx,
    );
}

/// A pending Delete on the selected row (its spinner and "Stopping agent…" in place of the title, in the fill's ink)
/// and a refused one on the idle row with new output (the helm's words under the row, in red). Motion is reduced, so
/// the spinner holds its first step and the picture can settle. The refusal is answered before the pending Delete is
/// sent, because the fake answers in order.
pub(super) fn session_list_deleting_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
    session_list_then(
        state,
        |_, state, _, cx| {
            let Some(operations) = session::current_operations(cx) else {
                return;
            };
            let start = |key: &str, guard, cx: &mut App| {
                if let Ok(command) = crate::session_list::delete::command(key, guard) {
                    let _ = operations.update(cx, |operations, cx| {
                        operations.start(&OpKey::session(key), command, cx, |_, _| {})
                    });
                }
            };
            start("c", DeleteGuard::Unconditional, cx);
            let mut keep = state.keep.borrow_mut();
            if let Some(helm) = keep
                .iter_mut()
                .find_map(|kept| kept.downcast_mut::<FakeHelm>())
            {
                for command in helm.commands() {
                    let refused = command.path == "/api/sessions/c";
                    let (status, body) = if refused {
                        (
                            409,
                            Some(serde_json::json!("the session's host is not connected")),
                        )
                    } else {
                        (200, None)
                    };
                    helm.reply(command.id, status, body);
                }
            }
            drop(keep);
            start("a", DeleteGuard::Unconditional, cx);
        },
        cx,
    );
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
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::session_list::rename::open("a", &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// [`session_lists_then`] in a column tall enough for the host status scene's six avatars above the rail's foot,
/// which the ordinary column would cover from the third avatar down.
pub(super) fn host_rail_lists(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    defer_after(state, window, cx);
    session_lists_at(state, px(676.), cx)
}

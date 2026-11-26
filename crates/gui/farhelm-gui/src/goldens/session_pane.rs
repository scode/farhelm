//! Golden scenes for the open session's toolbar, details bar, and interrupted and unavailable cards. The scene list,
//! `SCENES`, stays in `mod.rs`.

use crate::session_list::SessionList;
use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::tests::{TEST_NOW, listing, preferences, session};
use crate::session_pane::SessionPane;
use crate::sidebar::{self, Sidebar};
use crate::theme;
use crate::{local_state, local_time, session, ticker};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::Collection;
use gpui::{
    AnyElement, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    Styled, Window, div, px,
};
use std::cell::RefCell;

use super::{SceneState, session_at_age};

/// Keep the fake fleet and both columns alive for each capture. The scene view owns the window's focus and layers.
struct WelcomePaneScene {
    sidebar: Entity<Sidebar>,
    pane: Entity<SessionPane>,
}

/// Mount the actual sidebar and pane over a complete empty listing. The scene records these columns without running
/// the desktop window's startup sequence or changing the palette selected by the golden harness.
pub(super) fn welcome_pane_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let fleet = welcome_scene(state, window, cx);
    state.keep.borrow_mut().push(Box::new(fleet));
}

/// Show the Help dialog over a proven empty fleet to exercise the two instances in the same frame.
pub(super) fn welcome_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (helm, connection) = welcome_scene(state, window, cx);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    super::once_ui_session(state, helm, connection, cx, move |cx| {
        handle
            .update(cx, |_, window, cx| {
                let mark = crate::focus::Mark::take(cx);
                assert!(crate::welcome::open(&opener, &mark, window, cx).is_some());
            })
            .expect("the golden window is live");
    });
}

/// Set up the same empty fleet for both Welcome surfaces. The caller owns the connection's lifetime.
fn welcome_scene(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> (FakeHelm, Entity<session::Connection>) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let list = cx.new(SessionList::new);
    let sidebar = cx.new(|cx| Sidebar::new(list, cx));
    let pane = cx.new(|cx| SessionPane::new(window, cx));
    helm.connect(vec![
        listing(&[], &[]),
        hosts(&[local()]),
        preferences(&[], None),
    ]);
    state
        .keep
        .borrow_mut()
        .push(Box::new(WelcomePaneScene { sidebar, pane }));
    (helm, connection)
}

/// Give each column the same width it receives in the desktop at these window sizes.
pub(super) fn welcome_pane(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let keep = state.keep.borrow();
    let Some(scene) = keep
        .iter()
        .find_map(|kept| kept.downcast_ref::<WelcomePaneScene>())
    else {
        return div().into_any_element();
    };
    div()
        .size_full()
        .flex()
        .child(scene.sidebar.clone())
        .child(
            div()
                .flex_1()
                .min_w(sidebar::MAIN_PANE_MIN_WIDTH)
                .h_full()
                .child(scene.pane.clone()),
        )
        .into_any_element()
}

/// Each width gets its own pane and anchors. The panes share the same snapshot, as two views of the same session would.
struct ToolbarScene {
    panes: [Entity<SessionPane>; 2],
    helm: RefCell<FakeHelm>,
}

/// Open a suspended environment session. Its Resume supplies the primary look, and a tunnel mapping makes the narrow
/// row condense through the same layout decision as the desktop app.
pub(super) fn session_toolbar_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::session_list::environment_tests::{
        Provider, environment_host, idle, registry, suspended_at,
    };
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let panes = [(); 2].map(|()| cx.new(|cx| SessionPane::new(window, cx)));
    let row = idle("a").title("Review the build").host(2).tabs(1);
    helm.connect(vec![
        listing(&["a"], &[row]),
        preferences(&["a"], Some("a")),
        registry(&environment_host(
            Provider::Container,
            suspended_at(TEST_NOW * 1000),
        )),
    ]);
    // The session list normally restores the selection. This scene mounts only the panes.
    assert!(local_state::select("a", cx), "select the toolbar's session");
    window.set_driver_enabled(true);
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(connection));
    keep.push(Box::new(ToolbarScene {
        panes,
        helm: RefCell::new(helm),
    }));
}

/// Draw the same production toolbar first with room for the tunnel and tabs, then at the 650-pixel pane floor.
pub(super) fn session_toolbar(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let keep = state.keep.borrow();
    let Some(toolbar) = keep
        .iter()
        .find_map(|kept| kept.downcast_ref::<ToolbarScene>())
    else {
        return div().into_any_element();
    };
    let mut helm = toolbar.helm.borrow_mut();
    for command in helm.commands() {
        let body = (command.path == "/api/sessions/a/tunnels").then(|| {
            serde_json::json!({
                "available": true,
                "host_name": "build-environment.example.invalid",
                "consent": true,
                "tunnels": [{ "id": "preview", "local_port": 54320, "remote_port": 5173 }],
            })
        });
        helm.reply(command.id, 200, body);
    }
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .children(
            toolbar
                .panes
                .iter()
                .zip([("ordinary-toolbar", 1400.), ("condensed-toolbar", 650.)])
                .map(|(pane, (id, width))| {
                    // Header construction obtains its keyed anchors before the wrapper enters layout.
                    let header = window.with_element_namespace(id, |window| {
                        pane.update(cx, |pane, cx| pane.golden_header(window, cx))
                    });
                    div().id(id).w(px(width)).child(header)
                }),
        )
        .into_any_element()
}

/// A stable frame before the tunnel replies could show the ordinary look twice. Require both pills, all the controls
/// being compared, and distinct button heights so the golden always covers the real condensed layout.
pub(super) fn session_toolbar_unready(state: &SceneState, window: &Window) -> Option<String> {
    if !state
        .keep
        .borrow()
        .iter()
        .any(|kept| kept.is::<ToolbarScene>())
    {
        return None;
    }
    let nodes = window.driver_nodes();
    let whole = |node: &gpui::DriverNode, id: &str| {
        node.element_id.as_deref() == Some(id)
            && !node.bounds.is_empty()
            && node.visible_bounds == node.bounds
    };
    let resume_controls: Vec<_> = nodes
        .iter()
        .filter(|node| whole(node, crate::session_pane::header::ENVIRONMENT_ID))
        .map(|node| (node.label.as_deref(), node.disabled))
        .collect();
    if resume_controls != [(Some("Resume"), false); 2] {
        return Some(format!(
            "expected two whole enabled Resume buttons, got {resume_controls:?}"
        ));
    }
    for id in [
        "session-tunnel-pill",
        "session-header-restart-options",
        "session-header-replace-options",
        "session-header-stop",
        "session-header-restart",
        "session-header-replace",
        "session-header-clone",
        crate::terminal::text_size::GROUP_ID,
    ] {
        let count = nodes.iter().filter(|node| whole(node, id)).count();
        if count != 2 {
            return Some(format!("{id} is whole in {count} of the two toolbar rows"));
        }
    }
    let heights: Vec<_> = nodes
        .iter()
        .filter(|node| whole(node, "session-header-delete"))
        .map(|node| node.bounds.size.height)
        .collect();
    match heights.as_slice() {
        [ordinary, condensed] if ordinary > condensed => None,
        _ => Some(format!(
            "expected whole ordinary and condensed Delete buttons, got heights {heights:?}"
        )),
    }
}

/// The one-off hint appears above the terminal while the open Grok session has yet to report a conversation.
pub(super) fn hook_setup_hint(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .w(px(740.))
        .child(crate::session_pane::hook_band_golden(cx))
        .into_any_element()
}

/// Both flag-based trust hints use the words shown in an ordinary folder.
pub(super) fn flag_agent_trust_hints(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use crate::settings::TrustAgent;

    div()
        .w(px(740.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(
            [(TrustAgent::Codex, "a"), (TrustAgent::Muse, "b")].map(|(agent, key)| {
                crate::session_pane::trust_band_golden(agent, key, &cx.focus_handle(), cx)
            }),
        )
        .into_any_element()
}

/// Claude's band points to the caveats for letting Farhelm edit its settings.
pub(super) fn claude_trust_hint(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .w(px(740.))
        .child(crate::session_pane::trust_band_golden(
            crate::settings::TrustAgent::Claude,
            "a",
            &state.ring,
            cx,
        ))
        .into_any_element()
}

/// The open session's details bar for every kind of status and host it draws differently, from a fake helm's snapshot:
/// running on the local host (`Local (this machine)`, the red laptop), waiting on a remote that left its mark at the
/// cloud, exited with a stop annotation, interrupted (the amber ended color), a launch failure whose long detail ends in
/// an ellipsis at the badge's widest, a session nothing has classified (no badge, its age kept), one whose helm sent no
/// host name (no host), and one on a host the helm cannot reach (its title and host only). A host the bar cannot place
/// (no mark) needs a host list that was never read, which this scene's registry rules out; the details tests pin it.
/// Every session has the helm fixture's managed checkout but the one without a host name, so one bar shows no branch
/// mark, and a short command line but the exited one. The copy fields give up their width before the title and host
/// do (SPEC.md): a crowded bar shows them cut or not at all while its title and host are whole, and only a bar whose
/// title and host alone do not fit (the exited one's, the launch failure's) cuts those. The fake, its connection and
/// the bars' (idle) copy confirmations are kept for the scene's life.
pub(super) fn session_details_bar_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let row = |key, title, state: serde_json::Value, ago| {
        session_at_age(key, title, ago)
            .status(state)
            .invocation("claude --model opus")
    };
    let state_of = |state: &str| serde_json::json!({ "state": state });
    let rows = [
        row("a", "Refactor the API client", state_of("running"), 2 * 60)
            .host(1)
            .host_name(crate::hosts::identity::LOCAL_NAME),
        row("b", "Review the install docs", state_of("waiting"), 5 * 60)
            .host_name("deploy@buildbox.example.invalid"),
        row(
            "c",
            "Release notes for 0.40",
            serde_json::json!({ "state": "exited", "exit_code": 0 }),
            3 * 3_600,
        )
        .annotation("stopped by user")
        .host_name("deploy@buildbox.example.invalid")
        .invocation("codex -m gpt-5 -c 'model_reasoning_effort=high' --yolo"),
        row(
            "h",
            "Migrate the auth tokens",
            state_of("interrupted"),
            2 * 86_400,
        )
        .host_name("deploy@buildbox.example.invalid"),
        row(
            "d",
            "Try the new agent",
            serde_json::json!({
                "state": "error",
                "detail": "exec: \"claude-next\": executable file not found in $PATH",
            }),
            26 * 3_600,
        )
        .host_name("deploy@buildbox.example.invalid"),
        row("e", "Just created", state_of("unknown"), 5)
            .host_name("deploy@buildbox.example.invalid"),
        row(
            "f",
            "An old session from before host names",
            state_of("idle"),
            9 * 86_400,
        )
        .host_name("")
        .unmanaged(),
        row(
            "g",
            "Benchmark on the bench box",
            state_of("running"),
            86_400,
        )
        .host(3)
        .host_name("bench@bench.example.invalid")
        .stale(),
    ];
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    helm.connect(vec![
        listing(&order, &rows),
        hosts(&[
            local(),
            remote(2, "deploy@buildbox.example.invalid"),
            remote(3, "bench@bench.example.invalid"),
        ]),
    ]);
    let copies = crate::session_pane::details::Copies::new(cx);
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(copies));
}

/// The copy confirmations a details bar scene's setup kept, or `None` before the setup has run (the scene then draws
/// nothing, as it does before its store exists).
fn kept_copies(state: &SceneState) -> Option<crate::session_pane::details::Copies> {
    state.keep.borrow().iter().find_map(|kept| {
        kept.downcast_ref::<crate::session_pane::details::Copies>()
            .cloned()
    })
}

/// The bars of [`session_details_bar_setup`]'s sessions in listing order, one under another at a main pane's width,
/// then the running one again in a pane too narrow for it, where its title and host are cut together. Each is drawn
/// from the store as the pane draws it, the running mark on the pulse's first (full) step.
pub(super) fn session_details_bars(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use crate::session_pane::details::{Details, bar};
    use farhelm_proto::ui_channel::SessionRow;
    let now = crate::session_list::age::seconds(ticker::wall_now(cx));
    let (Some(copies), Some(store)) = (kept_copies(state), session::current_store(cx)) else {
        return div().into_any_element();
    };
    let collections = store.read(cx).collections();
    let shown: Vec<Details> = collections
        .meta(Collection::Sessions)
        .map(|meta| meta.order.clone())
        .unwrap_or_default()
        .iter()
        .filter_map(|key| collections.get::<SessionRow>(key))
        .map(|row| Details::of(row, collections, now, local_time::zone(cx)))
        .collect();
    let narrow = shown.first().cloned();
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(
            shown
                .into_iter()
                .map(|details| div().w(px(620.)).child(bar(details, &copies, 0, cx))),
        )
        .children(narrow.map(|details| div().w(px(300.)).child(bar(details, &copies, 0, cx))))
        .into_any_element()
}

/// The command line a copy-warning scene's session recorded: a right-to-left override and a line break inside an
/// ordinary command, the old bar's hidden-characters case (`header.spec.ts`, "details copy shows escaped peer text").
const HIDDEN_COMMAND: &str = "codex --prompt \u{202E}abc\n# tail";

/// One running session on the local host whose command line holds hidden characters, its command field confirming a
/// click that copied them (as a click would leave it, without one: the scene's clock never moves, so the confirmation
/// holds). The fake, its connection and the confirmations are kept for the scene's life.
pub(super) fn session_details_copy_warning_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let row = session_at_age("a", "Refactor the API client", 120)
        .host(1)
        .host_name(crate::hosts::identity::LOCAL_NAME)
        .cwd("/home/user/src/widgets")
        .invocation(HIDDEN_COMMAND);
    helm.connect(vec![listing(&["a"], &[row]), hosts(&[local()])]);
    let copies = crate::session_pane::details::Copies::new(cx);
    copies
        .command()
        .update(cx, |feedback, cx| feedback.confirm(HIDDEN_COMMAND, cx));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(copies));
}

/// [`session_details_copy_warning_setup`]'s bar at a main pane's width, with room above it for the warning it opens
/// upward: the command field reads "✓ Copied" and the warning names the command.
pub(super) fn session_details_copy_warning(
    state: &SceneState,
    _: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use crate::session_pane::details::{Details, bar};
    use farhelm_proto::ui_channel::SessionRow;
    let now = crate::session_list::age::seconds(ticker::wall_now(cx));
    let (Some(copies), Some(store)) = (kept_copies(state), session::current_store(cx)) else {
        return div().into_any_element();
    };
    let collections = store.read(cx).collections();
    let shown = collections
        .get::<SessionRow>("a")
        .map(|row| Details::of(row, collections, now, local_time::zone(cx)));
    div()
        .w(px(620.))
        .pt(px(48.))
        .children(shown.map(|details| bar(details, &copies, 0, cx)))
        .into_any_element()
}

/// The interrupted card the session pane draws in place of an interrupted session's terminals, for a session that can
/// resume its conversation (Restart and Replace) and for one whose conversation Farhelm never captured (Replace alone,
/// and why), each in a box the size of a narrow window's terminal area, as the old `.interrupted-card` filled it.
pub(super) fn session_interrupted_cards(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    session_recovery_cards(false, state, window, cx)
}

/// The same layout shows why a missing terminal needs recovery, with and without a resumable conversation.
pub(super) fn session_missing_terminal_cards(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    session_recovery_cards(true, state, window, cx)
}

/// Both scenes use the same window-sized boxes so the two explanations can be compared in the same layout.
fn session_recovery_cards(
    terminal_missing: bool,
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let row = |offer: &str| -> farhelm_proto::ui_channel::SessionRow {
        let fixture = if terminal_missing {
            session("a")
                .status(serde_json::json!({"state": "exited", "exit_code": 0}))
                .terminal_missing()
        } else {
            session("a").status(serde_json::json!({"state": "interrupted"}))
        };
        let mut entry = fixture.entry();
        entry.value["restart_offer"] = serde_json::json!(offer);
        entry.value["agent_kind"] = serde_json::json!("claude");
        serde_json::from_value(entry.value).unwrap_or_else(|error| panic!("a session row: {error}"))
    };
    let cards = ["resume", "not_captured"].map(|offer| {
        let row = row(offer);
        let drawn = crate::session_pane::card::Drawn {
            key: "a",
            row: &row,
            action: None,
            focus: &state.ring,
            question: None,
            launcher_wait: None,
            step: 0,
        };
        let card = div()
            .w(px(500.))
            .h(px(300.))
            .child(crate::session_pane::card::draw(
                drawn,
                gpui::WeakEntity::new_invalid(),
                window,
                cx,
            ));
        if terminal_missing || offer != "resume" {
            return card;
        }
        // Show the interruption above one card's pane, using the same banner as the main view.
        use crate::host_text::HostText;
        use crate::notices::{InterruptedUploads, Notice};
        let upload = InterruptedUploads::new(
            Some(HostText::from_host("Build artifacts")),
            vec![HostText::from_host("build-log.txt")],
            true,
        );
        div()
            .w(px(500.))
            .flex()
            .flex_col()
            .child(crate::main_view::banner(Notice::InterruptedUpload(
                std::rc::Rc::new(upload),
            )))
            .child(card)
    });
    div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .children(cards)
        .into_any_element()
}

/// The unavailable card the session pane draws while the open session's host cannot be reached, with the stale band
/// above it, for a running session on a host that does not answer (the transport's error as its diagnostics) and for an
/// exited one on a host that needs an update (the helm's remedy as its explanation), each over a box the size of a
/// narrow window's terminal area, as the old `.session-unavailable-overlay` covered it. No terminal under either, as
/// for a session opened that way, so each draws the terminal's ground and veil itself. Each card has a focus handle of
/// its own, made for the frame (nothing focuses it), so no two elements track one handle.
pub(super) fn session_unavailable_cards(
    _: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use farhelm_proto::ui_channel::HostStateView;
    let cases = [
        (
            session("a")
                .title("Refactor the API client")
                .activity(TEST_NOW - 3 * 60),
            HostStateView::Unreachable {
                cause: "transport-failure".into(),
                last_error:
                    "ssh: connect to host buildbox.example.invalid port 22: Connection refused"
                        .into(),
            },
        ),
        (
            session("b")
                .title("Release notes for 0.40")
                .status(serde_json::json!({ "state": "exited", "exit_code": 0 }))
                .annotation("stopped by user")
                .activity(TEST_NOW - 2 * 3_600),
            HostStateView::VersionSkew {
                peer_protocol: 7,
                peer_build: "0.39.0".into(),
                our_protocol: 8,
                our_build: "0.40.0".into(),
                remediation: "Update the host so it runs this helm's Farhelm 0.40.0".into(),
            },
        ),
    ];
    draw_unavailable_cards(
        cases.map(|(fixture, phase)| (fixture, crate::hosts::tests::host(2, "ssh", phase))),
        window,
        cx,
    )
}

/// Retained sessions show what happened to their environment. A deleting container has no reconnect promise, a
/// suspended Bash session offers Resume, and an unanswered container host shows its bounded transport error.
pub(super) fn environment_unavailable_cards(
    _: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use crate::session_list::environment_tests::{
        Provider, bash_session, deleting, environment_host, suspended_at,
    };
    use farhelm_proto::ui_channel::HostStateView;
    let cases = [
        (
            session("a")
                .title("Generate the release artifacts")
                .activity(TEST_NOW - 300),
            environment_host(Provider::Container, deleting(None)),
        ),
        (
            bash_session("b")
                .title("Inspect the benchmark logs")
                .activity(TEST_NOW - 120),
            environment_host(Provider::Sandbox, suspended_at((TEST_NOW - 60) * 1000)),
        ),
        (
            session("c").title("Review the migration notes").activity(TEST_NOW - 180),
            environment_host(Provider::Container, HostStateView::Unreachable {
                cause: "transport-failure".into(),
                last_error: concat!(
                    "Farhelm could not ask the container host whether the container is running: ",
                    "Farhelm could not reach this container host (exit status 1): ",
                    "SSH connection closed. No route to host",
                ).into(),
            }),
        ),
    ];
    draw_unavailable_cards(cases, window, cx)
}

/// Draws each stale-session band and card at the width of a narrow terminal pane.
fn draw_unavailable_cards(
    cases: impl IntoIterator<
        Item = (
            crate::session_list::tests::Fixture,
            farhelm_proto::ui_channel::HostView,
        ),
    >,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    use crate::session_list::status::HostLookup;
    use crate::session_pane::unavailable::{Band, Drawn, draw, notice};
    use farhelm_proto::ui_channel::SessionRow;
    let now = crate::session_list::age::seconds(ticker::wall_now(cx));
    let mut columns = Vec::new();
    for (fixture, host) in cases {
        let key = fixture.key;
        let row: SessionRow = serde_json::from_value(fixture.stale().entry().value)
            .unwrap_or_else(|error| panic!("a session row: {error}"));
        let zone = local_time::zone(cx);
        let notice = notice(
            &row.host_name,
            HostLookup::Known(&host),
            None,
            |_| None,
            zone,
        );
        let band = Band::of(&row, now, zone).map(|band| band.draw(theme::palette(cx)));
        let card = draw(
            Drawn {
                key,
                notice,
                focus: &cx.focus_handle().tab_stop(true),
                scroll: &gpui::ScrollHandle::new(),
                step: 0,
                alone: true,
            },
            window,
            cx,
        );
        columns.push(
            div()
                .w(px(500.))
                .flex()
                .flex_col()
                .children(band)
                .child(div().relative().h(px(300.)).child(card)),
        );
    }
    div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .children(columns)
        .into_any_element()
}

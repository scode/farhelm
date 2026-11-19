//! Golden scenes for the open session's pane: its details bar and the interrupted and unavailable cards. The scene list
//! itself, `SCENES` with each scene's name and size, stays in `mod.rs`.

use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::tests::{TEST_NOW, listing, session};
use crate::theme;
use crate::{local_time, session, ticker};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::Collection;
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

use super::SceneState;

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
    let row = |key, title, state: serde_json::Value, ago: i64| {
        session(key)
            .title(title)
            .status(state)
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
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
    let row = session("a")
        .title("Refactor the API client")
        .activity(TEST_NOW - 120)
        .seen(Some(TEST_NOW - 120))
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
        div()
            .w(px(500.))
            .h(px(300.))
            .child(crate::session_pane::card::draw(
                drawn,
                gpui::WeakEntity::new_invalid(),
                window,
                cx,
            ))
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
    use crate::session_list::status::HostLookup;
    use crate::session_pane::unavailable::{Band, Drawn, draw, notice};
    use farhelm_proto::ui_channel::{HostStateView, SessionRow};
    let now = crate::session_list::age::seconds(ticker::wall_now(cx));
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
    let mut columns = Vec::new();
    for (fixture, host_state) in cases {
        let row: SessionRow = serde_json::from_value(fixture.stale().entry().value)
            .unwrap_or_else(|error| panic!("a session row: {error}"));
        let host = crate::hosts::tests::host(2, "ssh", host_state);
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
                key: "a",
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

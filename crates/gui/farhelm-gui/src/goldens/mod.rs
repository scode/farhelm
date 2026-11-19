//! Headless screenshot goldens: one per component, per dialog frame and per overlay kind (tooltip, once more with help
//! in paragraphs, once more beside a sidebar with its notch and ring, popover, the prompt over its veil, once more with
//! a summary that names a host value, once more with a feature-drawn body, a header tile and a footer note, and the
//! prompt anchored to a row's menu button), the keyboard focus ring, host-supplied text over its spoofing corpus, the
//! notice banner, the terminal's styles and cursor shapes, an attached terminal's exit line and status line, a terminal
//! file link's hover display in each of its states, the scrim a pending Delete puts over the open session's terminal,
//! the interrupted card in its place (with Restart and Replace, and with Replace alone), the unavailable card with the
//! stale band above it (a host that does not answer, and one that needs an update), the open session's details bar
//! (every status and host it draws differently, and a narrow one, and once more with a copy confirmed and its
//! hidden-characters warning), the session list (a fleet of every status a row draws, the open row on a host the helm
//! cannot reach, an empty fleet, two rows marked under the marked bar, a row's open menu, a row's notification list, a
//! row's Delete question, and a pending and a refused Delete), the Rename editor, the Send feedback dialog (as it
//! opens, and with a remembered contact), the select closed and open, the YOLO confirmation drawn inline in a
//! launcher's body with its outlined permanent answer, the Settings dialog at Sessions, at Appearance, at Hosts, at
//! Recipes (empty, with recipes in each build state, with a recipe's per-place lines, and with a new recipe's editor
//! open), at Tensorlake (with no key saved, and with a key saved and its list of sandboxes not on this helm), at
//! Credentials, at Terminal and at Agent trust, the Hosts panel (closed, with Details, with a host failing to connect,
//! with a sandbox whose credentials were not delivered, with a host's setup running and failed, with outdated hosts'
//! inline Update, and with a host's Update running), a host's progress card (whole, and previewed in a short window),
//! the Add host dialog (on SSH, and on the Tensorlake sandbox kind with a key saved, also with a new recipe's editor
//! open in it) and the setup confirmation and the sandbox's creation question it leads to, a host's actions menu, the
//! host settings dialog at its top and scrolled to its end, the host refusal dialog for a refused Update, the version
//! readout while an updater runs (the check button at rest, lit, warned and focused; work in progress; a check's
//! outcomes; the update-menu toggle; its open menu; and the reinstall hover), the title bar the Linux window draws for
//! itself (ordinary and maximized), the Open-Source Notices window over a short fixture, and the Trash button empty,
//! full and freeing space, and the Trash dialog, alone and asking to delete everything or one host's archives, in both
//! appearances, compared pixel for pixel. (The terminal stays dark in both appearances, so it draws identically in both
//! pictures; only the scene's ground around it differs.)
//!
//! The agent approval card has its own two scenes, wide and narrow, and the Add host dialog as an agent's host add
//! request opens it one more, in both appearances. A dialog's notices made a stop of their own have one too, focused
//! with the keyboard's ring.
//!
//! These are test references, not documentation media: they pin what each component draws, so a change to a token, a
//! padding or a font shows up as a failing test with the new picture beside the old one, instead of drifting unnoticed.
//! They render with the bundled fonts only and the platform's headless renderer (software Vulkan on Linux), so the same
//! tree gives the same pixels from run to run; a different renderer version can still move antialiased edges, which is
//! why references are per platform and rewritten deliberately. They are ignored in ordinary test runs because they
//! need that renderer selected; run them with `scripts/check-gui-goldens.py`.
//!
//! The reference pictures are not in this repository (`scripts/gui-goldens.py` says why and where they are). Main
//! keeps the manifest `tests/goldens.txt`, one line per picture with its git blob id, and that script fetches the
//! pictures it names and publishes new ones. The test itself knows nothing of that: it reads the references from the
//! directory `FARHELM_GOLDENS_DIR` names, and fails rather than skips when the variable is unset or a picture is
//! missing. After an intended visual change, `scripts/check-gui-goldens.py --update` runs it with
//! `FARHELM_UPDATE_GOLDENS` naming a directory for the captures instead, and `scripts/gui-goldens.py publish` pushes
//! them and rewrites the manifest's lines, which the change's PR then carries ([`GoldenRun`]).
//!
//! The scene functions live in one file per area beside this one (`foundations.rs`, `session_list.rs` and so on), which
//! marks the ones `SCENES` names `pub(super)` for this file's glob imports; this file keeps the shared harness, the
//! helpers more than one area's scenes use, and the one `SCENES` list, whole and still one entry per line (`SCENES`
//! says why). A new scene adds its functions to its area's file and its entry to `SCENES`.

mod approvals;
mod desktop;
mod foundations;
mod hosts;
mod other_dialogs;
mod session_list;
mod session_pane;
mod settings;
mod templates;
mod terminal;

use crate::components::TextInputState;
use crate::layers::{self, Anchor, TooltipOwner};
use crate::session_list::SessionList;
use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::tests::{Fixture, TEST_NOW, listing, preferences, session};
use crate::terminal::TerminalView;
use crate::terminal::attached::AttachedTerminal;
use crate::theme::{self, Preference};
use crate::{root, session, test_support};
use farhelm_helm_client::fake::FakeHelm;
use farhelm_proto::ui_channel::CollectionState;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Pixels, Render, Styled, Window, WindowAppearance, div, point, px, size,
};
use gpui_component::Root;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

// The scene functions `SCENES` names, the only items the area files make `pub(super)`.
use approvals::*;
use desktop::*;
use foundations::*;
use hosts::*;
use other_dialogs::*;
use session_list::*;
use session_pane::*;
use settings::*;
use templates::*;
use terminal::*;

/// How many frames a scene may take to settle before the test gives up: icons decode on the background executor, so
/// the first frame can lack them. Two consecutive identical captures is the readiness signal; a scene that never
/// settles is a failure that says so, not a flaky pixel mismatch.
const MAX_SETTLE_FRAMES: usize = 6;

/// One scene: a name, its size in logical pixels, what it draws, and what it sets up once its window exists.
struct Scene {
    name: &'static str,
    size: (f32, f32),
    draw: fn(&SceneState, &mut Window, &mut App) -> AnyElement,
    setup: fn(&SceneState, &mut Window, &mut App),
}

/// State a scene keeps across frames: a text input's editable state, two of the kind that keeps line breaks and two
/// growing boxes, the owner token a tooltip scene shows its tooltip for (it must stay alive, or the layer manager takes
/// the tooltip down), the anchor a popover scene opens its menu below (recorded when the scene draws), the focus handle
/// the focus-ring scene's button takes, and the terminal views a terminal scene's setup makes (they need the real text
/// system to measure their cells, so they are built after the window exists), the attached terminals a scene shows, and
/// whatever those must keep alive (the fake helm and terminal streams behind them, and the subscription that makes
/// them), and what a session list scene does once its list has drawn ([`AfterDrawn`]).
#[derive(Clone)]
struct SceneState {
    input: TextInputState,
    kept: [TextInputState; 2],
    growing: [TextInputState; 2],
    tooltip_owner: Entity<TooltipOwner>,
    anchor: Rc<RefCell<Option<Anchor>>>,
    ring: FocusHandle,
    terminals: Rc<RefCell<Vec<Entity<TerminalView>>>>,
    attached: Rc<RefCell<Vec<Entity<AttachedTerminal>>>>,
    lists: Rc<RefCell<Vec<Entity<SessionList>>>>,
    keep: Rc<RefCell<Vec<Box<dyn std::any::Any>>>>,
    after: Rc<Cell<Option<AfterDrawn>>>,
}

/// The view a scene is drawn in: the window's ground, then the scene, centered, with the layer manager's overlay.
struct SceneView {
    scene: &'static Scene,
    state: SceneState,
    root: root::Attachment,
}

impl Render for SceneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let ground = div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(12.))
            .bg(theme::color(palette.bg_0))
            .font_family(crate::assets::UI_FONT)
            .child((self.scene.draw)(&self.state, window, cx));
        root::wrap(ground, &self.root, window, cx)
    }
}

/// Scenes with nothing to set up.
fn no_setup(_: &SceneState, _: &mut Window, _: &mut App) {}

// ---- Helpers the scenes of more than one area use -------------------------------------------------------------------

/// One press of Tab, dispatched as the platform would: the focus coordinator moves focus to the next tab stop, and the
/// keyboard becomes the window's last input, which is what makes a focused control draw its ring.
fn press_tab(window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            native_key: None,
            keystroke: gpui::Keystroke {
                key: "tab".into(),
                ..Default::default()
            },
            is_held: false,
            prefer_character_input: false,
        }),
        cx,
    );
}

/// Move the pointer to `at`, as a pointer coming to rest there, so the hover help under it shows through the control's
/// own wiring, placed against the control as a real hover's is, rather than a copy of its words shown at a fixed point.
/// A scene's frames do not wait out the hover delay, so a tooltip of the scene's own is shown first: help shows at once
/// while another shows, and the control's then replaces it.
fn rest_pointer_at(state: &SceneState, at: gpui::Point<Pixels>, window: &mut Window, cx: &mut App) {
    layers::show_tooltip_at(
        &state.tooltip_owner,
        "Warming".into(),
        point(px(0.), px(0.)),
        window,
        cx,
    );
    window.dispatch_event(
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: at,
            pressed_button: None,
            modifiers: gpui::Modifiers::default(),
        }),
        cx,
    );
}

/// [`rest_pointer_at`] the center of the control drawn under the element id `id`, for a scene whose setup runs once its
/// first frame is drawn: it turns on GPUI's driver record and draws a frame to find the control. Nothing happens when
/// no such control was drawn, and the scene's picture then shows no tooltip, which its comparison catches.
fn rest_pointer_on_control(state: &SceneState, id: &str, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    window.draw(cx).clear(cx);
    let center = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some(id))
        .map(|node| node.bounds.center());
    if let Some(center) = center {
        rest_pointer_at(state, center, window, cx);
    }
}

/// Nothing on the page: the prompt scene draws everything through the layer manager.
fn empty(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div().into_any_element()
}

/// Connect a fake helm with the sessions `rows` in that order (and a visit history that opens the first one) and make
/// the session list over it, with a registry of the local host, the remote one the rows run on and a remote the helm
/// cannot reach (which the fleet's stale row runs on, so its cause reads `Waiting for supervisor`), so the host rail
/// beside the list draws All and three avatars. The fake, the connection and the list are kept for the scene's life; the
/// list's own commands (remembering the selection, marking it seen) go to the fake unanswered, so the pictures show the
/// snapshot as the helm sent it.
fn session_list_scene(state: &SceneState, rows: &[Fixture], cx: &mut App) {
    let first = rows.first().map(|row| row.key);
    session_list_scene_opening(state, rows, first, cx);
}

/// [`session_list_scene`], with the visit history opening `open` instead of the first row (none for `None`).
fn session_list_scene_opening(
    state: &SceneState,
    rows: &[Fixture],
    open: Option<&str>,
    cx: &mut App,
) {
    session_list_scene_holding(state, rows, open, Vec::new(), cx);
}

/// [`session_list_scene_opening`], with `extra` collections in the helm's snapshot too (the notices, say).
fn session_list_scene_holding(
    state: &SceneState,
    rows: &[Fixture],
    open: Option<&str>,
    extra: Vec<CollectionState>,
    cx: &mut App,
) {
    let history: Vec<&str> = open.into_iter().collect();
    session_list_scene_preferring(state, rows, preferences(&history, None), extra, cx);
}

/// [`session_list_scene`], with the helm's preference row `preferences` (which names the open session through its
/// visit history) in place of the plain one, and `extra` collections in the helm's snapshot too.
fn session_list_scene_preferring(
    state: &SceneState,
    rows: &[Fixture],
    preferences: CollectionState,
    extra: Vec<CollectionState>,
    cx: &mut App,
) {
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    session_list_scene_with_listing(state, listing(&order, rows), preferences, extra, cx);
}

/// Draw the sidebar with a prepared listing when its collection metadata is part of the scene, as with a group roster.
fn session_list_scene_with_listing(
    state: &SceneState,
    sessions: CollectionState,
    preferences: CollectionState,
    extra: Vec<CollectionState>,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let mut registry = hosts(&[
        local(),
        remote(2, "deploy@buildbox.example.invalid"),
        remote(3, "bench@bench.example.invalid"),
    ]);
    registry.entries[2].value["state"] = serde_json::json!({
        "phase": "unreachable-reprobing",
        "cause": "transport-failure",
        "last_error": "timed out",
    });
    let mut snapshot = vec![sessions, preferences, registry];
    snapshot.extend(extra);
    helm.connect(snapshot);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// The fleet of [`session_list_setup`], which the row menu and Delete scenes draw too. Each row lists the helm
/// fixture's managed checkout, and the waiting row one terminal tab, which its Delete question names. The rows run on
/// the local host (the red laptop, inverted on the selected row) and on remotes (the cloud), with one launch of each
/// kind and shield: a guarded Claude launch (an identity color and the green shield), the fixture's Codex YOLO launch
/// (a grey mark and the amber slashed shield), a command with no agent asserted not YOLO (the Terminal mark), Cursor
/// under its own setting (the dashed shield), a guarded Muse launch, and a session from before launch kinds (the amber
/// question mark).
fn fleet() -> [Fixture; 6] {
    let status = |state: &str| serde_json::json!({ "state": state });
    let agent = |harness: &str, permissions: serde_json::Value, program: &str| {
        serde_json::json!({
            "kind": "agent",
            "selection": { "harness": harness, "permissions": permissions },
            "start": [program, "{farhelm_args}"],
            "resume": null,
        })
    };
    let row = |key, title, state, ago: i64| {
        session(key)
            .title(title)
            .status(status(state))
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
    };
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    [
        row("a", "Refactor the API client", "running", 5)
            .cwd("/home/user/src/api")
            .host(1)
            .launch(agent("claude", serde_json::json!("guarded"), "claude")),
        row("b", "Review the install docs", "waiting", 2 * MINUTE)
            .cwd("/home/user/src/docs")
            .tabs(1),
        row("c", "Fix the flaky upload test", "idle", 14 * MINUTE)
            .seen(Some(TEST_NOW - DAY))
            .cwd("/srv/ci/uploads")
            .launch(serde_json::json!({
                "kind": "command", "command": "./run-tests.sh", "yolo": false, "agent": null, "resume": null,
            })),
        row("d", "Update dependencies", "idle", 3 * HOUR)
            .cwd("/home/user/src/farhelm")
            .host(1)
            .launch(agent("cursor", serde_json::Value::Null, "cursor-agent")),
        row("f", "Benchmark on the build box", "running", DAY)
            .host(3)
            .stale()
            .cwd("/home/user/bench")
            .launch(agent("muse", serde_json::json!("guarded"), "muse")),
        row("e", "Release notes for 0.40", "exited", 26 * DAY)
            .status(serde_json::json!({ "state": "exited", "exit_code": null }))
            .annotation("stopped by user")
            .cwd("/home/user/src/farhelm")
            .launch(serde_json::json!({
                "kind": "legacy", "invocation": "codex", "agent_kind": "codex", "resume_template": null,
            })),
    ]
}

/// What a session list scene does once its list has drawn a frame: open a menu or ask a question, which need the
/// anchors that frame recorded, or start work. Taken by the first frame that finds it, which runs it once that frame
/// is done.
type AfterDrawn = fn(&Entity<SessionList>, &SceneState, &mut Window, &mut App);

/// Make the fleet's list, and run `after` on it once it has drawn.
fn session_list_then(state: &SceneState, after: AfterDrawn, cx: &mut App) {
    session_list_scene(state, &fleet(), cx);
    state.after.set(Some(after));
}

/// [`session_lists`], and the scene's [`AfterDrawn`] once the list has rows to anchor to.
fn session_lists_then(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    defer_after(state, window, cx);
    session_lists(state, window, cx)
}

/// Run the scene's [`AfterDrawn`], if it has one left, after this frame: by then the list has drawn under its UI
/// session, with rows to anchor to.
fn defer_after(state: &SceneState, window: &mut Window, cx: &mut App) {
    let list = state.lists.borrow().first().cloned();
    if let (Some(list), Some(after)) = (list, state.after.take()) {
        let staged = state.clone();
        window.defer(cx, move |window, cx| after(&list, &staged, window, cx));
    }
}

/// A host scene's one remaining step (opening the menu, scrolling the dialog), kept among the scene's kept values until
/// a frame makes it possible. It runs once: run on every frame, its own redraw would ask for the next frame forever.
struct Pending(Cell<bool>);

/// Whether the scene's [`Pending`] step is still to run, marking it run.
fn take_pending(state: &SceneState) -> bool {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Pending>())
        .is_some_and(|pending| pending.0.replace(false))
}

/// The session list a scene's setup made, in a column of the sidebar's width and surface.
fn session_lists(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    session_lists_at(state, px(394.), cx)
}

/// The session list a scene's setup made, in a column of the sidebar's width and surface, `height` tall.
fn session_lists_at(state: &SceneState, height: Pixels, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    div()
        .w(theme::metrics::SIDEBAR_WIDTH)
        .h(height)
        .bg(theme::color(palette.bg_1))
        .text_color(theme::color(palette.fg_0))
        .children(state.lists.borrow().iter().cloned())
        .into_any_element()
}

/// Every golden scene. A slice, one scene per entry, so that a new scene only adds lines (the shape
/// `docs/gpui/foundation-ownership.md` asks of the lists areas append to). A length in the type changed with every
/// addition, and during two rebases jj's automatic merge left that length wrong without a conflict marker; both were
/// caught and fixed by hand before building.
static SCENES: &[Scene] = &[
    Scene {
        name: "buttons",
        size: (560., 124.),
        draw: buttons,
        setup: no_setup,
    },
    Scene {
        name: "text-input",
        size: (320., 64.),
        draw: text_input,
        setup: text_input_setup,
    },
    Scene {
        name: "text-input-line-breaks",
        size: (320., 140.),
        draw: kept_line_breaks,
        setup: kept_line_breaks_setup,
    },
    Scene {
        name: "checkbox",
        size: (360., 48.),
        draw: checkboxes,
        setup: no_setup,
    },
    Scene {
        name: "switch",
        size: (700., 48.),
        draw: switches,
        setup: no_setup,
    },
    Scene {
        name: "command-well",
        size: (600., 200.),
        draw: command_wells,
        setup: no_setup,
    },
    Scene {
        name: "text-input-growing",
        size: (620., 300.),
        draw: growing,
        setup: growing_setup,
    },
    Scene {
        name: "dialog",
        size: (540., 300.),
        draw: dialog,
        setup: no_setup,
    },
    Scene {
        name: "dialog-notices-stop",
        size: (540., 340.),
        draw: dialog_notices_stop,
        setup: focus_ring_setup,
    },
    Scene {
        name: "tooltip",
        size: (400., 140.),
        draw: tooltip,
        setup: tooltip_setup,
    },
    Scene {
        name: "tooltip-paragraphs",
        size: (520., 320.),
        draw: tooltip_paragraphs,
        setup: tooltip_paragraphs_setup,
    },
    Scene {
        name: "tooltip-beside",
        size: (600., 160.),
        draw: tooltip_beside,
        setup: tooltip_beside_setup,
    },
    Scene {
        name: "focus-ring",
        size: (260., 64.),
        draw: focus_ring,
        setup: focus_ring_setup,
    },
    Scene {
        name: "segmented-control",
        size: (360., 170.),
        draw: segmented_control,
        setup: focus_ring_setup,
    },
    Scene {
        name: "radio-group",
        size: (300., 130.),
        draw: radio_group,
        setup: no_setup,
    },
    Scene {
        name: "radio-group-variants",
        size: (560., 420.),
        draw: radio_group_variants,
        setup: focus_ring_setup,
    },
    Scene {
        name: "popover",
        size: (340., 200.),
        draw: popover,
        setup: popover_setup,
    },
    Scene {
        name: "prompt",
        size: (560., 320.),
        draw: empty,
        setup: prompt_setup,
    },
    Scene {
        name: "prompt-host-text",
        size: (560., 320.),
        draw: empty,
        setup: prompt_host_text_setup,
    },
    Scene {
        name: "prompt-body",
        size: (640., 440.),
        draw: empty,
        setup: prompt_body_setup,
    },
    Scene {
        name: "anchored-prompt",
        size: (340., 190.),
        draw: anchored_prompt,
        setup: anchored_prompt_setup,
    },
    Scene {
        name: "inline-prompt",
        size: (1000., 300.),
        draw: inline_prompt,
        setup: inline_prompt_setup,
    },
    Scene {
        name: "host-text",
        size: (420., 500.),
        draw: host_text,
        setup: no_setup,
    },
    Scene {
        name: "notice-banner",
        size: (760., 64.),
        draw: notice_banner,
        setup: focus_ring_setup,
    },
    Scene {
        name: "approval-card",
        size: (684., 500.),
        draw: approval_card,
        setup: no_setup,
    },
    Scene {
        name: "approval-card-narrow",
        size: (308., 900.),
        draw: approval_card_narrow,
        setup: no_setup,
    },
    Scene {
        name: "terminal-styles",
        size: (380., 200.),
        draw: terminals,
        setup: terminal_styles_setup,
    },
    Scene {
        name: "terminal-cursors",
        size: (380., 80.),
        draw: terminals,
        setup: terminal_cursors_setup,
    },
    Scene {
        name: "terminal-status",
        size: (444., 336.),
        draw: attached_terminals,
        setup: terminal_status_setup,
    },
    Scene {
        name: "terminal-file-disclosures",
        size: (420., 520.),
        draw: terminal_file_disclosures,
        setup: no_setup,
    },
    Scene {
        name: "sequence-pill",
        size: (680., 240.),
        draw: sequence_pill,
        setup: sequence_pill_setup,
    },
    Scene {
        name: "terminal-delete-scrim",
        size: (444., 174.),
        draw: terminal_delete_scrim,
        setup: terminal_delete_scrim_setup,
    },
    Scene {
        name: "session-interrupted-card",
        size: (1036., 324.),
        draw: session_interrupted_cards,
        setup: no_setup,
    },
    Scene {
        name: "session-unavailable",
        size: (1036., 348.),
        draw: session_unavailable_cards,
        setup: no_setup,
    },
    Scene {
        name: "session-details-bar",
        size: (644., 368.),
        draw: session_details_bars,
        setup: session_details_bar_setup,
    },
    Scene {
        name: "session-details-copy-warning",
        size: (644., 102.),
        draw: session_details_copy_warning,
        setup: session_details_copy_warning_setup,
    },
    Scene {
        name: "session-list",
        size: (420., 418.),
        draw: session_lists,
        setup: session_list_setup,
    },
    Scene {
        name: "session-row-hover-beside",
        size: (800., 418.),
        draw: session_lists_beside,
        setup: session_row_hover_setup,
    },
    Scene {
        name: "session-summary-card",
        size: (800., 418.),
        draw: session_lists_beside,
        setup: session_summary_card_setup,
    },
    Scene {
        name: "session-list-selected-stale",
        size: (420., 418.),
        draw: session_lists,
        setup: session_list_selected_stale_setup,
    },
    Scene {
        name: "session-list-empty",
        size: (420., 418.),
        draw: session_lists,
        setup: session_list_empty_setup,
    },
    Scene {
        name: "session-list-compact",
        size: (420., 418.),
        draw: session_lists,
        setup: session_list_compact_setup,
    },
    Scene {
        name: "session-list-pinned",
        size: (420., 418.),
        draw: session_lists,
        setup: session_list_pinned_setup,
    },
    Scene {
        name: "session-list-groups",
        size: (420., 450.),
        draw: session_lists,
        setup: session_list_groups_setup,
    },
    Scene {
        name: "host-rail-status",
        size: (420., 700.),
        draw: host_rail_lists,
        setup: host_rail_status_setup,
    },
    Scene {
        name: "session-list-marked",
        size: (420., 418.),
        draw: session_lists_then,
        setup: session_list_marked_setup,
    },
    Scene {
        name: "session-row-menu",
        // The app's minimum window height, so the whole menu shows, Delete last, since Clone and Replace with joined
        // it; wide enough, with the sidebar centred, for the menu beside it, where it opens, rather than pushed back
        // over the list by the window's edge.
        size: (920., 560.),
        draw: session_lists_then,
        setup: session_row_menu_setup,
    },
    Scene {
        name: "notices-empty",
        size: (820., 560.),
        draw: session_lists_then,
        setup: notices_empty_setup,
    },
    Scene {
        name: "notices-bell-loud",
        size: (420., 418.),
        draw: session_lists_then,
        setup: notices_bell_loud_setup,
    },
    Scene {
        name: "notices-alerting",
        size: (820., 560.),
        draw: session_lists_then,
        setup: notices_alerting_setup,
    },
    Scene {
        name: "help-menu",
        // The app's minimum window height. On Linux the menu, with Open-Source Notices last, is a few pixels taller
        // than that window allows, so this shows it at its cap (the window's height less 16 pixels), scrolled to its
        // top; the macOS menu fits.
        size: (420., 560.),
        draw: session_lists_then,
        setup: help_menu_setup,
    },
    Scene {
        name: "session-bell-list",
        // Wide and tall enough for the whole list beside the sidebar, which the scene centres.
        size: (960., 680.),
        draw: session_lists_then,
        setup: session_bell_list_setup,
    },
    Scene {
        name: "row-delete-confirmation",
        // Wide enough, with the sidebar centred, for the question beside it, where it opens from the row's menu button.
        size: (1000., 418.),
        draw: session_lists_then,
        setup: row_delete_confirmation_setup,
    },
    Scene {
        name: "session-list-deleting",
        size: (420., 418.),
        draw: session_lists_then,
        setup: session_list_deleting_setup,
    },
    Scene {
        name: "announcements-dialog",
        size: (640., 520.),
        draw: empty,
        setup: announcements_dialog_setup,
    },
    Scene {
        name: "welcome-dialog",
        size: (600., 440.),
        draw: empty,
        setup: welcome_dialog_setup,
    },
    Scene {
        name: "rename-dialog",
        size: (560., 320.),
        draw: empty,
        setup: rename_dialog_setup,
    },
    Scene {
        name: "feedback-dialog",
        size: (600., 580.),
        draw: empty,
        setup: feedback_dialog_setup,
    },
    Scene {
        name: "feedback-dialog-reuse",
        size: (600., 620.),
        draw: empty,
        setup: feedback_dialog_reuse_setup,
    },
    Scene {
        name: "keyboard-shortcuts-dialog",
        size: (700., 1020.),
        draw: empty,
        setup: keyboard_shortcuts_dialog_setup,
    },
    Scene {
        name: "host-filter-empty",
        size: (420., 418.),
        draw: session_lists_then,
        setup: host_filter_empty_setup,
    },
    Scene {
        name: "session-status-segments",
        size: (420., 418.),
        draw: session_lists_then,
        setup: session_status_segments_setup,
    },
    Scene {
        name: "filter-empty-both-links",
        size: (420., 418.),
        draw: session_lists_then,
        setup: filter_empty_both_links_setup,
    },
    Scene {
        name: "select",
        size: (560., 64.),
        draw: selects,
        setup: no_setup,
    },
    Scene {
        name: "select-open",
        size: (260., 240.),
        draw: select_open,
        setup: select_open_setup,
    },
    Scene {
        name: "templates-dialog",
        size: (1010., 600.),
        draw: empty,
        setup: templates_dialog_setup,
    },
    Scene {
        name: "template-editor",
        size: (1010., 940.),
        draw: empty,
        setup: template_editor_setup,
    },
    Scene {
        name: "template-number-menu",
        size: (1010., 940.),
        draw: template_number_menu_scene,
        setup: template_number_menu_setup,
    },
    Scene {
        name: "template-launcher",
        // The card at its widest, near the window's top as the layer manager places it. Tall enough that the layer
        // manager's height cap (75% of the window) leaves all three rows in view, as the reference's taller window did.
        size: (800., 640.),
        draw: empty,
        setup: template_launcher_setup,
    },
    Scene {
        name: "template-leave-question",
        // The reference window's 1000 pixels of height, so the question shows whole below the editor's fields.
        size: (1010., 1000.),
        draw: template_leave_question_scene,
        setup: template_leave_question_setup,
    },
    Scene {
        name: "settings-sessions",
        size: (800., 830.),
        draw: empty,
        setup: settings_sessions_setup,
    },
    Scene {
        name: "settings-appearance",
        size: (800., 830.),
        draw: empty,
        setup: settings_appearance_setup,
    },
    Scene {
        name: "settings-hosts",
        size: (800., 830.),
        draw: empty,
        setup: settings_hosts_setup,
    },
    Scene {
        name: "settings-recipes",
        size: (800., 830.),
        draw: empty,
        setup: settings_recipes_setup,
    },
    Scene {
        name: "settings-recipe-editor",
        size: (800., 830.),
        draw: empty,
        setup: settings_recipe_editor_setup,
    },
    Scene {
        name: "settings-recipes-rows",
        size: (800., 900.),
        draw: empty,
        setup: settings_recipes_rows_setup,
    },
    Scene {
        name: "settings-recipes-places",
        size: (800., 830.),
        draw: empty,
        setup: settings_recipes_places_setup,
    },
    Scene {
        name: "settings-tensorlake",
        size: (800., 830.),
        draw: empty,
        setup: settings_tensorlake_setup,
    },
    Scene {
        name: "settings-tensorlake-sandboxes",
        size: (800., 830.),
        draw: settings_tensorlake_sandboxes,
        setup: settings_tensorlake_sandboxes_setup,
    },
    Scene {
        name: "settings-credentials",
        size: (800., 830.),
        draw: empty,
        setup: settings_credentials_setup,
    },
    Scene {
        name: "settings-terminal",
        size: (800., 830.),
        draw: empty,
        setup: settings_terminal_setup,
    },
    Scene {
        name: "settings-containers",
        size: (800., 830.),
        draw: empty,
        setup: settings_containers_setup,
    },
    Scene {
        name: "settings-checkouts",
        size: (800., 830.),
        draw: empty,
        setup: settings_checkouts_setup,
    },
    Scene {
        name: "settings-agent-trust",
        size: (800., 830.),
        draw: empty,
        setup: settings_agent_trust_setup,
    },
    Scene {
        name: "settings-notifications",
        size: (800., 830.),
        draw: empty,
        setup: settings_notifications_setup,
    },
    Scene {
        name: "settings-app",
        size: (800., 830.),
        draw: empty,
        setup: settings_app_setup,
    },
    Scene {
        name: "hosts-panel",
        size: (364., 418.),
        draw: hosts_panels,
        setup: hosts_panel_setup,
    },
    Scene {
        name: "hosts-panel-details",
        size: (364., 418.),
        draw: hosts_panels,
        setup: hosts_panel_details_setup,
    },
    Scene {
        name: "hosts-panel-host-unreachable",
        size: (364., 418.),
        draw: hosts_panels,
        setup: hosts_panel_host_unreachable_setup,
    },
    Scene {
        name: "hosts-panel-credentials-not-delivered",
        size: (364., 924.),
        draw: tall_hosts_panels,
        setup: hosts_panel_credentials_not_delivered_setup,
    },
    Scene {
        name: "host-menu",
        size: (640., 418.),
        draw: host_menu_scene,
        setup: host_menu_setup,
    },
    Scene {
        name: "host-settings-dialog",
        size: (560., 1000.),
        draw: empty,
        setup: host_settings_dialog_setup,
    },
    Scene {
        name: "host-settings-dialog-end",
        size: (560., 1000.),
        draw: host_settings_dialog_end,
        setup: host_settings_dialog_end_setup,
    },
    Scene {
        name: "tab-strip",
        size: (320., 60.),
        draw: tab_strip,
        setup: no_setup,
    },
    Scene {
        name: "dialog-launcher-frame",
        size: (640., 340.),
        draw: dialog_launcher_frame,
        setup: no_setup,
    },
    Scene {
        name: "host-add-dialog",
        size: (600., 600.),
        draw: empty,
        setup: host_add_dialog_setup,
    },
    Scene {
        name: "host-add-tensorlake-form",
        size: (600., 700.),
        draw: empty,
        setup: host_add_tensorlake_form_setup,
    },
    Scene {
        name: "host-add-tensorlake-recipe-editor",
        size: (600., 900.),
        draw: empty,
        setup: host_add_tensorlake_recipe_editor_setup,
    },
    Scene {
        name: "host-add-tensorlake-creation",
        size: (600., 900.),
        draw: empty,
        setup: host_add_tensorlake_creation_setup,
    },
    Scene {
        name: "host-add-dialog-agent",
        size: (600., 900.),
        draw: empty,
        setup: host_add_dialog_agent_setup,
    },
    Scene {
        name: "hosts-shown-with-a-host-chosen",
        size: (420., 418.),
        draw: session_lists_then,
        setup: hosts_shown_with_a_host_chosen_setup,
    },
    Scene {
        name: "host-add-setup-plan",
        size: (600., 1000.),
        draw: empty,
        setup: host_add_setup_plan_setup,
    },
    Scene {
        name: "host-setup-running-hosts-panel",
        size: (364., 924.),
        draw: tall_hosts_panels,
        setup: host_setup_running_hosts_panel_setup,
    },
    Scene {
        name: "host-setup-failed-hosts-panel",
        size: (364., 924.),
        draw: tall_hosts_panels,
        setup: host_setup_failed_hosts_panel_setup,
    },
    Scene {
        name: "host-remove-confirmation",
        size: (560., 480.),
        draw: empty,
        setup: host_remove_confirmation_setup,
    },
    Scene {
        name: "linux-title-bar",
        size: (924., 104.),
        draw: linux_title_bars,
        setup: no_setup,
    },
    Scene {
        name: "open-source-notices",
        size: (800., 364.),
        draw: notices_window,
        setup: no_setup,
    },
    Scene {
        name: "trash-button-empty",
        size: (120., 56.),
        draw: trash_scenes,
        setup: trash_button_empty_setup,
    },
    Scene {
        name: "trash-button-full",
        size: (120., 56.),
        draw: trash_scenes,
        setup: trash_button_full_setup,
    },
    Scene {
        name: "trash-button-emptying",
        size: (120., 56.),
        draw: trash_scenes,
        setup: trash_button_emptying_setup,
    },
    Scene {
        name: "checkout-trash-dialog",
        size: (760., 720.),
        draw: trash_scenes,
        setup: checkout_trash_dialog_setup,
    },
    Scene {
        name: "checkout-trash-confirm-all",
        size: (760., 720.),
        draw: trash_scenes,
        setup: checkout_trash_confirm_all_setup,
    },
    Scene {
        name: "checkout-trash-confirm-host",
        size: (760., 720.),
        draw: trash_scenes,
        setup: checkout_trash_confirm_host_setup,
    },
    Scene {
        name: "session-order-menu",
        size: (420., 418.),
        draw: session_lists_then,
        setup: session_order_menu_setup,
    },
    Scene {
        name: "version-readout-check",
        size: (420., 268.),
        draw: version_readout_check,
        setup: version_readout_check_setup,
    },
    Scene {
        name: "version-readout-progress",
        size: (420., 219.),
        draw: version_readout_progress,
        setup: no_setup,
    },
    Scene {
        name: "version-readout-outcomes",
        size: (420., 219.),
        draw: version_readout_outcomes,
        setup: version_readout_outcomes_setup,
    },
    Scene {
        name: "version-readout-update-ready",
        size: (420., 170.),
        draw: version_readout_update_ready,
        setup: version_readout_update_ready_setup,
    },
    Scene {
        name: "version-readout-menu",
        size: (720., 240.),
        draw: version_readout_menu,
        setup: version_readout_menu_setup,
    },
    Scene {
        name: "version-readout-tooltip",
        size: (640., 260.),
        draw: version_readout_tooltip,
        setup: version_readout_tooltip_setup,
    },
    Scene {
        name: "hosts-panel-updates",
        size: (364., 418.),
        draw: hosts_panels,
        setup: hosts_panel_updates_setup,
    },
    Scene {
        name: "host-update-running-hosts-panel",
        size: (364., 418.),
        draw: hosts_panels,
        setup: host_update_running_hosts_panel_setup,
    },
    Scene {
        name: "host-progress-card",
        size: (360., 520.),
        draw: progress_cards,
        setup: host_progress_card_setup,
    },
    Scene {
        name: "host-progress-card-preview",
        size: (360., 300.),
        draw: progress_cards,
        setup: host_progress_card_preview_setup,
    },
    Scene {
        name: "host-update-refusal",
        size: (640., 600.),
        draw: empty,
        setup: host_update_refusal_setup,
    },
    Scene {
        name: "scroll-box",
        size: (520., 136.),
        draw: scroll_boxes,
        setup: focus_ring_setup,
    },
    Scene {
        name: "host-uninstall-refusal",
        size: (640., 600.),
        draw: empty,
        setup: host_uninstall_refusal_setup,
    },
    Scene {
        name: "host-uninstall-confirmation",
        size: (600., 900.),
        draw: empty,
        setup: host_uninstall_confirmation_setup,
    },
    Scene {
        name: "host-uninstall-running-hosts-panel",
        size: (364., 924.),
        draw: tall_hosts_panels,
        setup: host_uninstall_running_hosts_panel_setup,
    },
    Scene {
        name: "hosts-panel-uninstall-notice",
        size: (364., 418.),
        draw: hosts_panels,
        setup: hosts_panel_uninstall_notice_setup,
    },
];

/// Draw and capture `window` until two consecutive captures match, at most [`MAX_SETTLE_FRAMES`] times.
fn settle(
    cx: &mut gpui::HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    name: &str,
) -> anyhow::Result<image::RgbaImage> {
    let mut previous = None;
    for _ in 0..MAX_SETTLE_FRAMES {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
        let capture = cx.capture_screenshot(window)?;
        if previous.as_ref() == Some(&capture) {
            return Ok(capture);
        }
        previous = Some(capture);
    }
    anyhow::bail!("{name} never settled: {MAX_SETTLE_FRAMES} frames, no two in a row the same")
}

/// What a golden run does, read from the two variables `scripts/check-gui-goldens.py` sets. The references are not in
/// the tree (the module docs say why), so there is no default to fall back on: a run with neither variable fails.
enum GoldenRun {
    /// `FARHELM_GOLDENS_DIR`: compare every capture with `<dir>/<os>/<scene>--<appearance>.png`, the layout
    /// `scripts/gui-goldens.py fetch` writes. A missing file is a failure like a differing one, never a skip.
    Compare(PathBuf),
    /// `FARHELM_UPDATE_GOLDENS`: write every capture to `<dir>/<os>/` and compare nothing, then write
    /// `<dir>/<os>.complete` holding the number of captures, which `scripts/gui-goldens.py publish` requires before it
    /// takes the directory: a run that stopped partway would otherwise drop every scene it never reached.
    Update(PathBuf),
}

impl GoldenRun {
    /// The run the environment asks for. Both variables hold absolute paths, as `scripts/check-gui-goldens.py` sets
    /// them, since the test runs in the crate's directory rather than wherever the caller stood.
    fn from_env() -> anyhow::Result<Self> {
        if let Some(captures) = std::env::var_os("FARHELM_UPDATE_GOLDENS") {
            return Ok(Self::Update(captures.into()));
        }
        let references = std::env::var_os("FARHELM_GOLDENS_DIR").ok_or_else(|| {
            anyhow::anyhow!(
                "FARHELM_GOLDENS_DIR is unset, and the references are not in the tree: run \
                 scripts/check-gui-goldens.py, which fetches them"
            )
        })?;
        Ok(Self::Compare(references.into()))
    }
}

/// Every scene in both appearances matches its reference exactly. Run through `scripts/check-gui-goldens.py`, which
/// selects the headless renderer and fetches the references; its `--update` sets `FARHELM_UPDATE_GOLDENS` instead,
/// for captures to publish ([`GoldenRun`]).
#[test]
#[ignore = "needs the headless renderer selected; run scripts/check-gui-goldens.py"]
fn component_goldens() -> anyhow::Result<()> {
    let run = GoldenRun::from_env()?;
    let os = std::env::consts::OS;
    // Captures go beside the test binary, inside the build directory, so a failure leaves the new picture to compare.
    let captures = std::env::current_exe()?
        .parent()
        .map(|deps| deps.join("gui-goldens"))
        .ok_or_else(|| anyhow::anyhow!("the test binary has no directory"))?;
    std::fs::create_dir_all(&captures)?;
    if let GoldenRun::Update(dir) = &run {
        std::fs::create_dir_all(dir.join(os))?;
    }
    let mut written = 0;
    let mut failures = Vec::new();
    for scene in SCENES {
        for (appearance, preference) in [("light", Preference::Light), ("dark", Preference::Dark)] {
            let name = format!("{}--{appearance}.png", scene.name);
            let mut cx = test_support::headless()?;
            cx.update(|cx| {
                theme::follow_system(WindowAppearance::Light, cx);
                theme::set_preference(preference, cx);
            });
            let mut view = None;
            let window =
                cx.open_window(size(px(scene.size.0), px(scene.size.1)), |window, cx| {
                    let state = SceneState {
                        input: TextInputState::new(window, cx),
                        growing: [
                            TextInputState::new_growing(3, window, cx),
                            TextInputState::new_growing(3, window, cx),
                        ],
                        kept: [
                            TextInputState::new_keeping_line_breaks(window, cx),
                            TextInputState::new_keeping_line_breaks(window, cx),
                        ],
                        tooltip_owner: cx.new(|_| TooltipOwner::default()),
                        anchor: Rc::new(RefCell::new(None)),
                        ring: cx.focus_handle().tab_stop(true),
                        terminals: Rc::new(RefCell::new(Vec::new())),
                        attached: Rc::new(RefCell::new(Vec::new())),
                        lists: Rc::new(RefCell::new(Vec::new())),
                        keep: Rc::new(RefCell::new(Vec::new())),
                        after: Rc::new(Cell::new(None)),
                    };
                    let scene_view = cx.new(|cx| SceneView {
                        scene,
                        state,
                        root: root::attach(window, cx),
                    });
                    view = Some(scene_view.clone());
                    cx.new(|cx| Root::new(scene_view, window, cx))
                })?;
            let view =
                view.ok_or_else(|| anyhow::anyhow!("{name}: the scene view was not built"))?;
            // One frame first, so what the scene draws (an anchor's bounds, a focusable button) exists for its setup.
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
            cx.update_window(window.into(), |_, window, cx| {
                // Cheap clones: handles and shared cells.
                let staged = view.read(cx).state.clone();
                (scene.setup)(&staged, window, cx);
            })?;
            let screenshot = settle(&mut cx, window.into(), &name)?;
            screenshot.save(captures.join(&name))?;
            let references = match &run {
                GoldenRun::Update(dir) => {
                    screenshot.save(dir.join(os).join(&name))?;
                    written += 1;
                    continue;
                }
                GoldenRun::Compare(dir) => dir.join(os),
            };
            let reference = references.join(&name);
            match image::open(&reference).map(|expected| expected.into_rgba8()) {
                Ok(expected) if expected == screenshot => {}
                Ok(_) => failures.push(format!(
                    "{name} differs from its reference (capture in {})",
                    captures.display()
                )),
                Err(error) => failures.push(format!("{}: {error}", reference.display())),
            }
        }
    }
    if let GoldenRun::Update(dir) = &run {
        std::fs::write(dir.join(format!("{os}.complete")), format!("{written}\n"))?;
    }
    anyhow::ensure!(
        failures.is_empty(),
        "golden failures: {}",
        failures.join("; ")
    );
    Ok(())
}

//! Headless screenshot goldens: one per component, per dialog frame and per overlay kind (tooltip, popover, the prompt
//! over its veil, once more with a summary that names a host value), the keyboard focus ring, host-supplied text over
//! its spoofing corpus, the notice banner, the terminal's styles and cursor shapes, and an attached terminal's exit line
//! and status line, in both appearances, compared pixel for pixel. (The terminal stays dark in both appearances, so it
//! draws identically in both pictures; only the scene's ground around it differs.)
//!
//! These are test references, not documentation media: they pin what each component draws, so a change to a token, a
//! padding or a font shows up as a failing test with the new picture beside the old one, instead of drifting unnoticed.
//! They render with the bundled fonts only and the platform's headless renderer (software Vulkan on Linux), so the same
//! tree gives the same pixels from run to run; a different renderer version can still move antialiased edges, which is
//! why references are per platform and rewritten deliberately. References live under `tests/goldens/<os>/`. They are
//! ignored in ordinary test runs because they need that renderer selected; run them with `scripts/check-gui-goldens.py`
//! (`--update` rewrites the references after an intended change).

use std::path::Path;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Render, Styled, Window, WindowAppearance, div, point, px, size,
};
use gpui_component::Root;

use crate::assets::Icon;
use crate::components::{
    Button, DialogFrame, HoverHelp, IconButton, Role, TextInput, TextInputState, Tier,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, Popover, TooltipOwner};
use crate::notices::Notice;
use crate::prompt::{self, Answer, Question};
use crate::session;
use crate::terminal::attached::{AttachedTerminal, OpenStream, TerminalTarget};
use crate::terminal::{CellMetrics, CursorPhase, TerminalView, TextSize};
use crate::test_support;
use crate::theme::{self, Preference};
use farhelm_grid_model::GridOutput;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::FakeTerminal;
use farhelm_helm_client::terminal::AttachmentLease;

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

/// State a scene keeps across frames: a text input's editable state, the owner token a tooltip scene shows its tooltip
/// for (it must stay alive, or the layer manager takes the tooltip down), the anchor a popover scene opens its menu
/// below (recorded when the scene draws), the focus handle the focus-ring scene's button takes, and the terminal views
/// a terminal scene's setup makes (they need the real text system to measure their cells, so they are built after the
/// window exists), the attached terminals a scene shows, and whatever those must keep alive (the fake helm and terminal
/// streams behind them, and the subscription that makes them).
#[derive(Clone)]
struct SceneState {
    input: TextInputState,
    tooltip_owner: Entity<TooltipOwner>,
    anchor: Rc<RefCell<Option<Anchor>>>,
    ring: FocusHandle,
    terminals: Rc<RefCell<Vec<Entity<TerminalView>>>>,
    attached: Rc<RefCell<Vec<Entity<AttachedTerminal>>>>,
    keep: Rc<RefCell<Vec<Box<dyn std::any::Any>>>>,
}

/// The view a scene is drawn in: the window's ground, then the scene, centered, with the layer manager's overlay.
struct SceneView {
    scene: &'static Scene,
    state: SceneState,
    _layers: layers::Attachment,
    focus: focus::Attachment,
}

impl Render for SceneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        focus::root(div(), &self.focus)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(12.))
            .bg(theme::color(palette.bg_0))
            .font_family(crate::assets::UI_FONT)
            .child((self.scene.draw)(&self.state, window, cx))
            .children(layers::overlay(window, cx))
    }
}

/// Scenes with nothing to set up.
fn no_setup(_: &SceneState, _: &mut Window, _: &mut App) {}

/// Every button tier, a disabled main action and an icon button, in one row.
fn buttons(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    div()
        .flex()
        .gap(px(8.))
        .items_center()
        .child(Button::new("ghost", "Ghost", help(), Role::Button))
        .child(Button::new("neutral", "Cancel", help(), Role::Button).tier(Tier::Neutral))
        .child(Button::new("primary", "Save", help(), Role::Button).tier(Tier::Primary))
        .child(
            Button::new("primary-off", "Save", help(), Role::Button)
                .tier(Tier::Primary)
                .disabled(true),
        )
        .child(
            Button::new("danger", "Delete", help(), Role::Button)
                .icon(Icon::Trash)
                .tier(Tier::Danger),
        )
        .child(IconButton::new(
            "more",
            Icon::More,
            "More actions",
            help(),
            Role::Button,
        ))
        .into_any_element()
}

/// Put a value in the text input, so the golden pins the font and size Kit's input draws with, not only its box.
fn text_input_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    state.input.set_text("api-refactor", window, cx);
}

/// A labeled text input holding a value.
fn text_input(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w(px(280.))
        .child(TextInput::new(
            "title",
            &state.input,
            "Session title",
            HoverHelp::tooltip("The session's new title"),
        ))
        .into_any_element()
}

/// The dialog frame with a title, a summary, a short body and a pinned Cancel/Delete pair.
fn dialog(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    DialogFrame::new(
        "delete",
        "Delete session?",
        div().child(
            "The session's terminal and its history are removed. The working directory stays.",
        ),
        vec![
            Button::new(
                "cancel",
                "Cancel",
                HoverHelp::tooltip("Keep the session"),
                Role::Button,
            )
            .tier(Tier::Neutral),
            Button::new(
                "confirm",
                "Delete",
                HoverHelp::tooltip("Delete the session"),
                Role::Button,
            )
            .tier(Tier::Danger),
        ],
    )
    .summary("api-refactor, on this machine")
    .into_any_element()
}

/// The tooltip scene's help: long enough to wrap inside the tooltip's 280-pixel cap.
const RESTART_HELP: &str = "Stop the agent and start it again in the same terminal, keeping the session's history and working directory.";

/// A neutral button, and its tooltip as the layer manager draws it (shadow, padding, wrapping inside the 280-pixel cap).
fn tooltip(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .items_start()
        .w(px(380.))
        .h(px(120.))
        .child(
            Button::new(
                "restart",
                "Restart",
                HoverHelp::tooltip(RESTART_HELP),
                Role::Button,
            )
            .tier(Tier::Neutral),
        )
        .into_any_element()
}

/// Show a long tooltip as if the pointer had come to rest over the button.
fn tooltip_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    layers::show_tooltip_at(
        &state.tooltip_owner,
        RESTART_HELP.into(),
        point(px(40.), px(30.)),
        window,
        cx,
    );
}

/// A neutral button and its ring, as a keyboard user sees the button after tabbing to it.
fn focus_ring(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .gap(px(12.))
        .child(
            Button::new(
                "cancel",
                "Cancel",
                HoverHelp::tooltip("Close"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .focus(&state.ring),
        )
        .child(
            Button::new("other", "Save", HoverHelp::tooltip("Save"), Role::Button)
                .tier(Tier::Primary),
        )
        .into_any_element()
}

/// Press Tab as the keyboard would, so the first button takes focus with the keyboard as the last input.
fn focus_ring_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    window.activate_window();
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

/// The connection-lost notice as the main view draws it, full width, with its Dismiss button focused from the keyboard
/// (the scene's setup is the focus-ring scene's Tab), so the golden pins the banner's red band and the white ring its
/// button wears there instead of the accent ring.
fn notice_banner(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w_full()
        .child(crate::main_view::banner(Notice::ConnectionLost))
        .into_any_element()
}

/// A ghost button that a menu opens below.
fn popover(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let anchor = Anchor::for_element("menu-anchor", window, cx);
    state.anchor.replace(Some(anchor.clone()));
    div()
        .w(px(300.))
        .h(px(160.))
        .child(
            div()
                .relative()
                .w(px(90.))
                .child(Button::new(
                    "actions",
                    "Actions",
                    HoverHelp::tooltip("More actions"),
                    Role::Button,
                ))
                .child(anchor.track()),
        )
        .into_any_element()
}

/// The popover's content: a menu panel with three items.
struct MenuScene;

impl Render for MenuScene {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, label: &'static str| {
            Button::new(id, label, HoverHelp::tooltip(label), Role::MenuItem)
        };
        layers::panel(cx)
            .w(px(180.))
            .child(item("rename", "Rename…"))
            .child(item("restart", "Restart"))
            .child(item("delete", "Delete…"))
    }
}

/// Open the menu below the anchor the scene drew.
fn popover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(anchor) = state.anchor.borrow().clone() else {
        return;
    };
    let view = cx.new(|_| MenuScene);
    let mark = Mark::take(cx);
    let container = cx.focus_handle();
    layers::open_popover(
        Popover {
            view: view.into(),
            container: container.clone(),
            start: container,
            anchor,
        },
        &mark,
        window,
        cx,
    );
}

/// Nothing on the page: the prompt scene draws everything through the layer manager.
fn empty(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div().into_any_element()
}

/// A delete confirmation, asked through the prompt over its veil.
fn prompt_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let question = Question::new(
        "Delete session?",
        Answer {
            label: "Delete".into(),
            help: HoverHelp::tooltip("Delete the session"),
            tier: Tier::Danger,
        },
        "api-refactor".to_string(),
    )
    .summary("api-refactor, on this machine")
    .line("The session's terminal and its history are removed. The working directory stays.");
    prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
}

/// The same delete confirmation with its summary naming the session by a title a host wrote, as the session delete
/// confirmation does: the title's right-to-left override shows as `<U+202E>` inside the dialog's header, and the app's
/// words around it keep their order.
fn prompt_host_text_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let title = HostText::from_host("invoice\u{202E}fdp.exe");
    let question = Question::new(
        "Delete session?",
        Answer {
            label: "Delete".into(),
            help: HoverHelp::tooltip("Delete the session"),
            tier: Tier::Danger,
        },
        "invoice".to_string(),
    )
    .summary(
        HostSentence::new("prompt-summary")
            .host(&title)
            .text(", on this machine"),
    )
    .line("The session's terminal and its history are removed. The working directory stays.");
    prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
}

/// The host-text element over the spoofing corpus, one value per line, then a sentence mixing the app's words with two
/// host values: every hidden or direction-changing character shows as `<U+XXXX>`.
///
/// Only the corpus entries the bundled Latin fonts can draw are pictured: the goldens render with the bundled fonts
/// alone (so a font installed on the machine cannot change a result), and goldens of fallback fonts are not in scope
/// yet, so a CJK or Hebrew value would only picture missing-glyph boxes. The unit tests in `host_text` cover the whole
/// corpus.
fn host_text(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let title = HostText::from_host("invoice\u{202E}fdp.exe");
    let host = HostText::from_host("build\u{200B}box");
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(
            host_text::tests::SPOOFING_CORPUS
                .iter()
                .filter(|(_, shown)| shown.chars().all(|ch| ch < '\u{0250}'))
                .map(|(raw, _)| host_text::element(&HostText::from_host(*raw))),
        )
        .child(
            HostSentence::new("host-text-sentence")
                .text("Delete ")
                .host(&title)
                .text(" on ")
                .host(&host)
                .text("?"),
        )
        .into_any_element()
}

/// One terminal's whole output for a golden scene, in the grid protocol's JSON description, decoded the way a live
/// attachment's frames are: a hello at `cols` by `rows`, plain state, the given rows, and one frame with `cursor`.
/// Panics on a message that does not decode: dropping it would draw a partial terminal, which `--update` would then
/// bake into the reference.
fn grid_output(
    cols: u16,
    rows: u16,
    screen: serde_json::Value,
    cursor: serde_json::Value,
) -> Vec<GridOutput> {
    let json = [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
        serde_json::json!({"t":"rows","rows":screen}),
        serde_json::json!({"t":"frame","rev":1,"cols":cols,"rows":rows,"cursor":cursor}),
    ];
    json.into_iter()
        .map(|value| serde_json::from_value(value).unwrap())
        .collect()
}

/// One run in the protocol's JSON form: `[col, width, text, [flags, fg, bg]]`, colors as wire values (-1 the default,
/// 0-255 a palette index, `0x1000000 | rgb` a truecolor).
fn run(col: u16, width: u16, text: &str, flags: u16, fg: i64, bg: i64) -> serde_json::Value {
    serde_json::json!([col, width, text, [flags, fg, bg]])
}

/// One screen row of runs.
fn screen_row(y: u16, runs: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!([y, {"w": false, "r": runs}])
}

/// A terminal view showing `output` behind no placeholder, with the cursor in `phase`, at the default text size.
/// Panics when the font cannot be measured or the output is refused, for the same reason as [`grid_output`]: an empty
/// terminal must fail the golden, never become its reference.
fn terminal_view(
    output: Vec<GridOutput>,
    phase: CursorPhase,
    cx: &mut App,
) -> Entity<TerminalView> {
    let metrics = CellMetrics::measure(TextSize::DEFAULT, cx).unwrap();
    let view = cx.new(|_| TerminalView::new(metrics));
    view.update(cx, |view, cx| {
        let (_, applied) = view.apply(output, cx);
        applied.unwrap();
        view.mark_replay_complete(cx).unwrap();
        view.set_cursor_phase(phase, cx);
    });
    view
}

/// The terminal's styles in the bundled font: the default colors, the sixteen palette colors as backgrounds, bold
/// brightening, dim, italic (drawn upright: no italic face is bundled yet), inverse, the five underline shapes,
/// strikethrough, hidden text that keeps its underline, a truecolor foreground, samples of the 256-color cube and gray
/// ramp, and box drawing. ASCII and box drawing only: no fallback font is bundled for goldens, because the migration's
/// orchestrator decided on 2026-11-13 that M1 has no fallback-font goldens.
///
/// The box drawing is two rows, a box beside shades and a pair of full blocks, so the picture shows whether lines and
/// blocks meet across cells both ways: programs draw their borders with these characters, and a cell that does not
/// match the glyph box leaves gaps between them. The two rows are the scene's last, rows 7 and 8: in this scene's
/// geometry they are the pair whose baselines GPUI would round a device pixel further apart than the glyph is tall
/// (`Baselines::origin_y` in the terminal module), so the full blocks stacked there show the seam if row baselines stop
/// stepping by whole device rows. The scene is at 14 points, one of the few sizes at which full blocks meet at all
/// (`Baselines`' NOTE has the others).
fn terminal_styles_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use farhelm_grid_model::style_flags as f;
    let palette: Vec<_> = (0..16)
        .map(|i| run(i * 2, 2, "  ", 0, -1, i64::from(i)))
        .collect();
    let bold: Vec<_> = (0..8)
        .map(|i| run(i * 2, 1, "B", f::BOLD, i64::from(i), -1))
        .collect();
    let cube: Vec<_> = [16, 52, 88, 124, 160, 196, 46, 21, 232, 244, 255]
        .into_iter()
        .enumerate()
        .map(|(i, index)| run(i as u16 * 2, 2, "  ", 0, -1, index))
        .collect();
    let screen = serde_json::json!([
        screen_row(0, vec![run(0, 12, "default text", 0, -1, -1)]),
        screen_row(1, palette),
        screen_row(2, bold),
        screen_row(
            3,
            vec![
                run(0, 3, "dim", f::DIM, -1, -1),
                run(4, 6, "italic", f::ITALIC, -1, -1),
                run(11, 7, "inverse", f::INVERSE, -1, -1),
                run(19, 8, "truecolr", 0, 0x1ff_8800, -1),
            ]
        ),
        screen_row(
            4,
            vec![
                run(0, 5, "solid", f::UNDERLINE, -1, -1),
                run(6, 6, "double", f::DOUBLE_UNDERLINE, -1, -1),
                run(13, 4, "wavy", f::UNDERCURL, -1, -1),
                run(18, 6, "dotted", f::DOTTED_UNDERLINE, -1, -1),
                run(25, 6, "dashed", f::DASHED_UNDERLINE, -1, -1),
            ]
        ),
        screen_row(
            5,
            vec![
                run(0, 6, "strike", f::STRIKETHROUGH, -1, -1),
                run(7, 6, "hidden", f::HIDDEN | f::UNDERLINE, -1, -1),
            ]
        ),
        screen_row(6, cube),
        screen_row(7, box_row(["┌", "─", "─", "┐"])),
        screen_row(8, box_row(["└", "─", "─", "┘"])),
    ]);
    let cursor = serde_json::json!({"x":0,"y":7,"visible":false,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(36, 9, screen, cursor),
        CursorPhase {
            focused: true,
            on: true,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// One row of the styles scene's box drawing: a box edge in columns 0-3, the three shades from full to light in 5-7,
/// and two full blocks side by side in 9-10.
fn box_row(edge: [&str; 4]) -> Vec<serde_json::Value> {
    let shades = ["█", "▓", "░", "", "█", "█"];
    let edge = edge.into_iter().enumerate();
    let shades = shades
        .into_iter()
        .enumerate()
        .map(|(i, text)| (i + 5, text));
    edge.chain(shades)
        .filter(|(_, text)| !text.is_empty())
        .map(|(col, text)| run(col as u16, 1, text, 0, -1, -1))
        .collect()
}

/// The cursor's shapes side by side, each its own small terminal: a focused block re-painting its glyph in the accent
/// color, a bar, an underline, an unfocused outline, and a block on a wide character's spacer cell covering the whole
/// two-cell box. The wide run's glyph has no bundled face; the golden pins the cursor's box, not that glyph.
fn terminal_cursors_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let cases = [
        (0, "block", true),
        (0, "bar", true),
        (0, "underline", true),
        (0, "block", false),
        (3, "block", true),
    ];
    for (x, shape, focused) in cases {
        let screen = serde_json::json!([screen_row(
            0,
            vec![
                run(0, 2, "ab", 0, -1, -1),
                run(2, 2, "\u{4e2d}", 0, -1, -1),
                run(4, 2, "cd", 0, -1, -1),
            ]
        )]);
        let cursor = serde_json::json!({"x":x,"y":0,"visible":true,"shape":shape,"blink":false});
        let view = terminal_view(
            grid_output(6, 2, screen, cursor),
            CursorPhase { focused, on: true },
            cx,
        );
        state.terminals.borrow_mut().push(view);
    }
}

/// Two attached terminals whose view another client took over, each showing the red status line with the helm's
/// reason and Take control above its kept screen (the old terminal's `.banner` with `.banner-reclaim`). The first one's
/// program exited, so it also has the old app's exit line (`.grid-terminal-exit`) below its last row, in the terminal's
/// reversed defaults; the second's program still runs. In both the program's last lines sit at the bottom of its screen,
/// where a shell leaves them, and the status line takes room the fit leaves to the grid, so the screen is lifted by
/// whole rows: its top rows go out of view and the last ones stay.
///
/// The owners need a UI session, so the setup connects a fake helm and makes them once its snapshot arrives; each
/// owner's opener plays its whole stream at once (attach, a replay at the fitted size, its end, the takeover), and the
/// settle loop draws until the picture stops changing.
fn terminal_status_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(Vec::new());
    // The subscription is kept in `keep` too, so the closure holds it only weakly: a strong handle would be a cycle that
    // outlives the app and fails its leak check.
    let (attached, keep) = (state.attached.clone(), Rc::downgrade(&state.keep));
    let made = cx.observe(&connection, move |connection, cx| {
        let Some(session) = connection.read(cx).session().cloned() else {
            return;
        };
        if !attached.borrow().is_empty() {
            return;
        }
        let Some(keep) = keep.upgrade() else {
            return;
        };
        for exit in [Some(2), None] {
            let target = TerminalTarget {
                session: "golden".into(),
                tab: None,
                lease: AttachmentLease::new(),
            };
            let metrics = CellMetrics::measure(TextSize::DEFAULT, cx).unwrap();
            let open = taken_over_stream(exit, keep.clone());
            let owner = cx.new(|cx| AttachedTerminal::new(&session, target, metrics, open, cx));
            attached.borrow_mut().push(owner);
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(made));
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// An opener whose fake stream replays a short session at the bottom of the fitted screen, with the program exited
/// with status `exit` if given, and is then taken over by another client. The fake is kept in `keep`.
fn taken_over_stream(
    exit: Option<i32>,
    keep: Rc<RefCell<Vec<Box<dyn std::any::Any>>>>,
) -> OpenStream {
    Box::new(move |attach| {
        let (mut fake, handle, events) = FakeTerminal::open(&attach);
        let asked = fake.attach();
        let last = asked.rows.saturating_sub(1);
        let (ending, cursor_x) = match exit {
            Some(_) => ("$ exit", 6),
            None => ("$ ", 2),
        };
        let screen = serde_json::json!([
            screen_row(last - 2, vec![run(0, 6, "$ make", 0, -1, -1)]),
            screen_row(last - 1, vec![run(0, 17, "error: tests fail", 0, 1, -1)]),
            screen_row(last, vec![run(0, ending.len() as u16, ending, 0, -1, -1)]),
        ]);
        let cursor =
            serde_json::json!({"x":cursor_x,"y":last,"visible":true,"shape":"block","blink":false});
        let mut output = grid_output(asked.cols, asked.rows, screen, cursor);
        if let Some(code) = exit {
            let exited = serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
                "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":{"code":code}});
            output.push(serde_json::from_value(exited).unwrap());
        }
        for message in output {
            fake.output(message);
        }
        fake.replay_complete();
        fake.detach(
            Some(farhelm_proto::DetachCode::TakenOver),
            farhelm_proto::ATTACH_REFUSED_TAKEN_OVER,
        );
        keep.borrow_mut().push(Box::new(fake));
        (handle, events)
    })
}

/// The attached terminals a scene made, one above the other, each in a pane of a fixed size.
fn attached_terminals(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(
            state
                .attached
                .borrow()
                .iter()
                .map(|owner| div().w(px(420.)).h(px(150.)).child(owner.clone())),
        )
        .into_any_element()
}

/// The terminals a terminal scene's setup made, in a row.
fn terminals(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .gap(px(12.))
        .children(state.terminals.borrow().iter().cloned())
        .into_any_element()
}

static SCENES: [Scene; 13] = [
    Scene {
        name: "buttons",
        size: (560., 64.),
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
        name: "dialog",
        size: (540., 300.),
        draw: dialog,
        setup: no_setup,
    },
    Scene {
        name: "tooltip",
        size: (400., 140.),
        draw: tooltip,
        setup: tooltip_setup,
    },
    Scene {
        name: "focus-ring",
        size: (260., 64.),
        draw: focus_ring,
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
        name: "host-text",
        size: (420., 440.),
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

/// Every scene in both appearances matches its reference exactly. Run through `scripts/check-gui-goldens.py`, which
/// selects the headless renderer; `FARHELM_UPDATE_GOLDENS=1` (set by that script's `--update`) rewrites references.
#[test]
#[ignore = "needs the headless renderer selected; run scripts/check-gui-goldens.py"]
fn component_goldens() -> anyhow::Result<()> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let references = manifest.join("tests/goldens").join(std::env::consts::OS);
    // Captures go beside the test binary, inside the build directory, so a failure leaves the new picture to compare.
    let captures = std::env::current_exe()?
        .parent()
        .map(|deps| deps.join("gui-goldens"))
        .ok_or_else(|| anyhow::anyhow!("the test binary has no directory"))?;
    let update = std::env::var_os("FARHELM_UPDATE_GOLDENS").is_some_and(|value| value == "1");
    std::fs::create_dir_all(&captures)?;
    if update {
        std::fs::create_dir_all(&references)?;
    }
    let mut failures = Vec::new();
    for scene in &SCENES {
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
                        tooltip_owner: cx.new(|_| TooltipOwner),
                        anchor: Rc::new(RefCell::new(None)),
                        ring: cx.focus_handle().tab_stop(true),
                        terminals: Rc::new(RefCell::new(Vec::new())),
                        attached: Rc::new(RefCell::new(Vec::new())),
                        keep: Rc::new(RefCell::new(Vec::new())),
                    };
                    let scene_view = cx.new(|cx| SceneView {
                        scene,
                        state,
                        _layers: layers::attach(window, cx),
                        focus: focus::attach(window, cx),
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
            let reference = references.join(&name);
            if update {
                screenshot.save(&reference)?;
                continue;
            }
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
    anyhow::ensure!(
        failures.is_empty(),
        "golden failures: {}",
        failures.join("; ")
    );
    Ok(())
}

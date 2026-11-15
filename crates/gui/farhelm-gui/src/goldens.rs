//! Headless screenshot goldens: one per component, per dialog frame and per overlay kind (tooltip, once more with help
//! in paragraphs, popover, the prompt over its veil, once more with a summary that names a host value, once more with a
//! feature-drawn body, a header tile and a footer note, and the prompt anchored to a row's menu button), the keyboard
//! focus ring, host-supplied text over its spoofing corpus, the notice banner, the terminal's styles and cursor shapes,
//! an attached terminal's exit line and status line, the scrim a pending Delete puts over the open session's terminal,
//! the interrupted card in its place (with Restart and Replace, and with Replace alone), the open session's details bar
//! (every status and host it draws differently, and a narrow one, and once more with a copy confirmed and its
//! hidden-characters warning), the session list (a fleet of every status a row draws, the open row on a host the helm
//! cannot reach, an empty fleet, a row's open menu, a row's notification list, a row's Delete question, and a pending
//! and a refused Delete), the Rename editor, the Send feedback dialog (as it opens, and with a remembered contact), the
//! select closed and open, the YOLO confirmation drawn inline in a launcher's body with its outlined permanent answer,
//! the Settings dialog at Sessions, at Appearance, at Hosts, at Recipes (empty, with recipes in each build state, with
//! a recipe's per-place lines, and with a new recipe's editor open), at Credentials, at Terminal and at Agent trust,
//! the Hosts panel (closed, with Details, with a host failing to connect, with a host's setup running and failed, with
//! outdated hosts' inline Update, and with a host's Update running), the Add host dialog and the setup confirmation it
//! leads to, a host's actions menu, the host settings dialog at its top and scrolled to its end, the host refusal
//! dialog for a refused Update, the version readout while an updater runs (the check button at rest, lit, warned and
//! focused; work in progress; a check's outcomes; the update-menu toggle; its open menu; and the reinstall hover), the
//! title bar the Linux window draws for itself (ordinary and maximized), the Open-Source Notices window over a short
//! fixture, and the Trash button empty, full and freeing space, and the Trash dialog, alone and asking to delete
//! everything or one host's archives, in both appearances, compared pixel for pixel. (The terminal stays dark in both
//! appearances, so it draws identically in both pictures; only the scene's ground around it differs.)
//!
//! The agent approval card has its own two scenes, wide and narrow, and the Add host dialog as an agent's host add
//! request opens it one more, in both appearances. A dialog's notices made a stop of their own have one too, focused
//! with the keyboard's ring.
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

use std::cell::Cell;

use farhelm_proto::DeleteGuard;

use crate::assets::Icon;
use crate::components::ScrollBox;
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, IconButton, Role, TextInput, TextInputState, Tier,
};
use crate::components::{RadioGroup, RadioOption};
use crate::components::{Select, SelectOption};
use crate::components::{SequenceChip, SequencePill};
use crate::components::{TabOption, TabStrip};
use std::cell::RefCell;
use std::rc::Rc;

use crate::desktop::updates;
use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::HostsPanel;
use crate::hosts::identity::host_mark;
use crate::layers::{self, Anchor, Popover, TooltipOwner};
use crate::local_state;
use crate::notices::Notice;
use crate::operations::OpKey;
use crate::prompt::{self, Answer, Question};
use crate::root;
use crate::session;
use crate::session_list::SessionList;
use crate::session_list::filters::Status;
use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::tests::{
    Fixture, TEST_NOW, listing, preferences, preferences_state, session,
};
use crate::terminal::attached::{AttachedTerminal, OpenStream, TerminalTarget};
use crate::terminal::{CellMetrics, CursorPhase, TerminalView, TextSize};
use crate::test_support;
use crate::theme::{self, Preference};
use crate::ticker;
use crate::update_readout::view::VersionReadout;
use crate::update_readout::{self, Activity, Readout};
use farhelm_grid_model::GridOutput;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::FakeTerminal;
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{
    ANNOUNCEMENTS_KEY, Availability, Collection, CollectionChanges, CollectionMeta,
    CollectionState, HostView, KeyedEntry, ListCounts, PREFERENCES_KEY, Preferences,
};

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

/// The checkbox, off, on, and on but unavailable: the old app's native checkbox tinted with the accent.
fn checkboxes(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    let checkbox =
        |id: &'static str, label: &'static str| Checkbox::new(id, label, help(), Role::CheckBox);
    div()
        .flex()
        .gap(px(16.))
        .items_center()
        .child(checkbox("off", "Resume"))
        .child(checkbox("on", "Resume").checked(true))
        .child(checkbox("on-off", "Resume").checked(true).disabled(true))
        .into_any_element()
}

/// The Settings switch, off, on, and on but unavailable (`.control-switch` in the old stylesheet).
fn switches(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    let switch =
        |id: &'static str| Checkbox::new(id, "Play a sound", help(), Role::Switch).switch();
    // Each in a box as wide as a narrow Settings row, since a switch row spans its container.
    let row = |switch: Checkbox| div().w(px(200.)).child(switch);
    div()
        .flex()
        .gap(px(24.))
        .items_center()
        .child(row(switch("off")))
        .child(row(switch("on").checked(true)))
        .child(row(switch("on-off").checked(true).disabled(true)))
        .into_any_element()
}

/// Every button tier but the outlined danger one, a disabled main action and an icon button, in a first row; in a
/// second, the host rail's entries (All pressed, a host's mark not pressed, and a host's mark pressed in the quieter
/// look the rail's choice wears while Hosts management is shown, on the rail's ground), the filter chip with
/// a mark, a name and its ✕, and the link tier. The outlined danger tier is pinned at rest by `inline-prompt`; its
/// unavailable and busy looks are not pinned yet.
fn buttons(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    let palette = theme::palette(cx);
    let rail = div()
        .flex()
        .gap(px(8.))
        .p(px(7.))
        .bg(theme::color(palette.bg_1))
        .child(
            Button::new("rail-all", "All hosts", help(), Role::Button)
                .tier(Tier::Rail)
                .pressed(true)
                .content(div().child("All")),
        )
        .child(
            Button::new("rail-host", "build-box", help(), Role::Button)
                .tier(Tier::Rail)
                .pressed(false)
                .content(host_mark(
                    HostIcon::Database,
                    HostColor::Teal,
                    px(24.),
                    palette,
                )),
        )
        .child(
            Button::new("rail-quiet", "render-box", help(), Role::Button)
                .tier(Tier::Rail)
                .pressed(true)
                .quiet(true)
                .content(host_mark(
                    HostIcon::Cloud,
                    HostColor::Default,
                    px(24.),
                    palette,
                )),
        );
    let chip = Button::new(
        "chip",
        "Filtered to host build-box; remove filter",
        help(),
        Role::Button,
    )
    .tier(Tier::Chip)
    .content(
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(host_mark(
                HostIcon::Laptop,
                HostColor::Red,
                px(14.),
                palette,
            ))
            .child("build-box")
            .child("✕"),
    );
    let second = div()
        .flex()
        .gap(px(8.))
        .items_center()
        .child(rail)
        .child(chip)
        .child(Button::new("link", "Show all hosts", help(), Role::Button).tier(Tier::Link));
    let first = div()
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
        ));
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(first)
        .child(second)
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

/// Fill the two growing boxes: one with four lines, the last long enough to wrap, so the box grows past its three
/// starting rows to fit them; and one with twenty lines, past the cap, so it stops at the cap and shows the first rows.
fn growing_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    state.growing[0].set_text(
        "Fix the reconnect loop.\nKeep the draft when the helm restarts.\n\nThen run the smoke test and report \
         what it printed, every line of it.",
        window,
        cx,
    );
    let many: Vec<String> = (1..=20).map(|n| format!("line {n}")).collect();
    state.growing[1].set_text(many.join("\n"), window, cx);
}

/// The growing multi-line box twice: grown from three rows to its four lines (one of them wrapped, one empty), and held
/// at its cap with twenty lines, scrolled to the top.
fn growing(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("A first message for the agent");
    div()
        .flex()
        .gap(px(12.))
        .items_start()
        .child(div().w(px(280.)).child(TextInput::new(
            "grown",
            &state.growing[0],
            "First message",
            help(),
        )))
        .child(div().w(px(280.)).child(TextInput::new(
            "capped",
            &state.growing[1],
            "First message",
            help(),
        )))
        .into_any_element()
}

/// Fill the two fields that keep line breaks: one with an ordinary title, and one with two lines, each wider than the
/// field, as a paste might bring. The long lines are what make the second picture change if the field ever wrapped or
/// grew: a short value would look the same either way.
fn kept_line_breaks_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    state.kept[0].set_text("api-refactor", window, cx);
    state.kept[1].set_text(
        "fix the reconnect loop that drops the session list\nand keep the draft when the helm restarts",
        window,
        cx,
    );
}

/// The field that keeps line breaks, twice: holding one line, where it has the single-line input's fill, border and
/// type (and the old Rename field's height, see [`TextInput`]), and holding two long lines, of which it shows one row,
/// clipped at the field's edge rather than wrapped.
fn kept_line_breaks(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w(px(280.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(state.kept.iter().enumerate().map(|(index, kept)| {
            TextInput::new(
                format!("title-{index}"),
                kept,
                "Session title",
                HoverHelp::tooltip("The session's new title"),
            )
        }))
        .into_any_element()
}

/// The dialog frame with a title, a summary, the header Close, a short body and a pinned Cancel/Delete pair.
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
    .close(
        "Close delete",
        HoverHelp::tooltip("Close this without deleting"),
        false,
        |_, _, _| {},
    )
    .into_any_element()
}

/// The dialog frame with a question in its notices whose answers are both disabled, as the old Trash dialog's global
/// Delete all question's were while a deletion was in flight, so the notices are a stop of their own
/// ([`DialogFrame::notices_stop`]). The setup's Tab focuses that stop, which comes first (the body holds no control and
/// the header no Close), so the golden pins its ring: its sides at the card's inner edge, as the body's ring lies, and
/// its top and bottom two pixels outside the notices. The question is drawn in the prompt's loud look, as the Trash
/// dialog asks it (`prompt.rs`: 16-pixel text on 20-pixel lines, 6 pixels between them, 10 by 12 pixels of padding
/// inside a 2-pixel red edge on the red wash), so the ring is pinned around what a real dialog's notices hold.
fn dialog_notices_stop(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = crate::theme::palette(cx);
    let answer = |id: &'static str, label: &'static str, tier: Tier| {
        Button::new(
            id,
            label,
            HoverHelp::tooltip("Golden scene button"),
            Role::Button,
        )
        .tier(tier)
        .disabled(true)
    };
    let question = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .py(px(10.))
        .px(px(12.))
        .text_size(px(16.))
        .line_height(px(20.))
        .bg(crate::theme::color(palette.danger_wash))
        .border_2()
        .border_color(crate::theme::color(palette.danger))
        .rounded(crate::theme::metrics::RADIUS_MD)
        .child("Delete 3 archived checkouts (1.2 GB) on this machine, build-box?")
        .child("Work in them that was never committed or pushed is lost permanently.")
        .child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .child(answer("question-cancel", "Cancel", Tier::Neutral))
                .child(answer(
                    "question-submit",
                    "Delete permanently",
                    Tier::Danger,
                )),
        );
    DialogFrame::new(
        "trash",
        "Trash",
        div().child("3 archived checkouts on reachable hosts."),
        vec![
            Button::new(
                "close",
                "Close",
                HoverHelp::tooltip("Golden scene button"),
                Role::Button,
            )
            .tier(Tier::Neutral),
        ],
    )
    .notices(question)
    .notices_stop("Global checkout deletion confirmation")
    .into_any_element()
}

/// The dialog frame as the session launcher draws it: the icon tile before the title, a width of its own, a summary
/// leading the answers' row and a notice pinned above it. Pins the tile's icon, which paints only with a color set on
/// the svg itself, and the footer's layout with a lead.
fn dialog_launcher_frame(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let help = |text: &'static str| HoverHelp::tooltip(text);
    DialogFrame::new(
        "launch",
        "New session",
        div().child("The launcher's fields go here."),
        vec![
            Button::new(
                "cancel",
                "Cancel",
                help("Golden scene button"),
                Role::Button,
            )
            .tier(Tier::Neutral),
            Button::new(
                "reset",
                "Reset choices",
                help("Golden scene button"),
                Role::Button,
            )
            .tier(Tier::Neutral),
            Button::new(
                "launch",
                "Launch host1",
                help("Golden scene button"),
                Role::Button,
            )
            .content("Launch")
            .tier(Tier::Primary)
            .refused(true),
        ],
    )
    .summary("Start an agent or a command on any host")
    .tile(
        gpui::svg()
            .path(Icon::Launch.path())
            .size(px(22.))
            .text_color(crate::theme::color(crate::theme::palette(cx).fg_0)),
    )
    .width(px(600.))
    // The launcher's footer summary is 12-pixel text and its notice 13-pixel, as here.
    .footer_lead(
        div()
            .flex()
            .flex_col()
            .text_size(crate::theme::metrics::TEXT_SM)
            .child("Codex · host1 · ~/src/farhelm")
            .child("Model: Default · Effort: Default · Permissions: Default"),
    )
    .notices(
        div()
            .text_size(crate::theme::metrics::TEXT_MD)
            .child("Not launched. Choose a structured harness before launching"),
    )
    .close(
        "Close session launcher",
        help("Golden scene button"),
        false,
        |_, _, _| {},
    )
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

/// The paragraph tooltip scene's help: a guarded Codex launch's agent-mark help, as the old app worded it (the mode
/// and its explanation under a line break, the flags, the caveat, the stored command in the monospace face), which runs
/// past the short tooltip's eight lines.
fn guarded_codex_help() -> Vec<HostSentence> {
    let command = HostText::from_host("codex -a on-request -s workspace-write");
    vec![
        HostSentence::new("golden-help-mode").text(
            "Codex — Guarded · Codex sandboxed auto\nCodex runs within its workspace sandbox and asks before stepping \
             outside it. An approval reviewer may decide requests according to your configuration.",
        ),
        HostSentence::new("golden-help-flags").text("Flags: -a on-request -s workspace-write"),
        HostSentence::new("golden-help-caveat").text(
            "Farhelm starts the agent with these choices, but can't guarantee how it behaves. Your own allow \
             rules, hooks, plugins, always-allow answers, sandbox settings or switching modes inside the session \
             can loosen it, and agents change between versions. See About permission modes in the agent \
             documentation.",
        ),
        HostSentence::new("golden-help-command")
            .text("Launched with: ")
            .mono(&command),
    ]
}

/// A neutral button, and paragraph help as the layer manager draws it: the wider card, a blank line between
/// paragraphs, and no eight-line cut.
fn tooltip_paragraphs(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .items_start()
        .w(px(500.))
        .h(px(300.))
        .child(
            Button::new(
                "agent",
                "Agent",
                HoverHelp::paragraphs(guarded_codex_help()),
                Role::Button,
            )
            .tier(Tier::Neutral),
        )
        .into_any_element()
}

/// Show the paragraph help as if the pointer had come to rest over the button.
fn tooltip_paragraphs_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    layers::show_tooltip_at(
        &state.tooltip_owner,
        layers::Help::Paragraphs(guarded_codex_help()),
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
    press_tab(window, cx);
}

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

/// The scroll box twice, as an agent approval's card draws its commands, in the monospace face: one taller than its
/// box, which the setup's Tab focuses, so the golden pins its ring two pixels in over the border, and a short one at
/// rest, held to its least height.
fn scroll_boxes(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    let command = |count: usize| {
        div().children((1..=count).map(|n| div().child(format!("--option-{n} value"))))
    };
    let column = |scroll_box: ScrollBox| div().w(px(220.)).child(scroll_box);
    div()
        .flex()
        .gap(px(16.))
        .child(column(
            ScrollBox::new("long", "Command", help(), Role::Region, command(12))
                .monospace()
                .max_height(px(96.)),
        ))
        .child(column(
            ScrollBox::new("short", "Resume command", help(), Role::Region, command(1))
                .monospace()
                .min_height(px(48.))
                .max_height(px(96.)),
        ))
        .into_any_element()
}

/// The tab strip as the launcher draws it: Agent chosen, Command not (`.launch-kind-tabs`).
fn tab_strip(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = |text: &'static str| HoverHelp::tooltip(text);
    div()
        .w(px(280.))
        .child(
            TabStrip::new("launch-kind", "Launch kind", help("Golden scene group"))
                .tab(TabOption::new("Agent", help("Golden scene tab"), "agent"))
                .tab(TabOption::new(
                    "Command",
                    help("Golden scene tab"),
                    "command",
                ))
                .chosen("agent"),
        )
        .into_any_element()
}

/// The segmented control both ways the old app used it: a radio group (Appearance's theme) and a button group (the
/// launcher's effort, one option unavailable), with an unavailable group below. The setup's Tab puts focus on the
/// first group's choice, so the golden also pins the inset ring in the inverse ink that a chosen segment wears.
fn segmented_control(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = |text: &'static str| HoverHelp::tooltip(text);
    let options = |group: RadioGroup<&'static str>, labels: &[&'static str]| {
        labels.iter().fold(group, |group, &label| {
            group.option(RadioOption::new(label, help("Golden scene option"), label))
        })
    };
    let theme = RadioGroup::new(
        "theme",
        "Color theme",
        help("Golden scene group"),
        Role::RadioGroup,
    )
    .chosen(Some("System"));
    let effort = RadioGroup::new("effort", "Effort", help("Golden scene group"), Role::Group)
        .chosen(Some("Medium"))
        .option(RadioOption::new(
            "Default",
            help("Golden scene option"),
            "Default",
        ))
        .option(RadioOption::new(
            "Medium",
            help("Golden scene option"),
            "Medium",
        ))
        .option(RadioOption::new("High", help("Golden scene option"), "High").disabled(true));
    let unavailable = RadioGroup::new(
        "off",
        "Status",
        help("Golden scene group"),
        Role::RadioGroup,
    )
    .chosen(Some("All"))
    .disabled(true);
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(options(theme, &["System", "Dark", "Light"]))
        .child(effort)
        .child(options(unavailable, &["All", "Running", "Idle"]))
        .into_any_element()
}

/// Plain radios, as the launcher's "Runs in YOLO mode" drew them: the required choice with nothing chosen yet, the
/// same choice made, and made but unavailable.
fn radio_group(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let yolo = |id: &'static str, chosen: Option<bool>| {
        RadioGroup::new(
            id,
            "Runs in YOLO mode",
            HoverHelp::tooltip("Golden scene group"),
            Role::RadioGroup,
        )
        .radios()
        .chosen(chosen)
        .option(RadioOption::new(
            "Yes (YOLO)",
            HoverHelp::tooltip("Golden scene option"),
            true,
        ))
        .option(RadioOption::new(
            "No",
            HoverHelp::tooltip("Golden scene option"),
            false,
        ))
    };
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(yolo("unanswered", None))
        .child(yolo("answered", Some(false)))
        .child(yolo("unavailable", Some(true)).disabled(true))
        .into_any_element()
}

/// The radio group's launcher and Settings variants, to compare with the old launcher with Codex chosen and the old
/// Settings' Terminal page: the harness row's chips in no group, the first chip focused from the keyboard (the setup's
/// Tab) so the golden pins a chip's outside ring; the destination type's glyph segments (with stand-in glyphs: the old
/// folder and branch are not icons yet); the permission modes with the red unchosen YOLO, then the same chosen;
/// glyph-only segments; and the terminal link choices with their description lines, stacked.
fn radio_group_variants(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = |text: &'static str| HoverHelp::tooltip(text);
    let option = |label: &'static str| RadioOption::new(label, help("Golden scene option"), label);
    let harness = ["Codex", "Claude", "Muse", "Cursor", "Grok"]
        .into_iter()
        .fold(
            RadioGroup::new(
                "harness",
                "Harness",
                help("Golden scene group"),
                Role::GenericContainer,
            )
            .chips()
            .chosen(Some("Claude")),
            |group, label| group.option(option(label)),
        );
    let destination = RadioGroup::new(
        "where",
        "Destination type",
        help("Golden scene group"),
        Role::Group,
    )
    .chosen(Some("Folder"))
    .option(option("Folder").icon(Icon::HostHouse))
    .option(option("Managed checkout").icon(Icon::HostCloud));
    let permissions = |id: &'static str, chosen: &'static str| {
        RadioGroup::new(
            id,
            "Permission mode",
            help("Golden scene group"),
            Role::Group,
        )
        .chosen(Some(chosen))
        .option(option("Guarded"))
        .option(option("YOLO").danger())
        .option(option("Custom"))
    };
    let glyphs = RadioGroup::new("glyphs", "Host", help("Golden scene group"), Role::Group)
        .chosen(Some("Local"))
        .option(option("Local").icon_only(Icon::HostLaptop))
        .option(option("Remote").icon_only(Icon::HostCloud));
    let links = RadioGroup::new(
        "links",
        "Terminal web links",
        help("Golden scene group"),
        Role::RadioGroup,
    )
    .chosen(Some("Open in Farhelm, don't pass the click on"))
    .option(option("Open in Farhelm, don't pass the click on").description(
        "Farhelm opens the link once. The program does not receive that click or a drag starting there.",
    ))
    .option(option("Open in Farhelm and also pass the click on").description(
        "Use this when the program needs link clicks. A program that opens links itself may open a second tab.",
    ))
    .option(option("Pass the click on only").description(
        "Let the program handle links. Farhelm does not open them; without mouse reporting, \
         clicking a link does nothing.",
    ));
    div()
        .w(px(520.))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(harness)
        .child(destination)
        .child(
            div()
                .flex()
                .gap(px(12.))
                .child(permissions("guarded", "Guarded"))
                .child(permissions("yolo", "YOLO"))
                .child(glyphs),
        )
        .child(div().w_full().child(links))
        .into_any_element()
}

/// An agent's approval card as the old app's reference screen showed it: an inherited spawn's command launch from this
/// machine, with the narrow and fleet-wide grants offered, at the card's full width. Pins the badge, the From line,
/// the facts with the Permissions chip, a box and the answer bar with Deny apart.
fn approval_card(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    approval_scene(approval_spawn(), 660., cx)
}

/// The same spawn at the old minimum window's card width, from a host with a long name: labels stacked over their
/// values, the From line wrapping with its ID and Copy ID under the title, and the answer buttons wrapping onto lines of
/// their own, each "Always allow" button shrinking and wrapping its host name inside the card rather than running past
/// it (the old stylesheet: "the button itself must shrink and wrap so the narrow pane never needs sideways
/// scrolling").
fn approval_card_narrow(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let host = "build-workstation-west-02.example";
    let mut approval = approval_spawn();
    approval.host_name = host.to_owned();
    approval.session.host_name = Some(host.to_owned());
    let farhelm_proto::approvals::ApprovalAction::Launch { host_name, .. } = &mut approval.action
    else {
        unreachable!("the approval scenes start from a launch");
    };
    *host_name = host.to_owned();
    approval_scene(approval, 284., cx)
}

/// The inherited spawn the approval card scenes start from.
fn approval_spawn() -> farhelm_proto::approvals::PendingApproval {
    use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, LaunchVerb, PendingApproval};
    PendingApproval {
        id: "golden".to_owned(),
        host_id: 1,
        host_name: "this machine".to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:a75bff30-1c2d-4e5f-8a9b-0c1d2e3f27a4"
                .to_owned(),
            title: Some("Review agent request".to_owned()),
            host_name: Some("this machine".to_owned()),
        },
        action: ApprovalAction::Launch {
            verb: LaunchVerb::SpawnInherited,
            host_name: "this machine".to_owned(),
            cwd: "/tmp".to_owned(),
            repo: None,
            title: None,
            launch: farhelm_proto::SessionLaunch::Command(farhelm_proto::CommandLaunch {
                command: "claude {farhelm_args}".to_owned(),
                yolo: false,
                agent: Some(farhelm_proto::LaunchHarness::Claude),
                resume: None,
            }),
            source: None,
        },
        own_host_session_action: true,
        expires_at_ms: 1,
    }
}

/// `approval` drawn as an expanded card `width` pixels wide, one request waiting, nothing in flight.
fn approval_scene(
    approval: farhelm_proto::approvals::PendingApproval,
    width: f32,
    cx: &mut App,
) -> AnyElement {
    use crate::approvals::card::{CardState, card};
    use crate::approvals::model::{Card, TunnelOffer};
    let model = Card::of(&approval, TunnelOffer::NotOffered);
    let state = CardState {
        width: px(width),
        waiting: 1,
        busy: false,
        refusal: None,
        measure: None,
        body_scroll: None,
    };
    div()
        .p(px(12.))
        .child(card(&model, &state, Rc::new(|_, _, _| {}), cx))
        .into_any_element()
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

/// The quick-launch sequence pill as it shows outside the launcher, centered over an area the size of a small main pane:
/// a bound number key's template, an unbound key and the ⚡ template as chips, the word `web` in progress and its
/// candidates hanging below the pill. There is no reference picture of the old pill; it was compared with the old
/// stylesheet's `.launch-sequence-*` rules instead.
fn sequence_pill(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w(px(640.))
        .h(px(200.))
        .flex()
        .items_center()
        .justify_center()
        .child(
            SequencePill::new(
                "Launch sequence: ",
                "Type names or number keys · Tab complete · Enter launch · Esc cancel",
            )
            .chip(SequenceChip::Name(HostText::from_host("my-codex")))
            .chip(SequenceChip::Words("Unbound key 3".into()))
            .chip(SequenceChip::Name(HostText::from_host("webbuilder")))
            .word(HostText::from_host("web"))
            .candidates(["web", "web-api", "webbuilder"].map(HostText::from_host)),
        )
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
        Popover::menu(view, container.clone(), anchor, container),
        &mark,
        window,
        cx,
    );
}

/// The launcher's Agent type choices, as a select's options: None, then two harnesses, then one that cannot be chosen,
/// whose reason runs past a narrow list.
fn agent_options() -> Vec<SelectOption> {
    let help = || HoverHelp::tooltip("Golden scene option");
    vec![
        SelectOption::new("none", "None", help()),
        SelectOption::new("claude", "Claude", help()),
        SelectOption::new("codex", "Codex", help()),
        SelectOption::new("cursor", "Cursor", help()).unavailable("Not installed on this host"),
    ]
}

/// The closed select three ways, each 160 pixels wide: an option chosen, nothing chosen with a placeholder, and
/// unavailable with an option chosen (`.launch-composer-effort-select` and the native select's chevron).
fn selects(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let select = |id: &'static str| {
        Select::new(
            id,
            "Agent type",
            HoverHelp::tooltip("Golden scene control"),
            Role::ComboBox,
        )
        .options(agent_options())
    };
    let boxed = |select: Select| div().w(px(160.)).child(select);
    div()
        .flex()
        .gap(px(16.))
        .items_center()
        .child(boxed(select("chosen").chosen("codex")))
        .child(boxed(select("placeholder").placeholder("Choose a host")))
        .child(boxed(select("off").chosen("codex").disabled(true)))
        .into_any_element()
}

/// A select 200 pixels wide whose list the setup opens from the keyboard, with room below for the list: the anchored
/// menu standing in for the native pop-up, at the control's width, Codex chosen and focused with the keyboard ring, and
/// the unavailable option's reason cut at the list's edge.
fn select_open(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w(px(200.))
        .h(px(200.))
        .child(
            Select::new(
                "agent",
                "Agent type",
                HoverHelp::tooltip("Golden scene control"),
                Role::ComboBox,
            )
            .options(agent_options())
            .chosen("codex")
            .focus(&state.ring),
        )
        .into_any_element()
}

/// Tab to the select (the focus-ring scene's setup), then Space, which opens its list.
fn select_open_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    focus_ring_setup(state, window, cx);
    window.dispatch_event(
        gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            native_key: None,
            keystroke: gpui::Keystroke {
                key: "space".into(),
                ..Default::default()
            },
            is_held: false,
            prefer_character_input: false,
        }),
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

/// A placeholder body for a question: a lead sentence naming a host, with Copy plan beside it, then three plain lines,
/// on the old plan's well. It pins where the prompt puts a feature's body and that a control in it is drawn, not any
/// feature's look: the host setup plan's numbered steps belong to the settings and hosts area.
struct PlaceholderBody {
    copy: FocusHandle,
}

impl Render for PlaceholderBody {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let lead = HostSentence::new("body-lead")
            .text("Farhelm will set up ")
            .host(&HostText::from_host("dev@new-box"))
            .text(":");
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .py(px(7.))
            .px(px(9.))
            .bg(theme::color(palette.well))
            .border_1()
            .border_color(theme::color(palette.border_dim))
            .rounded(theme::metrics::RADIUS_SM)
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.))
                    .child(div().flex_1().min_w_0().child(lead))
                    .child(
                        Button::new(
                            "copy-plan",
                            "Copy plan",
                            HoverHelp::tooltip("Copy the complete host plan"),
                            Role::Button,
                        )
                        .tier(Tier::Neutral)
                        .focus(&self.copy),
                    ),
            )
            .child("1. Create directories")
            .child("2. Install the farhelm binary")
            .child("3. Enable and start the supervisor")
    }
}

/// A question with a feature-drawn body, a header tile and a footer note, at the host setup confirmation's width
/// (the frame compared with reference `host-add-setup-plan`): the cloud mark in the tile, a placeholder body, the
/// "don't ask again" note wrapping above the answers, which stay right-aligned, then Cancel, the permanent answer and
/// Set up, focus on Set up.
fn prompt_body_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let answer = |label: &'static str, help: &'static str, tier: Tier| Answer {
        label: label.into(),
        help: HoverHelp::tooltip(help),
        tier,
    };
    let question = Question::new(
        "Set up host",
        answer(
            "Set up",
            "Set up: set up this host as the plan above describes",
            Tier::Primary,
        ),
        "dev@new-box".to_string(),
    )
    .summary("Farhelm is not running there yet")
    .cancel_answer(answer(
        "Cancel",
        "Cancel: change nothing on this host",
        Tier::Neutral,
    ))
    .alternative(
        answer(
            "Set up, don't ask again",
            "Set up this host, and set up new hosts from now on without asking",
            Tier::Neutral,
        ),
        |_, _, _| {},
    )
    .body(|_, _, cx| {
        cx.new(|cx| PlaceholderBody {
            copy: cx.focus_handle().tab_stop(true),
        })
    })
    .tile(|_, cx| {
        host_mark(
            HostIcon::Cloud,
            HostColor::Default,
            px(24.),
            theme::palette(cx),
        )
    })
    .note("Undo \"don't ask again\" with the gear at the foot of the navigation rail")
    .width(px(560.));
    prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
}

/// A row's menu button, as the session list will draw it at the end of a row, that the anchored question opens below.
fn anchored_prompt(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let anchor = Anchor::for_element("row-menu-anchor", window, cx);
    state.anchor.replace(Some(anchor.clone()));
    div()
        .w(px(300.))
        .h(px(150.))
        .child(
            div()
                .relative()
                .w(px(26.))
                .child(IconButton::new(
                    "row-menu",
                    Icon::More,
                    "Session actions for migrate auth tokens",
                    HoverHelp::tooltip("Session actions"),
                    Role::Button,
                ))
                .child(anchor.track()),
        )
        .into_any_element()
}

/// The row Delete question as the old app showed it for a running session (reference `row-delete-confirmation`): the
/// consequence, the session's title quoted, and Cancel beside "Confirm delete", asked below the row's menu button.
fn anchored_prompt_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(anchor) = state.anchor.borrow().clone() else {
        return;
    };
    let mark = Mark::take(cx);
    let title = HostText::from_host("migrate auth tokens");
    let question = Question::new(
        "Delete confirmation",
        Answer {
            label: "Confirm delete".into(),
            help: HoverHelp::tooltip("Delete this session and its state now"),
            tier: Tier::Danger,
        },
        "s1".to_string(),
    )
    .line("Still running \u{2014} deleting kills the agent:")
    .line(
        HostSentence::new("prompt-title")
            .text("\"")
            .host(&title)
            .text("\""),
    )
    .keeps_focus_on_empty_click();
    prompt::ask_anchored(question, anchor, None, &mark, |_, _, _| {}, window, cx);
}

/// A stand-in for the session launcher's body, on the dialog's surface, drawing the inline question its setup asked
/// (kept in the scene state's `keep`).
fn inline_prompt(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let question = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<prompt::Inline<String>>())
        .cloned();
    div()
        .w(px(960.))
        .p(px(20.))
        .bg(theme::color(palette.bg_2))
        .children(question)
        .into_any_element()
}

/// SPEC.md's YOLO confirmation as the old launcher drew it (reference `launcher-yolo-confirmation`): the loud block,
/// its heading, what YOLO means for the host, why this launch is one and that nothing started, then Cancel, the guarded
/// option, the outlined permanent answer and the filled one-time answer, focus on Cancel.
fn inline_prompt_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let host = HostText::from_host("Local (this machine)");
    let answer = |label: &'static str, help: &'static str, tier: Tier| Answer {
        label: label.into(),
        help: HoverHelp::tooltip(help),
        tier,
    };
    let question = Question::new(
        "Confirm YOLO launch",
        answer(
            "Start YOLO session anyway",
            "Start the session in YOLO mode, where the agent acts without asking",
            Tier::Danger,
        ),
        "launch".to_string(),
    )
    .summary(
        HostSentence::new("yolo-consequence")
            .text(
                "YOLO means the agent bypasses its safety checks: no person, classifier, AI reviewer or sandbox stops \
                 its actions on ",
            )
            .host(&host)
            .text(" without asking you."),
    )
    .line(
        "This launch uses YOLO permissions. Another permission mode would keep the agent's safety checks instead.",
    )
    .line(
        HostSentence::new("yolo-asks")
            .host(&host)
            .text(" asks before every YOLO launch. Nothing has been started."),
    )
    .cancel_answer(answer(
        "Cancel",
        "Cancel: go back without starting anything",
        Tier::Neutral,
    ))
    .alternative(
        answer(
            "Start guarded session instead",
            "Start this session in guarded mode instead",
            Tier::Primary,
        ),
        |_, _, _| {},
    )
    .fallible_alternative(
        answer(
            "Start, and don't ask again on this host",
            "Start it, and start YOLO sessions on this host without asking from now on",
            Tier::DangerOutline,
        ),
        |_, attempt, _, cx| attempt.succeeded(cx),
    )
    .starts_on_cancel()
    .loud();
    let inline = prompt::ask_inline(question, &mark, |_, _, _| {}, window, cx);
    state.keep.borrow_mut().push(Box::new(inline));
}

/// The host-text element over the spoofing corpus, one value per line, then a sentence mixing the app's words with two
/// host values, then one with a monospace host value (a path) beside an ordinary one: every hidden or
/// direction-changing character shows as `<U+XXXX>`, in either face.
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
        .child(
            HostSentence::new("host-text-mono")
                .text("Runs ")
                .mono(&HostText::from_host("/opt/farhelm\u{202E}/bin/farhelm"))
                .text(" on ")
                .host(&host),
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
/// brightening, dim, italic (in the bundled Italic face, so slanted), inverse, the five underline shapes,
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

/// A shell's last lines at the bottom of a small terminal, for the Delete scrim to lie over: enough text that the
/// picture shows how far the veil dims it.
fn terminal_delete_scrim_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let screen = serde_json::json!([
        screen_row(4, vec![run(0, 9, "$ cargo t", 0, -1, -1)]),
        screen_row(5, vec![run(0, 21, "test result: ok. 41 p", 0, 2, -1)]),
        screen_row(6, vec![run(0, 2, "$ ", 0, -1, -1)]),
    ]);
    let cursor = serde_json::json!({"x":2,"y":6,"visible":true,"shape":"block","blink":false});
    let view = terminal_view(
        grid_output(46, 7, screen, cursor),
        CursorPhase {
            focused: true,
            on: true,
        },
        cx,
    );
    state.terminals.borrow_mut().push(view);
}

/// The scene's terminal in a pane of a fixed size, on the terminal's ground as an attached terminal's pane is, under the
/// scrim the session pane draws over the open session's terminal while its Delete stops the agent (the old
/// `.terminal-delete-overlay`), its spinner on its first step.
fn terminal_delete_scrim(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .relative()
        .w(px(420.))
        .h(px(150.))
        .overflow_hidden()
        .bg(theme::color(theme::TERMINAL.background))
        .children(state.terminals.borrow().iter().cloned())
        .child(crate::session_pane::delete_scrim(
            DeleteGuard::Unconditional,
            0,
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
fn session_details_bar_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
fn session_details_bars(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
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
        .map(|row| Details::of(row, collections, now))
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
fn session_details_copy_warning_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
fn session_details_copy_warning(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use crate::session_pane::details::{Details, bar};
    use farhelm_proto::ui_channel::SessionRow;
    let now = crate::session_list::age::seconds(ticker::wall_now(cx));
    let (Some(copies), Some(store)) = (kept_copies(state), session::current_store(cx)) else {
        return div().into_any_element();
    };
    let collections = store.read(cx).collections();
    let shown = collections
        .get::<SessionRow>("a")
        .map(|row| Details::of(row, collections, now));
    div()
        .w(px(620.))
        .pt(px(48.))
        .children(shown.map(|details| bar(details, &copies, 0, cx)))
        .into_any_element()
}

/// The interrupted card the session pane draws in place of an interrupted session's terminals, for a session that can
/// resume its conversation (Restart and Replace) and for one whose conversation Farhelm never captured (Replace alone,
/// and why), each in a box the size of a narrow window's terminal area, as the old `.interrupted-card` filled it.
fn session_interrupted_cards(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let row = |offer: &str| -> farhelm_proto::ui_channel::SessionRow {
        let mut entry = session("a")
            .status(serde_json::json!({"state": "interrupted"}))
            .entry();
        entry.value["restart_offer"] = serde_json::json!(offer);
        entry.value["agent_kind"] = serde_json::json!("claude");
        serde_json::from_value(entry.value).unwrap_or_else(|error| panic!("a session row: {error}"))
    };
    let cards = ["resume", "not_captured"].map(|offer| {
        let row = row(offer);
        let drawn = crate::session_pane::card::Drawn {
            key: "a",
            row: &row,
            focus: &state.ring,
            question: None,
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

/// The terminals a terminal scene's setup made, in a row.
fn terminals(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .gap(px(12.))
        .children(state.terminals.borrow().iter().cloned())
        .into_any_element()
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
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
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
    let mut snapshot = vec![listing(&order, rows), preferences, registry];
    snapshot.extend(extra);
    helm.connect(snapshot);
    state.lists.borrow_mut().push(cx.new(SessionList::new));
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
}

/// A fleet with every mark and line a row draws differently: running (selected, so its inverse colors are pinned),
/// waiting, idle with new output, idle with nothing new, running on a host that is not connected (the grey mark, the
/// dimmed title and the cause), and exited with no code and a stop annotation on a line of its own. Their ages run from
/// `now` to weeks, counted from the test clock's start, so every bucket's width is in the picture.
fn session_list_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_scene(state, &fleet(), cx);
}

/// Rows on a host the helm cannot reach, the open one among them, so the selected row's inverse inks are pinned for a
/// stale row too: its title in `--on-accent-dim` rather than `--on-fill`, and its cause in the selected row's secondary
/// ink. Beside it, a stale exited row shows its ended words and its cause on one line, and an ordinary row on a
/// connected host stays as it is.
fn session_list_selected_stale_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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

/// The list in compact density (reference `session-list-compact`): every row one line, with no last line and no ended
/// line. The open row runs (its inverse inks pinned); then a row on a host the helm cannot reach, its clock after the
/// title; then each of the four ended marks in the status column, in its ink: a user's stop (the filled square), an
/// exit with a code, an interruption and a launch error; and last a stale exited row, whose ended mark and clock show
/// together.
fn session_list_compact_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
fn session_list_pinned_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
    let list = state.lists.borrow().first().cloned();
    if let (Some(list), Some(after)) = (list, state.after.take()) {
        let staged = state.clone();
        window.defer(cx, move |window, cx| after(&list, &staged, window, cx));
    }
    session_lists(state, window, cx)
}

/// The waiting row's actions menu, open beside its button with Rename, its first command, highlighted (reference
/// `session-row-menu`).
fn session_row_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| list.update(cx, |list, cx| list.toggle_menu("b", window, cx)),
        cx,
    );
}

/// The Help menu, open beside the host rail foot's `?` button with its first item highlighted (reference `help-menu`):
/// the items this platform offers with no updater running, the Linux golden with Open-Source Notices last.
fn help_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| {
            let help = list.read(cx).help_button().clone();
            help.update(cx, |help, cx| help.toggle(window, cx));
        },
        cx,
    );
}

/// The fleet of [`session_list_setup`] with `held` as the helm's notices, and the rail's notices panel open beside the
/// bell at the top of the rail's foot when `open`.
fn rail_notices_scene(state: &SceneState, held: &[serde_json::Value], open: bool, cx: &mut App) {
    let keys: Vec<String> = held.iter().map(|notice| notice["id"].to_string()).collect();
    let order: Vec<&str> = keys.iter().map(String::as_str).collect();
    let notices = CollectionState {
        collection: Collection::Notices,
        meta: crate::test_support::meta(&order),
        entries: held
            .iter()
            .map(|notice| KeyedEntry {
                key: notice["id"].to_string(),
                value: notice.clone(),
            })
            .collect(),
    };
    let rows = fleet();
    session_list_scene_holding(state, &rows, Some(rows[0].key), vec![notices], cx);
    if !open {
        return;
    }
    state.after.set(Some(|list, _, window, cx| {
        let bell = list.read(cx).notices_button().clone();
        bell.update(cx, |bell, cx| bell.toggle(window, cx));
    }));
}

/// The rail's notices panel with nothing in it (reference `notices-empty`): the header, "No notifications", and the
/// footer with Clear resolved unavailable and Close; the bell quiet, in the open look.
fn notices_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[], true, cx);
}

/// The rail's notices panel with one unread trash cleanup notice about the local host (reference `notices-alerting`):
/// its host and what happened, the checkout's repository and size, the reason in red, when it last failed, the promise
/// of a retry, and Retry now unavailable (the host's trash read is never answered here, so no listing shows the
/// checkout stuck) beside Open Trash.
fn notices_alerting_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[alerting_notice()], true, cx);
}

/// The rail with that unread notice and the panel closed: the bell filled, in the strong red, with its count "1" over
/// its corner, the look nothing else pictures (the open bell wears the open look's inverse ink instead).
fn notices_bell_loud_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    rail_notices_scene(state, &[alerting_notice()], false, cx);
}

/// The one unread trash cleanup notice about the local host that the alerting scenes show.
fn alerting_notice() -> serde_json::Value {
    serde_json::json!({
            "id": 1,
            "host": 1,
            "raised_at": 1_794_596_700_i64,
            "read": false,
            "resolved": false,
            "content": {
                "kind": "stuck_trash",
                "checkout_id": "archived-work",
                "name": "archived-work",
                "repository": "example/project",
                "bytes": 4096,
                "reason": "Permission denied",
                "at": 1_794_596_700_i64,
            },
    })
}

/// The list's order menu, open below its title as a click opens it (reference `sort-menu`): the title in its open look,
/// the shared header ("Session list"), the three orders with Recently active, the stored default, checked and
/// highlighted, and Compact, unchecked, after a separator.
fn session_order_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| list.update(cx, |list, cx| list.toggle_order_menu(window, cx)),
        cx,
    );
}

/// A row's notification list, open beside its bell (no reference: the old app's screenshots have no open bell). The
/// open row has nothing to tell; the row below has three notifications, newest first: one new since the list was last
/// opened (the red edge and New), whose text ends with a docs link, one already read, and one resolved (greyed and
/// marked Resolved). Its bell is loud, with one unread. The bell is pressed as a click would press it, and the pointer
/// then leaves for an empty corner, so neither a hover fill nor a tooltip is in the picture ([`press_bell_b`]).
fn session_bell_list_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    window.set_driver_enabled(true);
    let row = |key, title, ago: i64| {
        session(key)
            .title(title)
            .activity(TEST_NOW - ago)
            .seen(Some(TEST_NOW - ago))
    };
    let notifications = serde_json::json!([
        {
            "seq": 7,
            "at": TEST_NOW - 3 * 60,
            "text": "Farhelm ended this session's old terminal when it started, so its agent is no longer \
                     running. Restart starts it again in a new terminal. See \
                     https://farhelm.io/docs/using/new-terminals/",
        },
        {
            "seq": 6,
            "at": TEST_NOW - 2 * 3_600,
            "text": "Farhelm could not add its conversation hook to this launch, so Restart cannot resume it.",
        },
        {
            "seq": 5,
            "at": TEST_NOW - 26 * 3_600,
            "text": "When Farhelm checked, Codex's own record of this conversation was missing or no longer \
                     matched, so Restart stopped offering to resume it.",
            "resolved": true,
        },
    ]);
    let rows = [
        row("a", "Refactor the API client", 2 * 60).cwd("/home/user/src/api"),
        row("b", "Release notes for 0.40", 5 * 60)
            .cwd("/home/user/src/farhelm")
            .notifications(notifications, 6),
        row("c", "Benchmark on the build box", 3 * 3_600).cwd("/home/user/bench"),
    ];
    session_list_scene_opening(state, &rows, Some("a"), cx);
    state.after.set(Some(press_bell_b));
}

/// Press row `b`'s bell as a click would, where the last frame drew it, then move the pointer to the empty corner at
/// the top left (see [`session_bell_list_setup`]). The bell's place comes from GPUI's driver record, which the setup
/// turns on and which only reads what a frame drew, so the app keeps nothing for the scene. The store's first snapshot
/// reaches the list only once the executor has run, a frame or more after the list is first drawn, so until a frame has
/// drawn the bell this waits for the next frame the scene's settling draws; it asks for none itself, since a frame asked
/// for here would be drawn before the executor runs again and could never show the bell.
fn press_bell_b(_: &Entity<SessionList>, state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(bell) = window
        .driver_nodes()
        .iter()
        .find(|node| node.element_id.as_deref() == Some("session-bell-b"))
        .map(|node| node.bounds.center())
    else {
        state.after.set(Some(press_bell_b));
        return;
    };
    let pressed = |down: bool| {
        if down {
            gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
                button: gpui::MouseButton::Left,
                position: bell,
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            })
        } else {
            gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
                button: gpui::MouseButton::Left,
                position: bell,
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
            })
        }
    };
    window.dispatch_event(pressed(true), cx);
    window.dispatch_event(pressed(false), cx);
    window.dispatch_event(
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: point(px(2.), px(2.)),
            pressed_button: None,
            modifiers: gpui::Modifiers::default(),
        }),
        cx,
    );
}

/// The waiting row's Delete question, asked from its row as the menu's Delete asks it (reference
/// `row-delete-confirmation`): its tab, its running agent, its quoted title and its managed checkout. The
/// `anchored-prompt` golden pins the prompt's own look with made-up wording; this one pins the row's wording and where
/// it sits beside the list.
fn row_delete_confirmation_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| {
            let mark = Mark::take(cx);
            list.update(cx, |list, cx| list.delete("b", &mark, window, cx));
        },
        cx,
    );
}

/// The sequence pill fades in when it appears (`components::entrance`); with motion reduced it is drawn at once, so the
/// picture is the settled pill rather than the fade's first, transparent frame.
fn sequence_pill_setup(_: &SceneState, _: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
}

/// A pending Delete on the selected row (its spinner and "Stopping agent…" in place of the title, in the fill's ink)
/// and a refused one on the idle row with new output (the helm's words under the row, in red). Motion is reduced, so
/// the spinner holds its first step and the picture can settle. The refusal is answered before the pending Delete is
/// sent, because the fake answers in order.
fn session_list_deleting_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
fn host_filter_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
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
fn session_status_segments_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| filtered_to_waiting_on_buildbox(list, state, &["b"], cx),
        cx,
    );
}

/// An empty answer to a host and a status (reference `session-list-filter-empty`): "No waiting sessions on" the host's
/// mark and name, and both links under it, Show all hosts first.
fn filter_empty_both_links_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| filtered_to_waiting_on_buildbox(list, state, &[], cx),
        cx,
    );
}

/// Host management shown over the fleet with `buildbox` chosen (the old `hosts-panel` reference with a host chosen, as
/// in `host-rail-host-selected`): the Hosts panel in place of the list's column, Hosts pressed at the rail's foot, and
/// the chosen avatar in the quieter look the rail's choice wears while the panel shows. The fake answers the host query
/// and the list's other commands in order before the panel is shown, so the rail draws the accepted choice.
fn hosts_shown_with_a_host_chosen_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, state, _, cx| {
            list.update(cx, |list, cx| list.choose_host(Some(2), cx));
            {
                let mut keep = state.keep.borrow_mut();
                if let Some(helm) = keep
                    .iter_mut()
                    .find_map(|kept| kept.downcast_mut::<FakeHelm>())
                {
                    for command in helm.commands() {
                        helm.reply(command.id, 204, None);
                    }
                }
            }
            list.update(cx, |list, cx| list.show_hosts(cx));
        },
        cx,
    );
}

/// No sessions at all: the list says so.
fn session_list_empty_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_scene(state, &[], cx);
}

/// The Rename editor over a session the fake helm lists, as the shortcut or a row's menu opens it (reference
/// `rename-dialog`): the current title, the field holding it with focus, and Cancel and Save. The fake and the
/// connection are kept for the scene's life, like the session list's.
fn rename_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
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

/// The Announcements dialog as the `?` menu's history opens it: two shown announcements, newest first, the first with a
/// detail line, two paragraphs and a labelled link, the second plain. The old app's announcements dialog was never
/// captured as a reference scene, so SPEC.md and the old code are this picture's reference.
fn announcements_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::announcements::{Announcement, AnnouncementLink, AnnouncementQueue};
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let newest = Announcement {
        id: "2".to_owned(),
        title: "Sessions now restart in their own terminal".to_owned(),
        body: "Restart keeps the session's history and opens a fresh terminal beside it.\n\nNothing changes for \
               sessions you do not restart."
            .to_owned(),
        detail: Some("Since 0.40.0".to_owned()),
        link: Some(AnnouncementLink {
            label: "Release notes".to_owned(),
            url: "https://farhelm.io/docs/release-notes/".to_owned(),
        }),
    };
    let older = Announcement {
        id: "1".to_owned(),
        title: "Hosts update on their own".to_owned(),
        body: "Remote hosts now update when the app does.".to_owned(),
        detail: None,
        link: None,
    };
    helm.connect(vec![CollectionState {
        collection: Collection::Announcements,
        meta: test_support::meta(&[ANNOUNCEMENTS_KEY]),
        entries: vec![KeyedEntry {
            key: ANNOUNCEMENTS_KEY.into(),
            value: serde_json::to_value(AnnouncementQueue {
                pending: vec![],
                history: vec![newest, older],
            })
            .unwrap(),
        }],
    }]);
    // As for Rename: the queue reaches the store after this setup returns, so the history opens once the connection
    // has a UI session, and only once.
    let handle = window.window_handle();
    let opener = cx.focus_handle().tab_stop(true);
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let opener = opener.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::announcements::open_history(&opener, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The Welcome card (reference `welcome-dialog`): the title and its ×, the introduction, the two bordered pointers with
/// Read Get started and Send feedback now, the footer's note and Got it with focus. Opened as the `?` menu's Welcome item
/// opens it, which draws the same card as the window start does; the scene's ring handle stands in for the `?` button.
fn welcome_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    crate::welcome::open(&state.ring, &mark, window, cx);
}

/// The Send feedback dialog as the Help menu opens it (reference `feedback-dialog`), over a preference row
/// remembering `contact`: focus in the empty message, the contact field holding `contact`, and, when it is not empty,
/// Re-use checked under it; then what is sent with it, the footer's note, Cancel and Send, unavailable while the
/// message is empty. The opener is the scene's ring handle, which stands in for the `?` button. The fake and the
/// connection are kept for the scene's life, like the Rename editor's.
fn feedback_dialog_scene(
    state: &SceneState,
    contact: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![preferences_state(Preferences {
        feedback_contact: contact.map(str::to_owned),
        ..Preferences::default()
    })]);
    // The dialog seeds its contact from the store, which the snapshot reaches after this setup returns: open once the
    // connection has a UI session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::feedback::open(&opener, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// [`feedback_dialog_scene`] as the dialog first opens with nothing remembered (reference `feedback-dialog`).
fn feedback_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    feedback_dialog_scene(state, None, window, cx);
}

/// [`feedback_dialog_scene`] with a remembered contact, so Re-use for future feedback shows, checked.
fn feedback_dialog_reuse_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    feedback_dialog_scene(state, Some("you@example.test"), window, cx);
}

/// The Keyboard shortcuts overview as the `?` menu opens it, in this platform's forms (Linux, where the goldens run),
/// over a preference row with ⚡ on `deploy` and number keys 1 and 0 assigned, so both kinds of template row show. The
/// old app's overview was never captured as a reference scene, so SPEC.md and the old code are this picture's
/// reference. The opener is the scene's ring handle, which stands in for the `?` button; the fake and the connection
/// are kept for the scene's life, like the feedback dialog's.
fn keyboard_shortcuts_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![preferences_state(Preferences {
        quick_launch_template: Some("deploy".to_owned()),
        template_shortcuts: [("1", "api review"), ("0", "nightly build")]
            .into_iter()
            .map(|(key, name)| (key.to_owned(), name.to_owned()))
            .collect(),
        ..Preferences::default()
    })]);
    // The rows read the store, which the snapshot reaches after this setup returns: open once the connection has a UI
    // session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::shortcuts::open_from_menu(&opener, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The Settings dialog over the fake helm's preference row, opened at `section` as the gear opens it (references
/// `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-recipes`, `settings-containers`,
/// `settings-credentials`, `settings-terminal`, `settings-checkouts`, `settings-agent-trust`, `settings-notifications`
/// and `settings-app`): focus on the section's first switch (on the dialog itself at Appearance, Recipes, Containers,
/// Credentials, Terminal, Checkouts, Agent trust and Notifications), naming on because the row stores no choice for it,
/// and automatic remote-host updates turned on so the Hosts picture shows a switch in each state. Agent trust's reads
/// of each agent's trust, sent as it opens, are answered at once with the trust off and the hint not dismissed,
/// Checkouts' reads as the reference stack had them: no default folder, no post-clone command, and a checkout kept
/// ready, Notifications' read of the ntfy settings with the helm's defaults (off, `https://ntfy.sh`, no topic, Default
/// priority, no token), Credentials' reads with an empty catalog and no Tensorlake key saved, Recipes' read of the
/// Tensorlake account with a key saved (so the rows offer their builds) over an empty pushed catalog and no pushed
/// places, and Containers' reads with the reference's three container hosts, all ready, and no leftovers; nothing else
/// is answered, so the checks Containers then asks for stay unanswered and the rows keep the first read's state. The
/// opener is the scene's ring handle, which stands in for the gear. The fake and the connection are kept for the
/// scene's life, like the Rename editor's.
fn settings_scene(
    state: &SceneState,
    section: crate::settings::Section,
    window: &mut Window,
    cx: &mut App,
) {
    settings_scene_with(state, section, SettingsFixture::default(), window, cx);
}

/// What a Settings scene's fake helm holds beyond [`settings_scene`]'s defaults.
#[derive(Default)]
struct SettingsFixture {
    /// The pushed recipe catalog's entries, as `GET /api/tensorlake/recipes` lists them; no recipes when empty.
    recipes: Vec<serde_json::Value>,
    /// The pushed recipe places' entries, as `GET /api/recipe-places` lists them; no places when empty.
    places: Vec<serde_json::Value>,
    /// The registered hosts, in the first snapshot's hosts collection (where Recipes' place names come from); the
    /// snapshot carries no hosts collection when empty.
    hosts: Vec<farhelm_proto::ui_channel::HostView>,
    /// The platform the Terminal section decides its tunnel switch for, instead of this machine's, through the
    /// dialog's test-only setter (`settings::decide_tunnels_for_opened`) applied as it opens.
    tunnels: Option<farhelm_input_mapping::Platform>,
    /// Open a new recipe's editor as the dialog opens (`settings::open_new_recipe_for_opened`), as New recipe does; the
    /// section must be Recipes.
    recipe_editor: bool,
}

/// `entries` as a pushed collection's first snapshot, each filed under the key `key` gives it, in their order.
fn pushed_state(
    collection: Collection,
    entries: Vec<serde_json::Value>,
    key: impl Fn(&serde_json::Value) -> String,
) -> CollectionState {
    let entries: Vec<KeyedEntry> = entries
        .into_iter()
        .map(|value| KeyedEntry {
            key: key(&value),
            value,
        })
        .collect();
    let order: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
    CollectionState {
        collection,
        meta: test_support::meta(&order),
        entries,
    }
}

/// [`settings_scene`], with the fake helm holding `fixture`.
fn settings_scene_with(
    state: &SceneState,
    section: crate::settings::Section,
    fixture: SettingsFixture,
    window: &mut Window,
    cx: &mut App,
) {
    let SettingsFixture {
        recipes,
        places,
        hosts,
        tunnels,
        recipe_editor,
    } = fixture;
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let mut snapshot = vec![
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({ "auto_update_remote_hosts": true }),
            }],
        },
        pushed_state(Collection::Recipes, recipes, |recipe| {
            recipe["name"].as_str().unwrap_or_default().to_owned()
        }),
        // A place that does not decode gets no key, and the fake helm's snapshot then fails the scene loudly.
        pushed_state(Collection::RecipePlaces, places, |place| {
            serde_json::from_value::<farhelm_proto::RecipePlaceView>(place.clone())
                .map(|view| farhelm_proto::RecipePlaceView::key(&view.recipe, view.place))
                .unwrap_or_default()
        }),
    ];
    if !hosts.is_empty() {
        snapshot.push(crate::hosts::tests::hosts(&hosts));
    }
    helm.connect(snapshot);
    // The dialog reads the row from the store, which the snapshot reaches after this setup returns: open once the
    // connection has a UI session, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::settings::open_at(section, &opener, &mark, window, cx);
            if recipe_editor {
                crate::settings::open_new_recipe_for_opened(window, cx);
            }
        });
        if let Some(platform) = tunnels {
            crate::settings::decide_tunnels_for_opened(platform, cx);
        }
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            let trust = read.path.starts_with("/api/checkout-claude-trust")
                || read.path.starts_with("/api/checkout-agent-trust/");
            let answer = if trust {
                serde_json::json!({ "enabled": false, "hint_dismissed": false })
            } else {
                match read.path.as_str() {
                    "/api/checkout-root" => serde_json::json!({ "root": null }),
                    "/api/checkout-post-clone" => serde_json::json!({ "command": null }),
                    "/api/checkout-spare" => serde_json::json!({ "enabled": true }),
                    "/api/credentials" => serde_json::json!([]),
                    "/api/environment-places" => serde_json::json!([
                        { "place": { "kind": "podman_host", "host": 1 }, "name": "this machine", "reason": null, "refreshing": false },
                        { "place": { "kind": "podman_host", "host": 2 }, "name": "build-box", "reason": null, "refreshing": false },
                        { "place": { "kind": "podman_host", "host": 3 }, "name": "gpu-1", "reason": null, "refreshing": false },
                    ]),
                    "/api/container-orphans" => serde_json::json!([]),
                    // Credentials' key form pictures no key saved, as its reference does; Recipes pictures rows that
                    // can build, which takes a saved key.
                    "/api/tensorlake" => serde_json::json!({
                        "api_key_set": section == crate::settings::Section::Recipes,
                        "create_without_asking": false,
                        "helm_tag": "fixture",
                        "ssh_key": { "state": "not_created" },
                    }),
                    "/api/ntfy-alerts" => {
                        match serde_json::to_value(farhelm_proto::ntfy::Settings::default()) {
                            Ok(defaults) => defaults,
                            Err(_) => continue,
                        }
                    }
                    _ => continue,
                }
            };
            helm.reply(read.id, 200, Some(answer));
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The Templates dialog as the list header's Templates button opens it (reference `templates-dialog`): three templates
/// with summaries of different shapes, the first holding ⚡ (so its row has the chord line and the filled amber bolt)
/// and the second holding number key 2. Focus on the dialog itself, where it starts until the editor exists, so no
/// ring shows. The fake and the connection are kept for the scene's life, like Settings'.
fn templates_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let templates = [
        serde_json::json!({"name": "my-codex", "fields": {"kind": "agent", "agent": "codex", "model": "gpt-6-luna", "effort": "high"}}),
        serde_json::json!({"name": "review", "fields": {"permissions": "guarded", "name": "review"}}),
        serde_json::json!({"name": "webbuilder", "fields": {"kind": "command", "command": "make dev", "yolo": false, "destination": {"folder": "/src/web"}}}),
    ];
    helm.connect(vec![
        CollectionState {
            collection: Collection::Templates,
            meta: test_support::meta(&["my-codex", "review", "webbuilder"]),
            entries: templates
                .iter()
                .map(|value| KeyedEntry {
                    key: value["name"].as_str().unwrap_or_default().into(),
                    value: value.clone(),
                })
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({
                    "quick_launch_template": "my-codex",
                    "template_shortcuts": {"2": "review"},
                }),
            }],
        },
    ]);
    // Open once the connection has a UI session, as Settings' scene does. The scene's ring handle stands in for the
    // Templates button.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::templates::open(&opener, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The reference set's templates for the editor scenes (references `template-editor` and `template-number-menu`): a
/// placement-only template, the one the editor opens, and a Claude one, with number keys 1 and 8 bound as the
/// reference stack bound them.
const EDITOR_SCENE_TEMPLATES: [(&str, &str); 3] = [
    (
        "api-repo",
        r#"{"host": "install-build-box", "destination": {"folder": "/src/api"}}"#,
    ),
    (
        "codex-deep",
        r#"{"kind": "agent", "agent": "codex", "model": "gpt-6.1-sol", "effort": "high"}"#,
    ),
    (
        "quick-claude",
        r#"{"kind": "agent", "agent": "claude", "permissions": "yolo"}"#,
    ),
];

/// The Templates dialog with `codex-deep` open in its editor, as a click on its row opens it (reference
/// `template-editor`): focus in its name field, where opening a row puts it, and its model suggestions from the
/// catalog read the editor sends, which is answered at once with the reference stack's four Codex models. The fake,
/// the connection and the watch are kept for the scene's life, like Settings'.
fn template_editor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        CollectionState {
            collection: Collection::Templates,
            meta: test_support::meta(&["api-repo", "codex-deep", "quick-claude"]),
            entries: EDITOR_SCENE_TEMPLATES
                .iter()
                .map(|(name, fields)| KeyedEntry {
                    key: (*name).into(),
                    value: serde_json::json!({
                        "name": name,
                        "fields": serde_json::from_str::<serde_json::Value>(fields).unwrap_or_default(),
                    }),
                })
                .collect(),
        },
        CollectionState {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            entries: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({
                    "template_shortcuts": {"1": "codex-deep", "8": "quick-claude"},
                }),
            }],
        },
    ]);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let helm = Rc::new(RefCell::new(helm));
    let answering = helm.clone();
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::templates::open(&opener, &mark, window, cx);
            if let Some(dialog) = cx
                .try_global::<crate::templates::dialog::Opened>()
                .and_then(|opened| opened.0.upgrade())
            {
                dialog.update(cx, |dialog, cx| {
                    dialog.open_template("codex-deep", window, cx)
                });
            }
        });
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            if read.path == "/api/launch-catalog" {
                let models = ["gpt-6-luna", "gpt-5.6-terra", "gpt-6.1-sol", "gpt-6-astra"];
                let catalog: Vec<serde_json::Value> = models
                    .iter()
                    .map(|id| {
                        serde_json::json!({"id": id, "harness": "codex", "efforts": ["low", "medium", "high"]})
                    })
                    .collect();
                helm.reply(read.id, 200, Some(serde_json::Value::Array(catalog)));
            }
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// [`template_editor_setup`], then (reference `template-number-menu`) `codex-deep`'s number-key menu open over it, as
/// a click on its key button opens it: the template's name over "Number shortcut" in the shared menu header, then No
/// shortcut and the ten keys, 1 held by this template and 8 by `quick-claude`.
fn template_number_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    template_editor_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Nothing on the page, and once the dialog has drawn `codex-deep`'s key button, one click of it after that frame.
fn template_number_menu_scene(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let dialog = cx
        .try_global::<crate::templates::dialog::Opened>()
        .and_then(|opened| opened.0.upgrade());
    if let Some(dialog) = dialog
        && dialog.read(cx).key_button_drawn("codex-deep")
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            dialog.update(cx, |dialog, cx| {
                dialog.toggle_key_menu("codex-deep", window, cx)
            })
        });
    }
    div().into_any_element()
}

/// [`template_editor_setup`], then (reference `template-leave-question`) the name changed to `codex-deep-unsaved` and the
/// header Close pressed: "Unsaved changes" in the heading, and the leave question at the editor's foot in place of its
/// buttons, focus on its Save.
fn template_leave_question_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    template_editor_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Nothing on the page, and once the dialog has drawn its editor, the edit and the Close after that frame.
fn template_leave_question_scene(
    state: &SceneState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let dialog = cx
        .try_global::<crate::templates::dialog::Opened>()
        .and_then(|opened| opened.0.upgrade());
    if let Some(dialog) = dialog
        && dialog.read(cx).key_button_drawn("codex-deep")
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            dialog.update(cx, |dialog, cx| dialog.ask_leave_for_golden(window, cx))
        });
    }
    div().into_any_element()
}

/// Settings as the gear opens it, at Sessions.
fn settings_sessions_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Sessions, window, cx);
}

/// Settings at Appearance, System chosen because the row stores no appearance. Focus starts on the dialog itself
/// (`Section::first`), so no ring shows, as in the reference, where the section was reached by a click.
fn settings_appearance_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Appearance, window, cx);
}

/// Settings at Hosts.
fn settings_hosts_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Hosts, window, cx);
}

/// Settings at Recipes with no recipes and New recipe under "No recipes yet.", as in the reference. Focus starts on the
/// dialog itself, since the section's controls wait for its reads.
fn settings_recipes_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Recipes, window, cx);
}

/// Settings at Recipes with a new recipe's editor open, as New recipe opens it, against the reference
/// `settings-recipes`, whose set has no editor scene: the form from its top with focus on Back, Managed Ubuntu chosen
/// among the Software cards, and the editor's Cancel and Save pinned above the Close row, ruled off from it. The
/// dialog's body is shorter than the form, so the lower fields are below the fold, as a user first sees them.
fn settings_recipe_editor_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipe_editor: true,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Recipes with recipes in the states a row draws differently, against the reference `settings-recipes`,
/// whose stack had none: one built with both agents' versions and changed packages, so it warns, and rebuilding at its
/// fifth step, its line drawn at the pulse's full brightness and no action offered; one failed at its fourth step with
/// the build sandbox's reason and Retry; and one on a container image, which Tensorlake cannot run, so it warns and
/// offers nothing. Three rows, so the last one's warning fits above the footer unscrolled.
fn settings_recipes_rows_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let recipe = |name: &str,
                  base: serde_json::Value,
                  build: serde_json::Value,
                  snapshot: serde_json::Value| {
        serde_json::json!({
            "name": name,
            "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
            "base": base,
            "packages": ["jq", "ripgrep"],
            "claude": null, "codex": null,
            "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
            "tunnels": [{ "sandbox_port": 8080, "host": "127.0.0.1", "port": 8080 }],
            "snapshot": snapshot,
            "build": build,
            "prepared": null,
            "preparation": { "state": "idle" },
            "cleanup_pending": false,
        })
    };
    let ubuntu = serde_json::json!({ "kind": "tensorlake_ubuntu" });
    let idle = serde_json::json!({ "state": "idle" });
    let built = serde_json::json!({
        "id": "snap-1", "built_at": 1_793_714_525, "base": ubuntu, "disk_gb": 20, "packages": ["jq"],
        "claude_version": "2.0.1 (Claude Code)", "codex_version": "codex-cli 0.40.0",
    });
    let recipes = vec![
        recipe(
            "rust-dev",
            ubuntu.clone(),
            serde_json::json!({ "state": "building", "step": "packages", "started_at": 1_793_714_525 }),
            built,
        ),
        recipe(
            "team-image",
            serde_json::json!({ "kind": "image", "image": "team/devbox-base" }),
            serde_json::json!({ "state": "failed", "step": "connecting", "at": 1_793_714_525,
                "message": "the sandbox user cannot use sudo without a password" }),
            serde_json::Value::Null,
        ),
        recipe(
            "podman-dev",
            serde_json::json!({ "kind": "container_image", "reference": "example.test/team/dev:latest", "user": "dev" }),
            idle,
            serde_json::Value::Null,
        ),
    ];
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipes,
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Recipes with one recipe's per-place lines and a stale prepared snapshot, against the reference
/// `settings-recipes`, whose stack had no recipe: `rust-dev`, built, whose prepared snapshot the helm reports expired;
/// ready at `build-box` (a registered host, named from the hosts collection) with an older image awaiting cleanup; and
/// building at a host the registry no longer lists ("Unavailable place"), whose Rebuild is unavailable. The Tensorlake
/// account's place is pushed too and draws no line. No line shows an age, which the viewer's clock would move.
fn settings_recipes_places_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let recipe = serde_json::json!({
        "name": "rust-dev",
        "size": { "cpus": 2, "memory_gb": 4, "disk_gb": 20 },
        "base": { "kind": "tensorlake_ubuntu" },
        "packages": ["jq"],
        "claude": null, "codex": null,
        "claude_login_credential": null, "codex_login_credential": null, "github_token_credential": null,
        "tunnels": [],
        "snapshot": {
            "id": "snap-1", "built_at": 1_793_714_525, "base": { "kind": "tensorlake_ubuntu" }, "disk_gb": 20,
            "packages": ["jq"], "claude_version": "2.0.1 (Claude Code)", "codex_version": "codex-cli 0.40.0",
        },
        "build": { "state": "idle" },
        "prepared": {
            "id": "prepared-1", "prepared_at": 1_793_714_525, "farhelm_version": "0.38.0",
            "source_snapshot": "snap-1", "stale_reason": "expired",
        },
        "preparation": { "state": "idle" },
        "cleanup_pending": false,
    });
    let place = |place: serde_json::Value, fields: serde_json::Value| {
        let mut entry = serde_json::json!({
            "recipe": "rust-dev", "place": place, "ready": false, "building": false, "reason": null,
            "built_at": null, "prepared_at": null, "step": null, "cleanup_pending": false,
        });
        if let (Some(entry), Some(fields)) = (entry.as_object_mut(), fields.as_object()) {
            entry.extend(fields.clone());
        }
        entry
    };
    let places = vec![
        place(
            serde_json::json!({ "kind": "tensorlake_account" }),
            serde_json::json!({ "ready": true }),
        ),
        place(
            serde_json::json!({ "kind": "podman_host", "host": 7 }),
            serde_json::json!({ "ready": true, "built_at": 1_793_714_525, "cleanup_pending": true }),
        ),
        place(
            serde_json::json!({ "kind": "podman_host", "host": 9 }),
            serde_json::json!({ "building": true, "step": "installing packages" }),
        ),
    ];
    use crate::hosts::tests::{aliased, connected, host};
    settings_scene_with(
        state,
        crate::settings::Section::Recipes,
        SettingsFixture {
            recipes: vec![recipe],
            places,
            hosts: vec![aliased(
                host(7, "ssh", connected()),
                "build-box",
                "dev@build-box.invalid",
            )],
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// Settings at Credentials, as in the reference: nothing saved in any list and no Tensorlake key saved. Focus starts
/// on the dialog itself, since every control waits for the section's reads.
fn settings_credentials_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Credentials, window, cx);
}

/// Settings at Terminal, the default link choice chosen because the row stores none, and, decided for the Mac on every
/// machine, the tunnel switch below it off because the row stores no consent. Focus starts on the dialog itself
/// (`Section::first`), so no ring shows, as in the reference, where the section was reached by a click. The reference is
/// the browser interface, which had no tunnel switch; the old Mac app drew it under the link choices.
fn settings_terminal_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene_with(
        state,
        crate::settings::Section::Terminal,
        SettingsFixture {
            tunnels: Some(farhelm_input_mapping::Platform::Mac),
            ..SettingsFixture::default()
        },
        window,
        cx,
    );
}

/// The Add host dialog over the fake helm's reference hosts, as the host rail's Add host opens it (reference
/// `host-add-dialog`), with `dev@new-box` in the destination field, which holds focus. The opener is the scene's ring
/// handle. The fake and the connection are kept for the scene's life, like Settings'.
fn host_add_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    // The dialog opens only over a live UI session, which the snapshot's connection makes after this setup returns:
    // open once the connection has one, and only once.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::add::open(&opener, &mark, window, cx);
            let dialog = cx
                .try_global::<crate::hosts::add::tests::Opened>()
                .and_then(|opened| opened.0.upgrade());
            if let Some(dialog) = dialog {
                let input = dialog.read(cx).destination_input();
                input.set_text("dev@new-box", window, cx);
            }
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The Add host dialog as an approval card opens it for an agent's host add (no reference screen: the old app's dialog
/// for an agent was the same dialog with two unstyled description lists): who asked, what Add approves, and the fields
/// prefilled with the request's destination and alias. Pins the agent's lines above the ordinary fields.
fn host_add_dialog_agent_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use farhelm_proto::approvals::{ApprovalAction, ApprovalSession, PendingApproval};
    use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry};
    let request = PendingApproval {
        id: "golden-add".to_owned(),
        host_id: 2,
        host_name: "build-box".to_owned(),
        session: ApprovalSession {
            id: "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:a75bff30-1c2d-4e5f-8a9b-0c1d2e3f27a4"
                .to_owned(),
            title: Some("Provision the test fleet".to_owned()),
            host_name: Some("build-box".to_owned()),
        },
        action: ApprovalAction::HostAdd {
            destination: "ops@new-box.example".to_owned(),
            alias: Some("new-box".to_owned()),
        },
        own_host_session_action: false,
        expires_at_ms: 1,
    };
    let approvals = CollectionState {
        collection: Collection::Approvals,
        meta: crate::test_support::meta(&["golden-add"]),
        entries: vec![KeyedEntry {
            key: "golden-add".to_owned(),
            value: serde_json::to_value(&request).expect("an approval encodes"),
        }],
    };
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![
        crate::hosts::tests::hosts(&crate::hosts::tests::reference_hosts()),
        approvals,
    ]);
    let arming = cx.new(|_| crate::approvals::answers::Arming::armed_for_scene());
    // Opened once the connection has its UI session, as Add host's own scene opens.
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let arming = arming.clone();
        let request = request.clone();
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            if let Some(agent) = crate::approvals::host_add::AgentRequest::of(&request, arming) {
                let _ = crate::hosts::add::open_for_agent(agent, &opener, &mark, window, cx);
            }
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// Settings at Containers: the reference's three container hosts, each Ready, and no containers left behind, as in the
/// reference. Focus starts on the dialog itself (`Section::first`), so no ring shows.
fn settings_containers_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Containers, window, cx);
}

/// Settings at Checkouts, its fields empty and the spare switch on, as in the reference. Focus starts on the dialog
/// itself (`Section::first`), so no ring shows.
fn settings_checkouts_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Checkouts, window, cx);
}

/// Settings at Agent trust, every agent's trust off and every hint shown, as in the reference. Focus starts on the
/// dialog itself (`Section::first`), so no ring shows.
fn settings_agent_trust_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::AgentTrust, window, cx);
}

/// Settings at Notifications, as in the reference: the Sounds sub-heading with its switches at their defaults (the row
/// stores no choice for them: waiting and approval on, turn finished off), and below it the ntfy alerts form over the
/// helm's defaults. Focus starts on the dialog itself (`Section::first`), so no ring shows.
fn settings_notifications_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Notifications, window, cx);
}

/// Settings at App (reference `settings-app`), over a fake desktop updater kept as the app's
/// (`desktop::updates::fake`), so the section is listed, after Notifications, and shows its note and the switch on, the
/// app's default, with the on help. Focus starts on the switch, whose ring shows only under keyboard focus, so the
/// picture shows none.
fn settings_app_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::desktop::updates::fake::{Feed, Seen, fake_host};
    let (feed, seen) = (Rc::new(Feed::default()), Rc::new(Seen::default()));
    seen.automatic.set(true);
    crate::desktop::updates::keep(Some(fake_host(&feed, &seen, false)), cx);
    settings_scene(state, crate::settings::Section::App, window, cx);
}

/// The setup confirmation for the reference screens' plan (reference `host-add-setup-plan`), as a probe's plan opens it
/// in place of Add host: the plan, Copy plan, the footer note and the three answers, focus on Set up. The opener is the
/// scene's ring handle. The fake and the connection are kept for the scene's life, like Add host's.
fn host_add_setup_plan_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    let opener = state.ring.clone();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let offer = crate::hosts::setup::tests::reference_offer(opener.clone(), mark);
            crate::hosts::setup::confirm(offer, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The removal confirmation for the reference screens' `build-box` (reference `host-remove-confirmation`), as its menu's
/// Remove opens it: the host's mark, name, phase and build, its destination and what removal keeps, the footer note and
/// the three answers, focus on Remove. The question opens once the connection has a UI session, whose store the
/// snapshot made, as the setup confirmation's scene does; the fake, the connection and the watch are kept for the
/// scene's life.
fn host_remove_confirmation_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let _ = crate::hosts::remove::ask(2, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// Make the Hosts panel over a fake helm holding `listed` hosts, with Details on when `details` is; the scene draws it
/// through [`hosts_panels`]. The fake, the connection and the panel are kept for the scene's life.
fn hosts_panel_scene(state: &SceneState, listed: &[HostView], details: bool, cx: &mut App) {
    hosts_panel_scene_with(state, vec![crate::hosts::tests::hosts(listed)], details, cx);
}

/// [`hosts_panel_scene`] over a fake helm connected with `snapshot`, for a scene that needs more than the hosts.
fn hosts_panel_scene_with(
    state: &SceneState,
    snapshot: Vec<farhelm_proto::ui_channel::CollectionState>,
    details: bool,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(snapshot);
    let panel = cx.new(HostsPanel::new);
    if details {
        panel.update(cx, |panel, cx| panel.show_details(true, cx));
    }
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(panel));
}

/// The reference screens' three connected hosts, Details off (reference `hosts-panel`): the heading with its count,
/// switch and Update all, and each row's mark, name and phase.
fn hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), false, cx);
}

/// The same hosts with Details on (reference `hosts-panel-details`): each row's destination behind its alias and its
/// version, identity and session count.
fn hosts_panel_details_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), true, cx);
}

/// Details on with `build-box` failing to connect (reference `hosts-panel-host-unreachable`): its grey
/// "Connecting", and its attempt and the long SSH error wrapping under it.
fn hosts_panel_host_unreachable_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{aliased, connecting, host, reference_hosts};
    let mut listed = reference_hosts();
    listed[1] = aliased(
        host(
            2,
            "ssh",
            connecting(
                2,
                Some(
                    "SSH could not connect or log in to the host; SSH stderr: \"ssh: Could not resolve hostname \
                     reference-capture.invalid: Name or service not known\"; cause: connection closed before hello",
                ),
            ),
        ),
        "build-box",
        "reference-capture.invalid",
    );
    hosts_panel_scene(state, &listed, true, cx);
}

/// The reference screens' setup of `dev@new-box` (`host-setup-running-hosts-panel` and `host-setup-failed-hosts-panel`)
/// with Details on: the three reference hosts and `dev@new-box` failing to connect at `attempt`, its setup's nine steps
/// in `statuses`, the run in `status`, and `reason` as the failed step's and the run's message. Motion is reduced, so
/// the running step's ring and the status's dot hold still; the test clock does not move, so the elapsed time reads
/// "0:00" where the reference caught "0:06".
fn host_setup_scene(
    state: &SceneState,
    attempt: u32,
    status: farhelm_proto::provisioning::RunStatus,
    statuses: [farhelm_proto::provisioning::StepStatus; 9],
    reason: Option<&str>,
    cx: &mut App,
) {
    use crate::hosts::tests::{connecting, host, hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut new_box = host(
        4,
        "ssh",
        connecting(
            attempt,
            Some(
                "SSH could not connect or log in to the host; SSH stderr: \"ssh: Could not resolve hostname new-box: \
                 Temporary failure in name resolution\"; cause: connection closed before hello",
            ),
        ),
    );
    new_box.name = "dev@new-box".into();
    new_box.destination = Some("dev@new-box".into());
    let mut listed = reference_hosts();
    listed.push(new_box);
    let names = [
        "Create directories",
        "Upload the farhelm binary",
        "Install the farhelm binary",
        "Write the systemd user unit",
        "Reload systemd user units",
        "Enable and start the supervisor",
        "Enable lingering, to start at boot",
        "Remove the bundled tmux",
        "Connect to the supervisor",
    ];
    let mut setup = run(4, ProvisioningOperation::Add, status);
    setup.steps = names
        .into_iter()
        .zip(statuses)
        .map(|(name, status)| {
            let failed = status == StepStatus::Failed;
            let mut step = crate::hosts::progress::tests::step(name, status);
            step.message = reason.filter(|_| failed).map(str::to_owned);
            step
        })
        .collect();
    setup.message = reason.map(str::to_owned);
    hosts_panel_scene_with(state, vec![hosts(&listed), progress(&[setup])], true, cx);
}

/// `dev@new-box`'s setup at its first step (reference `host-setup-running-hosts-panel`): the inline status on its own
/// line under the name, the step list with the running step's amber ring and the rest pending.
fn host_setup_running_hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use farhelm_proto::provisioning::{RunStatus, StepStatus};
    let statuses = std::array::from_fn(|n| {
        if n == 0 {
            StepStatus::Running
        } else {
            StepStatus::Pending
        }
    });
    host_setup_scene(state, 3, RunStatus::Running, statuses, None, cx);
}

/// `dev@new-box`'s setup failed at its last step (reference `host-setup-failed-hosts-panel`): the phase words back,
/// eight steps checked green, the last crossed red with the helm's reason under it, and the run's message after them.
fn host_setup_failed_hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use farhelm_proto::provisioning::{RunStatus, StepStatus};
    let statuses = std::array::from_fn(|n| {
        if n == 8 {
            StepStatus::Failed
        } else {
            StepStatus::Completed
        }
    });
    host_setup_scene(
        state,
        4,
        RunStatus::Failed,
        statuses,
        Some(
            "step 9 (attach-supervisor) failed: injected attach-supervisor failure: host stderr \"Setup could not \
             finish\"; rerun provisioning to continue",
        ),
        cx,
    );
}

/// The Hosts panel with an Update on offer in two rows, Details off (no reference: the old reference screens show no
/// outdated host): `build-box` on an older build, "Old version" with the amber outlined `↑ Update`, and `gpu-1` refused
/// at the handshake on an older protocol, "Needs update" with the red one; the heading's Update all available.
fn hosts_panel_updates_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{reference_hosts, skew};
    let mut listed = reference_hosts();
    listed[1].state = farhelm_proto::ui_channel::HostStateView::Connected {
        identity: Some("c1182bd3-a604-4d1e-855d-ccbf6c992b0b".into()),
        build_version: "0.15.0".into(),
        old_version: true,
        newer_version: false,
        refresh: farhelm_proto::ui_channel::RefreshView::Ok { sessions: 1 },
    };
    listed[2].state = skew(1, 2);
    hosts_panel_scene(state, &listed, false, cx);
}

/// `build-box`'s Update running at its second of three steps, Details off (no reference): the row stays folded and its
/// inline status takes the phase words' place, the count, the running step and the clock without the visible verb.
/// Motion is reduced, so the dot holds still, and the test clock does not move, so the elapsed time reads "0:00".
fn host_update_running_hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut update = run(2, ProvisioningOperation::Update, RunStatus::Running);
    update.steps = [
        ("Upload the farhelm binary", StepStatus::Completed),
        ("Install the farhelm binary", StepStatus::Running),
        ("Connect to the supervisor", StepStatus::Pending),
    ]
    .into_iter()
    .map(|(name, status)| crate::hosts::progress::tests::step(name, status))
    .collect();
    hosts_panel_scene_with(
        state,
        vec![hosts(&reference_hosts()), progress(&[update])],
        false,
        cx,
    );
}

/// The host refusal dialog for a refused Update of `build-box` below the upgrade floor (no reference: the old dialog's
/// look is reference `host-uninstall-refusal`, the same dialog for Uninstall): "Update host · build-box", the red
/// summary, the destination and the helm's manual remedy naming the executable, and Close with focus.
fn host_update_refusal_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::hosts::refusal::{self, Refusal};
    let view = crate::hosts::tests::reference_hosts().remove(1);
    let words = HostText::from_host(
        "the host runs Farhelm 0.12.0, older than this helm's upgrade floor of 0.14.0; it is too old to \
         update in place. On that host, stop its Farhelm supervisor service. Its Farhelm executable is \
         /home/dev/.local/lib/farhelm/farhelm. Remove its installation directory only if that directory is \
         dedicated to Farhelm (normally ~/.local/lib/farhelm). Then remove the host from Farhelm and set it \
         up again",
    );
    let reason = crate::operations::Refusal {
        words,
        status: 409,
        markers: Vec::new(),
    };
    let mark = Mark::take(cx);
    let _ = refusal::open(
        Refusal::of("Update host", &view, &reason),
        &mark,
        window,
        cx,
    );
}

/// The host refusal dialog for an uninstall of `build-box` refused while a session still runs there (reference
/// `host-uninstall-refusal`, which the helm's real refusal drew): "Uninstall Farhelm · build-box", the red summary, the
/// destination and the helm's words, and Close with focus. No Remove: the helm marks no remedy on this refusal.
fn host_uninstall_refusal_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    use crate::hosts::refusal::{self, Refusal};
    let view = crate::hosts::tests::reference_hosts().remove(1);
    let reason = crate::operations::Refusal {
        words: HostText::from_host(
            "uninstall is refused while sessions on this host have not ended or still have terminal tabs open: \
             \"migrate auth tokens\". Stop those sessions and close their tabs, then choose uninstall again",
        ),
        status: 409,
        markers: Vec::new(),
    };
    let mark = Mark::take(cx);
    let _ = refusal::open(
        Refusal::of(crate::hosts::uninstall::ACTION, &view, &reason),
        &mark,
        window,
        cx,
    );
}

/// The uninstall confirmation for the reference screens' `build-box` (no reference: the old dialog was not captured),
/// as its menu's Uninstall opens it once the helm's plan arrives: the host's mark, name, phase and build, the numbered
/// plan with its paths and Copy plan, the kept data in its amber note, and Cancel and Uninstall, focus on Uninstall. The
/// question opens once the connection has a UI session, as the removal confirmation's scene does; the fake, the
/// connection and the watch are kept for the scene's life.
fn host_uninstall_confirmation_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            let view = crate::hosts::tests::reference_hosts().remove(1);
            crate::hosts::uninstall::confirm_for_golden(view, &mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// `build-box`'s uninstall running at its second of six steps, Details off (no reference): the running row opens by
/// itself, its inline status reading "Uninstalling", the count, the running step and the clock, and the run's steps
/// under it with Collapse. Motion is reduced, so the dot holds still, and the test clock does not move, so the elapsed
/// time reads "0:00".
fn host_uninstall_running_hosts_panel_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    use crate::hosts::tests::{hosts, progress, reference_hosts, run};
    use farhelm_proto::provisioning::{ProvisioningOperation, RunStatus, StepStatus};
    ticker::set_reduced_motion(true, cx);
    let mut uninstall = run(2, ProvisioningOperation::Uninstall, RunStatus::Running);
    uninstall.steps = crate::hosts::uninstall::tests::reference_steps()
        .steps
        .iter()
        .enumerate()
        .map(|(n, planned)| {
            let status = match n {
                0 => StepStatus::Completed,
                1 => StepStatus::Running,
                _ => StepStatus::Pending,
            };
            crate::hosts::progress::tests::step(&planned.name, status)
        })
        .collect();
    hosts_panel_scene_with(
        state,
        vec![hosts(&reference_hosts()), progress(&[uninstall])],
        false,
        cx,
    );
}

/// The Hosts panel after this window uninstalled `build-box` (no reference): the two hosts left and, above them, the
/// one-line removal notice, "Removed from build-box · data kept", with Copy and ×. The notice is shown once the
/// connection has a UI session, since it belongs to that session's operation store.
fn hosts_panel_uninstall_notice_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    let mut listed = crate::hosts::tests::reference_hosts();
    listed.remove(1);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(&listed)]);
    let panel = cx.new(HostsPanel::new);
    let shown = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if shown.get() || connection.read(cx).session().is_none() {
            return;
        }
        shown.set(true);
        crate::hosts::uninstall::show_notice_for_golden(
            "build-box",
            "/home/dev/.local/state/farhelm",
            cx,
        );
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(panel));
    keep.push(Box::new(watch));
}

/// The Hosts panel a scene's setup made, as [`hosts_panels`] draws it, in a column tall enough for a setup's nine
/// steps under its row and a failure's messages after them, as the reference screens' sidebar was.
fn tall_hosts_panels(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    div()
        .w(theme::metrics::SIDEBAR_WIDTH - px(56.))
        .h(px(900.))
        .bg(theme::color(palette.bg_1))
        .children(panel)
        .into_any_element()
}

/// The Hosts panel a scene's setup made, in the sidebar's column beside the rail (the sidebar's width less the rail's
/// 56 pixels) on the sidebar's surface.
fn hosts_panels(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    div()
        .w(theme::metrics::SIDEBAR_WIDTH - px(56.))
        .h(px(394.))
        .bg(theme::color(palette.bg_1))
        .children(panel)
        .into_any_element()
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

/// The reference screens' hosts in the Hosts panel, then `build-box`'s menu opened from its toggle (reference
/// `host-menu`): the header naming it, its destination and build, then Retry, highlighted, Update, Settings, and
/// Uninstall and Remove behind a separator.
/// The menu opens through [`host_menu_scene`] once the toggle has been drawn.
fn host_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    hosts_panel_scene(state, &crate::hosts::tests::reference_hosts(), false, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// The Hosts panel at the scene's left, so its menu has room beside it, and once its toggles have been drawn, one press
/// of `build-box`'s toggle after that frame.
fn host_menu_scene(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let panel = state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Entity<HostsPanel>>().cloned());
    if let Some(panel) = panel
        && panel.read(cx).toggle_drawn(2)
        && take_pending(state)
    {
        window.defer(cx, move |window, cx| {
            panel.update(cx, |panel, cx| panel.toggle_menu(2, window, cx))
        });
    }
    div()
        .size_full()
        .child(hosts_panels(state, window, cx))
        .into_any_element()
}

/// The reference screens' hosts, with `build-box`'s settings open as its menu opens them (reference
/// `host-settings-dialog`): the header with its mark, name, phase and build, then Connection, Appearance and the top of
/// Checkouts, focus on the destination's Edit. The dialog opens once the connection has a UI session, whose store the
/// snapshot made, as [`settings_scene`] opens Settings, and the checkout folder's read, sent as it opens, is answered
/// at once as the reference stack had it: no folder of the host's own and no default, so the field shows "No default
/// set". The fake, the connection and the watch are kept for the scene's life.
fn host_settings_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let (helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let helm = Rc::new(RefCell::new(helm));
    helm.borrow_mut().connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let handle = window.window_handle();
    // The scene's ring handle stands in for the menu's toggle, as it stands in for the gear in the Settings scenes.
    let opener = state.ring.clone();
    let opened = Cell::new(false);
    let answering = helm.clone();
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::hosts::open_settings(2, &opener, &mark, window, cx);
        });
        let mut helm = answering.borrow_mut();
        for read in helm.commands() {
            if read.path == "/api/hosts/2/checkout-root" {
                let answer = serde_json::json!({ "root": null, "default": null });
                helm.reply(read.id, 200, Some(answer));
            }
        }
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
}

/// The host settings dialog, then (reference `host-settings-dialog-end`) its body scrolled to its end.
fn host_settings_dialog_end_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    host_settings_dialog_setup(state, window, cx);
    state
        .keep
        .borrow_mut()
        .push(Box::new(Pending(Cell::new(true))));
}

/// Once the dialog is open, one turn of the wheel over its body, after that frame, far enough to reach its end.
fn host_settings_dialog_end(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    if layers::any_open(window, cx) && take_pending(state) {
        window.defer(cx, |window, cx| {
            window.dispatch_event(
                gpui::PlatformInput::Scroll(gpui::ScrollEvent {
                    // The middle of the scene, which the dialog's body covers whatever its header's height.
                    position: point(px(280.), px(500.)),
                    delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-4000.))),
                    ..Default::default()
                }),
                cx,
            );
        });
    }
    div().into_any_element()
}

/// The session list a scene's setup made, in a column of the sidebar's width and surface.
fn session_lists(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    div()
        .w(theme::metrics::SIDEBAR_WIDTH)
        .h(px(394.))
        .bg(theme::color(palette.bg_1))
        .text_color(theme::color(palette.fg_0))
        .children(state.lists.borrow().iter().cloned())
        .into_any_element()
}

/// The width the Linux title bar scene draws its bars at: the window width of the screenshot batch's narrow app variant,
/// a typical small window.
const LINUX_TITLE_BAR_WIDTH: f32 = 900.;

/// The Linux window's own title bar, the one it draws where the compositor draws none (`desktop::linux_title_bar`), as
/// the main window has it: above, the window in its ordinary state, with Maximize; below, maximized, with Restore in
/// its place. Drawn through the bar's drawing function rather than its window predicate, since GPUI's test platform
/// reports server-side decorations and so would never show the bar; the predicate has its own unit tests.
fn linux_title_bars(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let bar = |id, maximized| {
        crate::desktop::linux_title_bar::draw(
            id,
            crate::bootstrap::WINDOW_TITLE,
            "Close Farhelm",
            maximized,
            cx,
        )
    };
    div()
        .w(px(LINUX_TITLE_BAR_WIDTH))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(bar("title-bar-ordinary", false))
        .child(bar("title-bar-maximized", true))
        .into_any_element()
}

/// The notices scene's text: a short stand-in for `licenses_text()` with its sections' shape (a ported-code preamble,
/// then each section opened by the 80-column `=` rule `scripts/dependency-notices.py` writes, a title line and, for a
/// dependency, the line naming what uses it), including an unwrapped paragraph longer than the window's wrap column, so
/// the picture shows a cut line continuing on the next, and a tab-indented line. The rule is the window's most repeated
/// line, so the picture pins how the terminal face draws it.
const NOTICES_FIXTURE: &str = "\
Farhelm includes code ported from other projects, under the notices below.

================================================================================
example-terminal 1.2.3 (Apache-2.0)
Used by: farhelm-gui

\tCopyright 2020 The Example Terminal Authors.

Licensed under the Apache License, Version 2.0 (the \"License\"); you may not use this file except in compliance with the License, a copy of which is printed with the dependency notices.
================================================================================
Example Sans (SIL Open Font License 1.1)

Copyright 2021 The Example Sans Project Authors.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
No warranty is given; see the license for details.";

/// The height the notices scene crops the window to: the real window's text area is 640 pixels high, mostly empty
/// below a fixture this short, so the scene keeps the bar and the fixture's lines with some room under them.
const NOTICES_HEIGHT: f32 = 340.;

/// The Open-Source Notices window (`desktop::licenses`) over [`NOTICES_FIXTURE`], at the window's real opening width
/// (`licenses::opening_width`, which on Linux is what is left once GPUI Kit's frame takes its allowance back) and
/// cropped in height: the text in the terminal face, wrapped at the window's column. On Linux it has the window's own
/// title bar above, as under a compositor that draws none, in the ordinary state the window opens in; on macOS the
/// system draws the title bar, so the scene has none.
fn notices_window(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    use crate::desktop::{licenses, linux_title_bar};
    let lines: Rc<[gpui::SharedString]> = licenses::wrap(NOTICES_FIXTURE)
        .into_iter()
        .map(Into::into)
        .collect();
    let bar = cfg!(target_os = "linux").then(|| {
        linux_title_bar::draw(
            "notices-title-bar",
            licenses::TITLE,
            "Close this window",
            false,
            cx,
        )
    });
    div()
        .w(licenses::opening_width())
        .h(px(NOTICES_HEIGHT))
        .child(licenses::content(&lines, bar, cx))
        .into_any_element()
}

// ---- The version readout --------------------------------------------------------------------------------------------
//
// The sidebar header's version while the desktop updater runs (`update_readout::view`), one scene per group of states.
// No reference screenshot of these states exists (the old web build ran no updater), so each picture was checked
// against the old stylesheet's values (`crates/farhelm-ui/assets/app.css` at the reference commit) and the view's own
// docs. The version text is this build's own (`0.0.0-unreleased` on main), so a version bump redraws every one of
// these scenes.

/// One row of a version readout scene: the readout its fixture updater shows, and the step the row holds whatever spins
/// at (`VersionReadout::holding_step`), if it holds one.
type ReadoutRow = (Readout, Option<u8>);

/// The readouts a version readout scene made, one per row in the scene's order, kept among the scene's kept values.
struct Readouts(Vec<Entity<VersionReadout>>);

/// A readout in `activity`, with an update waiting when `ready`. Only the tooltip scene draws hover words, and it
/// builds its own, so these just name the scene.
fn golden_readout(activity: Activity, ready: bool) -> Readout {
    Readout {
        update_ready: ready,
        needs_reinstall: false,
        withdrawn: false,
        tooltip: "Golden scene readout".to_string(),
        activity,
        notice_id: 1,
        user_update_ready: false,
    }
}

/// The readouts a version readout scene has made so far, in row order; empty before its first frame.
fn kept_readouts(state: &SceneState) -> Vec<Entity<VersionReadout>> {
    state
        .keep
        .borrow()
        .iter()
        .find_map(|kept| kept.downcast_ref::<Readouts>())
        .map(|Readouts(readouts)| readouts.clone())
        .unwrap_or_default()
}

/// The rows `rows` describes, each a readout over a fixture updater of its own (`updates::fixture`: the app keeps one
/// updater, a scene shows several), in a row with the app header's look: the sidebar's width, 40 pixels high, 12
/// pixels of padding across, `--bg-1` with the row hairline under it, and the readout at the right end, as the header
/// draws it.
///
/// Unlike other scenes' entities, the readouts are made while drawing, on the scene's first frame, so that the frame
/// drawn before the setup already shows them: a setup that gives a readout keyboard focus, opens its menu or shows its
/// hover help needs it drawn, and a step deferred to a later frame could be missed by [`settle`], which stops at the
/// first two identical frames. Motion is reduced before any readout exists, so no spinner ever ticks and the capture
/// settles; a row that shows a turn holds its step instead.
fn readout_rows(state: &SceneState, rows: fn() -> Vec<ReadoutRow>, cx: &mut App) -> AnyElement {
    let mut readouts = kept_readouts(state);
    if readouts.is_empty() {
        ticker::set_reduced_motion(true, cx);
        readouts = rows()
            .into_iter()
            .map(|(readout, step)| {
                let updater = updates::fixture(readout, cx);
                cx.new(|cx| VersionReadout::new(updater, cx).holding_step(step))
            })
            .collect();
        state
            .keep
            .borrow_mut()
            .push(Box::new(Readouts(readouts.clone())));
    }
    let palette = theme::palette(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .children(readouts.into_iter().map(|readout| {
            div()
                .w(theme::metrics::SIDEBAR_WIDTH)
                .h(px(40.))
                .px(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .bg(theme::color(palette.bg_1))
                .border_b_1()
                .border_color(theme::color(palette.border_row))
                // The main view's ink, which the header inherits there.
                .text_color(theme::color(palette.fg_0))
                .child(div().min_w_0().child(readout))
        }))
        .into_any_element()
}

/// Give the readout in row `row` keyboard focus as a keyboard user would have it, then press Tab `tabs` more times.
/// The first Tab makes the keyboard the window's last input, which a focus ring needs ([`focus::shows_ring`]), and the
/// coordinator's move puts focus on the row's check button or toggle (their shared handle) without walking every row
/// before it; the further Tabs walk on from there, to a control the readout draws without a handle of its own (the
/// Check failed button).
fn focus_readout(state: &SceneState, row: usize, tabs: usize, window: &mut Window, cx: &mut App) {
    window.activate_window();
    press_tab(window, cx);
    let readout = kept_readouts(state)
        .get(row)
        .cloned()
        .expect("the scene's first frame made its readouts");
    let handle = readout.read(cx).shared_focus();
    focus::focus_control(&handle, &Mark::take(cx), window, cx);
    for _ in 0..tabs {
        press_tab(window, cx);
    }
}

/// Rows of the check button with no update waiting: at rest (the version in `--fg-1`, the `↻` in `--fg-2`); lit, as
/// under the pointer (both in `--fg-0`, the glyph a quarter turned); with a reinstall warning (`⚠` and the version in
/// `--warn`); the warning lit (the version in `--fg-0`, as the old sheet's later hover rule won, the `⚠` still
/// `--warn`); and the button with keyboard focus (lit, with the accent ring and no fill), which the setup gives it. The
/// setup also rests the pointer on the two lit rows ([`LIT_CHECK_ROWS`]).
fn version_readout_check_rows() -> Vec<ReadoutRow> {
    let warned = Readout {
        needs_reinstall: true,
        ..golden_readout(Activity::Idle, false)
    };
    vec![
        (golden_readout(Activity::Idle, false), None),
        (golden_readout(Activity::Idle, false), None),
        (warned.clone(), None),
        (warned, None),
        (golden_readout(Activity::Idle, false), None),
    ]
}

/// The rows of [`version_readout_check_rows`] drawn lit, as under the pointer: the plain button and the warned one.
const LIT_CHECK_ROWS: [usize; 2] = [1, 3];

/// [`version_readout_check_rows`], drawn.
fn version_readout_check(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    readout_rows(state, version_readout_check_rows, cx)
}

/// The pointer resting on each of [`LIT_CHECK_ROWS`], given through the hover state each readout reads, since a capture
/// has no pointer and one pointer could not rest on two rows; then keyboard focus on the last row's check button.
fn version_readout_check_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let readouts = kept_readouts(state);
    for row in LIT_CHECK_ROWS {
        let readout = readouts
            .get(row)
            .expect("the scene's first frame made its readouts");
        readout.update(cx, |readout, cx| readout.rest_pointer(cx));
    }
    focus_readout(state, 4, 0, window, cx);
}

/// The step the progress rows hold: three eighths of a turn, clearly turned and unlike the lit quarter turn.
const HELD_STEP: u8 = 3;

/// Rows of work in progress: a check with the glyph spinning (held at [`HELD_STEP`], in `--fg-0`) and "Checking…"
/// beside it; an install, the same with "Installing…"; and both again while an update waits, where the red toggle with
/// `↑` has no glyph to spin, so the accent ring (at the same step) comes before the words.
fn version_readout_progress_rows() -> Vec<ReadoutRow> {
    let turned = Some(HELD_STEP);
    let installing = || Activity::Installing("0.40.0".to_string());
    vec![
        (golden_readout(Activity::Checking, false), turned),
        (golden_readout(installing(), false), turned),
        (golden_readout(Activity::Checking, true), turned),
        (golden_readout(installing(), true), turned),
    ]
}

/// [`version_readout_progress_rows`], drawn.
fn version_readout_progress(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    readout_rows(state, version_readout_progress_rows, cx)
}

/// Rows of a check's outcome: Up to date in `--fg-1` beside the check button; Check failed, the failure button in
/// `--warn`; Check failed beside the red toggle while an update waits; and the failure button with keyboard focus
/// (the accent ring, no fill), which the setup gives it.
fn version_readout_outcomes_rows() -> Vec<ReadoutRow> {
    let failed = || Activity::Failed("the network is unreachable".to_string());
    vec![
        (
            golden_readout(Activity::UpToDate("0.39.0".to_string()), false),
            None,
        ),
        (golden_readout(failed(), false), None),
        (golden_readout(failed(), true), None),
        (golden_readout(failed(), false), None),
    ]
}

/// [`version_readout_outcomes_rows`], drawn.
fn version_readout_outcomes(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    readout_rows(state, version_readout_outcomes_rows, cx)
}

/// Keyboard focus on the last row's Check failed button: the row's check button first, then one Tab on to the button
/// beside it.
fn version_readout_outcomes_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    focus_readout(state, 3, 1, window, cx);
}

/// Rows of the update-menu toggle once an update is installed: red with `↑` before the version; with a withdrawal
/// (`⚠` in `--warn`, then `↑`, the rest red); and the toggle with keyboard focus (the accent ring and the control-hover
/// fill, the one readout control a focused fill is ported for), which the setup gives it.
fn version_readout_update_ready_rows() -> Vec<ReadoutRow> {
    let withdrawn = Readout {
        withdrawn: true,
        ..golden_readout(Activity::Idle, true)
    };
    vec![
        (golden_readout(Activity::Idle, true), None),
        (withdrawn, None),
        (golden_readout(Activity::Idle, true), None),
    ]
}

/// [`version_readout_update_ready_rows`], drawn.
fn version_readout_update_ready(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    readout_rows(state, version_readout_update_ready_rows, cx)
}

/// Keyboard focus on the last row's toggle.
fn version_readout_update_ready_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    focus_readout(state, 2, 0, window, cx);
}

/// One row, the red toggle, at the scene's top left, so the menu has room beside it.
fn version_readout_menu(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    top_left(readout_rows(
        state,
        || vec![(golden_readout(Activity::Idle, true), None)],
        cx,
    ))
}

/// The toggle pressed: its menu open beside it (10 pixels past its right edge, 4 above its top), headed "Farhelm update"
/// and "An installed update is ready", with Restart to update (focus on it, as a menu opens) and What's new. The toggle
/// keeps the readout's look with the menu open: no fill (the old toggle drew nothing for its expanded state).
fn version_readout_menu_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let readout = kept_readouts(state)
        .first()
        .cloned()
        .expect("the scene's first frame made its readout");
    readout.update(cx, |readout, cx| readout.press_toggle(window, cx));
    assert!(
        layers::any_open(window, cx),
        "the toggle's press opened the update menu (it needs the toggle drawn)"
    );
}

/// The reinstall readout's hover words exactly as `farhelm_desktop::updater::readout` builds them for an idle updater
/// that cannot install the newer release (`ReinstallReason::UpgradeFloor`): the advice, the idle hover for this build,
/// then the reinstall command, last with nothing after it because the user retypes it. This is the longest of the
/// reinstall notices, so it is the one the tooltip's eight-line cut hurts most. Copied rather than called, since this
/// crate cannot depend on the desktop crate (it depends on this one), so a change to that wording should be copied
/// here.
fn reinstall_tooltip() -> String {
    format!(
        "This installation cannot update to the newer release. Stop local sessions and their terminal tabs, quit \
         Farhelm, and run farhelm uninstall first; your data stays on disk but may not carry over cleanly. {} \
         Reinstall it by running this in a terminal: curl -fsSL https://get.farhelm.io/install.sh | sh",
        update_readout::idle_tooltip(farhelm_proto::BUILD_VERSION, true)
    )
}

/// `content` at the scene's top left, leaving the rest of the scene free for a menu or hover help beside it.
fn top_left(content: AnyElement) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_start()
        .child(content)
        .into_any_element()
}

/// One row, the check button with the reinstall warning, at the scene's top left, with room under it and to its right
/// for its hover, so the card is not pushed back by the window's edge (in the app it reaches over the main view).
fn version_readout_tooltip(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    top_left(readout_rows(
        state,
        || {
            let readout = Readout {
                needs_reinstall: true,
                tooltip: reinstall_tooltip(),
                ..golden_readout(Activity::Idle, false)
            };
            vec![(readout, None)]
        },
        cx,
    ))
}

/// The check button's hover help for the reinstall readout, as if the pointer had come to rest over its version: the
/// short tooltip, cut at eight lines in its 280-pixel card, as the old app's was. With this notice the cut hides the
/// reinstall command at the end of the words, a known risk the parity file records (the coordinator kept the old
/// app's cut on 2026-11-14). The words come from the check button's own help function, shown here directly because
/// the button hands them to the layer manager only on a real hover, which a golden has no time for.
fn version_readout_tooltip_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let words = update_readout::view::check_help(&reinstall_tooltip());
    layers::show_tooltip_at(
        &state.tooltip_owner,
        layers::Help::Line(host_text::Line::App(words.into())),
        point(px(330.), px(32.)),
        window,
        cx,
    );
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
        // The app's minimum window height, so the whole menu shows, Open-Source Notices last.
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
        size: (800., 830.),
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

/// A capable host's trash listing as the trash scenes answer it: `checkouts` (each its id, name, repository, archive
/// time and size), in `root`'s `.farhelm-trash`, with removal active when `removing`.
fn trash_listing(
    root: &str,
    checkouts: &[(&str, &str, &str, i64, u64)],
    removing: bool,
) -> serde_json::Value {
    let checkouts: Vec<serde_json::Value> = checkouts
        .iter()
        .map(|(id, name, repository, archived_at, bytes)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "repository": repository,
                "path": format!("{root}/.farhelm-trash/{name}"),
                "archived_at": archived_at,
                "bytes": bytes,
            })
        })
        .collect();
    serde_json::json!({
        "checkouts": checkouts,
        "issues": [],
        "removing": removing,
        "reports_retired": true,
    })
}

/// The reference screens' trash (`checkout-trash-dialog`): two archives on this machine and one on `build-box`, with
/// their sizes, repositories and archive times; `gpu-1`'s trash is empty.
fn trash_reference(host: i64) -> serde_json::Value {
    match host {
        1 => trash_listing(
            "/Users/you/work",
            &[
                (
                    "web-fix-login",
                    "web-fix-login-20261113T174055Z",
                    "acme/web",
                    1_794_591_655,
                    48_234_496,
                ),
                (
                    "api-2",
                    "api-2-20261110T191055Z",
                    "acme/api",
                    1_794_337_855,
                    212_965_785,
                ),
            ],
            false,
        ),
        2 => trash_listing(
            "/home/dev/work",
            &[(
                "api-1",
                "api-1-20261112T191055Z",
                "acme/api",
                1_794_510_655,
                186_961_100,
            )],
            false,
        ),
        _ => trash_listing("/home/dev/work", &[], false),
    }
}

/// How far a trash scene goes once every host has answered.
#[derive(Clone, Copy)]
enum TrashOpen {
    /// The button alone.
    Button,
    /// The dialog open.
    Dialog,
    /// The dialog open and, once its measured reads are answered too, asking to delete everything on the host (`Some`,
    /// its "Delete all on") or on every reachable host (`None`, Delete all).
    Asking(Option<i64>),
}

/// The reference screens' three hosts, all connected, with a Trash button over them that reads each host's trash and
/// gets `listing_of(host)` back at once; once every host has answered, the scene goes as far as `open` says, the
/// dialog's settle pause off, and its measured reads are answered the same way. The fake, the connection, the button and
/// the watch that answers are kept for the scene's life.
fn trash_scene(
    state: &SceneState,
    listing_of: fn(i64) -> serde_json::Value,
    open: TrashOpen,
    window: &mut Window,
    cx: &mut App,
) {
    use crate::trash::TrashButton;
    let (helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let helm = Rc::new(RefCell::new(helm));
    helm.borrow_mut().connect(vec![crate::hosts::tests::hosts(
        &crate::hosts::tests::reference_hosts(),
    )]);
    let button = cx.new(TrashButton::new);
    let window = window.window_handle();
    let answering = helm.clone();
    let opened = Cell::new(matches!(open, TrashOpen::Button));
    // The question waits for the dialog's own measured reads: a host being read offers nothing to delete.
    let ask = Cell::new(match open {
        TrashOpen::Asking(host) => Some(host),
        TrashOpen::Button | TrashOpen::Dialog => None,
    });
    let watch = cx.observe(&button, move |button, cx| {
        {
            let mut helm = answering.borrow_mut();
            for read in helm.commands() {
                let host = read
                    .body
                    .as_ref()
                    .and_then(|body| body["host"].as_i64())
                    .unwrap_or_default();
                helm.reply(read.id, 200, Some(listing_of(host)));
            }
        }
        let ready = button
            .update(cx, |button, cx| button.snapshot(cx))
            .all_ready();
        if opened.get() {
            if let Some(host) = ask.get().filter(|_| ready) {
                ask.set(None);
                let dialog = button.read(cx).dialog_view();
                let _ = window.update(cx, |_, window, cx| {
                    if let Some(dialog) = dialog {
                        dialog.update(cx, |dialog, cx| dialog.ask(host, window, cx));
                    }
                });
            }
            return;
        }
        if !ready {
            return;
        }
        opened.set(true);
        let _ = window.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            button.update(cx, |button, cx| {
                button.open(focus::ReturnTo::Opener, &mark, window, cx);
                if let Some(dialog) = button.dialog_view() {
                    dialog.update(cx, |dialog, _| dialog.without_pause());
                }
            });
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(button));
    keep.push(Box::new(watch));
}

/// The Trash button a trash scene made, at the top left of the sidebar's raised heading surface.
fn trash_scenes(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let button = state.keep.borrow().iter().find_map(|kept| {
        kept.downcast_ref::<Entity<crate::trash::TrashButton>>()
            .cloned()
    });
    div()
        .size_full()
        .flex()
        .items_start()
        .p(px(8.))
        .bg(theme::color(palette.bg_2))
        .children(button)
        .into_any_element()
}

/// The Trash button over hosts whose trash is empty: the outline can, no count.
fn trash_button_empty_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(
        state,
        |_| trash_listing("/work", &[], false),
        TrashOpen::Button,
        window,
        cx,
    );
}

/// The Trash button over the reference trash (reference `main-running-session`'s header): the open-lid can and its
/// count, 3.
fn trash_button_full_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(state, trash_reference, TrashOpen::Button, window, cx);
}

/// The Trash button while this machine frees space, under reduced motion so the picture holds still: the can upside
/// down, unlifted, with its count.
fn trash_button_emptying_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
    trash_scene(
        state,
        |host| {
            let mut listing = trash_reference(host);
            if host == 1 {
                listing["removing"] = serde_json::json!(true);
            }
            listing
        },
        TrashOpen::Button,
        window,
        cx,
    );
}

/// The Trash dialog over the reference trash (reference `checkout-trash-dialog`): its title with the open can and the
/// total, the intro, each host's "Delete all on", this machine's two archives and build-box's one with their sizes,
/// repositories, archive times and Deletes, each host's folder, and the footer's Close, where focus starts (its ring
/// shows only under keyboard focus, so the picture shows none), and Delete all.
fn checkout_trash_dialog_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(state, trash_reference, TrashOpen::Dialog, window, cx);
}

/// The Trash dialog over the reference trash asking to delete everything: the question in its red band in the region
/// above the footer, naming the three checkouts on this machine and build-box with their size and the warning, Cancel
/// and Delete permanently (where focus is, its ring hidden as on any pointer-opened question).
fn checkout_trash_confirm_all_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(state, trash_reference, TrashOpen::Asking(None), window, cx);
}

/// The Trash dialog over the reference trash asking to delete build-box's one archive: the question in its red band
/// inside build-box's card, under its header.
fn checkout_trash_confirm_host_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    trash_scene(
        state,
        trash_reference,
        TrashOpen::Asking(Some(2)),
        window,
        cx,
    );
}

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
                        tooltip_owner: cx.new(|_| TooltipOwner),
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

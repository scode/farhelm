//! Headless screenshot goldens: one per component, per dialog frame and per overlay kind (tooltip, once more with help
//! in paragraphs, popover, the prompt over its veil, once more with a summary that names a host value, once more with
//! a feature-drawn body, a header tile and a footer note, and the prompt anchored to a row's menu button),
//! the keyboard focus ring, host-supplied text over its spoofing corpus, the notice banner, the terminal's styles and
//! cursor shapes, an attached terminal's exit line and status line, the scrim a pending Delete puts over the open
//! session's terminal, the session list (a fleet of every status a row draws, the open row on a host the helm cannot
//! reach, an empty fleet, a row's open menu, a row's Delete question, and a pending and a refused Delete), the Rename
//! editor, the select closed and open, the
//! YOLO confirmation drawn inline in a launcher's body with its outlined permanent answer, the Settings dialog at
//! Sessions, at Appearance, at Hosts, at Terminal and at Agent trust, the Hosts panel (closed, with Details, and with a
//! host failing to connect), the Add host dialog and the setup confirmation it leads to, a host's actions menu, and the
//! host settings dialog at its top and scrolled to its end, in both appearances, compared pixel for pixel.
//! (The terminal stays dark in both appearances, so it draws identically in both pictures; only the scene's ground
//! around it differs.)
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
use crate::components::{
    Button, Checkbox, DialogFrame, HoverHelp, IconButton, Role, TextInput, TextInputState, Tier,
};
use crate::components::{RadioGroup, RadioOption};
use crate::components::{Select, SelectOption};
use crate::components::{TabOption, TabStrip};
use std::cell::RefCell;
use std::rc::Rc;

use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::HostsPanel;
use crate::hosts::identity::host_mark;
use crate::layers::{self, Anchor, Popover, TooltipOwner};
use crate::notices::Notice;
use crate::operations::OpKey;
use crate::prompt::{self, Answer, Question};
use crate::root;
use crate::session;
use crate::session_list::SessionList;
use crate::session_list::filters::Status;
use crate::session_list::host_rail::tests::{hosts, local, remote};
use crate::session_list::tests::{Fixture, TEST_NOW, listing, preferences, session};
use crate::terminal::attached::{AttachedTerminal, OpenStream, TerminalTarget};
use crate::terminal::{CellMetrics, CursorPhase, TerminalView, TextSize};
use crate::test_support;
use crate::theme::{self, Preference};
use crate::ticker;
use farhelm_grid_model::GridOutput;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::FakeTerminal;
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{
    Availability, Collection, CollectionChanges, CollectionMeta, CollectionState, HostView,
    KeyedEntry, ListCounts, PREFERENCES_KEY,
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
            start: Some(container),
            anchor,
            returns_to: focus::ReturnTo::Opener,
        },
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
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    let order: Vec<&str> = rows.iter().map(|row| row.key).collect();
    let history: Vec<&str> = open.into_iter().collect();
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
    helm.connect(vec![
        listing(&order, rows),
        preferences(&history, None),
        registry,
    ]);
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

/// The fleet of [`session_list_setup`], which the row menu and Delete scenes draw too. Each row lists the helm
/// fixture's one terminal tab and managed checkout. The rows run on the local host (the red laptop, inverted on the
/// selected row) and on remotes (the cloud), with one launch of each kind and shield: a guarded Claude launch (an
/// identity color and the green shield), the fixture's Codex YOLO launch (a grey mark and the amber slashed shield), a
/// command with no agent asserted not YOLO (the Terminal mark), Cursor under its own setting (the dashed shield), a
/// guarded Muse launch, and a session from before launch kinds (the amber question mark).
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
        row("b", "Review the install docs", "waiting", 2 * MINUTE).cwd("/home/user/src/docs"),
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

/// The waiting row's actions menu, open below its button with Rename, its first command, highlighted (reference
/// `session-row-menu`).
fn session_row_menu_setup(state: &SceneState, _: &mut Window, cx: &mut App) {
    session_list_then(
        state,
        |list, _, window, cx| list.update(cx, |list, cx| list.toggle_menu("b", window, cx)),
        cx,
    );
}

/// The waiting row's Delete question, asked from its row as the menu's Delete asks it (reference
/// `row-delete-confirmation`): its tab, its running agent, its quoted title and its managed checkout. The
/// `anchored-prompt` golden pins the prompt's own look with made-up wording; this one pins the row's wording and where
/// it sits over the list.
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
            let mut keep = state.keep.borrow_mut();
            let Some(helm) = keep
                .iter_mut()
                .find_map(|kept| kept.downcast_mut::<FakeHelm>())
            else {
                return;
            };
            let gone: Vec<String> = fleet().iter().map(|row| row.key.to_owned()).collect();
            helm.push(CollectionChanges {
                collection: Collection::Sessions,
                meta: CollectionMeta {
                    availability: Availability::Available,
                    order: Vec::new(),
                    counts: Some(ListCounts {
                        total: 6,
                        matching: Some(0),
                        truncated: false,
                    }),
                },
                upsert: Vec::new(),
                remove: gone,
            });
            for command in helm.commands() {
                helm.reply(command.id, 204, None);
            }
        },
        cx,
    );
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
    let mut keep = state.keep.borrow_mut();
    let Some(helm) = keep
        .iter_mut()
        .find_map(|kept| kept.downcast_mut::<FakeHelm>())
    else {
        return;
    };
    let gone: Vec<String> = fleet()
        .iter()
        .map(|row| row.key.to_owned())
        .filter(|key| !shown.contains(&key.as_str()))
        .collect();
    helm.push(CollectionChanges {
        collection: Collection::Sessions,
        meta: CollectionMeta {
            availability: Availability::Available,
            order: shown.iter().map(|key| key.to_string()).collect(),
            counts: Some(ListCounts {
                total: 6,
                matching: Some(shown.len() as u64),
                truncated: false,
            }),
        },
        upsert: Vec::new(),
        remove: gone,
    });
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

/// The Settings dialog over the fake helm's preference row, opened at `section` as the gear opens it (references
/// `settings-sessions`, `settings-appearance`, `settings-hosts`, `settings-terminal`, `settings-checkouts`,
/// `settings-agent-trust` and `settings-notifications`): focus on the section's first switch (on the dialog itself at
/// Appearance, Terminal, Checkouts, Agent trust and Notifications), naming on because the row stores no choice for it,
/// and automatic remote-host updates turned on so the Hosts picture shows a switch in each state. Agent trust's reads
/// of each agent's trust, sent as it opens, are answered at once with the trust off and the hint not dismissed,
/// Checkouts' reads as the reference stack had them: no default folder, no post-clone command, and a checkout kept
/// ready, and Notifications' read of the ntfy settings with the helm's defaults (off, `https://ntfy.sh`, no topic,
/// Default priority, no token); nothing else is answered. The opener is the scene's ring handle, which stands in for
/// the gear. The fake and the connection are kept for the scene's life, like the Rename editor's.
fn settings_scene(
    state: &SceneState,
    section: crate::settings::Section,
    window: &mut Window,
    cx: &mut App,
) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![CollectionState {
        collection: Collection::Preferences,
        meta: test_support::meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::json!({ "auto_update_remote_hosts": true }),
        }],
    }]);
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
        });
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
    // Open once the connection has a UI session, as Settings' scene does.
    let handle = window.window_handle();
    let opened = std::cell::Cell::new(false);
    let watch = cx.observe(&connection, move |connection, cx| {
        if opened.get() || connection.read(cx).session().is_none() {
            return;
        }
        opened.set(true);
        let _ = handle.update(cx, |_, window, cx| {
            let mark = Mark::take(cx);
            crate::templates::open(&mark, window, cx);
        });
    });
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(watch));
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

/// Settings at Terminal, the default link choice chosen because the row stores none. Focus starts on the dialog itself
/// (`Section::first`), so no ring shows, as in the reference, where the section was reached by a click.
fn settings_terminal_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Terminal, window, cx);
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

/// Settings at Notifications, its ntfy alerts form over the helm's defaults, as in the reference but for the Sounds
/// sub-heading above it, which is not built. Focus starts on the dialog itself (`Section::first`), so no ring shows.
fn settings_notifications_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    settings_scene(state, crate::settings::Section::Notifications, window, cx);
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

/// Make the Hosts panel over a fake helm holding `listed` hosts, with Details on when `details` is; the scene draws it
/// through [`hosts_panels`]. The fake, the connection and the panel are kept for the scene's life.
fn hosts_panel_scene(state: &SceneState, listed: &[HostView], details: bool, cx: &mut App) {
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = session::start(handle, events, None, cx);
    helm.connect(vec![crate::hosts::tests::hosts(listed)]);
    let panel = cx.new(HostsPanel::new);
    if details {
        panel.update(cx, |panel, cx| panel.show_details(true, cx));
    }
    let mut keep = state.keep.borrow_mut();
    keep.push(Box::new(helm));
    keep.push(Box::new(connection));
    keep.push(Box::new(panel));
}

/// The reference screens' three connected hosts, Details off (reference `hosts-panel`): the heading with its count and
/// switch, and each row's mark, name and phase.
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
/// `host-menu`): the header naming it, its destination and build, and Settings, its one command so far, highlighted.
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
        name: "terminal-delete-scrim",
        size: (444., 174.),
        draw: terminal_delete_scrim,
        setup: terminal_delete_scrim_setup,
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
        name: "session-row-menu",
        size: (420., 418.),
        draw: session_lists_then,
        setup: session_row_menu_setup,
    },
    Scene {
        name: "row-delete-confirmation",
        size: (420., 418.),
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
        name: "rename-dialog",
        size: (560., 320.),
        draw: empty,
        setup: rename_dialog_setup,
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
        name: "settings-terminal",
        size: (800., 830.),
        draw: empty,
        setup: settings_terminal_setup,
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

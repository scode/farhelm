//! Golden scenes for the shared foundations: the component layer's controls and dialog frame, the layer manager's
//! overlays, the focus ring, the prompt and host-supplied text. The scene list itself, `SCENES` with each scene's name
//! and size, stays in `mod.rs`.

use crate::assets::Icon;
use crate::components::{
    Button, Checkbox, CommandWell, DialogFrame, HoverHelp, IconButton, RadioGroup, RadioOption,
    Role, ScrollBox, Select, SelectOption, SequenceChip, SequencePill, TabOption, TabStrip,
    TextInput, Tier,
};
use crate::focus::Mark;
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::host_mark;
use crate::layers::{self, Anchor, Popover};
use crate::notices::Notice;
use crate::prompt::{self, Answer, Question};
use crate::theme;
use crate::ticker;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, IntoElement, ParentElement, Render,
    Styled, Window, div, point, px,
};

use super::{SceneState, press_tab, rest_pointer_on_control};

/// The checkbox, off, on, and on but unavailable: the old app's native checkbox tinted with the accent.
pub(super) fn checkboxes(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn switches(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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

/// The command well (`.ssh-help-command` and `.tensorlake-command` in the old stylesheet) at SSH help's width: a short
/// command whose BEL shows escaped, and a long one that wraps inside its well, each with Copy centered beside it.
pub(super) fn command_wells(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let well = |id: &'static str, command: String| {
        let help = HoverHelp::tooltip("Golden scene control");
        CommandWell::new(id, HostText::from_host(command), "Copy", help)
    };
    let long = format!(
        "ssh -o ControlPath=/tmp/{} -N 'user@host.example.invalid'",
        "control-path-".repeat(20)
    );
    div()
        .w(px(560.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .text_color(theme::color(palette.fg_0))
        .child(well(
            "short",
            "ssh -N -f -o ControlPath=/state/s.sock -- 'dev@build\u{7}box'".to_owned(),
        ))
        .child(well("long", long))
        .into_any_element()
}

/// Every button tier but the outlined danger one, a disabled main action and an icon button, in a first row; in a
/// second, the host rail's entries (All pressed, a host's mark not pressed, and a host's mark pressed in the quieter
/// look the rail's choice wears while Hosts management is shown, on the rail's ground), the filter chip with
/// a mark, a name and its ✕, and the link tier. The outlined danger tier is pinned at rest by `inline-prompt`; its
/// unavailable and busy looks are not pinned yet.
pub(super) fn buttons(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn text_input_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    state.input.set_text("api-refactor", window, cx);
}

/// A labeled text input holding a value.
pub(super) fn text_input(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn growing_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
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
pub(super) fn growing(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn kept_line_breaks_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
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
pub(super) fn kept_line_breaks(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn dialog(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn dialog_notices_stop(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn dialog_launcher_frame(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn tooltip(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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

/// Rest the pointer on the button, so its long tooltip shows placed against it: below it with the old 28-pixel gap, as
/// the button sits at the scene's top with no room above.
pub(super) fn tooltip_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    rest_pointer_on_control(state, "restart", window, cx);
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
pub(super) fn tooltip_paragraphs(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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

/// Rest the pointer on the button, so its paragraph help shows placed against it, as for [`tooltip_setup`].
pub(super) fn tooltip_paragraphs_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    rest_pointer_on_control(state, "agent", window, cx);
}

/// The beside tooltip scene's help: a session row's status, the short help a row's marks show beside the sidebar.
const BESIDE_HELP: &str = "Idle since 10:42. Click the dot to mark it read.";

/// A sidebar column holding one session-row-like control, which the setup shows beside help for: the card 6 pixels
/// past the column's edge, the notch pointing back at the control and the accent ring just inside it. The control
/// draws the bounds tracker of the scene's owner token, whose help the setup shows, as a control draws its own while
/// its help shows.
pub(super) fn tooltip_beside(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let palette = theme::palette(cx);
    let row = div()
        .relative()
        .mx(px(8.))
        .px(px(8.))
        .h(px(30.))
        .flex()
        .items_center()
        .rounded(theme::metrics::RADIUS_MD)
        .child("Fixture 1")
        .child(state.tooltip_owner.read(cx).track());
    let sidebar = div()
        .w(px(200.))
        .h_full()
        .flex()
        .flex_col()
        .justify_center()
        .bg(theme::color(palette.bg_1))
        .border_r_1()
        .border_color(theme::color(palette.border))
        .text_color(theme::color(palette.fg_0))
        .child(row);
    // The edge goes on a box around the bordered column, so the help is measured from outside its border.
    let outer = div()
        .relative()
        .h_full()
        .child(sidebar)
        .child(layers::beside_edge("golden-help-edge", window, cx));
    // Wide enough that the card fits beside the column: a narrower scene shows the fallback above the row instead.
    div()
        .w(px(576.))
        .h(px(136.))
        .flex()
        .child(outer)
        .into_any_element()
}

/// Show the row's help beside the sidebar as if the pointer had come to rest on it.
pub(super) fn tooltip_beside_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let cues = layers::BesideCues {
        on_fill: false,
        radius: theme::metrics::RADIUS_MD,
    };
    let hint = layers::Hint::placed(BESIDE_HELP.into(), layers::HelpPlacement::Beside(cues));
    layers::show_tooltip_at(
        &state.tooltip_owner,
        hint,
        point(px(60.), px(80.)),
        window,
        cx,
    );
}

/// A neutral button and its ring, as a keyboard user sees the button after tabbing to it.
pub(super) fn focus_ring(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn focus_ring_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    window.activate_window();
    press_tab(window, cx);
}

/// The scroll box twice, as an agent approval's card draws its commands, in the monospace face: one taller than its
/// box, which the setup's Tab focuses, so the golden pins its ring two pixels in over the border, and a short one at
/// rest, held to its least height.
pub(super) fn scroll_boxes(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn tab_strip(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn segmented_control(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn radio_group(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn radio_group_variants(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn notice_banner(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w_full()
        .child(crate::main_view::banner(Notice::ConnectionLost))
        .into_any_element()
}

/// The quick-launch sequence pill as it shows outside the launcher, centered over an area the size of a small main pane:
/// a bound number key's template, an unbound key and the ⚡ template as chips, the word `web` in progress and its
/// candidates hanging below the pill. There is no reference picture of the old pill; it was compared with the old
/// stylesheet's `.launch-sequence-*` rules instead.
pub(super) fn sequence_pill(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn popover(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn popover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
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
pub(super) fn selects(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn select_open(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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
pub(super) fn select_open_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
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

/// A delete confirmation, asked through the prompt over its veil.
pub(super) fn prompt_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let question = Question::new(
        "Delete session?",
        Answer::new(
            "Delete",
            HoverHelp::tooltip("Delete the session"),
            Tier::Danger,
        ),
        "api-refactor".to_string(),
    )
    .summary("api-refactor, on this machine")
    .line("The session's terminal and its history are removed. The working directory stays.");
    prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
}

/// The same delete confirmation with its summary naming the session by a title a host wrote, as the session delete
/// confirmation does: the title's right-to-left override shows as `<U+202E>` inside the dialog's header, and the app's
/// words around it keep their order.
pub(super) fn prompt_host_text_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let title = HostText::from_host("invoice\u{202E}fdp.exe");
    let question = Question::new(
        "Delete session?",
        Answer::new(
            "Delete",
            HoverHelp::tooltip("Delete the session"),
            Tier::Danger,
        ),
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
pub(super) fn prompt_body_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let answer = |label: &'static str, help: &'static str, tier: Tier| {
        Answer::new(label, HoverHelp::tooltip(help), tier)
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
pub(super) fn anchored_prompt(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn anchored_prompt_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(anchor) = state.anchor.borrow().clone() else {
        return;
    };
    let mark = Mark::take(cx);
    let title = HostText::from_host("migrate auth tokens");
    let question = Question::new(
        "Delete confirmation",
        Answer::new(
            "Confirm delete",
            HoverHelp::tooltip("Delete this session and its state now"),
            Tier::Danger,
        ),
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
pub(super) fn inline_prompt(state: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
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
pub(super) fn inline_prompt_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let host = HostText::from_host("Local (this machine)");
    let answer = |label: &'static str, help: &'static str, tier: Tier| {
        Answer::new(label, HoverHelp::tooltip(help), tier)
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
pub(super) fn host_text(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
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

/// The sequence pill fades in when it appears (`components::entrance`); with motion reduced it is drawn at once, so the
/// picture is the settled pill rather than the fade's first, transparent frame.
pub(super) fn sequence_pill_setup(_: &SceneState, _: &mut Window, cx: &mut App) {
    ticker::set_reduced_motion(true, cx);
}

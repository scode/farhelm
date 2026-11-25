//! The Agent tab's choices: the harness chips, effort, permissions with their help and the Custom flags editor, and
//! the line that says what an agent-choice edit changed or refused.
//!
//! Ported from the old app's `launch_controls.rs` (`LaunchControls`, `CustomPermissionFlags`) and the harness row in
//! `list/create_form.rs`, at reference commit `24bc0580`. Every choice is one of the draft's edit methods, so a hand
//! choice has exactly the effects SPEC.md gives it (reconciling an agent switch, clearing an effort a model does not
//! offer, dropping Custom flags on leaving Custom); this module only draws the choices the draft and `farhelm_proto`
//! allow, in the old controls' shapes:
//!
//! - **Harness**: compact chips in picker order, using the session list's colored marks. Bash and Zsh follow a rule.
//! - **Effort**: a segmented button group of Default and the offered efforts, or a select once that is more than five
//!   (the old switch to a native `<select>`).
//! - **Permissions**: a segmented button group of what the agent type offers, YOLO in danger ink, then a "?" whose
//!   hover explains the mode in force and whose press opens the docs on permission modes.
//! - **Custom flags**: under the permissions while Custom is chosen, with the words they parse to, or why they do not.
//!
//! Enter on any of them applies it and launches (SPEC.md: "Enter on a choice button ... applies that choice and then
//! launches"), except Custom, whose Enter only chooses, since a Custom choice with no flags yet cannot launch.
//!
//! An agent switch that made the shown permission more dangerous puts up the red banner, with Undo, and outlines the
//! permission choices (`escalation`); the draft decides when.

use farhelm_proto::custom_agents::CustomAgent;
use farhelm_proto::launcher::{EFFORT_ORDER, compatible_efforts};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::changed::Changed;
use super::{Launcher, row};
use crate::components::{
    Button, CustomFlags, HoverHelp, RadioGroup, RadioOption, Role, Select, SelectOption, Tier,
};
use crate::focus::{self, Mark};
use crate::harness;
use crate::host_text::{HostSentence, HostText, Line};
use crate::launcher::draft::{Edit, Mode, RestartBaseline, harness_label};
use crate::launcher::request::capitalized;
use crate::launcher::words::{HARNESS_PICKER_ORDER, PERMISSION_DOCS, guarded_permission_help};
use crate::permission::words::{
    DEFAULT_EFFORT_HELP, effort_label, permission_display, permission_help, permission_tooltip,
};
use crate::theme::{self, metrics};

/// Above this many options (Default included) effort is a select rather than segments, as the old launcher switched
/// to a native `<select>` past five.
const MAX_EFFORT_SEGMENTS: usize = 5;

/// Align the free-text model input and the help button with the small segments' 28px control and its well's edge
/// and padding. The effort select keeps its usual size when there are too many choices for segments.
pub(super) const MODEL_ROW_CONTROL_HEIGHT: Pixels = px(34.);

/// The approval modes, after Guarded and YOLO and Custom, in the old fixed order; each shows for whichever agent type
/// offers it (Goose all three, OMP Approve alone).
const APPROVAL_MODES: [LaunchPermission; 3] = [
    LaunchPermission::Approve,
    LaunchPermission::SmartApprove,
    LaunchPermission::Chat,
];

#[derive(Clone, PartialEq)]
enum HarnessChoice {
    BuiltIn(LaunchHarness),
    Custom(String),
}

impl Launcher {
    /// The agent-choice edit `edit` happened: its notice replaces the one shown (none clears it), the draft changed,
    /// and an agent switch or escalation it carries takes away or puts up the escalation banner (`escalation`).
    pub(super) fn chose(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_edit_escalation(&edit, cx);
        self.notice = edit.notice;
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// The prefix the helm remembers for `harness`, from the pushed preferences.
    pub(super) fn remembered_wrapper(
        &self,
        harness: LaunchHarness,
        cx: &App,
    ) -> Option<Vec<String>> {
        self.store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| {
                preferences
                    .remembered_wrapper_prefixes
                    .get(&harness)
                    .cloned()
            })
    }

    pub(super) fn choose_harness(
        &mut self,
        harness: LaunchHarness,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let wrapper = self.remembered_wrapper(harness, cx);
        let edit = self
            .draft
            .choose_harness(harness, self.catalog.as_deref(), wrapper.as_deref());
        self.chose(edit, window, cx);
        self.check_hook_question(window, cx);
    }

    pub(super) fn choose_effort(&mut self, effort: Option<LaunchEffort>, cx: &mut Context<Self>) {
        self.draft.choose_effort(effort);
        self.notice = None;
        self.changed(cx);
    }

    /// Resolve a chip or search row against the pushed catalog at selection time.
    pub(super) fn choose_custom_agent(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let agent = self
            .store
            .read(cx)
            .collections()
            .get::<CustomAgent>(id)
            .cloned();
        let Some(agent) = agent else {
            self.notice = Some("That custom agent is no longer registered".into());
            cx.notify();
            return;
        };
        let wrapper = self.remembered_wrapper(LaunchHarness::Custom, cx);
        let edit =
            self.draft
                .choose_custom_agent(&agent, self.catalog.as_deref(), wrapper.as_deref());
        self.chose(edit, window, cx);
    }

    /// A permission chosen by hand. Leaving Custom empties its flags field with
    /// the draft's flags; choosing Custom moves focus on to its flags field, by key or click, as the old launcher did.
    /// The field appears only now, so the focus coordinator makes the move once a frame draws it, unless the user moves
    /// on first.
    pub(super) fn choose_permissions(
        &mut self,
        permissions: Option<LaunchPermission>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mark = Mark::take(cx);
        self.draft.choose_permissions(permissions);
        self.notice = None;
        self.sync_fields(window, cx);
        self.changed(cx);
        if permissions == Some(LaunchPermission::Custom) {
            focus::focus_in_layer(&self.flags.focus_handle(cx), &mark, window, cx);
        }
    }

    /// Enter on a permission segment, after the shared rule applied its choice: launch, unless that choice is Custom,
    /// which only chooses (its flags come first).
    fn permission_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.agent.permissions != Some(LaunchPermission::Custom) {
            self.press(window, cx);
        }
    }

    /// The efforts the effort control offers (old `displayed_efforts`): what the agent type and model offer, plus the
    /// chosen one and Restart with's stored one when they are not (an old launch's effort stays visible and
    /// choosable), each in its place in the effort order.
    fn displayed_efforts(&self, harness: LaunchHarness) -> Vec<LaunchEffort> {
        let agent = &self.draft.agent;
        let mut efforts = compatible_efforts(
            harness,
            agent.model.as_deref(),
            self.catalog.as_deref().unwrap_or_default(),
        );
        let stored = match &self.draft.mode {
            Mode::RestartWith(restart) => match &restart.baseline {
                RestartBaseline::Agent(selection) => {
                    selection.effort.as_ref().and_then(|value| value.known())
                }
                RestartBaseline::Command(_) => None,
            },
            Mode::New | Mode::Clone | Mode::ReplaceWith { .. } => None,
        };
        let rank = |effort: &LaunchEffort| EFFORT_ORDER.iter().position(|e| e == effort);
        for kept in [agent.effort, stored].into_iter().flatten() {
            if efforts.contains(&kept) {
                continue;
            }
            let at = efforts
                .iter()
                .position(|effort| rank(effort) > rank(&kept))
                .unwrap_or(efforts.len());
            efforts.insert(at, kept);
        }
        efforts
    }

    /// Model, Effort and Permissions travel together when they fit. Wrap a whole field when an agent's controls
    /// need more room, with any Restart-with change marker directly below its own field.
    pub(super) fn model_choices(
        &self,
        model: Option<Vec<AnyElement>>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let harness = self.draft.agent.harness;
        let field = || div().flex().flex_col().gap(px(4.)).max_w_full();
        let effort = harness.is_none_or(LaunchHarness::offers_effort).then(|| {
            field()
                .child(self.effort_row(harness, palette, cx))
                .children(self.changed_marker(Changed::Effort, palette))
        });
        let permissions = (!harness.is_some_and(LaunchHarness::is_shell)).then(|| {
            field()
                .child(self.permission_row(harness, palette, cx))
                .children(self.changed_marker(Changed::Permissions, palette))
        });
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(12.))
            .children(model.map(|rows| {
                field()
                    .flex_grow_1()
                    .flex_basis(px(144.))
                    .min_w(px(144.))
                    .children(rows)
            }))
            .children(effort)
            .children(permissions)
    }

    /// Custom agents take raw contract values and use a fixed YOLO label because the wrapper owns permission handling.
    pub(super) fn custom_agent_fields(&self, palette: &theme::Palette) -> Vec<AnyElement> {
        const HELP: &str = "Lowercase letters, digits, hyphens and underscores. Leave blank to use the agent's default.";
        let input =
            |id, row_id, label, field: &crate::components::TextInputState, value: &str, changed| {
                let error = if value.is_empty() {
                    None
                } else {
                    farhelm_proto::custom_agents::validate_model_or_effort(value).err()
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .flex_1()
                    .min_w_0()
                    .child(row(
                        row_id,
                        label,
                        self.launcher_input(id, field, label, HoverHelp::tooltip(HELP))
                            .monospace()
                            .invalid(error.is_some()),
                        palette,
                    ))
                    .children(error.map(|error| {
                        status_line("launcher-custom-value-error", capitalized(error), palette)
                    }))
                    .children(self.changed_marker(changed, palette))
            };
        let model = input(
            "launcher-custom-model",
            "launcher-custom-model-row",
            "Model",
            &self.custom_model,
            self.draft.agent.model.as_deref().unwrap_or_default(),
            Changed::Model,
        );
        let effort = input(
            "launcher-custom-effort",
            "launcher-custom-effort-row",
            "Effort",
            &self.custom_effort,
            &self.draft.agent.custom_effort,
            Changed::Effort,
        );
        let permissions = div().flex().items_center().gap(px(8.))
            .child(div().px(px(8.)).py(px(4.)).rounded_full()
                .bg(theme::color(palette.danger_bg_dim))
                .text_color(theme::color(palette.danger))
                .child("YOLO"))
            .child(Button::new(
                "launcher-permission-help", "About permission modes",
                HoverHelp::tooltip("Custom agents always count as YOLO. Hosts that ask before YOLO launches still ask for confirmation."),
                Role::Button,
            ).content("?").tier(Tier::Neutral).square(MODEL_ROW_CONTROL_HEIGHT)
                .on_click(|_, _, cx| cx.open_url(PERMISSION_DOCS)));
        vec![
            div()
                .flex()
                .gap(px(12.))
                .child(model)
                .child(effort)
                .into_any_element(),
            row(
                "launcher-permissions-row",
                "Permissions",
                permissions,
                palette,
            )
            .into_any_element(),
        ]
    }

    /// The shell chips and rule can wrap as a unit onto the same line as the last agent chips.
    /// Keep the separate shell IDs used to capture the shell choices in the docs.
    pub(super) fn harness_choices(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (on_change, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let option = |harness| {
            let label = harness_label(harness);
            let glyph = harness::glyph(harness);
            // The option's help is also its accessible description. The divider itself carries no words.
            let help = if harness.is_shell() {
                format!("Use {label} as the session shell")
            } else {
                format!("Use {label} for the session")
            };
            RadioOption::new(
                label,
                HoverHelp::tooltip(help),
                HarnessChoice::BuiltIn(harness),
            )
            .mark(
                harness::icon(glyph),
                harness::ink(glyph, palette).unwrap_or(palette.fg_1),
            )
        };
        let chips = RadioGroup::new(
            "launcher-harness",
            "Harness",
            HoverHelp::tooltip("Harness: the agent or shell this session starts"),
            Role::GenericContainer,
        )
        .chips();
        let (shells, agents): (Vec<_>, Vec<_>) = HARNESS_PICKER_ORDER
            .iter()
            .copied()
            .partition(|harness| harness.is_shell());
        let chips = agents
            .into_iter()
            .map(option)
            .fold(chips, RadioGroup::option);
        let custom = self.store.read(cx).collections().ordered::<CustomAgent>();
        let chips = if custom.is_empty() {
            chips
        } else {
            let glyph = harness::glyph(LaunchHarness::Custom);
            custom.into_iter().fold(
                chips.separated_named_chips(
                    "launcher-custom-row",
                    "launcher-custom",
                    Some("Custom"),
                ),
                |chips, agent| {
                    let label = HostSentence::new(gpui::SharedString::from(format!(
                        "launcher-custom-name-{}",
                        agent.id
                    )))
                    .host(&HostText::from_host(agent.display_name.as_str()));
                    chips.option(
                        RadioOption::new(
                            "",
                            HoverHelp::tooltip(format!("Use custom agent {}", agent.id)),
                            HarnessChoice::Custom(agent.id.clone()),
                        )
                        .catalog_label(agent.id.clone(), label.into())
                        .mark(
                            harness::icon(glyph),
                            harness::ink(glyph, palette).unwrap_or(palette.fg_1),
                        ),
                    )
                },
            )
        };
        let chips = shells
            .into_iter()
            .map(option)
            .fold(
                chips.separated_chips("launcher-shells-row", "launcher-shells"),
                RadioGroup::option,
            )
            .chosen(self.draft.agent.custom_agent.as_ref().map_or_else(
                || self.draft.agent.harness.map(HarnessChoice::BuiltIn),
                |id| Some(HarnessChoice::Custom(id.clone())),
            ))
            .on_change(move |harness, window, cx| {
                let _ = on_change.update(cx, |launcher, cx| match harness {
                    HarnessChoice::BuiltIn(harness) => {
                        launcher.choose_harness(*harness, window, cx)
                    }
                    HarnessChoice::Custom(id) => launcher.choose_custom_agent(id, window, cx),
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            });
        div()
            .id("launcher-harness-row")
            .debug_selector(|| "launcher-harness-row".into())
            .min_w_0()
            .child(chips)
    }

    /// Effort (`.launch-composer-effort-choice`): Default and the displayed efforts as segments, or as a select past
    /// [`MAX_EFFORT_SEGMENTS`]. Before an agent type is chosen only Default is offered.
    fn effort_row(
        &self,
        harness: Option<LaunchHarness>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let efforts = harness
            .map(|harness| self.displayed_efforts(harness))
            .unwrap_or_default();
        let chosen = self.draft.agent.effort;
        let (on_change, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let help = HoverHelp::tooltip("Effort: how hard the agent reasons");
        let control = if efforts.len() + 1 > MAX_EFFORT_SEGMENTS {
            let options = std::iter::once(SelectOption::new(
                "default",
                "Default",
                HoverHelp::tooltip(DEFAULT_EFFORT_HELP),
            ))
            .chain(efforts.iter().map(|effort| {
                SelectOption::new(
                    effort.as_cli_arg(),
                    effort_label(Some(*effort)),
                    HoverHelp::tooltip(format!("Use {} reasoning effort", effort.as_cli_arg())),
                )
            }));
            let offered = efforts.clone();
            Select::new("launcher-effort", "Effort", help, Role::ComboBox)
                .options(options)
                .chosen(chosen.map_or("default", LaunchEffort::as_cli_arg))
                .on_choose(move |value, _, cx| {
                    let effort = offered
                        .iter()
                        .copied()
                        .find(|effort| effort.as_cli_arg() == value.as_ref());
                    let _ = on_change.update(cx, |launcher, cx| launcher.choose_effort(effort, cx));
                })
                .on_enter(move |window, cx| {
                    let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                })
                .into_any_element()
        } else {
            let group = RadioGroup::new("launcher-effort", "Effort", help, Role::Group).option(
                RadioOption::new("Default", HoverHelp::tooltip(DEFAULT_EFFORT_HELP), None),
            );
            efforts
                .iter()
                .fold(group, |group, effort| {
                    group.option(RadioOption::new(
                        effort_label(Some(*effort)),
                        HoverHelp::tooltip(format!("Use {} reasoning effort", effort.as_cli_arg())),
                        Some(*effort),
                    ))
                })
                .compact_segments()
                .chosen(Some(chosen))
                .on_change(move |effort, _, cx| {
                    let _ =
                        on_change.update(cx, |launcher, cx| launcher.choose_effort(*effort, cx));
                })
                .on_enter(move |window, cx| {
                    let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                })
                .into_any_element()
        };
        row("launcher-effort-row", "Effort", control, palette).into_any_element()
    }

    /// Permissions (`.launch-composer-permissions-choice`): the modes the agent type offers, in the old order, and the
    /// help button.
    ///
    /// Which segment shows as chosen is the old rule: the stored choice, or for no stored choice the agent's own
    /// default mode (Guarded before any agent type, since New's default asks for it; YOLO for an agent whose omission
    /// is YOLO), so an omission never shows as a choice the agent would not make.
    fn permission_row(
        &self,
        harness: Option<LaunchHarness>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let permissions = self.draft.agent.permissions;
        let omitted = harness.and_then(LaunchHarness::omitted_permission);
        let offers =
            |permission| harness.is_some_and(|harness| harness.offers_permission(permission));
        let mut group = RadioGroup::new(
            "launcher-permissions",
            "Permission mode",
            HoverHelp::tooltip("Permissions: what the agent may do without asking you"),
            Role::Group,
        );
        if harness.is_none_or(|harness| harness.offers_permission(LaunchPermission::Guarded)) {
            group = group.option(RadioOption::new(
                permission_display(Some(LaunchPermission::Guarded)),
                HoverHelp::tooltip(guarded_permission_help(harness)),
                Some(LaunchPermission::Guarded),
            ));
        }
        if harness.is_none_or(|harness| harness.offers_permission(LaunchPermission::Yolo)) {
            group = group.option(
                RadioOption::new(
                    permission_display(Some(LaunchPermission::Yolo)),
                    HoverHelp::tooltip(permission_tooltip(LaunchPermission::Yolo)),
                    Some(LaunchPermission::Yolo),
                )
                .danger(),
            );
        }
        if offers(LaunchPermission::Custom) {
            group = group.option(RadioOption::new(
                permission_display(Some(LaunchPermission::Custom)),
                HoverHelp::tooltip(permission_tooltip(LaunchPermission::Custom)),
                Some(LaunchPermission::Custom),
            ));
        }
        for permission in APPROVAL_MODES
            .into_iter()
            .filter(|permission| offers(*permission))
        {
            group = group.option(RadioOption::new(
                permission_display(Some(permission)),
                HoverHelp::tooltip(permission_tooltip(permission)),
                Some(permission),
            ));
        }
        let shown = match permissions {
            Some(permission) => Some(permission),
            None if harness.is_none() => Some(LaunchPermission::Guarded),
            None => omitted,
        };
        let (on_change, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let group = group
            .compact_segments()
            .chosen(Some(shown))
            .on_change(move |permission, window, cx| {
                let _ = on_change.update(cx, |launcher, cx| {
                    launcher.choose_permissions(*permission, window, cx)
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.permission_enter(window, cx));
            });
        let controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            // The escalation banner's thick red outline goes around the permission choices themselves.
            .child(self.with_escalation_outline(group, palette, cx))
            .children(harness.map(|harness| {
                Button::new(
                    "launcher-permission-help",
                    "About permission modes",
                    HoverHelp::tooltip(permission_help(
                        harness,
                        harness.request_permission(permissions),
                    )),
                    Role::Button,
                )
                .content("?")
                .tier(Tier::Neutral)
                .square(MODEL_ROW_CONTROL_HEIGHT)
                .on_click(|_, _, cx| cx.open_url(PERMISSION_DOCS))
            }));
        row("launcher-permissions-row", "Permissions", controls, palette).into_any_element()
    }

    /// The Custom flags editor (old `CustomPermissionFlags`): the flags as typed, under their label, in the shared
    /// Custom flags block ([`CustomFlags`]), which says what they mean and draws the words they parse to (each its own
    /// escaped box, so a hidden character shows), or why they do not parse, lined up under the field.
    pub(super) fn flags_editor(
        &self,
        harness: Option<LaunchHarness>,
        palette: &theme::Palette,
    ) -> AnyElement {
        let agent = harness.map_or("the agent", harness_label);
        let field = row(
            "launcher-flags-row",
            "Permission flags",
            self.launcher_input(
                "launcher-flags",
                &self.flags,
                "Permission flags",
                HoverHelp::tooltip(permission_tooltip(LaunchPermission::Custom)),
            )
            .monospace(),
            palette,
        );
        CustomFlags::new(
            "launcher",
            field,
            self.draft.agent.permission_flags.clone(),
            agent,
        )
        .into_any_element()
    }
}

/// A line of warning-colored words below the affected control, announced politely as a
/// status (the old line was no live region, so a choice's side effect went unannounced). Host words use a `Line`
/// that preserves direction isolation on screen and supplies the escaped name to assistive technology.
pub(super) fn status_line(
    id: &'static str,
    words: impl Into<Line>,
    palette: &theme::Palette,
) -> Stateful<Div> {
    let words = words.into();
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(words.accessible_text())
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.warn))
        .child(words)
}

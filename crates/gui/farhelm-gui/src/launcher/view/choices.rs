//! The Agent tab's choices: the harness chips, effort, permissions with their help and the Custom flags editor, and
//! the line that says what an agent-choice edit changed or refused.
//!
//! Ported from the old app's `launch_controls.rs` (`LaunchControls`, `CustomPermissionFlags`) and the harness row in
//! `list/create_form.rs`, at reference commit `24bc0580`. Every choice is one of the draft's edit methods, so a hand
//! choice has exactly the effects SPEC.md gives it (reconciling an agent switch, clearing an effort a model does not
//! offer, dropping Custom flags on leaving Custom); this module only draws the choices the draft and `farhelm_proto`
//! allow, in the old controls' shapes:
//!
//! - **Harness**: chips in the old picker order, a pressed-button row with no group around it, as the old `div` was.
//! - **Effort**: a segmented button group of Default and the offered efforts, or a select once that is more than five
//!   (the old switch to a native `<select>`).
//! - **Permissions**: a segmented button group of what the agent type offers, YOLO in danger ink, then a "?" whose
//!   hover explains the mode in force and whose press opens the docs on permission modes.
//! - **Custom flags**: under the permissions while Custom is chosen, with the words they parse to, or why they do not.
//!
//! Enter on any of them applies it and launches (SPEC.md: "Enter on a choice button ... applies that choice and then
//! launches"), except Custom, whose Enter only chooses, since a Custom choice with no flags yet cannot launch.
//!
//! NOTE: Not built yet: the model field (with its catalog list), and Restart with's "Changed" markers beside each
//! choice. An agent switch's escalation (the red banner, with Undo) is computed by the draft and not yet shown.

use farhelm_proto::launcher::{EFFORT_ORDER, compatible_efforts};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::changed::Changed;
use super::{LABEL_COLUMN, LABEL_GAP, Launcher, row};
use crate::components::{
    Button, CustomFlags, HoverHelp, RadioGroup, RadioOption, Role, Select, SelectOption, Tier,
};
use crate::focus::{self, Mark};
use crate::launcher::draft::{Edit, Field, Mode, RestartBaseline, harness_label};
use crate::launcher::words::{HARNESS_PICKER_ORDER, PERMISSION_DOCS, guarded_permission_help};
use crate::permission::words::{
    effort_label, permission_display, permission_help, permission_tooltip,
};
use crate::theme::{self, metrics};

/// Above this many options (Default included) effort is a select rather than segments, as the old launcher switched
/// to a native `<select>` past five.
const MAX_EFFORT_SEGMENTS: usize = 5;

/// The approval modes, after Guarded and YOLO and Custom, in the old fixed order; each shows for whichever agent type
/// offers it (Goose all three, OMP Approve alone).
const APPROVAL_MODES: [LaunchPermission; 3] = [
    LaunchPermission::Approve,
    LaunchPermission::SmartApprove,
    LaunchPermission::Chat,
];

impl Launcher {
    /// The agent-choice edit `edit` happened: its notice replaces the one shown (none clears it), and the draft
    /// changed. The escalation it may carry is the banner's, which comes later.
    pub(super) fn chose(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
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
    }

    pub(super) fn choose_effort(&mut self, effort: Option<LaunchEffort>, cx: &mut Context<Self>) {
        self.draft.choose_effort(effort);
        self.notice = None;
        self.changed(cx);
    }

    /// A permission chosen by hand (`None` is the agent's own setting). Leaving Custom empties its flags field with
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
                RestartBaseline::Agent(selection) => selection.effort,
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

    /// The Agent tab's choice rows, in the old order: harness (not in Restart with, which holds it), effort and
    /// permissions. The Custom flags and the last choice's notice sit further down the tab
    /// ([`Self::flags_editor`], [`status_line`]).
    pub(super) fn choice_rows(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if !self.draft.holds(Field::AgentType) {
            rows.push(self.harness_row(palette, cx));
        }
        let harness = self.draft.agent.harness;
        if harness.is_none_or(LaunchHarness::offers_effort) {
            rows.push(self.effort_row(harness, palette, cx));
            rows.extend(self.changed_marker(Changed::Effort, palette));
        }
        rows.push(self.permission_row(harness, palette, cx));
        rows.extend(self.changed_marker(Changed::Permissions, palette));
        rows
    }

    /// The harness chips (`.launch-composer-harness-choice`): every agent type, the chosen one pressed.
    fn harness_row(&self, palette: &theme::Palette, cx: &mut Context<Self>) -> AnyElement {
        let (on_change, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let chips = HARNESS_PICKER_ORDER.iter().fold(
            RadioGroup::new(
                "launcher-harness",
                "Harness",
                HoverHelp::tooltip("Harness: the agent this session starts"),
                Role::GenericContainer,
            )
            .chips(),
            |chips, &harness| {
                let label = harness_label(harness);
                chips.option(RadioOption::new(
                    label,
                    HoverHelp::tooltip(format!("Use {label} for the session")),
                    harness,
                ))
            },
        );
        let chips = chips
            .chosen(self.draft.agent.harness)
            .on_change(move |harness, window, cx| {
                let _ = on_change.update(cx, |launcher, cx| {
                    launcher.choose_harness(*harness, window, cx)
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            });
        row("launcher-harness-row", "Harness", chips, palette).into_any_element()
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
                HoverHelp::tooltip("Use the agent's default reasoning effort"),
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
                RadioOption::new(
                    "Default",
                    HoverHelp::tooltip("Use the agent's default reasoning effort"),
                    None,
                ),
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
        if harness.is_some_and(|harness| harness.omitted_permission().is_none()) {
            group = group.option(RadioOption::new(
                "Agent's own setting",
                HoverHelp::tooltip(
                    "Agent's own setting: Farhelm cannot force or classify this permission mode",
                ),
                None,
            ));
        }
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
            .child(group)
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
                .on_click(|_, _, cx| cx.open_url(PERMISSION_DOCS))
            }));
        row("launcher-permissions-row", "Permissions", controls, palette).into_any_element()
    }

    /// The Custom flags editor (old `CustomPermissionFlags`): the flags as typed, beside their label, in the shared
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
            super::launcher_input(
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
        .indent(px(LABEL_COLUMN + LABEL_GAP))
        .into_any_element()
    }
}

/// A line of warning-colored words in the control column (`.launch-composer-choice-error`), announced politely as a
/// status (the old line was no live region, so a choice's side effect went unannounced), and named by its words since
/// plain text is not in the accessibility tree.
pub(super) fn status_line(
    id: &'static str,
    words: String,
    palette: &theme::Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Status)
        .aria_label(SharedString::from(words.clone()))
        .pl(px(LABEL_COLUMN + LABEL_GAP))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.warn))
        .child(words)
}

//! The Agent tab's model field: a text field with a list floating under it, ported from the old launcher's model
//! combobox (`launch_controls.rs`, `LaunchControls`, and its handlers in `list/create_form.rs`, at reference commit
//! `24bc0580`).
//!
//! How it behaves, as the old one did:
//!
//! - **Closed**, the field shows the chosen model, or "Harness default", with a hint beside it saying how many catalog
//!   models the agent type has. Field and list are at most 440 pixels wide.
//! - **Focusing it** empties it for typing and opens the list under it: the default, the agent type's catalog models
//!   (or every type's, after the "Show every harness's models" row), filtered by what is typed. The list is the layer
//!   manager's non-focusing popover, so focus and every key stay in the field; Up and Down move the highlighted row
//!   (the field hands them over instead of moving its caret), which the list scrolls into view, at most 260 pixels
//!   tall; a click on a row picks it.
//! - **Enter** applies the highlighted row, or else what was typed ([`super::super::model::model_enter_target`]), and
//!   closes the list; a typed id another agent type owns, or one with no agent type to own it, is said under the field
//!   instead, until the next completed choice. In every mode, Restart with included, Enter only chooses, with the
//!   list open or closed: a typed model is never launched by the key that entered it, and Launch (or Restart) is a
//!   separate press (the old launcher's "choosing a model must not submit the launcher"; SPEC.md gives the model field
//!   its own Enter rule). A held Enter (an auto-repeat) applies nothing, as the old field's did. Restart with holds the
//!   agent type, so its list offers no other agent's models.
//! - **Leaving the field**, Escape or a press outside closes the list and puts the chosen model back in the field.
//!
//! The rows are drawn from the launcher's own state by the launcher's shared floating-list view (`field_list`); the
//! launcher keeps the highlighted row and the show-all switch, which no other surface shares, and what is typed is the
//! field's own text.

use farhelm_proto::LaunchHarness;
use gpui::{
    AnyElement, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div,
    px,
};

use super::changed::Changed;
use super::field_list::{Drawn, FieldList};
use super::{Launcher, choices::status_line, row};
use crate::components::{HoverHelp, OptionRow, Role, Vertical};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::{Field, LaunchDraft, harness_label};
use crate::launcher::model::{
    ModelEnterTarget, ModelOption, NEEDS_HARNESS, model_enter_target, model_options,
    owned_elsewhere_message,
};
use crate::theme::{self, metrics};

/// `.launch-composer-listbox`: `width: min(100%, 440px)`; the field is held to the same width so the list, as wide as
/// its anchor, matches it.
const MAX_WIDTH: f32 = 440.;
/// What the model field and its first row say for the agent type's own configured model.
pub(super) const HARNESS_DEFAULT: &str = "Harness default";
/// The list's element id, which tests find it by.
const MODEL_LIST: &str = "launcher-model-list";

/// What the model field holds besides the draft: the open list and what the user is doing in it. None of it is shared
/// with any other surface. Focusing the field starts the highlight and the error over; the show-all switch stays as
/// the user left it while the launcher is open.
#[derive(Default)]
pub(super) struct ModelField {
    /// The list under the field, with the row moved to with the arrows.
    list: FieldList,
    /// Whether the list shows every agent type's models.
    show_all: bool,
    /// Why the last Enter changed nothing ([`NEEDS_HARNESS`], [`owned_elsewhere_message`]); cleared by the next
    /// completed choice, by typing, by Escape, and by focusing the field again, and kept when focus leaves it (by Tab,
    /// or a press outside), so the user can follow it to the agent it names.
    error: Option<String>,
}

/// What the closed field shows: the chosen model, spelled out while it holds an invisible character (`shown_text`; the old
/// field showed a seeded model escaped too), or the agent type's default. The field empties itself when focused for
/// typing, so the escaped spelling never becomes a typed model.
pub(super) fn model_display(draft: &LaunchDraft) -> String {
    match &draft.agent.model {
        Some(model) => super::shown_text::display(model),
        None => HARNESS_DEFAULT.to_owned(),
    }
}

impl Launcher {
    /// Whether the model list is open: opened by the field and not since closed by the layer manager.
    pub(super) fn model_list_open(&self, window: &Window, cx: &App) -> bool {
        self.model_field.list.is_open(window, cx)
    }

    /// The list's rows for what is typed now. Restart with holds the agent type, so there the list never offers other
    /// agents' models, and has no show-all row (the old launcher's fixed-harness list).
    fn model_rows(&self, cx: &App) -> Vec<ModelOption> {
        let mut rows = model_options(
            self.catalog.as_deref().unwrap_or_default(),
            self.draft.agent.harness,
            &self.model_input.text(cx),
            self.model_field.show_all,
        );
        if self.draft.holds(Field::AgentType) {
            rows.retain(|row| *row != ModelOption::ShowAll);
        }
        rows
    }

    /// Whether the field holds a model not yet applied: typed into its open list, or said under it as not applicable.
    pub(super) fn model_pending(&self, cx: &App) -> bool {
        self.model_field.error.is_some()
            || self.model_field.list.opened() && !self.model_input.text(cx).is_empty()
    }

    /// The field took focus: empty it for typing and open the list.
    pub(super) fn model_focused(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_field.error = None;
        self.model_field.list.highlighted = None;
        self.model_input.set_text("", window, cx);
        self.open_model_list(window, cx);
        cx.notify();
    }

    /// The field lost focus: close the list and show the chosen model again. An error from the last Enter stays.
    pub(super) fn model_blurred(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_model_list(window, cx);
        cx.notify();
    }

    /// The user typed in the field.
    pub(super) fn model_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_field.error = None;
        self.model_field.list.highlighted = None;
        if !self.model_list_open(window, cx) {
            self.open_model_list(window, cx);
        }
        cx.notify();
    }

    /// Up or Down in the field: move the highlighted row, wrapping, while the list is open and has rows.
    pub(super) fn model_arrow(
        &mut self,
        arrow: Vertical,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.model_rows(cx).len();
        if self.model_list_open(window, cx) && self.model_field.list.arrow(arrow, count) {
            cx.notify();
        }
    }

    /// Enter in the field: apply what the open list's highlighted row or the typed text means. It never presses the
    /// main action, in any mode (the module docs).
    pub(super) fn model_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_list_open(window, cx) {
            let typed = self.model_input.text(cx);
            let target = model_enter_target(
                &self.model_rows(cx),
                self.model_field.list.highlighted,
                &typed,
                self.catalog.as_deref().unwrap_or_default(),
                self.draft.agent.harness,
            );
            match target {
                ModelEnterTarget::Nothing => self.close_model_list(window, cx),
                ModelEnterTarget::Option(option) => self.pick_model(option, window, cx),
                ModelEnterTarget::Custom { id, .. } => {
                    let edit = self.draft.choose_custom_model(&id, self.catalog.as_deref());
                    self.model_field.error = None;
                    self.close_model_list(window, cx);
                    self.chose(edit, window, cx);
                }
                ModelEnterTarget::NeedsHarness(_) => {
                    self.model_field.error = Some(NEEDS_HARNESS.to_owned());
                }
                ModelEnterTarget::OwnedElsewhere { id, owners } => {
                    self.model_field.error = Some(owned_elsewhere_message(&id, &owners));
                }
            }
        }
        cx.notify();
    }

    /// A row picked by a click or by Enter. Show-all only switches the list; every other row is a completed choice,
    /// which closes it and clears the last error.
    fn pick_model(&mut self, option: ModelOption, window: &mut Window, cx: &mut Context<Self>) {
        match option {
            ModelOption::ShowAll => {
                self.model_field.show_all = !self.model_field.show_all;
                self.model_field.list.highlighted = None;
            }
            ModelOption::HarnessDefault => {
                self.draft.choose_default_model();
                self.notice = None;
                self.model_field.error = None;
                self.close_model_list(window, cx);
                self.changed(cx);
            }
            ModelOption::Model { id, harness } => {
                let wrapper = self.remembered_wrapper(harness, cx);
                let edit = self.draft.choose_catalog_model(
                    harness,
                    &id,
                    self.catalog.as_deref(),
                    wrapper.as_deref(),
                );
                self.model_field.error = None;
                self.close_model_list(window, cx);
                self.chose(edit, window, cx);
            }
        }
        cx.notify();
    }

    /// Open the list under the field, as a popover that never takes focus.
    fn open_model_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_field
            .list
            .open(MODEL_LIST, draw_model_rows, window, cx);
    }

    /// Close the list if it is open, drop the highlight, and show the chosen model in the field.
    fn close_model_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_field.list.close(window, cx);
        let shown = model_display(&self.draft);
        self.model_input.set_text(shown, window, cx);
    }

    /// The model row: the field, its hint, and why the last Enter changed nothing. Not drawn for an agent type that
    /// offers no model choice (SPEC.md: such a composer hides the model).
    ///
    /// It also notices a list the layer manager closed on its own (Escape, a press outside), which tells the field
    /// nothing ([`FieldList::follow`]): the field then shows the chosen model again, and drops the last error if it
    /// still has focus (Escape, as the old Escape did; a press outside moves focus away, which keeps the error).
    pub(super) fn model_row(
        &mut self,
        palette: &theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Vec<AnyElement>> {
        let harness = self.draft.agent.harness;
        if harness.is_some_and(|harness| !harness.offers_model()) {
            return None;
        }
        if self.model_field.list.follow(window, cx) {
            if self.model_input.focus_handle(cx).is_focused(window) {
                self.model_field.error = None;
            }
            self.close_model_list(window, cx);
        }
        let tracker = self
            .model_field
            .list
            .track_anchor("launcher-model-anchor", window, cx);
        let hint = match harness {
            Some(harness) => {
                let count = self
                    .catalog
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter(|model| model.latest && model.harness == harness)
                    .count();
                format!("{count} for {}", harness_label(harness))
            }
            None => "Choose a harness".to_owned(),
        };
        let weak = cx.weak_entity();
        let field = self
            .launcher_input(
                "launcher-model",
                &self.model_input,
                "Model",
                HoverHelp::tooltip(
                    "Model: the agent's model; type to filter, or a custom model id and Enter",
                ),
            )
            .monospace()
            .on_vertical(move |arrow, window, cx| {
                let _ = weak.update(cx, |launcher, cx| launcher.model_arrow(arrow, window, cx));
            });
        let control = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(MAX_WIDTH))
                    .child(field)
                    .child(tracker),
            )
            .child(
                div()
                    .id("launcher-model-hint")
                    .role(Role::Label)
                    .aria_label(SharedString::from(hint.clone()))
                    .flex_none()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child(hint),
            );
        let mut rows =
            vec![row("launcher-model-row", "Model", control, palette).into_any_element()];
        rows.extend(self.changed_marker(Changed::Model, palette));
        if let Some(error) = &self.model_field.error {
            rows.push(
                status_line("launcher-model-error", error.clone(), palette).into_any_element(),
            );
        }
        Some(rows)
    }
}

/// The open model list's rows (`field_list`): the default, the catalog's models and the show-all switch below a rule,
/// with the highlighted row selected.
fn draw_model_rows(
    launcher: &Launcher,
    list: Stateful<Div>,
    weak: &WeakEntity<Launcher>,
    _: &Window,
    cx: &App,
) -> Drawn {
    let palette = theme::palette(cx);
    let list = list.role(Role::ListBox).aria_label("Model");
    let options = launcher.model_rows(cx);
    let active = launcher.model_field.list.highlighted;
    let chosen = launcher.draft.agent.harness;
    let show_all = launcher.model_field.show_all;
    let mut children: Vec<AnyElement> = Vec::with_capacity(options.len() + 1);
    // The child index of the highlighted row, for scrolling it into view; the separator before show-all shifts it.
    let mut active_child = None;
    for (index, option) in options.into_iter().enumerate() {
        if option == ModelOption::ShowAll {
            // The rule between the models and the switch (`.launch-composer-listbox` draws one above it).
            children.push(
                div()
                    .h(px(1.))
                    .flex_none()
                    .bg(theme::color(palette.border_row))
                    .into_any_element(),
            );
        }
        if active == Some(index) {
            active_child = Some(children.len());
        }
        let id = match &option {
            ModelOption::HarnessDefault => "launcher-model-default".to_owned(),
            ModelOption::Model { id, harness } => {
                format!("launcher-model-option-{}-{id}", harness_label(*harness))
            }
            ModelOption::ShowAll => "launcher-model-show-all".to_owned(),
        };
        let words = row_words(&id, &option, show_all, chosen);
        let label = words.accessible_name();
        let help = match option {
            ModelOption::ShowAll => "Switch which agents' models the list shows",
            ModelOption::HarnessDefault | ModelOption::Model { .. } => {
                "Use this model for the session"
            }
        };
        let weak = weak.clone();
        children.push(
            OptionRow::new(id, label, HoverHelp::tooltip(help), Role::ListBoxOption)
                .selected(active == Some(index))
                .child(words)
                .on_click(move |_, window, cx| {
                    let option = option.clone();
                    let _ = weak.update(cx, |launcher, cx| launcher.pick_model(option, window, cx));
                })
                .into_any_element(),
        );
    }
    Drawn {
        list,
        children,
        highlighted: active_child,
    }
}

/// Row `row`'s words, drawn and named the same way: the default, a model id (host text, escaped) with its agent's name
/// when the list shows more than one agent's models, or the show-all switch saying what it would do.
fn row_words(
    row: &str,
    option: &ModelOption,
    show_all: bool,
    chosen: Option<LaunchHarness>,
) -> HostSentence {
    // Each row's words are a labelled node of their own, so each needs its own id.
    let words = HostSentence::new(SharedString::from(format!("{row}-words")));
    match option {
        ModelOption::HarnessDefault => words.text(HARNESS_DEFAULT),
        ModelOption::Model { id, harness } => {
            let words = words.host(&HostText::from_host(id.as_str()));
            if show_all || chosen.is_none() {
                words.text(format!(" ({})", harness_label(*harness)))
            } else {
                words
            }
        }
        ModelOption::ShowAll if show_all => words.text("Show chosen harness's models"),
        ModelOption::ShowAll => words.text("Show every harness's models"),
    }
}

//! The launcher's inline Save as template panel: a template name, the checklist of what the launcher offers to keep,
//! and Save and Cancel.
//!
//! SPEC.md, "Launch templates": "New, Clone and Restart with also offer "Save as template": an inline name field and
//! checklist"; "A taken template name is refused without overwriting. Enter in the name field saves without launching;
//! Escape returns to the launcher. Successful saving closes the launcher and opens the new template in the Templates
//! editor; errors preserve the launcher draft." Ported from the old app's `SaveTemplatePanel`
//! (`list/save_template.rs` at reference commit `24bc0580`) and its `.save-template-*` rules in `assets/app.css`.
//!
//! Here's the TLDR of its rules:
//!
//! - **What it shows.** The [`Candidate`] fixed when the panel opened: each offered field as a checkbox with its value,
//!   which is host-supplied text and so drawn through the host-text element; the session name's Base name | Fixed name
//!   choice with its help; and a line when the launcher's host has no usable install identity, which is then not
//!   offered.
//! - **Saving.** The name is checked first (the new-name rules, then the helm's shape check), then the checked fields
//!   by the editor's rules, then against the names the store holds, which already holds every template this window
//!   saved (the helm pushes a command's changes before its reply). A refusal is said in the panel. The save is
//!   `PUT /api/templates/{name}`, work under the template's key in the operation store, like the editor's save. While it
//!   is in flight the panel holds still (its name field read-only, its controls busy), and so do the launcher's Cancel,
//!   Close and Escape. The helm's refusal is said in the panel, the draft stays, and nothing is retried.
//! - **What the owner does.** The launcher draws the panel at the end of its scrolling body and hears two things: the
//!   panel is done without saving (Cancel), and a template was saved (closing itself and opening Templates on it). The
//!   launcher's dismissal hook closes the panel on Escape anywhere in the launcher, as the old panel took the
//!   launcher's Escape first.

use std::rc::Rc;

use farhelm_proto::launcher::{
    LaunchCatalogModel, LaunchTemplate, TemplateDestination, TemplateNameKind,
    check_new_template_name, check_template_shape,
};
use gpui::{
    AnyElement, AnyWindowHandle, App, Context, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use super::editor::{NAME_KIND_HELP, save_command, stored_names};
use super::fields::{self, Field, capitalized, taken_refusal};
use super::save::{Candidate, choice_text};
use crate::components::{
    Button, Checkbox, HoverHelp, RadioGroup, RadioOption, Role, TextInput, TextInputEvent,
    TextInputState, Tier,
};
use crate::host_text::{HostSentence, HostText};
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session;
use crate::theme::{self, metrics};

/// `.save-template-panel`: 8 pixels between its blocks and 12 around them.
const GAP: f32 = 8.;
const PAD: f32 = 12.;
/// `.templates-hint` and the panel's small lines: 12-pixel text at a 1.5 line height.
const SMALL_LINE_HEIGHT: f32 = 18.;

/// What the panel calls when the user is done with it without saving (its Cancel).
pub(crate) type OnCancel = Rc<dyn Fn(&mut Window, &mut App)>;

/// What the panel calls once the helm stored the template named by the string, with whether the user moved on while it
/// was saved (the operation store's [`crate::operations::Settled::user_moved_on`]).
pub(crate) type OnSaved = Rc<dyn Fn(String, bool, &mut Window, &mut App)>;

/// The open panel. See the module docs.
pub(crate) struct SavePanel {
    candidate: Candidate,
    /// The launcher's label for its host when the panel opened, shown as the Host field's value: the template stores the
    /// install identity, but the label is what the user recognizes.
    host_label: Option<HostText>,
    /// The launcher's model catalog when the panel opened, which judges a checked effort as the editor does.
    catalog: Option<Vec<LaunchCatalogModel>>,
    /// The template name field.
    name: TextInputState,
    /// The offered fields the user keeps checked, starting as the candidate's.
    checked: Vec<Field>,
    /// The session name's kind as chosen here: `None` is a base name.
    name_kind: Option<TemplateNameKind>,
    /// Why the last Save did not save, in the app's words or the helm's.
    error: Option<HostSentence>,
    /// Whether the save is in flight.
    saving: bool,
    /// The window the panel is drawn in, for the save's completion, which runs outside any window's update.
    window: AnyWindowHandle,
    on_cancel: OnCancel,
    on_saved: OnSaved,
    _name_events: Subscription,
}

impl SavePanel {
    /// A panel offering `candidate`, as the launcher's draft was when Save as template was pressed.
    pub(crate) fn new(
        candidate: Candidate,
        host_label: Option<HostText>,
        catalog: Option<Vec<LaunchCatalogModel>>,
        on_cancel: OnCancel,
        on_saved: OnSaved,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SavePanel {
        let name = TextInputState::new(window, cx);
        // Enter in the name field saves, and never reaches the launcher's Launch (SPEC.md: "Enter in the name field saves
        // without launching").
        let name_events = name.subscribe(window, cx, |panel: &mut Self, event, window, cx| {
            if let TextInputEvent::Submitted { .. } = event {
                panel.save(window, cx);
            }
        });
        SavePanel {
            checked: candidate.checked.clone(),
            name_kind: candidate.fields.name_kind,
            candidate,
            host_label,
            catalog,
            name,
            error: None,
            saving: false,
            window: window.window_handle(),
            on_cancel,
            on_saved,
            _name_events: name_events,
        }
    }

    /// The name field's focus handle, where focus goes when the panel opens.
    pub(crate) fn name_focus(&self, cx: &App) -> gpui::FocusHandle {
        self.name.focus_handle(cx)
    }

    /// Whether the save is in flight, when the launcher holds still.
    pub(crate) fn saving(&self) -> bool {
        self.saving
    }

    /// The template the panel would save now: its name as typed, the checked fields with the chosen name kind.
    fn template(&self, cx: &App) -> LaunchTemplate {
        LaunchTemplate {
            name: self.name.text(cx).to_string(),
            fields: self.candidate.selected(&self.checked, self.name_kind),
        }
    }

    /// Why the template cannot be saved, in the old panel's order: the new-name rules (Save as template always checks
    /// them, SPEC_impl.md), the helm's shape check, the checked fields by the editor's rules, then a name the store
    /// already holds.
    fn refusal(&self, template: &LaunchTemplate, cx: &App) -> Option<HostSentence> {
        let plain = |reason: String| HostSentence::new("save-template-error-text").text(reason);
        if let Err(reason) =
            check_new_template_name(&template.name).and_then(|()| check_template_shape(template))
        {
            return Some(plain(capitalized(reason)));
        }
        let draft = fields::Draft {
            name: template.name.clone(),
            fields: template.fields.clone(),
            wrapper_input: None,
        };
        if let Some(reason) = draft.refusal(self.catalog.as_deref()) {
            return Some(plain(reason));
        }
        let names = stored_names(cx);
        let names: Option<Vec<&str>> = names
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect());
        taken_refusal(&template.name, names.as_deref())
    }

    /// Save: check the template and write it as work under its key. Nothing while a save is in flight.
    fn save(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let template = self.template(cx);
        let command = match self.refusal(&template, cx) {
            Some(reason) => Err(reason),
            None => save_command(&template).ok_or_else(|| {
                HostSentence::new("save-template-error-text")
                    .text("This template cannot be sent to Farhelm as it stands")
            }),
        };
        let command = match command {
            Ok(command) => command,
            Err(reason) => {
                self.error = Some(reason);
                cx.notify();
                return;
            }
        };
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let panel = cx.weak_entity();
        let window = self.window;
        let name = template.name.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(
                &OpKey::template(&template.name),
                command,
                cx,
                move |settled, cx| {
                    let outcome = settled.outcome.clone();
                    let moved_on = settled.user_moved_on;
                    // A completion runs from the store's own task, outside any window's update. The owner hears of the
                    // save only after the panel's own update has ended, since it drops the panel with the launcher.
                    let _ = window.update(cx, |_, window, cx| {
                        let saved = panel
                            .update(cx, |panel, cx| panel.settled(outcome, cx))
                            .ok()
                            .flatten();
                        if let Some(on_saved) = saved {
                            on_saved(name, moved_on, window, cx);
                        }
                    });
                },
            )
        });
        match started {
            Ok(()) => {
                self.saving = true;
                self.error = None;
                self.name.set_read_only(true, cx);
            }
            // The template's key already has a write in flight (an editor's save of the same name, say): the store's
            // shared reason after this panel's lead-in, and nothing is queued.
            Err(NotStarted::Busy(_)) => {
                self.error = Some(
                    HostSentence::new("save-template-error-text")
                        .text(format!("Not saved: {}", NotStarted::BUSY_REASON)),
                );
            }
            // A retired store has closed the launcher with it, and a template name that cannot be a path segment was
            // refused by the shape check above.
            Err(not_started) => tracing::debug!(?not_started, "template save not started"),
        }
        cx.notify();
    }

    /// The save settled: the owner's success handler to call, or the helm's words in the panel, the launcher's draft
    /// kept and nothing retried (SPEC.md, "One GUI at a time").
    fn settled(&mut self, outcome: Outcome, cx: &mut Context<Self>) -> Option<OnSaved> {
        self.saving = false;
        self.name.set_read_only(false, cx);
        cx.notify();
        match outcome {
            // The template route answers at once and never accepts a write as long work; both mean done.
            Outcome::Done(_) | Outcome::Accepted(_) => Some(self.on_saved.clone()),
            Outcome::Refused(refusal) => {
                self.error =
                    Some(HostSentence::new("save-template-error-text").host(&refusal.words));
                None
            }
        }
    }

    /// Check or uncheck `field`.
    fn toggle(&mut self, field: Field, on: bool, cx: &mut Context<Self>) {
        self.checked.retain(|checked| *checked != field);
        if on {
            self.checked.push(field);
        }
        cx.notify();
    }

    /// One offered field (`.save-template-choice`): its checkbox, labelled with the field's name, beside its value; the
    /// session name adds its kind choice. The value is host-supplied text, drawn escaped, in the monospace face for a
    /// wrapper prefix, a command line, a resume command and a folder, as the old panel drew them.
    fn choice(&self, field: Field, cx: &mut Context<Self>) -> AnyElement {
        let palette = theme::palette(cx);
        let label = field.label();
        let weak = cx.weak_entity();
        let checkbox = Checkbox::new(
            SharedString::from(format!("save-template-field-{}", field.key())),
            format!("{label}:"),
            HoverHelp::tooltip(format!(
                "Keep this {} in the template",
                label.to_ascii_lowercase()
            )),
            Role::CheckBox,
        )
        .checked(self.checked.contains(&field))
        .disabled(self.saving)
        .on_toggle(move |on, _, cx| {
            let _ = weak.update(cx, |panel, cx| panel.toggle(field, on, cx));
        });
        let shown = match field {
            Field::Host => self.host_label.clone(),
            _ => choice_text(field, &self.candidate.fields).map(HostText::from_host),
        };
        let monospace = matches!(field, Field::Wrapper | Field::Command | Field::Resume)
            || (field == Field::Destination
                && matches!(
                    self.candidate.fields.destination,
                    Some(TemplateDestination::Folder(_))
                ));
        let value = shown.map(|text| {
            let value = div()
                .min_w_0()
                .text_color(theme::color(palette.fg_0))
                .child(
                    HostSentence::new(SharedString::from(format!(
                        "save-template-value-{}",
                        field.key()
                    )))
                    .host(&text),
                );
            if monospace {
                value.font_family(crate::assets::TERMINAL_FONT)
            } else {
                value
            }
        });
        let kind = (field == Field::Name).then(|| {
            let weak = cx.weak_entity();
            [
                (None, "Base name"),
                (Some(TemplateNameKind::Fixed), "Fixed name"),
            ]
            .into_iter()
            .fold(
                RadioGroup::new(
                    "save-template-name-kind",
                    "Session name kind",
                    HoverHelp::tooltip(format!("Session name kind. {NAME_KIND_HELP}")),
                    Role::Group,
                ),
                |group, (value, text)| {
                    group.option(RadioOption::new(
                        text,
                        HoverHelp::tooltip(format!("Session name: {text}. {NAME_KIND_HELP}")),
                        value,
                    ))
                },
            )
            .chosen(Some(self.name_kind))
            // The kind belongs to the name: it is chosen only while the name is kept.
            .disabled(self.saving || !self.checked.contains(&Field::Name))
            .on_change(move |kind, _, cx| {
                let kind = *kind;
                let _ = weak.update(cx, |panel, cx| {
                    panel.name_kind = kind;
                    cx.notify();
                });
            })
        });
        div()
            .debug_selector(move || format!("save-template-choice-{}", field.key()))
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(12.))
            .gap_y(px(6.))
            .min_w_0()
            .text_size(metrics::TEXT_MD)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .min_w_0()
                    .child(checkbox)
                    .children(value),
            )
            .children(kind.map(|kind| div().flex_none().child(kind)))
            .into_any_element()
    }
}

/// A quiet 12-pixel line in the panel (`.save-template-panel > p`), named by its words for assistive technology. A
/// label rather than a live status: its words are fixed while the panel is open, so there is nothing to announce.
fn small_line(id: &'static str, text: &'static str, palette: &theme::Palette) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Label)
        .aria_label(text)
        .text_size(metrics::TEXT_SM)
        .line_height(px(SMALL_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text)
}

impl Render for SavePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let offered: Vec<Field> = Field::ALL
            .into_iter()
            .filter(|field| field.is_set(&self.candidate.fields))
            .collect();
        let choices: Vec<AnyElement> = offered
            .iter()
            .flat_map(|&field| {
                let mut elements = vec![self.choice(field, cx)];
                if field == Field::Name {
                    elements.push(
                        small_line("save-template-name-kind-hint", NAME_KIND_HELP, palette)
                            .into_any_element(),
                    );
                }
                elements
            })
            .collect();
        let (save, cancel) = (cx.weak_entity(), cx.weak_entity());
        div()
            .id("save-template-panel")
            .debug_selector(|| "save-template-panel".into())
            .role(Role::Region)
            .aria_label("Save as template")
            .flex()
            .flex_col()
            .gap(px(GAP))
            .p(px(PAD))
            .min_w_0()
            .border_1()
            .border_color(theme::color(palette.border))
            .child(
                // The name field and the heading under it, the panel's top. Opening the panel focuses the name field,
                // which the focus coordinator scrolls into view as far as the field itself; the old WebKit `focus()`
                // centered it, showing part of the checklist too (recorded in the parity file).
                div()
                    .flex()
                    .flex_col()
                    .gap(px(GAP))
                    .child(
                        // `label` with its input: the caption over the field, 6 pixels apart.
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(metrics::TEXT_SM)
                                    .text_color(theme::color(palette.fg_1))
                                    .child("Template name"),
                            )
                            .child(
                                // Named for tests, which click the field by where it is drawn.
                                div().debug_selector(|| "save-template-name".into()).child(
                                    TextInput::new(
                                        "save-template-name",
                                        &self.name,
                                        "Template name",
                                        HoverHelp::tooltip(
                                            "Template name: choose a new name for this setup",
                                        ),
                                    ),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(metrics::TEXT_MD)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::color(palette.fg_0))
                            .child("Choose which parts of this setup the template keeps"),
                    ),
            )
            .children(choices)
            .children(self.candidate.fields.host.is_none().then(|| {
                small_line(
                    "save-template-host-unidentified",
                    "This host has not been identified, so it cannot be saved in a template",
                    palette,
                )
            }))
            .children(self.error.clone().map(|error| {
                div()
                    .id("save-template-error")
                    .debug_selector(|| "save-template-error".into())
                    .role(Role::Alert)
                    .aria_label(error.accessible_name())
                    .text_size(metrics::TEXT_SM)
                    .line_height(px(SMALL_LINE_HEIGHT))
                    .text_color(theme::color(palette.warn_bright))
                    .child(error)
            }))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        Button::new(
                            "save-template-save",
                            if self.saving { "Saving…" } else { "Save" },
                            HoverHelp::tooltip(
                                "Save: keep the checked choices and open the new template",
                            ),
                            Role::Button,
                        )
                        .tier(Tier::Primary)
                        .busy(self.saving)
                        .on_click(move |_, window, cx| {
                            let _ = save.update(cx, |panel, cx| panel.save(window, cx));
                        }),
                    )
                    .child(
                        Button::new(
                            "save-template-cancel",
                            "Cancel",
                            HoverHelp::tooltip(
                                "Cancel: return to the launcher without saving a template",
                            ),
                            Role::Button,
                        )
                        .tier(Tier::Neutral)
                        .busy(self.saving)
                        .on_click(move |_, window, cx| {
                            let on_cancel = cancel
                                .update(cx, |panel, _| {
                                    (!panel.saving).then(|| panel.on_cancel.clone())
                                })
                                .ok()
                                .flatten();
                            if let Some(on_cancel) = on_cancel {
                                on_cancel(window, cx);
                            }
                        }),
                    ),
            )
    }
}

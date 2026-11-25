//! Helm-wide custom agent registrations. The pushed catalog owns navigation, and each open editor owns its draft.

use farhelm_proto::custom_agents::CustomAgent;
use farhelm_proto::ui_channel::{ApiPath, Availability, Collection, Method};
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, InteractiveElement as _,
    IntoElement as _, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};

use super::{
    Section, SettingsDialog, card, choices,
    editing::{self, EditState, SaveReceipt},
};
use crate::components::{
    Button, Checkbox, HoverHelp, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::host_text::{HostSentence, HostText, Line};
use crate::operations::{Command, NotStarted, OpKey, Outcome, Work};
use crate::{session, store::HelmStore, theme};

pub(super) const COMMAND_HELP: &str = "Program name on PATH (e.g. my-agent), absolute path (/opt/bin/my-agent), or ~/path. Used as-is on every host and environment.";
const RESUME_HELP: &str = "Pass --resume <id> when starting a session with a conversation ID and when restarting a session.";
const REPORT_HELP: &str = "Expect this agent to call farhelm custom-agent report-conversation --id <id>. Paired with session resumption, this enables Restart.";
const CONTRACT_NOTE: &str = "Custom agents always count as YOLO (hosts that ask before YOLO launches will confirm). The custom-agent contract is minimal and has no stability guarantee yet — see the custom agents documentation for updates.";

impl Section {
    /// A key with no ID belongs to Add. Other settings pages have no editor in this collection.
    pub(super) fn custom_agent_key(&self) -> Option<Option<String>> {
        match self {
            Self::CustomAgent(id) => Some(Some(id.clone())),
            Self::AddCustomAgent => Some(None),
            _ => None,
        }
    }

    pub(super) fn nav_button_id(&self) -> SharedString {
        match self {
            Self::CustomAgent(id) => format!("settings-section-custom-{id}").into(),
            _ => self.button_id().into(),
        }
    }

    /// The stable ID remains usable when a pushed rename changes the ordinary Settings label.
    pub(super) fn display_label(&self, cx: &App) -> Line {
        let Self::CustomAgent(id) = self else {
            return self.label().into();
        };
        HostSentence::new(SharedString::from(format!("custom-agent-name-{id}")))
            .host(&registration_name(id, cx))
            .into()
    }

    /// The registered name stays isolated from the surrounding tooltip words, including after a pushed rename.
    pub(super) fn nav_help(&self, label: &str, cx: &App) -> HoverHelp {
        match self {
            Self::CustomAgent(id) => HoverHelp::sentence(
                HostSentence::new(SharedString::from(format!("custom-agent-nav-help-{id}")))
                    .text("Show ")
                    .host(&registration_name(id, cx))
                    .text(" settings"),
            ),
            _ => HoverHelp::tooltip(format!("Show {label} settings")),
        }
    }
}

/// A deleted registration's retained draft still has its stable ID to label it.
fn registration_name(id: &str, cx: &App) -> HostText {
    let store = session::current_store(cx);
    let name = store
        .as_ref()
        .and_then(|store| store.read(cx).collections().get::<CustomAgent>(id))
        .map_or(id, |agent| agent.display_name.as_str());
    HostText::from_host(name)
}

/// Say why the list may be incomplete even while the editor can show a registration from the last successful read.
pub(super) fn catalog_notice(cx: &App) -> Option<AnyElement> {
    let store = session::current_store(cx)?;
    let meta = store
        .read(cx)
        .collections()
        .meta(Collection::CustomAgents)?;
    let Availability::Unavailable { error } = &meta.availability else {
        return None;
    };
    let sentence = HostSentence::new("custom-agent-catalog-reason")
        .text("Could not load custom agents: ")
        .host(&HostText::from_host(error.as_str()));
    Some(
        choices::outcome_line(
            "custom-agent-catalog-error",
            Role::Alert,
            theme::color(theme::palette(cx).danger),
        )
        .aria_label(sentence.accessible_name())
        .child(sentence)
        .into_any_element(),
    )
}

impl SettingsDialog {
    /// A removed registration keeps a navigation row only while this dialog still holds a draft for it.
    pub(super) fn custom_sections(&self, cx: &App) -> Vec<Section> {
        let mut sections = session::current_store(cx).map_or_else(Vec::new, |store| {
            store
                .read(cx)
                .collections()
                .ordered::<CustomAgent>()
                .into_iter()
                .map(|agent| Section::CustomAgent(agent.id.clone()))
                .collect()
        });
        sections.extend(self.custom_agents.iter().filter_map(|(key, editor)| {
            key.as_ref()
                .filter(|_| editor.read(cx).removed)
                .cloned()
                .map(Section::CustomAgent)
        }));
        sections
    }

    pub(super) fn open_custom_editor(
        &mut self,
        key: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.custom_agents.contains_key(&key) {
            return;
        }
        let definition = key.as_ref().and_then(|id| {
            let store = session::current_store(cx)?;
            store.read(cx).collections().get::<CustomAgent>(id).cloned()
        });
        if key.is_some() && definition.is_none() {
            return;
        }
        let parent = cx.weak_entity();
        let editor = cx.new(|cx| Editor::new(parent, definition, window, cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        self.custom_agents.insert(key, editor);
    }

    pub(super) fn custom_editor(&self) -> Option<&Entity<Editor>> {
        self.custom_agents.get(&self.shown.custom_agent_key()?)
    }

    /// A deleted registration leaves the page, including when another app deleted it.
    /// An unavailable collection must not look like a deletion.
    pub(super) fn sync_custom_agents(
        &mut self,
        store: Option<&Entity<HelmStore>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = store else { return };
        let collections = store.read(cx).collections();
        if !collections
            .meta(Collection::CustomAgents)
            .is_some_and(|meta| matches!(meta.availability, Availability::Available))
        {
            return;
        }
        let selected = self.shown.custom_agent_key().flatten();
        let mut selected_deleted = false;
        let mut updates = Vec::new();
        self.custom_agents.retain(|key, editor| {
            let Some(id) = key else { return true };
            let draft = editor.read(cx);
            let Some(saved) = collections.get::<CustomAgent>(id) else {
                // A pushed delete can arrive before a PUT's reply. Keep the editor and the receipt it owns
                // while it has an outstanding or unsaved edit. An explicit Delete from this page discards both.
                let keep = !draft.deleting && (draft.saving || draft.dirty(cx));
                selected_deleted |= selected.as_ref() == Some(id) && (!keep || !draft.removed);
                if keep && !draft.removed {
                    updates.push((editor.clone(), None));
                }
                return keep;
            };
            if draft.removed || draft.stored.as_ref() != Some(saved) {
                updates.push((editor.clone(), Some(saved.clone())));
            }
            true
        });
        for (editor, saved) in updates {
            editor.update(cx, |editor, cx| {
                if let Some(saved) = saved {
                    editor.saw_saved(saved, window, cx);
                } else {
                    editor.removed = true;
                    editor.confirm_delete = false;
                    cx.notify();
                }
            });
        }
        if selected_deleted
            || selected.is_some_and(|id| !self.custom_agents.contains_key(&Some(id)))
        {
            self.show(Section::AddCustomAgent, window, cx);
        }
    }
}

/// The dirty state compares the draft against the last pushed definition. Edits can be left behind on another page
/// and are still included in the dialog's common Save or Discard question.
pub(super) struct Editor {
    pub(super) save_receipt: SaveReceipt,
    parent: WeakEntity<SettingsDialog>,
    stored: Option<CustomAgent>,
    /// The helm removed the registration while this editor still held an unsaved or in-flight edit.
    removed: bool,
    name: TextInputState,
    id: TextInputState,
    command: TextInputState,
    resumption: bool,
    reporting: bool,
    /// Set until the user changes Add's generated ID. Edits to the display name preserve a deliberately chosen ID.
    generated_id: Option<String>,
    saving: bool,
    deleting: bool,
    confirm_delete: bool,
    error: Option<HostText>,
}

impl Editor {
    fn new(
        parent: WeakEntity<SettingsDialog>,
        stored: Option<CustomAgent>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = TextInputState::new(window, cx);
        let id = TextInputState::new(window, cx);
        let command = TextInputState::new(window, cx);
        name.set_placeholder("Acme Code", window, cx);
        id.set_placeholder("acme-code", window, cx);
        command.set_placeholder("~/bin/acme-code", window, cx);
        let mut this = Self {
            save_receipt: SaveReceipt::default(),
            parent,
            name,
            id,
            command,
            generated_id: stored.is_none().then(String::new),
            stored,
            removed: false,
            resumption: false,
            reporting: false,
            saving: false,
            deleting: false,
            confirm_delete: false,
            error: None,
        };
        this.restore(window, cx);
        this.name
            .subscribe(window, cx, |editor, event, window, cx| {
                if matches!(event, TextInputEvent::Changed) {
                    if editor.generated_id.is_some() {
                        let generated = default_id(editor.name.text(cx).as_ref());
                        editor.generated_id = Some(generated.clone());
                        editor.id.set_text(generated, window, cx);
                    }
                    editor.edited(cx);
                }
            })
            .detach();
        this.id
            .subscribe(window, cx, |editor, event, _, cx| {
                if matches!(event, TextInputEvent::Changed) {
                    if editor.generated_id.as_deref() != Some(editor.id.text(cx).as_ref()) {
                        editor.generated_id = None;
                    }
                    editor.edited(cx);
                }
            })
            .detach();
        this.command
            .subscribe(window, cx, |editor, event, _, cx| {
                if matches!(event, TextInputEvent::Changed) {
                    editor.edited(cx);
                }
            })
            .detach();
        this
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    fn draft(&self, cx: &App) -> CustomAgent {
        CustomAgent {
            id: self.id.text(cx).to_string(),
            display_name: self.name.text(cx).to_string(),
            command: self.command.text(cx).to_string(),
            supports_resumption: self.resumption,
            supports_reporting: self.reporting,
        }
    }

    fn dirty(&self, cx: &App) -> bool {
        let draft = self.draft(cx);
        self.stored.as_ref().map_or_else(
            || {
                !draft.display_name.is_empty()
                    || !draft.id.is_empty()
                    || !draft.command.is_empty()
                    || draft.supports_resumption
                    || draft.supports_reporting
            },
            |saved| draft != *saved,
        )
    }

    pub(super) fn edit_state(&self, cx: &App) -> EditState {
        EditState::of(self.saving, !self.deleting && self.dirty(cx))
    }

    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.stored.as_ref();
        self.generated_id = saved.is_none().then(String::new);
        self.name.set_text(
            saved.map_or("", |agent| &agent.display_name).to_owned(),
            window,
            cx,
        );
        self.id
            .set_text(saved.map_or("", |agent| &agent.id).to_owned(), window, cx);
        self.command.set_text(
            saved.map_or("", |agent| &agent.command).to_owned(),
            window,
            cx,
        );
        self.id.set_read_only(saved.is_some(), cx);
        self.resumption = saved.is_some_and(|agent| agent.supports_resumption);
        self.reporting = saved.is_some_and(|agent| agent.supports_reporting);
        self.edited(cx);
    }

    /// Save acknowledgments and external writes may arrive while fields are dirty. A clean editor follows the helm,
    /// while a dirty editor keeps the text that the user was editing.
    fn saw_saved(&mut self, saved: CustomAgent, window: &mut Window, cx: &mut Context<Self>) {
        let clean = !self.dirty(cx);
        self.removed = false;
        self.stored = Some(saved);
        if clean {
            self.restore(window, cx);
        } else {
            cx.notify();
        }
    }

    fn busy(&self) -> bool {
        self.saving || self.deleting
    }

    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.busy() || self.confirm_delete {
            return false;
        }
        let draft = self.draft(cx);
        if let Err(reason) = draft.validate() {
            self.fail(reason, cx);
            return false;
        }
        let add = self.stored.is_none();
        let existing = !add && !self.removed;
        let path = if existing {
            ApiPath::api(&["custom-agents", &draft.id])
        } else {
            ApiPath::api(&["custom-agents"])
        };
        let (Ok(path), Ok(body)) = (path, serde_json::to_value(&draft)) else {
            self.fail("Could not build the custom-agent request", cx);
            return false;
        };
        let method = if existing { Method::Put } else { Method::Post };
        let command = Command::new(Work::Save, method, path, Some(body), "Save custom agent");
        let Some(operations) = session::current_operations(cx) else {
            self.fail_start("Not saved:", None, cx);
            return false;
        };
        let key = OpKey::custom_agent(&draft.id);
        let view = cx.weak_entity();
        let window = window.window_handle();
        let sent = operations
            .update(cx, |operations, cx| {
                operations.start(&key, command, cx, move |settled, cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = view.update(cx, |editor, cx| {
                            editor.saving = false;
                            editor.set_read_only(cx);
                            editor.save_receipt.settle(&settled.outcome, cx);
                            match &settled.outcome {
                                Outcome::Done(_) if !add => {
                                    editor.stored = Some(draft);
                                    editor.removed = false;
                                    editor.edited(cx);
                                }
                                Outcome::Done(_) => {
                                    // The channel applies the new registration before the result.
                                    // Discard Add's draft and go to the stable page when Add is still shown.
                                    editor.restore(window, cx);
                                    let _ = editor.parent.update(cx, |dialog, cx| {
                                        if dialog.shown == Section::AddCustomAgent {
                                            dialog.show(Section::CustomAgent(draft.id), window, cx);
                                        }
                                    });
                                }
                                Outcome::Refused(refusal) => {
                                    editor.error = Some(refusal.words.clone());
                                    cx.notify();
                                }
                                Outcome::Accepted(_) => {
                                    editor.fail("The helm did not confirm the save", cx)
                                }
                            }
                        });
                    });
                })
            })
            .inspect_err(|reason| self.fail_start("Not saved:", Some(reason), cx))
            .is_ok();
        if sent {
            self.saving = true;
            self.set_read_only(cx);
            self.edited(cx);
        }
        sent
    }

    /// A successful Delete ends this editor's ownership of its draft, including when no frame saw the pending delete.
    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(saved) = self.stored.as_ref() else {
            return;
        };
        if self.busy() || !self.confirm_delete {
            return;
        }
        let Ok(path) = ApiPath::api(&["custom-agents", &saved.id]) else {
            return;
        };
        let Some(operations) = session::current_operations(cx) else {
            self.confirm_delete = false;
            self.fail_start("Not deleted:", None, cx);
            return;
        };
        let key = OpKey::custom_agent(&saved.id);
        let editor_key = Some(saved.id.clone());
        let owner = cx.entity_id();
        let parent = self.parent.clone();
        let view = cx.weak_entity();
        let window = window.window_handle();
        let command = Command::new(
            Work::Save,
            Method::Delete,
            path,
            None,
            "Delete custom agent",
        );
        let sent = operations
            .update(cx, |operations, cx| {
                operations.start(&key, command, cx, move |settled, cx| {
                    if matches!(settled.outcome, Outcome::Done(_)) {
                        // Changes precedes the reply, but a frame may see both at once. Retire our own draft
                        // before it can look like an external deletion whose unsaved edits should be kept.
                        let _ = window.update(cx, |_, window, cx| {
                            let _ = parent.update(cx, |dialog, cx| {
                                if dialog
                                    .custom_agents
                                    .get(&editor_key)
                                    .is_some_and(|current| current.entity_id() == owner)
                                {
                                    dialog.custom_agents.remove(&editor_key);
                                    if dialog.shown.custom_agent_key() == Some(editor_key) {
                                        dialog.show(Section::AddCustomAgent, window, cx);
                                    }
                                    cx.notify();
                                }
                            });
                        });
                        return;
                    }
                    let _ = view.update(cx, |editor, cx| {
                        editor.deleting = false;
                        editor.confirm_delete = false;
                        editor.set_read_only(cx);
                        if let Outcome::Refused(refusal) = &settled.outcome {
                            editor.error = Some(refusal.words.clone());
                        }
                        cx.notify();
                    });
                })
            })
            .inspect_err(|reason| {
                self.confirm_delete = false;
                self.fail_start("Not deleted:", Some(reason), cx);
            })
            .is_ok();
        if sent {
            self.deleting = true;
            self.set_read_only(cx);
            self.edited(cx);
        }
    }

    fn set_read_only(&self, cx: &mut App) {
        self.name.set_read_only(self.busy(), cx);
        self.command.set_read_only(self.busy(), cx);
        self.id
            .set_read_only(self.stored.is_some() || self.busy(), cx);
    }

    /// The editor still owns its draft when a write sends nothing. Name the refused action and say what prevented it.
    fn fail_start(&mut self, lead: &str, reason: Option<&NotStarted>, cx: &mut Context<Self>) {
        let detail = match reason {
            Some(NotStarted::Busy(_)) => NotStarted::BUSY_REASON,
            Some(NotStarted::Retired) => "the connection to the helm was reset. Try again.",
            Some(NotStarted::Path(_)) => "Farhelm could not build the request.",
            None => "Farhelm is disconnected from the helm. Try again after it reconnects.",
        };
        self.fail(format!("{lead} {detail}"), cx);
    }

    fn fail(&mut self, reason: impl Into<std::sync::Arc<str>>, cx: &mut Context<Self>) {
        self.error = Some(HostText::from_host(reason));
        cx.notify();
    }

    /// Use the Settings caption so a changed field is marked without shifting the input.
    fn field(
        id: &'static str,
        input: &TextInputState,
        label: &'static str,
        hint: &'static str,
        technical: bool,
        edited: Option<&theme::Palette>,
        cx: &App,
    ) -> Div {
        let field = TextInput::new(id, input, label, HoverHelp::tooltip(hint));
        let field = if technical { field.monospace() } else { field };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(choices::labelled(label, id, field, edited))
            .child(choices::help(hint, cx))
    }

    fn capability(&self, resume: bool, cx: &Context<Self>) -> Checkbox {
        let saved = self.stored.as_ref();
        let (id, label, hint, checked, before) = if resume {
            (
                "custom-agent-resumption",
                "Supports session resumption",
                RESUME_HELP,
                self.resumption,
                saved.is_some_and(|agent| agent.supports_resumption),
            )
        } else {
            (
                "custom-agent-reporting",
                "Supports session reporting",
                REPORT_HELP,
                self.reporting,
                saved.is_some_and(|agent| agent.supports_reporting),
            )
        };
        let weak = cx.weak_entity();
        let caption = editing::caption(label, (checked != before).then_some(theme::palette(cx)));
        Checkbox::new(id, label, HoverHelp::tooltip(hint), Role::CheckBox)
            .content(card::switch_copy(caption, hint, cx))
            .checked(checked)
            .disabled(self.busy())
            .on_toggle(move |on, _, cx| {
                let _ = weak.update(cx, |editor, cx| {
                    if resume {
                        editor.resumption = on;
                    } else {
                        editor.reporting = on;
                    }
                    editor.edited(cx);
                });
            })
    }

    pub(super) fn footer(
        editor: &Entity<Self>,
        cx: &App,
    ) -> (AnyElement, Option<AnyElement>, bool) {
        let this = editor.read(cx);
        let weak = editor.downgrade();
        let busy = this.busy();
        let confirming = this.confirm_delete;
        let mut row = div().flex().justify_end().gap(px(8.));
        if confirming {
            let cancel = weak.clone();
            row = row
                .child(
                    button(
                        "custom-agent-keep",
                        "Cancel",
                        "Keep this registration",
                        Tier::Neutral,
                    )
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let _ = cancel.update(cx, |editor, cx| {
                            editor.confirm_delete = false;
                            editor.edited(cx);
                        });
                    }),
                )
                .child(
                    button(
                        "custom-agent-confirm-delete",
                        "Delete",
                        "Delete this registration",
                        Tier::Danger,
                    )
                    .busy(busy)
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |editor, cx| editor.delete(window, cx));
                    }),
                );
        } else {
            let delete = weak.clone();
            let discard = weak.clone();
            row = row
                .children((this.stored.is_some() && !this.removed).then(|| {
                    button(
                        "custom-agent-delete",
                        "Delete",
                        "Remove this registration from the launcher",
                        Tier::DangerGhost,
                    )
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let _ = delete.update(cx, |editor, cx| {
                            editor.confirm_delete = true;
                            editor.edited(cx);
                        });
                    })
                }))
                .child(
                    button(
                        "custom-agent-discard",
                        "Discard",
                        "Restore the saved fields",
                        Tier::Ghost,
                    )
                    .disabled(!this.dirty(cx) || busy)
                    .on_click(move |_, window, cx| {
                        let _ = discard.update(cx, |editor, cx| editor.restore(window, cx));
                    }),
                )
                .child(
                    button(
                        "custom-agent-save",
                        if this.removed {
                            "Register again"
                        } else {
                            "Save"
                        },
                        "Save this custom agent on the helm",
                        Tier::Primary,
                    )
                    .disabled(!this.dirty(cx))
                    .busy(busy)
                    .on_click(move |_, window, cx| {
                        let _ = weak.update(cx, |editor, cx| editor.save(window, cx));
                    }),
                );
        }
        let notice = this.error.as_ref().map(|reason| {
            choices::failure_outcome("custom-agent-error", reason, cx).into_any_element()
        }).or_else(|| confirming.then(|| {
            div().child("Delete this custom agent? Existing sessions keep their saved resume command.")
                .into_any_element()
        }));
        (row.into_any_element(), notice, confirming)
    }
}

/// Give the golden a refused Save through the same key admission the Settings button uses.
#[cfg(test)]
pub(crate) fn stage_custom_agent_save_refusal(window: &mut Window, cx: &mut App) {
    let editor = crate::layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).custom_editor().cloned())
        .expect("the golden opens a saved custom agent");
    editor.update(cx, |editor, cx| {
        editor.reporting = !editor.reporting;
        assert!(editor.dirty(cx));
        let key = OpKey::custom_agent(&editor.draft(cx).id);
        let operations = session::current_operations(cx).expect("connected");
        operations
            .update(cx, |store, cx| store.hold(&key, Work::Save, cx))
            .expect("hold the saved registration's key");
        assert!(!editor.save(window, cx));
    });
}

/// Populate the Add draft once so the golden shows all the fields that can acquire markers, including its generated ID.
#[cfg(test)]
pub(crate) fn stage_custom_agent_draft(window: &mut Window, cx: &mut App) {
    let editor = crate::layers::last_opened::<SettingsDialog>(cx)
        .and_then(|dialog| dialog.read(cx).custom_editor().cloned())
        .expect("the golden opens Add custom agent");
    editor.update(cx, |editor, cx| {
        assert!(editor.stored.is_none());
        editor.name.edit_for_golden("Local Code", window, cx);
        editor
            .command
            .edit_for_golden("~/bin/local-code", window, cx);
        editor.resumption = true;
        editor.reporting = true;
        editor.edited(cx);
    });
}

fn button(id: &'static str, label: &'static str, help: &'static str, tier: Tier) -> Button {
    Button::new(id, label, HoverHelp::tooltip(help), Role::Button).tier(tier)
}

/// Generate an editable route key from the display name. Non-ASCII names remain valid display names even where
/// they produce no suggested ID.
fn default_id(name: &str) -> String {
    let mut id = String::new();
    let mut separated = false;
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() {
            if separated && !id.is_empty() {
                id.push('-');
            }
            id.push(char::from(byte.to_ascii_lowercase()));
            separated = false;
        } else {
            separated = true;
        }
        if id.len() >= 32 {
            break;
        }
    }
    id.truncate(32);
    id.trim_end_matches('-').to_owned()
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let draft = self.draft(cx);
        let saved = self.stored.as_ref();
        // Add uses the same empty baseline as its footer. A registered agent compares against the last saved values.
        let edited = |text: &str, before: &str| (text != before).then_some(theme::palette(cx));
        let command = HostText::from_host(if draft.command.is_empty() {
            "<cmd>"
        } else {
            &draft.command
        });
        let preview = |id: &'static str, label, tail: &str| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(label)
                // The registration cannot reorder Farhelm's flag words. Both parts keep the terminal face.
                .child(
                    div()
                        .debug_selector(|| id.to_owned())
                        .font_family(crate::assets::TERMINAL_FONT)
                        .child(
                            HostSentence::new(id)
                                .host(&command)
                                .text(format!(" [--model <model>] [--effort <effort>]{tail}")),
                        ),
                )
        };
        let settings = card::rows(
            [
                Self::field(
                    "custom-agent-name",
                    &self.name,
                    "Display name",
                    "Shown in the launcher and session list",
                    false,
                    edited(
                        &draft.display_name,
                        saved.map_or("", |agent| &agent.display_name),
                    ),
                    cx,
                ),
                Self::field(
                    "custom-agent-id",
                    &self.id,
                    "ID",
                    "Used in launch templates and CLI flags. The ID stays fixed after saving.",
                    true,
                    edited(&draft.id, saved.map_or("", |agent| &agent.id)),
                    cx,
                ),
                Self::field(
                    "custom-agent-command",
                    &self.command,
                    "Command",
                    COMMAND_HELP,
                    true,
                    edited(&draft.command, saved.map_or("", |agent| &agent.command)),
                    cx,
                ),
            ],
            cx,
        );
        let capabilities = card::rows([self.capability(true, cx), self.capability(false, cx)], cx);
        let commands = card::rows(
            std::iter::once(preview("custom-agent-start-preview", "Start", "")).chain(
                self.resumption
                    .then(|| preview("custom-agent-resume-preview", "Resume", " --resume <id>")),
            ),
            cx,
        );
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(self.removed.then(|| {
                // This is the only explanation of why a retained draft needs to be registered again.
                let notice =
                    "This registration was deleted. Register it again to keep these edits.";
                div()
                    .id("custom-agent-deleted-notice")
                    .role(Role::Status)
                    .aria_label(notice)
                    .child(notice)
            }))
            .child(card::heading("Agent command", None, None, cx))
            .child(settings)
            .child(card::heading("Contract capabilities", None, None, cx))
            .child(capabilities)
            .child(card::heading(
                "How Farhelm launches this agent",
                None,
                None,
                cx,
            ))
            .child(commands)
            .child(
                div()
                    .text_color(theme::color(theme::palette(cx).fg_1))
                    .child(CONTRACT_NOTE),
            )
    }
}

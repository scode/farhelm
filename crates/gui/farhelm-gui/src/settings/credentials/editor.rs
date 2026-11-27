//! One credential's editor, drawn in place of the Credentials overview: Back, the agent choice for a new agent login,
//! Name, the write-only secret, the agent's help, the recipe-use line, the storage warning, and Delete, Cancel and
//! Save.
//!
//! Ported from the old app's `CredentialEditor` (`credentials_settings.rs` at reference commit `24bc0580`), with its
//! words; the rules are in the parent module's docs ([`super`]).
//!
//! The answers and operation outcomes sit in Settings' pinned footer, even when a long login fills the page.

use farhelm_helm_client::Method;
use farhelm_proto::credentials::{Credential, CredentialKind, check_secret};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, PathPromptOptions, Render, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, WeakEntity, Window, div, px,
};

use super::super::choices;
use super::{
    Credentials, Group, delivery_failures, malformed, read_login, save_command, storage_warning,
    usage,
};
use crate::components::{
    Button, HoverHelp, RadioGroup, RadioOption, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::drafts::DraftInput;
use crate::focus::{self, Mark};
use crate::host_text::{self, HostText};
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session;
use crate::theme;

/// `.credential-editor`'s gap between its parts.
const EDITOR_GAP: f32 = 12.;
/// The Codex login box's starting rows, the old `<textarea rows="8">`.
const LOGIN_ROWS: usize = 8;

/// The agents an agent login can be for, in the old segments' order.
const AGENTS: [(CredentialKind, farhelm_proto::LaunchHarness); 2] = [
    (
        CredentialKind::ClaudeLogin,
        farhelm_proto::LaunchHarness::Claude,
    ),
    (
        CredentialKind::CodexLogin,
        farhelm_proto::LaunchHarness::Codex,
    ),
];

/// A Codex login selected from disk. The editor paints only the basename and whether the login passes the helm's
/// shape check. Save uses the same bytes that were checked, even if the file changes on disk.
struct ChosenLogin {
    filename: HostText,
    contents: String,
    validated: bool,
}

impl ChosenLogin {
    /// Load once on the background executor. The shared check validates JSON shape without testing the account.
    fn read(path: &std::path::Path) -> Result<Self, &'static str> {
        let contents = read_login(path)?;
        let filename = path
            .file_name()
            .ok_or("Could not read that login file.")?
            .to_string_lossy();
        let validated = check_secret(CredentialKind::CodexLogin, &contents).is_ok();
        Ok(Self {
            filename: HostText::from_host(filename.into_owned()),
            contents,
            validated,
        })
    }
}

/// One credential's editor (see the parent module's docs). A view of its own, dropped by Back, Cancel, a done save or
/// delete, or the dialog closing. Back and Cancel are unavailable while a save or the Delete is out (SPEC.md,
/// "Settings": waiting for the helm's round trip is expected), so while the dialog stays open the editor that sent a
/// request is still the open one when the reply comes; only closing the dialog leaves that request to finish in the
/// operation store without it.
pub(in crate::settings) struct Editor {
    pub(in crate::settings) save_receipt: crate::settings::editing::SaveReceipt,
    parent: WeakEntity<Credentials>,
    group: Group,
    kind: CredentialKind,
    /// The saved credential being replaced, or `None` for a new one.
    existing: Option<Credential>,
    name: TextInputState,
    /// Text the user enters: a masked line for a token or a growing monospace box for a Codex paste.
    secret: DraftInput,
    /// Replaces the paste box until Clear. The contents never enter a text field or its undo history.
    chosen_login: Option<ChosenLogin>,
    /// Why the last Save or Delete did not happen, or the last chosen file could not be read (the editor's own check or
    /// the helm's refusal), with the secret as it stood then. Shown while the secret is unchanged ([`Editor::error`]),
    /// so an edit of the secret drops it as the old editor's `oninput` did; an edit of the name drops it outright.
    /// Replaced by the next Save, Delete or file choice.
    error: Option<(HostText, String)>,
    /// Whether the editor's Delete is out.
    deleting: bool,
    /// The open file dialog and the read of the file chosen in it, for a Codex login; Choose file… is busy meanwhile.
    /// Clear, an agent switch, Save and closing abandon the task. The paste box's edit revision fences the result, even
    /// if the user returned to the original text.
    choosing: Option<Task<()>>,
    /// The name field's edits, which drop the last error. The secret field has no such watch of the editor's own: a
    /// text field's state takes one subscription (`TextInputState::subscribe`), which the draft holds, and a second one
    /// would never see an edit. The editor uses the draft's edit revision for file selection and its value for errors.
    _name_edits: Subscription,
}

impl Editor {
    pub(super) fn new(
        parent: WeakEntity<Credentials>,
        group: Group,
        existing: Option<Credential>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let kind = existing
            .as_ref()
            .map_or(group.words().new_kind, |credential| credential.kind);
        let name = TextInputState::new(window, cx);
        if let Some(existing) = &existing {
            name.set_text(existing.name.clone(), window, cx);
            name.set_read_only(true, cx);
        }
        let name_edits = name.subscribe(window, cx, |editor: &mut Self, event, _, cx| {
            if let TextInputEvent::Changed = event {
                editor.error = None;
                cx.notify();
            }
        });
        Editor {
            save_receipt: crate::settings::editing::SaveReceipt::default(),
            parent,
            group,
            kind,
            secret: Self::secret_input(kind, existing.is_some(), window, cx),
            chosen_login: None,
            existing,
            name,
            error: None,
            deleting: false,
            choosing: None,
            _name_edits: name_edits,
        }
    }

    /// An empty entry field with a placeholder that explains what a blank replacement means.
    fn secret_input(
        kind: CredentialKind,
        replacing: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DraftInput {
        let (state, placeholder) = match kind {
            CredentialKind::CodexLogin => (
                TextInputState::new_growing(LOGIN_ROWS, window, cx),
                if replacing {
                    "Login saved; paste a new one to replace it"
                } else {
                    "Paste the contents of auth.json"
                },
            ),
            CredentialKind::GithubToken
            | CredentialKind::ClaudeLogin
            | CredentialKind::EndpointToken => (
                TextInputState::new_masked(window, cx),
                if replacing {
                    "Token saved; paste a new one to replace it"
                } else {
                    "Paste the token"
                },
            ),
        };
        state.set_placeholder(placeholder, window, cx);
        // Enter saves nothing: the old editor's fields sat in no form, so Enter in its token field did nothing (and the
        // Codex box takes Enter as a line break).
        DraftInput::new(
            "",
            state,
            |editor: &mut Self| &mut editor.secret,
            |_, _, _| {},
            window,
            cx,
        )
    }

    /// Save and its error line both use the active source of the draft, including when the paste box is hidden.
    fn secret_value(&self) -> &str {
        match &self.chosen_login {
            Some(login) => &login.contents,
            None => self.secret.draft().value(),
        }
    }

    /// Record `words` as why the last action did not happen, against the secret as it stands now.
    fn fail(&mut self, words: HostText) {
        self.error = Some((words, self.secret_value().to_owned()));
    }

    /// The error to show: the last one, while the secret is still what it was when it came.
    fn error(&self) -> Option<&HostText> {
        self.error
            .as_ref()
            .filter(|(_, secret)| secret == self.secret_value())
            .map(|(words, _)| words)
    }

    /// Where focus starts when the editor opens: the name for a new credential, the secret for a saved one, whose name
    /// is fixed. The old editor put it on its title, which here is not a control.
    pub(in crate::settings) fn start(&self, cx: &App) -> FocusHandle {
        if self.existing.is_some() {
            self.secret.input().focus_handle(cx)
        } else {
            self.name.focus_handle(cx)
        }
    }

    /// The entry field's text and the value Save would use. GPUI Kit's field reports no accessible value.
    #[cfg(test)]
    pub(in crate::settings) fn secret(&self, cx: &App) -> (String, String) {
        (
            self.secret.input().text(cx).to_string(),
            self.secret_value().to_owned(),
        )
    }

    fn replacing(&self) -> bool {
        self.existing.is_some()
    }

    /// Whether Save, Delete, Back, Cancel, the agent choice and Choose file take no press: while a save or the Delete is
    /// out.
    fn busy(&self) -> bool {
        self.secret.draft().is_saving() || self.deleting
    }

    /// Whether the editor's Delete is out, which holds a close ([`crate::settings::SettingsDialog::close_busy`]).
    pub(in crate::settings) fn deleting(&self) -> bool {
        self.deleting
    }

    /// The name the editor saves under: the saved one, or what was typed.
    fn name(&self, cx: &App) -> String {
        match &self.existing {
            Some(existing) => existing.name.clone(),
            None => self.name.text(cx).to_string(),
        }
    }

    /// The edited golden fills the masked field with a synthetic value so Save is available.
    #[cfg(test)]
    pub(in crate::settings) fn secret_for_golden(&self) -> TextInputState {
        self.secret.input().clone()
    }

    /// A new agent login switched agents: the secret starts again, empty, in the new agent's field.
    fn choose_agent(&mut self, kind: CredentialKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.replacing() || self.busy() || kind == self.kind {
            return;
        }
        self.kind = kind;
        self.secret = Self::secret_input(kind, false, window, cx);
        self.chosen_login = None;
        self.choosing = None;
        self.error = None;
        cx.notify();
    }

    /// The stored secret is write-only. An existing credential's only edit is a replacement paste or selected file.
    /// A new credential also has an editable name and agent choice.
    pub(in crate::settings) fn edit_state(&self, cx: &App) -> crate::settings::editing::EditState {
        let dirty = !self.secret_value().is_empty()
            || (!self.replacing()
                && (!self.name(cx).is_empty() || self.kind != self.group.words().new_kind));
        crate::settings::editing::EditState::of(
            self.secret.draft().is_saving(),
            !self.deleting && dirty,
        )
    }

    /// File staging replaces the paste input, while clearing a file changes its source without replacing the input.
    /// Compare both along with the destination and paste revision so no confirmation stores the secret bytes.
    pub(in crate::settings) fn edit_revision(
        &self,
        cx: &App,
    ) -> (FocusHandle, u64, bool, String, CredentialKind) {
        (
            self.secret.input().focus_handle(cx),
            self.secret.edit_revision(),
            self.chosen_login.is_some(),
            self.name(cx),
            self.kind,
        )
    }

    /// Save the named credential and report whether its own operation was admitted.
    pub(in crate::settings) fn save(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let replacing = self.replacing();
        if self.busy() {
            return false;
        }
        // Save fixes the draft being sent, including while the file picker is still waiting.
        self.choosing = None;
        let secret = self.secret_value().to_owned();
        let name = self.name(cx);
        let kind = self.kind;
        if let Some(problem) = malformed(&name, kind, replacing, &secret) {
            self.fail(HostText::from_host(problem));
            cx.notify();
            return false;
        }
        let mark = Mark::take(cx);
        let group = self.group;
        let saving = name.clone();
        let started = DraftInput::save(
            self,
            |editor| &mut editor.secret,
            &OpKey::credential(&name),
            // Use the active value. The paste draft still owns the in-flight state for both sources.
            move |_| save_command(&saving, kind, replacing, &secret),
            window,
            cx,
            move |editor, settled, cx| {
                editor.name.set_read_only(editor.replacing(), cx);
                editor.save_receipt.settle(&settled.outcome, cx);
                match &settled.outcome {
                    Outcome::Done(body) => {
                        let undelivered = delivery_failures(replacing, body.clone());
                        let _ = editor.parent.update(cx, |parent, cx| {
                            parent.close_editor(group, undelivered, &mark, None, cx)
                        });
                    }
                    Outcome::Refused(refusal) => editor.fail(refusal.words.clone()),
                    Outcome::Accepted(_) => {}
                }
            },
        );
        if started.is_ok() {
            self.name.set_read_only(true, cx);
        }
        let sent = started.is_ok();
        self.started(started, cx);
        sent
    }

    /// Delete the saved credential under its key. Does nothing for a new one, while busy, or while a recipe uses it
    /// (SPEC.md: "Delete is unavailable while a recipe uses a credential"), when Delete is drawn unavailable or not at
    /// all. No question, as in the old editor.
    fn delete(&mut self, cx: &mut Context<Self>) {
        let Some(existing) = &self.existing else {
            return;
        };
        if self.busy() || !existing.used_by.is_empty() {
            return;
        }
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match super::path(Some(&existing.name)) {
            Ok(path) => super::command(Method::Delete, path, None),
            Err(error) => {
                tracing::debug!(?error, "credential delete not built");
                return;
            }
        };
        let key = OpKey::credential(&existing.name);
        let mark = Mark::take(cx);
        let group = self.group;
        let editor = cx.weak_entity();
        let parent = self.parent.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                let refused = matches!(settled.outcome, Outcome::Refused(_));
                // Before the editor goes: a close waiting on this Delete resumes after this completion.
                let _ = editor.update(cx, |editor, cx| {
                    editor.save_receipt.settled_other(refused, cx)
                });
                match settled.outcome.clone() {
                    Outcome::Done(_) => {
                        let _ = parent.update(cx, |parent, cx| {
                            parent.close_editor(group, Vec::new(), &mark, None, cx)
                        });
                    }
                    Outcome::Refused(refusal) => {
                        let _ = editor.update(cx, |editor, cx| {
                            editor.deleting = false;
                            editor.fail(refusal.words);
                            cx.notify();
                        });
                    }
                    Outcome::Accepted(_) => {}
                }
            })
        });
        if started.is_ok() {
            self.deleting = true;
        }
        self.started(started, cx);
    }

    /// After a Save or Delete was asked: a started one drops the last error. Nothing else works on the credential's
    /// key while the editor is open (Save and Delete wait for each other, and a close waits for both), so the store
    /// never refuses one as busy and there is no busy line to show.
    fn started(&mut self, started: Result<(), NotStarted>, cx: &mut Context<Self>) {
        match started {
            Ok(()) => self.error = None,
            Err(not_started) => tracing::debug!(?not_started, "credential not saved"),
        }
        cx.notify();
    }

    /// Choose file… stages a file without submitting it. A read failure leaves the active draft in place, and the
    /// error is independent of the file's contents.
    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.choosing.is_some() {
            return;
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        self.error = None;
        let before = self.secret.edit_revision();
        self.choosing = Some(cx.spawn_in(window, async move |editor, cx| {
            // A cancelled or unavailable dialog leaves the active draft in place.
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let read = match path {
                Some(path) => Some(
                    cx.background_executor()
                        .spawn(async move { ChosenLogin::read(&path) })
                        .await,
                ),
                None => None,
            };
            let _ = editor.update_in(cx, |editor, window, cx| {
                // Choose file… is free again however the dialog ended.
                editor.choosing = None;
                let Some(read) = read else {
                    cx.notify();
                    return;
                };
                if editor.secret.edit_revision() != before {
                    // Typed or pasted into while the file was chosen or read: the newer text wins.
                    cx.notify();
                    return;
                }
                match read {
                    Ok(login) => {
                        editor.secret =
                            Self::secret_input(editor.kind, editor.replacing(), window, cx);
                        editor.chosen_login = Some(login);
                    }
                    Err(problem) => editor.fail(HostText::from_host(problem)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Discard the selected file and restore the empty paste box, ready for keyboard input.
    fn clear_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.chosen_login.take().is_none() {
            return;
        }
        self.choosing = None;
        self.error = None;
        let mark = Mark::take(cx);
        let target = self.secret.input().focus_handle(cx);
        focus::focus_in_layer(&target, &mark, window, cx);
        cx.notify();
    }

    /// Back and Cancel: drop the editor and its text, with focus back on its list's Add button; the overview's
    /// delivery-failure lines go and the catalog is read again, as the old editor's `on_done(Vec::new())` did. Nothing
    /// while a save or the Delete is out: the editor waits for the helm's answer, whose done reply closes it with that
    /// save's delivery failures and whose refusal keeps the draft with the helm's words.
    fn leave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let mark = Mark::take(cx);
        let group = self.group;
        let _ = self.parent.update(cx, |parent, cx| {
            parent.close_editor(group, Vec::new(), &mark, Some(window), cx)
        });
    }

    /// Keep the editor's commands and their outcomes in the dialog footer. The overview's hidden key draft
    /// keeps its state but does not take over these buttons while the editor is open.
    pub(in crate::settings) fn footer(
        editor: &Entity<Editor>,
        cx: &App,
    ) -> (AnyElement, Option<AnyElement>, bool) {
        let this = editor.read(cx);
        let weak = editor.downgrade();
        let replacing = this.replacing();
        let busy = this.busy();
        let in_use = this
            .existing
            .as_ref()
            .is_some_and(|existing| !existing.used_by.is_empty());
        let delete = replacing.then(|| {
            let weak = weak.clone();
            Button::new(
                "credential-delete",
                "Delete",
                HoverHelp::tooltip("Delete this saved credential"),
                Role::Button,
            )
            .tier(Tier::DangerGhost)
            .disabled(in_use)
            .busy(busy)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |editor, cx| editor.delete(cx));
            })
        });
        let cancel = {
            let weak = weak.clone();
            Button::new(
                "credential-cancel",
                "Cancel",
                HoverHelp::tooltip("Discard this unsaved credential draft"),
                Role::Button,
            )
            .tier(Tier::Ghost)
            .disabled(busy)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |editor, cx| editor.leave(window, cx));
            })
        };
        let save = Button::new(
            "credential-save",
            "Save",
            HoverHelp::tooltip("Save this credential in the helm's state"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .disabled(!replacing && this.secret_value().trim().is_empty())
        // Busy, not disabled, while saving, so a clicked Save keeps focus (SPEC_impl.md, "Drafts").
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |editor, cx| editor.save(window, cx));
        });

        let row = div()
            .flex()
            .justify_end()
            .gap(px(8.))
            .children(delete)
            .child(cancel)
            .child(save)
            .into_any_element();
        let error = this.error();
        let outcome = (error.is_some() || busy).then(|| {
            let palette = theme::palette(cx);
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(
                    error.map(|reason| choices::failure_outcome("credential-error", reason, cx)),
                )
                .children(busy.then(|| {
                    choices::outcome_line(
                        "credential-saving",
                        Role::Status,
                        theme::color(palette.fg_0),
                    )
                    .aria_label("Saving…")
                    .child("Saving…")
                }))
                .into_any_element()
        });
        (row, outcome, false)
    }

    /// A fresh read listed `catalog`: a saved credential's editor takes its row, so the recipe-use line and Delete's
    /// availability follow it; the typed secret is untouched.
    pub(super) fn refresh(&mut self, catalog: &[Credential], cx: &mut Context<Self>) {
        let Some(existing) = &mut self.existing else {
            return;
        };
        if let Some(row) = catalog.iter().find(|row| row.name == existing.name)
            && row != existing
        {
            *existing = row.clone();
            cx.notify();
        }
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let weak = cx.weak_entity();
        let busy = self.busy();
        let replacing = self.replacing();
        let codex = self.kind == CredentialKind::CodexLogin;
        let label = match self.kind {
            CredentialKind::GithubToken => "GitHub token",
            CredentialKind::ClaudeLogin | CredentialKind::CodexLogin => "Agent login",
            CredentialKind::EndpointToken => "Endpoint token",
        };
        let title = match &self.existing {
            Some(existing) => format!("{label}: {}", existing.name),
            None => match self.kind {
                CredentialKind::GithubToken => "New GitHub token".to_owned(),
                CredentialKind::ClaudeLogin | CredentialKind::CodexLogin => {
                    "New agent login".to_owned()
                }
                CredentialKind::EndpointToken => "New endpoint token".to_owned(),
            },
        };
        let in_use = self
            .existing
            .as_ref()
            .is_some_and(|existing| !existing.used_by.is_empty());

        let back = {
            let weak = weak.clone();
            Button::new(
                "credential-back",
                "‹ Credentials",
                HoverHelp::tooltip("Return to credentials without saving this draft"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .compact()
            .disabled(busy)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |editor, cx| editor.leave(window, cx));
            })
        };
        // The agent choice, only for a new agent login: a saved one's kind is fixed.
        let agent = (Group::of(self.kind) == Group::AGENT_LOGINS).then(|| {
            let weak = weak.clone();
            AGENTS
                .into_iter()
                .fold(
                    RadioGroup::new(
                        "credential-agent",
                        "Agent",
                        HoverHelp::tooltip("The agent this login is for"),
                        Role::RadioGroup,
                    ),
                    |group, (kind, harness)| {
                        let word = crate::harness::name(harness);
                        group.option(
                            RadioOption::new(
                                word,
                                HoverHelp::tooltip(format!("A {word} subscription login")),
                                kind,
                            )
                            .icon(crate::harness::icon(crate::harness::glyph(harness))),
                        )
                    },
                )
                .chosen(Some(self.kind))
                .disabled(replacing || busy)
                .on_change(move |&kind, window, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.choose_agent(kind, window, cx));
                })
        });
        let name = choices::field_with_caption(
            "credential-name",
            TextInput::new(
                "credential-name",
                &self.name,
                "Name",
                HoverHelp::tooltip(
                    "The name recipes use for this credential; it cannot be changed after it is saved",
                ),
            )
            .height(px(choices::FIELD_HEIGHT)),
            crate::settings::card::caption(
                "Name",
                !replacing && !self.name.text(cx).is_empty(),
                palette,
            ),
        );
        let secret = match &self.chosen_login {
            Some(login) => {
                let weak = weak.clone();
                let validation = if login.validated {
                    "Validated"
                } else {
                    "Invalid login file"
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .w_full()
                    .min_w_0()
                    .child(host_text::element(&login.filename).min_w_0().truncate())
                    .child(crate::settings::editing::dot(false, palette))
                    .child(choices::help(validation, cx))
                    .child(
                        Button::new(
                            "credential-clear-file",
                            "Clear",
                            HoverHelp::tooltip("Discard this file and return to the paste box"),
                            Role::Button,
                        )
                        .accessible_name("Clear chosen login file")
                        .tier(Tier::Neutral)
                        .busy(busy)
                        .on_click(move |_, window, cx| {
                            let _ = weak.update(cx, |editor, cx| editor.clear_file(window, cx));
                        }),
                    )
            }
            None => {
                let label = if codex { "auth.json" } else { "Token" };
                let secret = TextInput::new(
                    "credential-secret",
                    self.secret.input(),
                    label,
                    HoverHelp::tooltip(if replacing {
                        "Paste a new value to replace the saved one; leave it empty to keep it"
                    } else {
                        "Paste the value to save; Farhelm never shows it again"
                    }),
                );
                choices::field_with_caption(
                    "credential-secret",
                    if codex {
                        secret.monospace()
                    } else {
                        secret.height(px(choices::FIELD_HEIGHT))
                    },
                    crate::settings::card::caption(label, !self.secret_value().is_empty(), palette),
                )
            }
        };
        let choose = codex.then(|| {
            let weak = weak.clone();
            div().flex().child(
                Button::new(
                    "credential-choose-file",
                    "Choose file…",
                    HoverHelp::tooltip(
                        "Choose an auth.json file. Save submits the selected login.",
                    ),
                    Role::Button,
                )
                .accessible_name("Choose auth.json file")
                .tier(Tier::Neutral)
                .busy(busy || self.choosing.is_some())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.choose_file(window, cx));
                }),
            )
        });
        let agent_help: Vec<&'static str> = match self.kind {
            CredentialKind::CodexLogin => vec![
                "On a machine with a browser, run codex login and sign in with ChatGPT, then paste or choose \
                 ~/.codex/auth.json.",
                "Use one ChatGPT login per host. Copies share a rotating refresh token: when one copy renews it, other \
                 copies cannot renew and need a fresh sign-in. To share inference access, prefer a proxy or load \
                 balancer through an API endpoint. You are responsible for keeping that use within your provider's \
                 terms of service.",
            ],
            CredentialKind::ClaudeLogin => vec![
                "Run claude setup-token on a machine where you can sign in to Claude, then paste the token it prints. \
                 It starts with sk-ant-oat01- and lasts a year. You need a Pro, Max, Team or Enterprise plan.",
            ],
            CredentialKind::GithubToken | CredentialKind::EndpointToken => Vec::new(),
        };

        div()
            .id("credential-editor")
            .role(Role::Region)
            .aria_label(title.clone())
            .flex()
            .flex_col()
            .gap(px(EDITOR_GAP))
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(back)
                    .child(
                        div()
                            .id("credential-editor-title")
                            .debug_selector(|| "credential-editor-title".to_owned())
                            .role(Role::Heading)
                            .aria_label(title.clone())
                            .text_size(theme::metrics::TEXT_MD)
                            .line_height(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::color(palette.fg_0))
                            .child(title),
                    ),
            )
            .child(crate::settings::card::rows([
                div().flex().flex_col().gap(px(EDITOR_GAP))
                    .children(agent.map(|agent| {
                        div().flex().flex_col().gap(px(4.))
                            .child(crate::settings::editing::caption(
                                "Agent",
                                (!replacing && self.kind != self.group.words().new_kind).then_some(palette),
                            ))
                            .child(div().flex().child(agent))
                    }))
                    .child(name)
                    .child(secret)
                    .children(choose)
                    .children(agent_help.into_iter().map(|help| choices::help(help, cx)))
                    .child(choices::help(usage(self.existing.as_ref()), cx)),
            ], cx))
            .child(storage_warning(
                "credential-storage-warning",
                "Farhelm stores these in plain text in this helm's state, on the machine the helm runs on. Anyone \
                 who can read that folder can read them. Farhelm never shows a saved value again.",
                cx,
            ))
            .children(in_use.then(|| {
                choices::help(
                    "Delete is unavailable while a recipe uses this credential.",
                    cx,
                )
            }))
    }
}

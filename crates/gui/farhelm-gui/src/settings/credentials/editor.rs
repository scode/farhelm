//! One credential's editor, drawn in place of the Credentials overview: Back, the agent choice for a new agent login,
//! Name, the write-only secret, the agent's help, the recipe-use line, the storage warning, and Delete, Cancel and
//! Save.
//!
//! Ported from the old app's `CredentialEditor` (`credentials_settings.rs` at reference commit `24bc0580`), with its
//! words; the rules are in the parent module's docs ([`super`]).
//!
//! NOTE: The old editor's answers stuck to the bottom of the dialog's scrolling body (`position: sticky`), so Save
//! stayed in view over a long Codex login. GPUI has no sticky positioning, so here they follow the editor's content and
//! scroll with it, as the section list does (`docs/gpui/parity/settings-and-hosts.md`, "Deviations").

use farhelm_helm_client::Method;
use farhelm_proto::credentials::{Credential, CredentialKind};
use gpui::{
    App, Context, EntityId, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, PathPromptOptions, Render, Role, StatefulInteractiveElement as _,
    Styled as _, Subscription, Task, WeakEntity, Window, div, px,
};

use super::super::Refused;
use super::super::choices::{self, LABEL_LINE_HEIGHT, LABEL_TEXT};
use super::{
    Credentials, Group, delivery_failures, malformed, read_login, save_command, storage_warning,
    usage,
};
use crate::components::{
    Button, HoverHelp, RadioGroup, RadioOption, TextInput, TextInputEvent, TextInputState, Tier,
};
use crate::drafts::DraftInput;
use crate::focus::Mark;
use crate::host_text::HostText;
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session;
use crate::theme;

/// `.credential-editor`'s gap between its parts.
const EDITOR_GAP: f32 = 12.;
/// The Codex login box's starting rows, the old `<textarea rows="8">`.
const LOGIN_ROWS: usize = 8;
/// The editor's name for [`Refused`]: Save and Delete are the controls whose press the key can refuse, and the line
/// saying so is drawn under the id [`Refused::line`] makes of it, `credential-editor-busy`.
const REFUSED: &str = "credential-editor";

/// The agents an agent login can be for, in the old segments' order.
const AGENTS: [(CredentialKind, &str); 2] = [
    (CredentialKind::ClaudeLogin, "Claude Code"),
    (CredentialKind::CodexLogin, "Codex"),
];

/// One credential's editor (see the parent module's docs). A view of its own, dropped by Back, Cancel, a done save or
/// delete, or the dialog closing; a save or delete it started goes on in the operation store without it.
pub(in crate::settings) struct Editor {
    parent: WeakEntity<Credentials>,
    group: Group,
    kind: CredentialKind,
    /// The saved credential being replaced, or `None` for a new one.
    existing: Option<Credential>,
    name: TextInputState,
    /// The write-only secret: a masked line for a token, a growing monospace box for Codex's `auth.json`. Made again
    /// when a new agent login switches agents, which empties it, as the old editor did, and when a chosen file fills it.
    secret: DraftInput,
    /// Why the last Save or Delete did not happen, or the last chosen file could not be read (the editor's own check or
    /// the helm's refusal), with the secret as it stood then. Shown while the secret is unchanged ([`Editor::error`]),
    /// so an edit of the secret drops it as the old editor's `oninput` did; an edit of the name drops it outright.
    /// Replaced by the next Save, Delete or file choice.
    error: Option<(HostText, String)>,
    /// Whether the editor's Delete is out.
    deleting: bool,
    /// Whether the last Save or Delete sent nothing because work on this credential was already in flight.
    refused: Refused,
    /// The open file dialog and the read of the file chosen in it, for a Codex login; Choose file… is busy meanwhile.
    /// Dropped, which abandons both, by a switch of agent, a Save or the editor closing, and its result is dropped if the
    /// box's text differs from when the dialog opened. That value comparison stands in for SPEC_impl.md's "generation
    /// check discards reads superseded by paste, agent selection or submission": an edit that restores the exact text
    /// would let the file through, which is harmless.
    choosing: Option<Task<()>>,
    /// The name field's edits, which drop the last error. The secret field has no such watch of the editor's own: a
    /// text field's state takes one subscription (`TextInputState::subscribe`), which the draft holds, and a second one
    /// would never see an edit. So the editor compares the draft's value instead.
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
            parent,
            group,
            kind,
            secret: Self::secret_input(kind, existing.is_some(), "", window, cx),
            existing,
            name,
            error: None,
            deleting: false,
            refused: Refused::default(),
            choosing: None,
            _name_edits: name_edits,
        }
    }

    /// The secret's field for `kind`, holding `text` (a chosen file's) as its untouched draft, with the old placeholder
    /// for a new or a saved credential.
    fn secret_input(
        kind: CredentialKind,
        replacing: bool,
        text: &str,
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
            text,
            state,
            |editor: &mut Self| &mut editor.secret,
            |_, _, _| {},
            window,
            cx,
        )
    }

    /// Record `words` as why the last action did not happen, against the secret as it stands now.
    fn fail(&mut self, words: HostText) {
        self.error = Some((words, self.secret.draft().value().clone()));
    }

    /// The error to show: the last one, while the secret is still what it was when it came.
    fn error(&self) -> Option<&HostText> {
        self.error
            .as_ref()
            .filter(|(_, secret)| secret == self.secret.draft().value())
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

    /// The secret field's text and what its draft holds, for tests: GPUI Kit's field reports no accessible value, and a
    /// field showing text its draft does not hold is the bug a chosen file must not cause.
    #[cfg(test)]
    pub(in crate::settings) fn secret(&self, cx: &App) -> (String, String) {
        (
            self.secret.input().text(cx).to_string(),
            self.secret.draft().value().clone(),
        )
    }

    fn replacing(&self) -> bool {
        self.existing.is_some()
    }

    /// Whether Save, Delete, the agent choice and Choose file take no press: while a save or the Delete is out.
    fn busy(&self) -> bool {
        self.secret.draft().is_saving() || self.deleting
    }

    /// The name the editor saves under: the saved one, or what was typed.
    fn name(&self, cx: &App) -> String {
        match &self.existing {
            Some(existing) => existing.name.clone(),
            None => self.name.text(cx).to_string(),
        }
    }

    /// A new agent login switched agents: the secret starts again, empty, in the new agent's field.
    fn choose_agent(&mut self, kind: CredentialKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.replacing() || self.busy() || kind == self.kind {
            return;
        }
        self.kind = kind;
        self.secret = Self::secret_input(kind, false, "", window, cx);
        self.choosing = None;
        self.error = None;
        cx.notify();
    }

    /// Save: check the draft, then send it under the credential's key (the parent module's docs). Does nothing while
    /// busy, or for a new credential with no secret typed, when Save is drawn busy or unavailable.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let secret = self.secret.draft().value().clone();
        let replacing = self.replacing();
        if self.busy() || (!replacing && secret.trim().is_empty()) {
            return;
        }
        // A file still being read must not land in the field once this value is on its way.
        self.choosing = None;
        let name = self.name(cx);
        let kind = self.kind;
        if let Some(problem) = malformed(&name, kind, replacing, &secret) {
            self.fail(HostText::from_host(problem));
            cx.notify();
            return;
        }
        let mark = Mark::take(cx);
        let group = self.group;
        let saving = name.clone();
        let started = DraftInput::save(
            self,
            |editor| &mut editor.secret,
            &OpKey::credential(&name),
            move |secret| save_command(&saving, kind, replacing, secret),
            window,
            cx,
            move |editor, settled, cx| {
                editor.name.set_read_only(editor.replacing(), cx);
                match &settled.outcome {
                    Outcome::Done(body) => {
                        let undelivered = delivery_failures(replacing, body.clone());
                        let id = cx.entity_id();
                        let _ = editor.parent.update(cx, |parent, cx| {
                            parent.close_editor(id, group, undelivered, &mark, None, cx)
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
        self.started(started, cx);
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
        // The reply may come after Back and another editor's opening; only this editor may be closed by it.
        let id = cx.entity_id();
        let parent = self.parent.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&key, command, cx, move |settled, cx| {
                match settled.outcome.clone() {
                    Outcome::Done(_) => {
                        let _ = parent.update(cx, |parent, cx| {
                            parent.close_editor(id, group, Vec::new(), &mark, None, cx)
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

    /// After a Save or Delete was asked: a started one drops the last error, a busy key says so ([`Refused`]).
    fn started(&mut self, started: Result<(), NotStarted>, cx: &mut Context<Self>) {
        self.refused.record(REFUSED, &started);
        match started {
            Ok(()) => self.error = None,
            Err(not_started) => tracing::debug!(?not_started, "credential not saved"),
        }
        cx.notify();
    }

    /// Choose file…: open the platform's file dialog for an `auth.json`, and fill the box with the chosen file's text,
    /// which is a draft like a paste and saves nothing ([`Editor::choosing`] says what abandons it). A file over the 64
    /// KiB cap, or one that cannot be read as text, says so and leaves the box as it was; the words never quote the file.
    fn choose_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        self.error = None;
        let before = self.secret.draft().value().clone();
        self.choosing = Some(cx.spawn_in(window, async move |editor, cx| {
            // `None` for a cancelled dialog, one the platform could not open, or no file in it: the box stays as it was.
            let path = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let read = match path {
                Some(path) => Some(
                    cx.background_executor()
                        .spawn(async move { read_login(&path) })
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
                if *editor.secret.draft().value() != before {
                    // Typed or pasted into while the file was chosen or read: the newer text wins.
                    cx.notify();
                    return;
                }
                match read {
                    // A new field holding the text as its untouched draft: setting a field's text raises no change
                    // event, so the draft would not learn it otherwise.
                    Ok(text) => {
                        editor.secret =
                            Self::secret_input(editor.kind, editor.replacing(), &text, window, cx);
                    }
                    Err(problem) => editor.fail(HostText::from_host(problem)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Back and Cancel: drop the editor `id` and its text, with focus back on its list's Add button; the overview's
    /// delivery-failure lines go and the catalog is read again, as the old editor's `on_done(Vec::new())` did.
    fn leave(
        parent: &WeakEntity<Credentials>,
        id: EntityId,
        group: Group,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mark = Mark::take(cx);
        let _ = parent.update(cx, |parent, cx| {
            parent.close_editor(id, group, Vec::new(), &mark, Some(window), cx)
        });
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
            let (parent, id, group) = (self.parent.clone(), cx.entity_id(), self.group);
            Button::new(
                "credential-back",
                "‹ Credentials",
                HoverHelp::tooltip("Return to credentials without saving this draft"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .on_click(move |_, window, cx| Self::leave(&parent, id, group, window, cx))
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
                    |group, (kind, word)| {
                        group.option(RadioOption::new(
                            word,
                            HoverHelp::tooltip(format!("A {word} subscription login")),
                            kind,
                        ))
                    },
                )
                .chosen(Some(self.kind))
                .disabled(replacing || busy)
                .on_change(move |&kind, window, cx| {
                    let _ = weak.update(cx, |editor, cx| editor.choose_agent(kind, window, cx));
                })
        });
        let name = choices::labelled(
            "Name",
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
        );
        let secret_words = if codex { "auth.json" } else { "Token" };
        let secret = TextInput::new(
            "credential-secret",
            self.secret.input(),
            secret_words,
            HoverHelp::tooltip(if replacing {
                "Paste a new value to replace the saved one; leave it empty to keep it"
            } else {
                "Paste the value to save; Farhelm never shows it again"
            }),
        );
        let secret = choices::labelled(
            secret_words,
            "credential-secret",
            if codex {
                secret.monospace()
            } else {
                secret.height(px(choices::FIELD_HEIGHT))
            },
        );
        let choose = codex.then(|| {
            let weak = weak.clone();
            div().flex().child(
                Button::new(
                    "credential-choose-file",
                    "Choose file…",
                    HoverHelp::tooltip(
                        "Choose an auth.json file to fill the box; nothing is saved until Save",
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

        let delete = replacing.then(|| {
            let weak = weak.clone();
            Button::new(
                "credential-delete",
                "Delete",
                HoverHelp::tooltip("Delete this saved credential"),
                Role::Button,
            )
            .tier(Tier::Danger)
            .disabled(in_use)
            .busy(busy)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |editor, cx| editor.delete(cx));
            })
        });
        let cancel = {
            let (parent, id, group) = (self.parent.clone(), cx.entity_id(), self.group);
            Button::new(
                "credential-cancel",
                "Cancel",
                HoverHelp::tooltip("Discard this unsaved credential draft"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .on_click(move |_, window, cx| Self::leave(&parent, id, group, window, cx))
        };
        let save = Button::new(
            "credential-save",
            "Save",
            HoverHelp::tooltip("Save this credential in the helm's state"),
            Role::Button,
        )
        .tier(Tier::Primary)
        .disabled(!replacing && self.secret.draft().value().trim().is_empty())
        // Busy, not disabled, while saving, so a clicked Save keeps focus (SPEC_impl.md, "Drafts").
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |editor, cx| editor.save(window, cx));
        });

        div()
            .id("credential-editor")
            .role(Role::Region)
            .aria_label(title.clone())
            .flex()
            .flex_col()
            .gap(px(EDITOR_GAP))
            .w_full()
            .min_w_0()
            .child(div().flex().child(back))
            .child(
                div()
                    .id("credential-editor-title")
                    .debug_selector(|| "credential-editor-title".to_owned())
                    .role(Role::Heading)
                    .aria_label(title.clone())
                    .text_size(px(LABEL_TEXT))
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(title),
            )
            .children(agent)
            .child(name)
            .child(secret)
            .children(choose)
            .children(agent_help.into_iter().map(|help| choices::help(help, cx)))
            .child(choices::help(usage(self.existing.as_ref()), cx))
            .child(storage_warning(
                "credential-storage-warning",
                "Farhelm stores these in plain text in this helm's state, on the machine the helm runs on. Anyone \
                 who can read that folder can read them. Farhelm never shows a saved value again.",
                cx,
            ))
            .children(
                self.error()
                    .map(|reason| choices::failure_outcome("credential-error", reason, cx)),
            )
            .children(choices::busy_line(&self.refused, REFUSED, cx))
            .children(in_use.then(|| {
                choices::help(
                    "Delete is unavailable while a recipe uses this credential.",
                    cx,
                )
            }))
            // `.dialog-actions`: Delete, Cancel and Save at the editor's end, in the old order.
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .w_full()
                    .children(delete)
                    .child(cancel)
                    .child(save),
            )
            .children(busy.then(|| {
                choices::outcome_line("credential-saving", Role::Status, theme::color(palette.fg_0))
                    .aria_label("Saving…")
                    .child("Saving…")
            }))
    }
}

//! The recipe editor's credential choices: the GitHub token, and each agent's None, API endpoint or Subscription with
//! the endpoint's base URL and token and the subscription's login, all chosen by name from the helm's credential
//! catalog.
//!
//! SPEC.md, "Topology" ("The endpoint picker offers None and endpoint-token names, preserves the selected name across
//! URL changes, and points to Settings → Credentials for entering or replacing values. An unavailable draft choice
//! stays visible until the user chooses another or None; it cannot be saved as a missing reference"; "Each agent's
//! recipe choice is **None | API endpoint | Subscription** … Switching modes retains each draft but saves only the
//! selected choice"; the GitHub block). Ported from the old editor's GitHub and Agents blocks, its `CredentialPicker`
//! and `EndpointFields` (`tensorlake/recipe_editor.rs` at reference commit `24bc0580`), with their words. Here's the
//! TLDR of its rules:
//!
//! - **Names, never values.** A recipe stores only credential names; each picker offers None and the catalog's names
//!   of its kind, and nothing here sees a secret.
//! - **Every mode keeps its draft.** An agent's endpoint URL and token, its login and its mode are separate fields that
//!   switching modes never clears; the draft model sends only the selected mode's choice ([`super::draft`]).
//! - **One catalog read per opening.** The catalog is not pushed, so the editor reads `GET /api/credentials` once when
//!   it opens, as the old one loaded it on mounting. Until that answers, every picker is unavailable and "Loading
//!   credentials…" shows; a failed read shows the helm's reason with Retry loading credentials, which reads again. A
//!   credential added in Credentials while the editor stays open shows at the editor's next opening.
//! - **A missing choice stays visible.** A name the catalog does not list (deleted since, or never of that kind) stays
//!   the picker's choice, labelled "(unavailable)", with the old line saying to choose another, until the user does
//!   ([`Select::retained`]). The helm refuses a save that names a missing or wrong-kind credential, so nothing here
//!   checks the reference itself.
//!
//! NOTE: Credential names reach the pickers' labels as plain text, not host text: they are the user's own entries in
//! this helm's catalog, which the helm holds to the recipe-name grammar (lowercase letters, digits and hyphens), as the
//! Credentials section draws them.

use farhelm_helm_client::ApiPath;
use farhelm_proto::LaunchHarness;
use farhelm_proto::credentials::{Credential, CredentialKind};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{Editor, Input, caption, field, label};
use crate::components::{HoverHelp, RadioGroup, RadioOption, Select, SelectOption};
use crate::host_text::HostText;
use crate::settings::recipes::draft::{AgentMode, Draft, EndpointDraft};
use crate::settings::{
    card,
    choices::{self, LABEL_GAP},
};
use crate::theme::{self, metrics};

/// The endpoint URL's placeholder, the old one.
const URL_PLACEHOLDER: &str = "http://127.0.0.1:8080";

/// One of the two agents a recipe authenticates, as the draft model has a field set for each. Its words are the old
/// editor's, which named Claude Code in full where it labelled a field and "Claude" in the group and login names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Agent {
    Claude,
    Codex,
}

/// What one agent's block shows and how its picker's lines read.
struct AgentWords {
    /// The agent's product name ("Claude Code"), over its mode choice and in its endpoint fields' labels.
    label: &'static str,
    /// The element ids' stem ("recipe-claude").
    id: &'static str,
    /// The block's accessible name and its mode choice's.
    group: &'static str,
    mode: &'static str,
    /// The endpoint group's name, its URL field's and its token picker's labels, and the token picker's element id.
    endpoint: &'static str,
    url: &'static str,
    token: &'static str,
    token_id: &'static str,
    /// The subscription login's picker label and element id, the kind it offers and the line under it when its choice
    /// is missing.
    login: &'static str,
    login_id: &'static str,
    login_kind: CredentialKind,
    login_missing: &'static str,
    /// The help under the block.
    help: &'static str,
}

impl Agent {
    const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    /// The Settings editor uses the same mark as sessions of the agent whose sign-in it configures.
    fn harness(self) -> LaunchHarness {
        match self {
            Self::Claude => LaunchHarness::Claude,
            Self::Codex => LaunchHarness::Codex,
        }
    }

    fn words(self) -> AgentWords {
        match self {
            Agent::Claude => AgentWords {
                label: "Claude Code",
                id: "recipe-claude",
                group: "Claude authentication",
                mode: "Claude authentication mode",
                endpoint: "Claude Code endpoint",
                url: "Claude Code base URL",
                token: "Claude Code endpoint token",
                token_id: "recipe-claude-token",
                login: "Claude login",
                login_id: "recipe-claude-login",
                login_kind: CredentialKind::ClaudeLogin,
                login_missing: "The selected Claude login is unavailable. Choose another login.",
                help: "Subscription uses a Claude setup-token from Settings → Credentials. Farhelm supplies the recipe's \
                       credentials to new sessions and terminals on this host. Restart a running session to pick up a \
                       replacement.",
            },
            Agent::Codex => AgentWords {
                label: "Codex",
                id: "recipe-codex",
                group: "Codex authentication",
                mode: "Codex authentication mode",
                endpoint: "Codex endpoint",
                url: "Codex base URL",
                token: "Codex endpoint token",
                token_id: "recipe-codex-token",
                login: "Codex login",
                login_id: "recipe-codex-login",
                login_kind: CredentialKind::CodexLogin,
                login_missing: "The selected Codex login is unavailable. Choose another login.",
                help: "Farhelm supplies API endpoint credentials when it launches a Codex session. Restart a running \
                       session to pick up a replacement. Subscription uses a Codex login from Settings → Credentials. \
                       Farhelm writes auth.json when that saved login changes and leaves Codex's refreshed file alone. \
                       Switching away leaves auth.json on the host; sign out in Codex to remove it.",
            },
        }
    }

    /// The draft's fields for this agent: its endpoint, mode and login.
    fn of(self, draft: &Draft) -> (&EndpointDraft, AgentMode, &str) {
        match self {
            Agent::Claude => (
                &draft.claude,
                draft.claude_mode,
                &draft.claude_login_credential,
            ),
            Agent::Codex => (
                &draft.codex,
                draft.codex_mode,
                &draft.codex_login_credential,
            ),
        }
    }
}

/// One agent's part of the draft, as [`Choices::drafted`] reads it from the form: the editor spreads it over the
/// draft's per-agent fields.
pub(super) struct AgentDraft {
    pub(super) endpoint: EndpointDraft,
    pub(super) mode: AgentMode,
    pub(super) login: String,
}

/// One agent's drafts: its mode, and each mode's own field, all kept whatever the mode.
pub(super) struct AgentChoice {
    mode: AgentMode,
    url: Input,
    /// The endpoint token's name, empty for None.
    token: String,
    /// The subscription login's name, empty for none chosen.
    login: String,
}

/// The editor's credential choices and the catalog they are chosen from (see the module docs).
pub(super) struct Choices {
    /// The catalog as its read answered; `None` until it does.
    catalog: Option<Vec<Credential>>,
    /// Why the last read failed, until a read answers.
    unreadable: Option<HostText>,
    /// The GitHub token's name, empty for None.
    github: String,
    /// Claude's and Codex's choices.
    claude: AgentChoice,
    codex: AgentChoice,
}

impl Choices {
    /// The choices `baseline` holds, with nothing read yet.
    pub(super) fn new(baseline: &Draft, window: &mut Window, cx: &mut Context<Editor>) -> Self {
        let agent = |agent: Agent, window: &mut Window, cx: &mut Context<Editor>| {
            let (endpoint, mode, login) = agent.of(baseline);
            AgentChoice {
                mode,
                url: Input::placeholder(&endpoint.base_url, URL_PLACEHOLDER, window, cx),
                token: endpoint.token_credential.clone(),
                login: login.to_owned(),
            }
        };
        Choices {
            catalog: None,
            unreadable: None,
            github: baseline.github_token_credential.clone(),
            claude: agent(Agent::Claude, window, cx),
            codex: agent(Agent::Codex, window, cx),
        }
    }

    /// `agent`'s choices: a named field per agent, so no table order can pair an agent with the other's choices.
    fn agent(&self, agent: Agent) -> &AgentChoice {
        match agent {
            Agent::Claude => &self.claude,
            Agent::Codex => &self.codex,
        }
    }

    /// `agent`'s choices, for a pick or a mode choice to change.
    fn agent_mut(&mut self, agent: Agent) -> &mut AgentChoice {
        match agent {
            Agent::Claude => &mut self.claude,
            Agent::Codex => &mut self.codex,
        }
    }

    /// The GitHub token's name as chosen, empty for None.
    pub(super) fn github(&self) -> String {
        self.github.clone()
    }

    /// `agent`'s choices as the draft holds them: every mode's field, whatever the mode.
    pub(super) fn drafted(&self, agent: Agent, cx: &App) -> AgentDraft {
        let choice = self.agent(agent);
        AgentDraft {
            endpoint: EndpointDraft {
                base_url: choice.url.text(cx),
                token_credential: choice.token.clone(),
            },
            mode: choice.mode,
            login: choice.login.clone(),
        }
    }

    /// The endpoint URL fields, for locking them while a save is out.
    pub(super) fn inputs(&self) -> impl Iterator<Item = &Input> {
        [&self.claude.url, &self.codex.url].into_iter()
    }

    /// `agent`'s endpoint URL field, for tests that type into it.
    #[cfg(test)]
    pub(super) fn url(&self, agent: Agent) -> &Input {
        &self.agent(agent).url
    }
}

/// Which picker: the GitHub token's, or one agent's endpoint token's or login's. It says what the picker is called,
/// which kind of credential it offers, and what its choice changes.
#[derive(Clone, Copy)]
enum Pick {
    Github,
    Token(Agent),
    Login(Agent),
}

/// What a picker is called and offers, from [`Pick::words`].
struct PickWords {
    /// The picker's element id, spelled out per picker: element ids the test harness finds are `'static`.
    id: &'static str,
    label: &'static str,
    kind: CredentialKind,
    /// The line under the picker while its choice is not in the catalog.
    missing: &'static str,
}

impl Pick {
    fn words(self) -> PickWords {
        const ENDPOINT_MISSING: &str =
            "The selected endpoint credential is unavailable. Choose another or None.";
        match self {
            Pick::Github => PickWords {
                id: "recipe-github",
                label: "GitHub token",
                kind: CredentialKind::GithubToken,
                missing: "The selected GitHub credential is unavailable. Choose another or None.",
            },
            Pick::Token(agent) => {
                let words = agent.words();
                PickWords {
                    id: words.token_id,
                    label: words.token,
                    kind: CredentialKind::EndpointToken,
                    missing: ENDPOINT_MISSING,
                }
            }
            Pick::Login(agent) => {
                let words = agent.words();
                PickWords {
                    id: words.login_id,
                    label: words.login,
                    kind: words.login_kind,
                    missing: words.login_missing,
                }
            }
        }
    }
}

impl Editor {
    /// Read the credential catalog (the module docs); its answer replaces what the pickers offer.
    pub(super) fn read_credentials(&mut self, cx: &mut Context<Self>) {
        self.choices.unreadable = None;
        let view = cx.weak_entity();
        choices::read_route(
            ApiPath::api(&["credentials"]),
            "GET /api/credentials",
            cx,
            move |answer: Result<Vec<Credential>, HostText>, cx| {
                let _ = view.update(cx, |editor, cx| {
                    match answer {
                        Ok(catalog) => editor.choices.catalog = Some(catalog),
                        Err(reason) => editor.choices.unreadable = Some(reason),
                    }
                    cx.notify();
                });
            },
        );
        cx.notify();
    }

    /// A picker chose `name` (empty for None).
    fn pick(&mut self, pick: Pick, name: String, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        match pick {
            Pick::Github => self.choices.github = name,
            Pick::Token(agent) => self.choices.agent_mut(agent).token = name,
            Pick::Login(agent) => self.choices.agent_mut(agent).login = name,
        }
        cx.notify();
    }

    /// An agent's mode choice.
    fn choose_mode(&mut self, agent: Agent, mode: AgentMode, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.choices.agent_mut(agent).mode = mode;
        cx.notify();
    }

    /// One picker (`CredentialPicker`): its label over a select of None and the catalog's names of its kind, `chosen`
    /// kept visible when the catalog lacks it, and the picker's missing line under it then. Unavailable until the
    /// catalog is read and while a save is out.
    fn picker(&self, pick: Pick, chosen: &str, cx: &Context<Self>) -> AnyElement {
        let saved = match pick {
            Pick::Github => &self.baseline.github_token_credential,
            Pick::Token(agent) => &agent.of(&self.baseline).0.token_credential,
            Pick::Login(agent) => agent.of(&self.baseline).2,
        };
        let PickWords {
            id,
            label: text,
            kind,
            missing,
        } = pick.words();
        let loaded = self.choices.catalog.is_some();
        let names: Vec<&str> = self
            .choices
            .catalog
            .iter()
            .flatten()
            .filter(|credential| credential.kind == kind)
            .map(|credential| credential.name.as_str())
            .collect();
        let gone = !chosen.is_empty() && !names.contains(&chosen);
        let weak = cx.weak_entity();
        let retained = if loaded {
            format!("{chosen} (unavailable)")
        } else {
            chosen.to_owned()
        };
        let select = Select::new(id, text, HoverHelp::tooltip(text), Role::ComboBox)
            .options(
                std::iter::once(SelectOption::new(
                    "",
                    "None",
                    HoverHelp::tooltip("No credential"),
                ))
                .chain(names.iter().map(|name| {
                    SelectOption::new(
                        SharedString::from(name.to_string()),
                        SharedString::from(name.to_string()),
                        HoverHelp::tooltip("A credential saved in Settings → Credentials"),
                    )
                })),
            )
            .chosen(SharedString::from(chosen.to_owned()))
            .retained(
                retained,
                HoverHelp::tooltip("Not in Settings → Credentials; choose another or None"),
            )
            .disabled(!loaded || self.busy())
            .on_choose(move |value, _, cx| {
                let name = value.to_string();
                let _ = weak.update(cx, |editor, cx| editor.pick(pick, name, cx));
            });
        div()
            .flex()
            .flex_col()
            .gap(px(LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(label(text, self.host.has_back() && chosen != saved, cx))
            .child(select)
            .children(
                (loaded && gone)
                    .then(|| choices::failure_line(format!("{id}-missing"), missing, cx)),
            )
            .into_any_element()
    }

    /// The catalog's loading line, or its failure with Retry loading credentials, above the pickers.
    fn catalog_state(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        if let Some(reason) = &self.choices.unreadable {
            let weak = cx.weak_entity();
            let retry = choices::retry_loading(
                "recipe-credentials-retry",
                "Retry loading credentials",
                "Load the named credentials again",
                move |cx| {
                    let _ = weak.update(cx, |editor, cx| editor.read_credentials(cx));
                },
            )
            .disabled(self.busy());
            let line = choices::failure_outcome("recipe-credentials-error", reason, cx);
            return choices::failure_with_retry(line, retry).into();
        }
        if self.choices.catalog.is_none() {
            return vec![
                choices::help("Loading credentials…", cx)
                    .id("recipe-credentials-loading")
                    .role(Role::Status)
                    .into_any_element(),
            ];
        }
        Vec::new()
    }

    /// One agent's block: its name, its mode choice, and the chosen mode's fields, then its help.
    fn agent_block(&self, agent: Agent, cx: &Context<Self>) -> AnyElement {
        let words = agent.words();
        let choice = self.choices.agent(agent);
        let (saved_endpoint, saved_mode, _) = agent.of(&self.baseline);
        let weak = cx.weak_entity();
        let modes = [
            (
                AgentMode::None,
                "None",
                "Farhelm sets nothing up for this agent's sign-in",
            ),
            (
                AgentMode::Endpoint,
                "API endpoint",
                "Point the agent at an API endpoint, with an optional saved token",
            ),
            (
                AgentMode::Subscription,
                "Subscription",
                "Sign the agent in with a saved subscription login",
            ),
        ]
        .into_iter()
        .fold(
            RadioGroup::new(
                format!("{}-mode", words.id),
                words.mode,
                HoverHelp::tooltip(words.mode),
                Role::RadioGroup,
            ),
            |group, (mode, text, help)| {
                group.option(RadioOption::new(text, HoverHelp::tooltip(help), mode))
            },
        )
        .chosen(Some(choice.mode))
        .disabled(self.busy())
        .on_change(move |&mode, _, cx| {
            let _ = weak.update(cx, |editor, cx| editor.choose_mode(agent, mode, cx));
        });
        let in_settings = self.host.has_back();
        let modes = if in_settings {
            modes.small_segments().fit_words(true)
        } else {
            modes
        };
        let fields = match choice.mode {
            AgentMode::None => None,
            AgentMode::Endpoint => Some(
                div()
                    .id(SharedString::from(format!("{}-endpoint", words.id)))
                    .role(Role::Group)
                    .aria_label(words.endpoint)
                    .flex()
                    .flex_col()
                    .gap(px(LABEL_GAP))
                    .child(field(
                        words.url,
                        format!("{}-url", words.id),
                        &choice.url,
                        "The endpoint's base URL; a tunnel's sandbox port works here",
                        true,
                        self.host.has_back() && choice.url.text(cx) != saved_endpoint.base_url,
                        cx,
                    ))
                    .child(self.picker(Pick::Token(agent), &choice.token, cx))
                    .into_any_element(),
            ),
            AgentMode::Subscription => Some(self.picker(Pick::Login(agent), &choice.login, cx)),
        };
        let name = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(in_settings.then(|| card::agent_mark(agent.harness(), cx)))
            .child(
                crate::settings::editing::caption(
                    words.label,
                    (in_settings && choice.mode != saved_mode).then(|| theme::palette(cx)),
                )
                .text_size(metrics::TEXT_MD)
                .text_color(theme::color(theme::palette(cx).fg_0)),
            );
        let mode = div().flex().child(modes);
        let (name, mode) = if in_settings {
            (name.flex_wrap().child(mode), None)
        } else {
            (name, Some(mode))
        };
        div()
            .id(SharedString::from(words.id))
            .role(Role::Group)
            .aria_label(words.group)
            .flex()
            .flex_col()
            .gap(px(LABEL_GAP))
            .child(name)
            .children(mode)
            .children(fields)
            .child(choices::help(words.help, cx))
            .into_any_element()
    }

    /// The GitHub and Agents blocks, under the tunnels, with the catalog's state above them.
    pub(super) fn credential_blocks(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        let mut blocks = self.catalog_state(cx);
        let in_settings = self.host.has_back();
        let group = |title, block| {
            let heading = if in_settings {
                card::heading(title, None, None, cx)
            } else {
                caption(title, cx)
            };
            super::editor_group(heading, block, in_settings, cx).into_any_element()
        };
        blocks.push(
            group("GitHub", div()
                .flex()
                .flex_col()
                .gap(px(LABEL_GAP))
                .child(self.picker(Pick::Github, &self.choices.github, cx))
                .child(choices::help(
                    "Sessions get this token as GH_TOKEN, and git uses it for github.com over HTTPS, including fresh \
                     checkouts of private repositories. Add or replace tokens in Settings → Credentials.",
                    cx,
                ))
            ),
        );
        blocks.push(
            group("Agents", div()
                .flex()
                .flex_col()
                .gap(px(LABEL_GAP))
                .child(choices::help(
                    "Choose how each agent authenticates. An API endpoint's base URL can point at a tunnel's sandbox \
                     port, such as http://127.0.0.1:8080. Saved tokens and subscription logins are in Settings → \
                     Credentials.",
                    cx,
                ))
                .children(Agent::ALL.map(|agent| self.agent_block(agent, cx)))
                .child(choices::help(
                    "Add or replace endpoint tokens in Settings → Credentials. This recipe saves their names, not \
                     their values.",
                    cx,
                ))
            ),
        );
        blocks
    }
}

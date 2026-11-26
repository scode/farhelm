//! The launcher's composer search, without its view: what a query means, which result rows it offers in which order,
//! which row is preselected, and what the results area says when there are none. Plain functions of the draft's
//! values, tested without a window; the rows' words are [`HostSentence`]s only so host values stay escaped.
//!
//! SPEC.md, "Creation" (the launch-composer search sentences) and "Managed checkouts" (the labels paragraph). Ported
//! from the old app's `launch_composer.rs` (`scoped_query`, `search_results`, `name_host_search_results`,
//! `finish_search_results`, `empty_search_message`, `default_search_index`, `grouped_search_results`) at reference
//! commit `24bc0580`. Here's the TLDR:
//!
//! - **One choice at a time.** A query is never split into words: it offers rows, and accepting one makes one change
//!   and clears the box, so a whole setup is built word, Enter, word, Enter. A spaced phrase (`smart approve`) is
//!   matched whole.
//! - **Labels.** Only the first colon can end a label, and only the known labels count, case-insensitively: `name:`,
//!   `host:`, `harness:`, `model:`, `effort:`, `perms:`, `folder:`, `recent:`, `gh:` and `tl:`. Anything else before
//!   the first colon leaves the whole query unlabelled, so `vendor:model:v2` still finds that model.
//! - **A chosen harness scopes.** Models and efforts follow the draft's agent type; before one is chosen only YOLO and
//!   Guarded are offered as permissions, and only on the Agent tab. Effort and permission words with no harness say
//!   "Choose a harness first" rather than becoming a session name.
//! - **The name fallback.** Unlabelled text that matched nothing else offers "Set session name", so typing a name and
//!   pressing Enter names the session.
//!
//! - **Templates** are offered only under `tl:` (`tl:` alone lists them all), from the templates' own rows
//!   (`crate::templates::rows`, which the quick switcher and the Cmd+J list share), matched by their rule.
//!
//! Every row is a draft choice the view applies through the draft's own edit methods, the same ones a hand edit uses
//! (a template row through the templates' application path); this module only builds rows. The group order is the old
//! one.
//! - **From the launch history** (`super::view::history`'s reads of the chosen host's and of the helm's new
//!   environments): saved folders, the host's alone, offered after a typed path's "Use this path"; whole recent setups,
//!   last, ranked as the recent setups strip ranks them but neither capped nor narrowed by the draft, each naming where
//!   it ran; and models from past launches outside the current catalog suggestions, each with the agent type it ran
//!   with. Older known models get their own group.
//!
//! - **Repositories** are offered only under `gh:`: the choices the managed-checkout destination gives the query
//!   (`super::view::checkout`'s `repository_choices`, handed in by the view), so a valid typed `owner/repo` is always
//!   the first row. Matching repositories from the helm-wide launch history follow, in recency order. Taking one
//!   makes the destination a managed checkout of that repository, as the repository field's pick does.

use farhelm_helm_client::Collections;
use farhelm_proto::launcher::{
    EFFORT_ORDER, LaunchCatalogModel, compatible_efforts, is_catalog_family_word,
};
use farhelm_proto::session_requests::{LaunchHistoryEntry, LaunchHistoryReply};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{GithubRepo, LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::ElementId;

use super::draft::{Tab, harness_label, repo_identifier};
use super::model::OLDER_MODELS;
use super::recents::{RecentFilter, destination_label, placement, ranked, selection_summary};
use super::view::host_name_with_phase;
use super::words::{HARNESS_SEARCH_ORDER, guarded_permission_help};
use crate::host_text::{HostSentence, HostText};
use crate::permission::words::{
    DEFAULT_EFFORT_HELP, DEFAULT_EFFORT_WORD, effort_label, effort_word, permission_tooltip,
};
use crate::templates::rows::{self as template_rows, Row as TemplateRow};

// ---------------------------------------------------------------------------------------------------------------------
// The query
// ---------------------------------------------------------------------------------------------------------------------

/// Which kind of row a query asks for: every kind, or one label's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    All,
    Name,
    Host,
    Harness,
    Model,
    Effort,
    Perms,
    Folder,
    Recent,
    Github,
    Templates,
}

/// The query's scope and the value to match: the text after a known label, trimmed, or the whole trimmed query.
pub fn scoped(query: &str) -> (Scope, &str) {
    let query = query.trim();
    let Some((label, value)) = query.split_once(':') else {
        return (Scope::All, query);
    };
    let scope = [
        ("name", Scope::Name),
        ("host", Scope::Host),
        ("harness", Scope::Harness),
        ("model", Scope::Model),
        ("effort", Scope::Effort),
        ("perms", Scope::Perms),
        ("folder", Scope::Folder),
        ("recent", Scope::Recent),
        ("gh", Scope::Github),
        ("tl", Scope::Templates),
    ]
    .into_iter()
    .find(|(word, _)| label.eq_ignore_ascii_case(word))
    .map(|(_, scope)| scope);
    match scope {
        Some(scope) => (scope, value.trim()),
        None => (Scope::All, query),
    }
}

/// Whether `value` is a word that only makes sense once an agent type is chosen, asked on the Agent tab with none
/// chosen: an effort or permission label, or one of their bare words (whole words only, so a name like "highway" stays
/// a name). The words are the rows' own search words, so an effort added to the shared list is covered too.
fn needs_harness(scope: Scope, value: &str, agent_without_harness: bool) -> bool {
    if !agent_without_harness {
        return false;
    }
    match scope {
        Scope::Effort | Scope::Perms => true,
        Scope::All => {
            value.eq_ignore_ascii_case(DEFAULT_EFFORT_WORD)
                || EFFORT_ORDER
                    .iter()
                    .any(|effort| value.eq_ignore_ascii_case(&effort_word(Some(*effort))))
                || SearchPermission::ALL
                    .iter()
                    .any(|permission| value.eq_ignore_ascii_case(permission.word()))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------------------------------------------------

/// A launch permission ordered and named for search.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchPermission {
    Yolo,
    Guarded,
    Custom,
    Approve,
    SmartApprove,
    Chat,
}

impl SearchPermission {
    /// In the old search's candidate order.
    const ALL: [SearchPermission; 6] = [
        SearchPermission::Yolo,
        SearchPermission::Guarded,
        SearchPermission::Custom,
        SearchPermission::Approve,
        SearchPermission::SmartApprove,
        SearchPermission::Chat,
    ];

    /// The permission that choosing the result writes to the draft.
    pub const fn permission(self) -> LaunchPermission {
        match self {
            SearchPermission::Yolo => LaunchPermission::Yolo,
            SearchPermission::Guarded => LaunchPermission::Guarded,
            SearchPermission::Custom => LaunchPermission::Custom,
            SearchPermission::Approve => LaunchPermission::Approve,
            SearchPermission::SmartApprove => LaunchPermission::SmartApprove,
            SearchPermission::Chat => LaunchPermission::Chat,
        }
    }

    /// The word a query matches it by.
    const fn word(self) -> &'static str {
        match self {
            SearchPermission::Yolo => "yolo",
            SearchPermission::Guarded => "guarded",
            SearchPermission::Custom => "custom",
            SearchPermission::Approve => "approve",
            SearchPermission::SmartApprove => "smart approve",
            SearchPermission::Chat => "chat",
        }
    }

    /// The row's visible words after "Permissions: ".
    const fn label(self) -> &'static str {
        match self {
            SearchPermission::Yolo => "YOLO",
            SearchPermission::Guarded => "Guarded",
            SearchPermission::Custom => "Custom",
            SearchPermission::Approve => "Approve",
            SearchPermission::SmartApprove => "Smart approve",
            SearchPermission::Chat => "Chat",
        }
    }

    /// Whether `harness` offers this choice; with no harness, only YOLO and Guarded, the two a draft can hold before an
    /// agent type is chosen.
    fn offered_by(self, harness: Option<LaunchHarness>) -> bool {
        match harness {
            None => matches!(self, SearchPermission::Yolo | SearchPermission::Guarded),
            Some(harness) => harness.offers_permission(self.permission()),
        }
    }
}

/// One result row: a single draft choice.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// Set the session name to this text.
    Name(String),
    /// Run on this host (registry id). `label` is the name the host list shows (with its phase while not connected);
    /// `name` is the helm's name for it, which an exact query preselects it by.
    Host {
        id: i64,
        name: String,
        label: HostText,
    },
    Harness(LaunchHarness),
    /// A registration's ID selects it even after the display name changes.
    CustomAgent {
        id: String,
        label: HostText,
    },
    /// Apply the template of this name (as stored: host text).
    Template(String),
    /// A catalog model, with the agent type that owns it: choosing it chooses that agent type too.
    Model {
        id: String,
        harness: LaunchHarness,
        latest: bool,
    },
    /// A model from a past launch outside the current suggestions. Keep the spelling and agent it ran with.
    HistoryModel {
        id: String,
        harness: LaunchHarness,
    },
    /// An effort, or the agent's default (`None`).
    Effort(Option<LaunchEffort>),
    Permission(SearchPermission),
    /// Use the typed path as the folder.
    UsePath(String),
    /// Use the typed path as the folder and list the folders under it on the host (`super::view::browse`).
    BrowsePath(String),
    /// A folder an earlier launch on this host started in, as the user typed it then.
    Folder(String),
    /// A whole earlier setup, from the chosen host's history or the helm's of new environments, with where it ran as
    /// the row names it (`super::recents::placement`).
    Recent {
        entry: Box<LaunchHistoryEntry>,
        placement: HostText,
    },
    /// Start in a managed checkout of this repository (only under `gh:`).
    Github(GithubRepo),
}

/// The groups rows are shown in, in this order; an empty group is left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Names,
    Hosts,
    Harnesses,
    CustomAgents,
    Shells,
    Templates,
    Models,
    OlderModels,
    Efforts,
    Permissions,
    Folders,
    Repositories,
    RecentSetups,
}

impl Group {
    /// The group's heading and accessible name, the old app's.
    pub const fn label(self) -> &'static str {
        match self {
            Group::Names => "Session name",
            Group::Hosts => "Hosts",
            Group::Harnesses => "Harnesses",
            Group::CustomAgents => "Custom agents",
            Group::Shells => "Shells",
            Group::Templates => "Templates",
            Group::Models => "Models",
            Group::OlderModels => OLDER_MODELS,
            Group::Efforts => "Efforts",
            Group::Permissions => "Permissions",
            Group::Folders => "Folders",
            Group::Repositories => "GitHub repositories",
            Group::RecentSetups => "Recent setups",
        }
    }
}

impl Row {
    /// Which group the row is listed under.
    pub const fn group(&self) -> Group {
        match self {
            Row::Name(_) => Group::Names,
            Row::Host { .. } => Group::Hosts,
            Row::Harness(harness) if harness.is_shell() => Group::Shells,
            Row::Harness(_) => Group::Harnesses,
            Row::CustomAgent { .. } => Group::CustomAgents,
            Row::Template(_) => Group::Templates,
            Row::Model { latest: false, .. } => Group::OlderModels,
            Row::Model { latest: true, .. } | Row::HistoryModel { .. } => Group::Models,
            Row::Effort(_) => Group::Efforts,
            Row::Permission(_) => Group::Permissions,
            Row::UsePath(_) | Row::BrowsePath(_) | Row::Folder(_) => Group::Folders,
            Row::Recent { .. } => Group::RecentSetups,
            Row::Github(_) => Group::Repositories,
        }
    }

    /// The row's visible words, which are also its accessible name (the old app's row text): "Set session name: " and
    /// the name, "Host: " and the host as the host list names it, "Model: " and the id with its agent type. A past
    /// launch is labelled so it is distinct from the same model in the catalog. Host-supplied values are parts of
    /// the sentence, so they are drawn and named escaped.
    pub fn words(&self, id: impl Into<ElementId>, collections: &Collections) -> HostSentence {
        let sentence = HostSentence::new(id);
        match self {
            Row::Name(name) => sentence
                .text("Set session name: ")
                .host(&HostText::from_host(name.as_str())),
            Row::Host { label, .. } => sentence.text("Host: ").host(label),
            Row::Harness(harness) => {
                let label = if harness.is_shell() {
                    "Shell"
                } else {
                    "Harness"
                };
                sentence.text(format!("{label}: {}", harness_label(*harness)))
            }
            Row::CustomAgent { id, label } => sentence
                .host(label)
                .text(" · ")
                .mono(&HostText::from_host(id.as_str())),
            Row::Template(name) => sentence
                .text("Template: ")
                .host(&HostText::from_host(name.as_str())),
            Row::Model { id, harness, .. } | Row::HistoryModel { id, harness } => {
                let suffix = if matches!(self, Row::HistoryModel { .. }) {
                    ", past launch"
                } else {
                    ""
                };
                sentence
                    .text("Model: ")
                    .host(&HostText::from_host(id.as_str()))
                    .text(format!(" ({}{suffix})", harness_label(*harness)))
            }
            Row::Effort(effort) => sentence.text(format!("Effort: {}", effort_label(*effort))),
            Row::Permission(permission) => {
                sentence.text(format!("Permissions: {}", permission.label()))
            }
            // A saved folder reads as a typed path does, as in the old launcher.
            Row::UsePath(path) | Row::Folder(path) => sentence
                .text("Use this path: ")
                .mono(&HostText::from_host(path.as_str())),
            Row::BrowsePath(path) => sentence
                .text("Browse this path: ")
                .mono(&HostText::from_host(path.as_str())),
            // The whole setup, as the old row's accessible name said it; the view draws it on two lines.
            // The choices are host text too: a custom model id is whatever the user typed.
            Row::Recent { entry, placement } => sentence
                .text("Recent setup: ")
                .host(&HostText::from_host(destination_label(entry)))
                .text(" · ")
                .host(placement)
                .text(" · ")
                .host(&HostText::from_host(selection_summary(
                    &entry.selection,
                    collections,
                ))),
            Row::Github(repo) => sentence
                .text("Managed checkout: ")
                .host(&HostText::from_host(repo_identifier(repo))),
        }
    }

    /// The row's hover help, the old app's (`search_result_tooltip`). A Guarded row explains the chosen agent's guarded
    /// mode, or says to choose an agent first; a name row repeats the name, escaped like the row itself.
    pub fn help(
        &self,
        harness: Option<LaunchHarness>,
        id: impl Into<ElementId>,
        collections: &Collections,
    ) -> HostSentence {
        let sentence = HostSentence::new(id);
        let words = match self {
            Row::Name(name) => {
                return sentence
                    .text("Use \"")
                    .host(&HostText::from_host(name.as_str()))
                    .text("\" as the session's name");
            }
            Row::Host { .. } => "Run the session on this host".to_owned(),
            Row::Harness(harness) if harness.is_shell() => {
                format!("Start a {} shell", harness_label(*harness))
            }
            Row::Harness(_) => "Use this agent for the session".to_owned(),
            Row::CustomAgent { .. } => "Use this registered agent for the session".to_owned(),
            Row::Template(_) => "Apply this template's settings to the draft".to_owned(),
            Row::Model { .. } | Row::HistoryModel { .. } => {
                "Use this model for the session".to_owned()
            }
            Row::Effort(Some(_)) => "Use this reasoning effort for the session".to_owned(),
            Row::Effort(None) => DEFAULT_EFFORT_HELP.to_owned(),
            Row::Permission(permission) => match permission.permission() {
                LaunchPermission::Guarded => guarded_permission_help(harness),
                permission => permission_tooltip(permission).to_owned(),
            },
            Row::UsePath(_) | Row::Folder(_) => {
                "Use this folder: the session will start here".to_owned()
            }
            Row::BrowsePath(_) => "List the folders under this path".to_owned(),
            Row::Github(_) => "Start in a managed checkout of this repository".to_owned(),
            // The setup without its "Recent setup:" lead, as the old row's tooltip said it.
            Row::Recent { entry, placement } => {
                return sentence
                    .host(&HostText::from_host(destination_label(entry)))
                    .text(" · ")
                    .host(placement)
                    .text(" · ")
                    .host(&HostText::from_host(selection_summary(
                        &entry.selection,
                        collections,
                    )));
            }
        };
        sentence.text(words)
    }

    /// The word an exact query preselects the row by, if it can be preselected.
    fn exact_word(&self) -> Option<String> {
        match self {
            Row::Harness(harness) => Some(harness_word(*harness)),
            Row::CustomAgent { id, .. } => Some(id.clone()),
            // Unicode lowercasing, as template names are matched (`crate::templates::rows::matches`).
            Row::Template(name) => Some(name.to_lowercase()),
            Row::Model { id, .. } | Row::HistoryModel { id, .. } => Some(id.to_ascii_lowercase()),
            Row::Effort(effort) => Some(effort_word(*effort)),
            Row::Permission(permission) => Some(permission.word().to_owned()),
            Row::Host { name, .. } => Some(name.to_ascii_lowercase()),
            Row::Name(_)
            | Row::UsePath(_)
            | Row::BrowsePath(_)
            | Row::Folder(_)
            | Row::Recent { .. }
            | Row::Github(_) => None,
        }
    }
}

/// A harness's search word: its label, lowercased (`opencode`, not the protocol's `open_code`).
fn harness_word(harness: LaunchHarness) -> String {
    harness_label(harness).to_ascii_lowercase()
}

/// Whether `value` looks like a path, which offers "Use this path": it starts with `/`, `~` or `.`, or holds a `/`.
fn path_shaped(value: &str) -> bool {
    value.starts_with(['/', '~', '.']) || value.contains('/')
}

// ---------------------------------------------------------------------------------------------------------------------
// The search
// ---------------------------------------------------------------------------------------------------------------------

/// What the search reads besides the query: the draft's tab, agent type and model, the model catalog, and the
/// registry's hosts.
pub struct Inputs<'a> {
    pub query: &'a str,
    pub tab: Tab,
    pub harness: Option<LaunchHarness>,
    pub model: Option<&'a str>,
    /// The catalog's models, or `None` while there are none to offer: its read is out, or it failed.
    pub catalog: Option<&'a [LaunchCatalogModel]>,
    pub custom_agents: &'a [&'a farhelm_proto::custom_agents::CustomAgent],
    /// Whether the catalog's read is still out, which a `model:` query with no rows says ("Loading models…") instead
    /// of warning that nothing matched. A failed read is not pending: there is nothing to wait for, so it warns
    /// (SPEC.md allows the loading reason only for a pending list).
    pub catalog_pending: bool,
    /// Every host the registry lists; the search leaves out session-owned ones itself, as the old one did.
    pub hosts: &'a [HostView],
    /// Every template as the templates' rows give it (`crate::templates::rows::rows` with an empty filter: the store's
    /// templates in the helm's order), or why the templates cannot be listed, which a `tl:` query shows in place of rows.
    pub templates: Result<&'a [TemplateRow], &'a HostSentence>,
    /// The recent setups' history (`super::recents::merged`): the chosen host's while it is read and current, with the
    /// helm's launches into new environments; `None` with neither, which offers no saved folders, recent setups or
    /// past models.
    pub history: Option<&'a LaunchHistoryReply>,
    /// The chosen host as the rows of its own history name it; a new environment's setup names its own placement
    /// instead (`super::recents::placement`).
    pub host: Option<&'a HostText>,
    /// A `gh:` query's repository choices, filtered from the launcher's helm-wide history.
    pub repositories: &'a [GithubRepo],
    /// The history read's state gives a `gh:` query the right explanation when there are no choices.
    pub repository_history: &'a RepositoryHistory,
}

/// The launcher's single repository history read. A failure has no suggestions but has its own visible status.
pub enum RepositoryHistory {
    /// The UI connection has not yet accepted the read. Drawing can try to send it again.
    NotAsked,
    /// The read is out; an empty list means "not yet", not "no matches".
    Loading,
    /// The helm's repositories from past launches, possibly none.
    Loaded(Vec<GithubRepo>),
    /// The read failed. It is not retried while this launcher is open (one read per opening), and the helm's own
    /// words are not shown: the user only needs to know that typing owner/repo still works.
    Failed,
}

/// A line the results area shows when there are no rows, and whether it is a warning (drawn in the warning color) or a
/// hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub text: &'static str,
    pub warning: bool,
}

/// What a query offers: its rows in group order (grouped as listed), the row preselected, and the line shown instead
/// when there are none.
#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    pub rows: Vec<Row>,
    /// The index into `rows` preselected: the first exact match by the query's priority, else the first row.
    pub default: usize,
    pub message: Option<Message>,
    /// Why a `tl:` query has no rows when the helm reports its templates unavailable: the old launcher's "Couldn't
    /// load templates:" with the helm's words, shown in the results area in place of [`Results::message`].
    pub reason: Option<HostSentence>,
}

impl Results {
    /// The rows grouped under their headings, in order, for drawing.
    pub fn groups(&self) -> Vec<(Group, Vec<(usize, &Row)>)> {
        let mut groups: Vec<(Group, Vec<(usize, &Row)>)> = Vec::new();
        for (index, row) in self.rows.iter().enumerate() {
            match groups.last_mut() {
                Some((group, rows)) if *group == row.group() => rows.push((index, row)),
                _ => groups.push((row.group(), vec![(index, row)])),
            }
        }
        groups
    }
}

/// The rows `inputs` offer (see the module docs).
pub fn search(inputs: &Inputs<'_>) -> Results {
    let (scope, value) = scoped(inputs.query);
    let agent_tab = inputs.tab == Tab::Agent;
    // Models and efforts follow the agent type only on the Agent tab; the Command tab searches every agent type's
    // models.
    let harness = inputs.harness.filter(|_| agent_tab);
    let agent_without_harness = agent_tab && inputs.harness.is_none();
    let family_word = is_catalog_family_word(value);
    // A word such as `grok` has two meanings. An unset agent gives the agent name priority. With an agent set,
    // use model scope so a missing model cannot switch agents or become a session name.
    let colliding_harness = (scope == Scope::All && family_word)
        .then(|| {
            HARNESS_SEARCH_ORDER
                .into_iter()
                .find(|kind| harness_label(*kind).eq_ignore_ascii_case(value))
        })
        .flatten();
    let (scope, preferred_harness) = match (colliding_harness, harness) {
        (Some(_), Some(_)) => (Scope::Model, None),
        (preferred, _) => (scope, preferred),
    };
    // Template names compare by Unicode lowercase, as they are matched; every other row by ASCII lowercase.
    let lower = match scope {
        Scope::Templates => value.to_lowercase(),
        _ => value.to_ascii_lowercase(),
    };
    let mut rows = Vec::new();
    let any = |wanted: Scope| scope == wanted || scope == Scope::All;
    let matches = |word: &str| lower.is_empty() || word.contains(&lower);
    let blank_all = scope == Scope::All && value.is_empty();
    if any(Scope::Model) && inputs.catalog.is_none() && preferred_harness.is_none() && family_word {
        // An exact family word waits for the release catalog. A history or name row could consume it first.
        let pending = inputs.catalog_pending;
        return Results {
            rows,
            default: 0,
            message: empty_message(
                Scope::Model,
                value,
                false,
                pending,
                inputs.repository_history,
                false,
            ),
            reason: None,
        };
    }

    if scope == Scope::Name && !value.is_empty() {
        rows.push(Row::Name(value.to_owned()));
    }
    if scope == Scope::Host {
        for host in inputs.hosts.iter().filter(|host| !host.is_session_owned) {
            let label = host_name_with_phase(host);
            let offered = if value.eq_ignore_ascii_case("local") {
                host.is_local()
            } else {
                matches(&host.name.to_ascii_lowercase())
                    || matches(&label.raw().to_ascii_lowercase())
            };
            if offered {
                rows.push(Row::Host {
                    id: host.id,
                    name: host.name.clone(),
                    label,
                });
            }
        }
    }
    if any(Scope::Harness) && !blank_all {
        rows.extend(
            HARNESS_SEARCH_ORDER
                .iter()
                .filter(|harness| matches(&harness_word(**harness)))
                .map(|harness| Row::Harness(*harness)),
        );
        let unicode = value.to_lowercase();
        rows.extend(
            inputs
                .custom_agents
                .iter()
                .filter(|agent| {
                    matches(&agent.id) || agent.display_name.to_lowercase().contains(&unicode)
                })
                .map(|agent| Row::CustomAgent {
                    id: agent.id.clone(),
                    label: HostText::from_host(agent.display_name.as_str()),
                }),
        );
    }
    // Templates only under `tl:`: an unlabelled query is about the launch being built, and template names there would
    // crowd out its agent types and models (the old search's rule).
    let mut reason = None;
    if scope == Scope::Templates {
        match inputs.templates {
            Ok(templates) => rows.extend(
                templates
                    .iter()
                    .filter(|template| template_rows::matches(value, &template.name))
                    .map(|template| Row::Template(template.name.clone())),
            ),
            Err(unavailable) => reason = Some(unavailable.clone()),
        }
    }
    if any(Scope::Model) && !blank_all {
        rows.extend(
            inputs
                .catalog
                .unwrap_or_default()
                .iter()
                .filter(|model| harness.is_none_or(|chosen| chosen == model.harness))
                .filter(|model| {
                    matches(&model.id.to_ascii_lowercase()) || model.matches_family_word(value)
                })
                .map(|model| Row::Model {
                    id: model.id.clone(),
                    harness: model.harness,
                    latest: model.latest,
                }),
        );
        // Retain past-launch matches for models outside the current suggestions, including recognized older IDs.
        // A current suggestion already covers its own history match (including an alternate spelling).
        if let Some(history) = inputs.history {
            let catalog = inputs.catalog.unwrap_or_default();
            for entry in ranked(history, &RecentFilter::default(), None) {
                let Some(id) = &entry.selection.model else {
                    continue;
                };
                let owner = entry.selection.harness;
                // A custom model belongs to its registration. The complete recent setup keeps that ID.
                if owner.uses_custom_catalog() {
                    continue;
                }
                let suggested = catalog.iter().any(|model| {
                    model.latest && model.harness == owner && model.id == owner.catalog_model_id(id)
                });
                let row = Row::HistoryModel {
                    id: id.clone(),
                    harness: owner,
                };
                if harness.is_none_or(|chosen| chosen == owner)
                    && matches(&id.to_ascii_lowercase())
                    && !suggested
                    && !rows.contains(&row)
                {
                    rows.push(row);
                }
            }
        }
    }
    if any(Scope::Effort)
        && !blank_all
        && let Some(harness) = harness
    {
        if scope == Scope::Effort && harness.offers_effort() && matches(DEFAULT_EFFORT_WORD) {
            rows.push(Row::Effort(None));
        }
        let efforts = compatible_efforts(harness, inputs.model, inputs.catalog.unwrap_or_default());
        rows.extend(
            efforts
                .into_iter()
                .filter(|effort| matches(&effort_word(Some(*effort))))
                .map(|effort| Row::Effort(Some(effort))),
        );
    }
    if any(Scope::Perms) && !blank_all && (harness.is_some() || agent_without_harness) {
        rows.extend(
            SearchPermission::ALL
                .into_iter()
                .filter(|permission| permission.offered_by(harness))
                .filter(|permission| match scope {
                    // A labelled query matches by the word's start; an unlabelled one offers only YOLO and Guarded,
                    // anywhere in the word.
                    Scope::Perms => permission.word().starts_with(&lower),
                    _ => {
                        matches!(
                            permission,
                            SearchPermission::Yolo | SearchPermission::Guarded
                        ) && permission.word().contains(&lower)
                    }
                })
                .map(Row::Permission),
        );
    }
    // A typed path offers to be used as it is, and to be browsed; typing itself never reaches the host.
    if any(Scope::Folder) && !blank_all && path_shaped(value) {
        rows.push(Row::UsePath(value.to_owned()));
        rows.push(Row::BrowsePath(value.to_owned()));
    }
    if let Some(history) = inputs.history {
        // Saved folders, matched on the folder as typed (so a visible `<U+…>` never stands for the character it
        // escapes), uncapped, after a typed path's own row.
        if any(Scope::Folder) && !blank_all {
            rows.extend(
                history
                    .folders
                    .iter()
                    .filter(|folder| matches(&folder.display_cwd.to_ascii_lowercase()))
                    .map(|folder| Row::Folder(folder.display_cwd.clone())),
            );
        }
        // A recent setup keeps the registration ID even after a rename or deletion. Search the current display
        // name too, since that is what the user sees in the row.
        if any(Scope::Recent) && !blank_all {
            let unicode = value.to_lowercase();
            rows.extend(
                ranked(history, &RecentFilter::default(), None)
                    .into_iter()
                    .filter(|entry| {
                        if entry.selection.custom_agent.as_deref().is_some_and(|id| {
                            matches(id)
                                || inputs.custom_agents.iter().any(|agent| {
                                    agent.id == id
                                        && agent.display_name.to_lowercase().contains(&unicode)
                                })
                        }) {
                            return true;
                        }
                        let words = format!(
                            "{} {} {}",
                            destination_label(entry),
                            harness_label(entry.selection.harness),
                            entry.selection.model.as_deref().unwrap_or_default()
                        );
                        matches(&words.to_ascii_lowercase())
                    })
                    .filter_map(|entry| {
                        Some(Row::Recent {
                            placement: placement(entry, inputs.host, inputs.hosts)?,
                            entry: Box::new(entry.clone()),
                        })
                    }),
            );
        }
    }

    if scope == Scope::Github {
        rows.extend(inputs.repositories.iter().cloned().map(Row::Github));
    }

    // The name fallback: unlabelled text that offered nothing else names the session, unless it is a word that needs an
    // agent type first.
    let needs_harness = needs_harness(scope, value, agent_without_harness);
    if rows.is_empty() && scope == Scope::All && !value.is_empty() && !needs_harness {
        rows.push(Row::Name(value.to_owned()));
    }
    // The rows above are already built in group order. The sort is a guard: whatever order a later kind of row is
    // added in, the rows still come out in group order, and the stable sort keeps each group's rows as built.
    rows.sort_by_key(|row| row.group() as u8);
    let family = inputs.catalog.unwrap_or_default().iter().find(|model| {
        harness.is_none_or(|chosen| chosen == model.harness) && model.matches_family_word(value)
    });
    let default = default_index(scope, &lower, &rows, family, preferred_harness);
    let message = if rows.is_empty() && reason.is_none() {
        empty_message(
            scope,
            value,
            needs_harness,
            inputs.catalog_pending,
            inputs.repository_history,
            inputs.query.is_empty(),
        )
    } else {
        None
    };
    Results {
        rows,
        default,
        message,
        reason,
    }
}

/// Under a label, prefer an exact match in that label's group. An unlabelled exact agent word can choose that agent
/// before a model of the same name when no agent is set. Otherwise, retain the ordinary per-group precedence.
/// A past launch can match an ID, while a family word only selects a current catalog row.
fn default_index(
    scope: Scope,
    lower: &str,
    rows: &[Row],
    family: Option<&LaunchCatalogModel>,
    preferred_harness: Option<LaunchHarness>,
) -> usize {
    if lower.is_empty() {
        return 0;
    }
    if let Some(harness) = preferred_harness
        && let Some(index) = rows.iter().position(|row| *row == Row::Harness(harness))
    {
        return index;
    }
    let priority: &[Group] = match scope {
        Scope::All => &[
            // The documented bare shell words must stay shortcuts even if
            // the launch history has a custom model with the same name.
            Group::Shells,
            Group::CustomAgents,
            Group::Efforts,
            Group::Permissions,
            Group::Models,
            Group::OlderModels,
            Group::Harnesses,
        ],
        Scope::Host => &[Group::Hosts],
        Scope::Harness => &[Group::Harnesses, Group::CustomAgents, Group::Shells],
        Scope::Model => &[Group::Models, Group::OlderModels],
        Scope::Effort => &[Group::Efforts],
        Scope::Perms => &[Group::Permissions],
        Scope::Templates => &[Group::Templates],
        Scope::Name | Scope::Folder | Scope::Recent | Scope::Github => &[],
    };
    priority
        .iter()
        .find_map(|group| {
            rows.iter().position(|row| {
                row.group() == *group
                    && (row.exact_word().is_some_and(|word| word == lower)
                        || matches!(row, Row::Model { id, harness, latest: true } if
                            family.is_some_and(|model| model.harness == *harness && model.id == *id)))
            })
        })
        .unwrap_or(0)
}

/// The line shown when a query offers no rows (the old `empty_search_message`); `None` for an empty box.
fn empty_message(
    scope: Scope,
    value: &str,
    needs_harness: bool,
    catalog_pending: bool,
    repository_history: &RepositoryHistory,
    box_empty: bool,
) -> Option<Message> {
    if box_empty {
        return None;
    }
    let (text, warning) = if needs_harness {
        ("Choose a harness first", true)
    } else {
        match scope {
            Scope::All => ("Type a search or clear the search box to launch", false),
            Scope::Name => ("Type a session name after name:", false),
            Scope::Host => ("No host matches", true),
            Scope::Harness => ("No harness matches", true),
            Scope::Model if catalog_pending => ("Loading models…", false),
            Scope::Model => (
                "No model matches; enter a custom model ID in the model field",
                true,
            ),
            Scope::Effort => ("No effort matches", true),
            Scope::Perms => ("No permissions match", true),
            Scope::Folder => ("No folder matches", true),
            Scope::Recent => ("No recent setup matches", true),
            Scope::Github => return Some(repository_empty_message(value, repository_history)),
            Scope::Templates => ("No template matches", true),
        }
    };
    Some(Message { text, warning })
}

/// A failed read is visible on either repository surface. The pairs the user can type remain separate from history.
pub(super) fn repository_empty_message(query: &str, history: &RepositoryHistory) -> Message {
    use RepositoryHistory::*;
    let (text, warning) = match (query.trim().is_empty(), history) {
        (_, Failed) => ("Couldn't load recent repositories; enter owner/repo", true),
        (true, _) | (_, NotAsked) => ("Type owner/repo to choose a repository", false),
        (false, Loading) => ("Loading repositories…", false),
        (false, Loaded(_)) => ("No repository matches; enter owner/repo", true),
    };
    Message { text, warning }
}

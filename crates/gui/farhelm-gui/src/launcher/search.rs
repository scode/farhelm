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
//! Every row is a draft choice the view applies through the draft's own edit methods, the same ones a hand edit uses;
//! this module only builds rows. The group order is the old one with the groups not built yet left out (Templates
//! between Harnesses and Models, then GitHub repositories and Recent setups after Folders), so each later piece adds
//! its group in its place.
//!
//! NOTE: Not built yet, each with the piece that brings its data: folder-history and recent-setup rows and models from
//! past launches (the launch-history read), "Browse this path" (Browse), `gh:` repositories (managed checkouts) and
//! `tl:` templates (the templates work, which adds its rows to [`search`]). Their labels are parsed already, so a
//! scoped query for them says it has no match rather than becoming a name.

use farhelm_proto::launcher::{EFFORT_ORDER, LaunchCatalogModel, compatible_efforts};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission};
use gpui::ElementId;

use super::draft::{Tab, harness_label};
use super::view::host_name_with_phase;
use super::words::{HARNESS_SEARCH_ORDER, guarded_permission_help};
use crate::host_text::{HostSentence, HostText};
use crate::permission::words::{effort_label, permission_tooltip};

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
            value.eq_ignore_ascii_case(&effort_word(None))
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

/// A permission choice as the search offers it: the launch permissions plus the agent's own setting (no permission).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchPermission {
    Yolo,
    Guarded,
    OwnSetting,
    Custom,
    Approve,
    SmartApprove,
    Chat,
}

impl SearchPermission {
    /// In the old search's candidate order.
    const ALL: [SearchPermission; 7] = [
        SearchPermission::Yolo,
        SearchPermission::Guarded,
        SearchPermission::OwnSetting,
        SearchPermission::Custom,
        SearchPermission::Approve,
        SearchPermission::SmartApprove,
        SearchPermission::Chat,
    ];

    /// The draft's permission for this choice: `None` for the agent's own setting.
    pub const fn permission(self) -> Option<LaunchPermission> {
        match self {
            SearchPermission::Yolo => Some(LaunchPermission::Yolo),
            SearchPermission::Guarded => Some(LaunchPermission::Guarded),
            SearchPermission::OwnSetting => None,
            SearchPermission::Custom => Some(LaunchPermission::Custom),
            SearchPermission::Approve => Some(LaunchPermission::Approve),
            SearchPermission::SmartApprove => Some(LaunchPermission::SmartApprove),
            SearchPermission::Chat => Some(LaunchPermission::Chat),
        }
    }

    /// The word a query matches it by.
    const fn word(self) -> &'static str {
        match self {
            SearchPermission::Yolo => "yolo",
            SearchPermission::Guarded => "guarded",
            SearchPermission::OwnSetting => "own",
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
            SearchPermission::OwnSetting => "Agent's own setting",
            SearchPermission::Custom => "Custom",
            SearchPermission::Approve => "Approve",
            SearchPermission::SmartApprove => "Smart approve",
            SearchPermission::Chat => "Chat",
        }
    }

    /// Whether `harness` offers this choice; with no harness, only YOLO and Guarded, the two a draft can hold before an
    /// agent type is chosen.
    fn offered_by(self, harness: Option<LaunchHarness>) -> bool {
        match (harness, self.permission()) {
            (None, _) => matches!(self, SearchPermission::Yolo | SearchPermission::Guarded),
            // The agent's own setting is a choice only where omitting the permission is not itself a mode.
            (Some(harness), None) => harness.omitted_permission().is_none(),
            (Some(harness), Some(permission)) => harness.offers_permission(permission),
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
    /// A catalog model, with the agent type that owns it: choosing it chooses that agent type too.
    Model {
        id: String,
        harness: LaunchHarness,
    },
    /// An effort, or the agent's default (`None`).
    Effort(Option<LaunchEffort>),
    Permission(SearchPermission),
    /// Use the typed path as the folder.
    UsePath(String),
}

/// The groups rows are shown in, in this order; an empty group is left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Names,
    Hosts,
    Harnesses,
    Models,
    Efforts,
    Permissions,
    Folders,
}

impl Group {
    /// The group's heading and accessible name, the old app's.
    pub const fn label(self) -> &'static str {
        match self {
            Group::Names => "Session name",
            Group::Hosts => "Hosts",
            Group::Harnesses => "Harnesses",
            Group::Models => "Models",
            Group::Efforts => "Efforts",
            Group::Permissions => "Permissions",
            Group::Folders => "Folders",
        }
    }
}

impl Row {
    /// Which group the row is listed under.
    pub const fn group(&self) -> Group {
        match self {
            Row::Name(_) => Group::Names,
            Row::Host { .. } => Group::Hosts,
            Row::Harness(_) => Group::Harnesses,
            Row::Model { .. } => Group::Models,
            Row::Effort(_) => Group::Efforts,
            Row::Permission(_) => Group::Permissions,
            Row::UsePath(_) => Group::Folders,
        }
    }

    /// The row's visible words, which are also its accessible name (the old app's row text): "Set session name: " and
    /// the name, "Host: " and the host as the host list names it, "Model: " and the id with its agent type, "Use this
    /// path: " and the path in the monospace face; the rest are the app's own words. Host-supplied values are parts of
    /// the sentence, so they are drawn and named escaped.
    pub fn words(&self, id: impl Into<ElementId>) -> HostSentence {
        let sentence = HostSentence::new(id);
        match self {
            Row::Name(name) => sentence
                .text("Set session name: ")
                .host(&HostText::from_host(name.as_str())),
            Row::Host { label, .. } => sentence.text("Host: ").host(label),
            Row::Harness(harness) => sentence.text(format!("Harness: {}", harness_label(*harness))),
            Row::Model { id, harness } => sentence
                .text("Model: ")
                .host(&HostText::from_host(id.as_str()))
                .text(format!(" ({})", harness_label(*harness))),
            Row::Effort(effort) => sentence.text(format!("Effort: {}", effort_label(*effort))),
            Row::Permission(permission) => {
                sentence.text(format!("Permissions: {}", permission.label()))
            }
            Row::UsePath(path) => sentence
                .text("Use this path: ")
                .mono(&HostText::from_host(path.as_str())),
        }
    }

    /// The row's hover help, the old app's (`search_result_tooltip`). A Guarded row explains the chosen agent's guarded
    /// mode, or says to choose an agent first; a name row repeats the name, escaped like the row itself.
    pub fn help(&self, harness: Option<LaunchHarness>, id: impl Into<ElementId>) -> HostSentence {
        let sentence = HostSentence::new(id);
        let words = match self {
            Row::Name(name) => {
                return sentence
                    .text("Use \"")
                    .host(&HostText::from_host(name.as_str()))
                    .text("\" as the session's name");
            }
            Row::Host { .. } => "Run the session on this host".to_owned(),
            Row::Harness(_) => "Use this agent for the session".to_owned(),
            Row::Model { .. } => "Use this model for the session".to_owned(),
            Row::Effort(Some(_)) => "Use this reasoning effort for the session".to_owned(),
            Row::Effort(None) => "Use the agent's default reasoning effort".to_owned(),
            Row::Permission(permission) => match permission.permission() {
                None => {
                    "Use the agent's own setting, which Farhelm cannot force or classify".to_owned()
                }
                Some(LaunchPermission::Guarded) => guarded_permission_help(harness),
                Some(permission) => permission_tooltip(permission).to_owned(),
            },
            Row::UsePath(_) => "Use this folder: the session will start here".to_owned(),
        };
        sentence.text(words)
    }

    /// The word an exact query preselects the row by, if it can be preselected.
    fn exact_word(&self) -> Option<String> {
        match self {
            Row::Harness(harness) => Some(harness_word(*harness)),
            Row::Model { id, .. } => Some(id.to_ascii_lowercase()),
            Row::Effort(effort) => Some(effort_word(*effort)),
            Row::Permission(permission) => Some(permission.word().to_owned()),
            Row::Host { name, .. } => Some(name.to_ascii_lowercase()),
            Row::Name(_) | Row::UsePath(_) => None,
        }
    }
}

/// A harness's search word: its label, lowercased (`opencode`, not the protocol's `open_code`).
fn harness_word(harness: LaunchHarness) -> String {
    harness_label(harness).to_ascii_lowercase()
}

/// An effort's search word: its value, lowercased; `default` for the agent's default.
fn effort_word(effort: Option<LaunchEffort>) -> String {
    match effort {
        Some(_) => effort_label(effort).to_ascii_lowercase(),
        None => "default".to_owned(),
    }
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
    /// Whether the catalog's read is still out, which a `model:` query with no rows says ("Loading models…") instead
    /// of warning that nothing matched. A failed read is not pending: there is nothing to wait for, so it warns
    /// (SPEC.md allows the loading reason only for a pending list).
    pub catalog_pending: bool,
    /// Every host the registry lists; the search leaves out session-owned ones itself, as the old one did.
    pub hosts: &'a [HostView],
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
    let lower = value.to_ascii_lowercase();
    let agent_tab = inputs.tab == Tab::Agent;
    // Models and efforts follow the agent type only on the Agent tab; the Command tab searches every agent type's
    // models.
    let harness = inputs.harness.filter(|_| agent_tab);
    let agent_without_harness = agent_tab && inputs.harness.is_none();
    let mut rows = Vec::new();
    let any = |wanted: Scope| scope == wanted || scope == Scope::All;
    let matches = |word: &str| lower.is_empty() || word.contains(&lower);
    let blank_all = scope == Scope::All && value.is_empty();

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
    }
    if any(Scope::Model) && !blank_all {
        rows.extend(
            inputs
                .catalog
                .unwrap_or_default()
                .iter()
                .filter(|model| harness.is_none_or(|chosen| chosen == model.harness))
                .filter(|model| matches(&model.id.to_ascii_lowercase()))
                .map(|model| Row::Model {
                    id: model.id.clone(),
                    harness: model.harness,
                }),
        );
    }
    if any(Scope::Effort)
        && !blank_all
        && let Some(harness) = harness
    {
        if scope == Scope::Effort && harness.offers_effort() && matches("default") {
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
    if any(Scope::Folder) && !blank_all && path_shaped(value) {
        rows.push(Row::UsePath(value.to_owned()));
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
    let default = default_index(scope, &lower, &rows);
    let message = if rows.is_empty() {
        empty_message(
            scope,
            value,
            needs_harness,
            inputs.catalog_pending,
            inputs.query.is_empty(),
        )
    } else {
        None
    };
    Results {
        rows,
        default,
        message,
    }
}

/// The row preselected for an exact query: under a label, the first exact match in that label's own group; unlabelled,
/// an exact effort, then permission, then model, then agent type (so `medium` lands on the effort, not a model named
/// `medium-context`). Else the first row.
fn default_index(scope: Scope, lower: &str, rows: &[Row]) -> usize {
    if lower.is_empty() {
        return 0;
    }
    let priority: &[Group] = match scope {
        Scope::All => &[
            Group::Efforts,
            Group::Permissions,
            Group::Models,
            Group::Harnesses,
        ],
        Scope::Host => &[Group::Hosts],
        Scope::Harness => &[Group::Harnesses],
        Scope::Model => &[Group::Models],
        Scope::Effort => &[Group::Efforts],
        Scope::Perms => &[Group::Permissions],
        Scope::Name | Scope::Folder | Scope::Recent | Scope::Github | Scope::Templates => &[],
    };
    priority
        .iter()
        .find_map(|group| {
            rows.iter().position(|row| {
                row.group() == *group && row.exact_word().is_some_and(|word| word == lower)
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
            Scope::Github if value.is_empty() => ("Type owner/repo to choose a repository", false),
            Scope::Github => ("No repository matches; enter owner/repo", true),
            Scope::Templates => ("No template matches", true),
        }
    };
    Some(Message { text, warning })
}

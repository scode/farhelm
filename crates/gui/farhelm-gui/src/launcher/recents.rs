//! Recent setups, without GPUI: which of the chosen host's earlier launches the launcher offers again, in what order, and
//! the words a row says about one.
//!
//! SPEC.md, "Creation" ("Recent setups: the helm remembers bounded successful agent-launch combinations and used folders
//! per target-install identity. A recent row fills every saved choice and destination ... Recent-setup rows appear only
//! when the current filter actually matches history"). Ported from the old launcher's `ranked_recents`,
//! `matching_recents`, `destination_recents`, `canonical_destination`, `launch_destination`, `recent_destination_label`
//! and the `selection_summary` family (`launch_composer.rs` at reference commit `24bc0580`), with their tests. Here's the
//! TLDR:
//!
//! - **One row per setup.** Earlier launches group by their choices (permissions normalized the way the helm reads
//!   them, so an omitted permission and the agent's explicit default are one setup) and their destination (a folder by
//!   its canonical path, so two spellings of one folder group; a managed checkout by its repository). The newest launch
//!   of a group is the row.
//! - **Frequency, then recency.** Groups sort by how many launches they hold, ties going to the newer.
//! - **What "the current filter" is.** Not the search box: the draft's own agent type, model and effort, and its
//!   permission only when the user chose it (a remembered permission New starts with must not hide recents), each
//!   narrowing only once set; and its destination, a folder (matched through the history's canonical paths) or a chosen
//!   repository. The strip shows at most three.
//! - **Launches into a new environment** come from the helm's own history of them, which outlives the disposable host
//!   they ran on, merged into the chosen host's ([`merged`]). Such a setup carries its recipe and place, which group it
//!   like its choices and which its row names in place of a host ([`placement`]). It is offered for any folder, since
//!   its folder is the environment's home, but under a chosen repository only with that repository (the old
//!   `ranked_recents`). It never becomes a host's remembered folder ([`remembered_folder`]).

use std::cmp::Reverse;

use farhelm_proto::session_requests::{LaunchHistoryEntry, LaunchHistoryReply};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{
    GithubRepo, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, LaunchTarget,
    join_permission_flags,
};

use super::draft::{harness_label, joined};
use crate::host_text::HostText;
use crate::permission::words::{effort_label, permission_display};

/// The strip's row count (`matching_recents`' `take(3)`).
pub const STRIP_ROWS: usize = 3;

/// The old "all defaults" words for a row whose choices are the agent's defaults throughout.
pub const ALL_DEFAULTS: &str = "Defaults";

/// The draft's choices that narrow the recent setups: each narrows only once set. `permissions` is the draft's only
/// when the user chose it (see the module docs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentFilter {
    pub harness: Option<LaunchHarness>,
    pub model: Option<String>,
    pub effort: Option<LaunchEffort>,
    pub permissions: Option<LaunchPermission>,
}

/// The draft's destination, as recent setups are matched to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftDestination<'a> {
    /// A folder, as the field holds it.
    Folder(&'a str),
    /// A managed checkout of a chosen repository.
    Repository(&'a GithubRepo),
    /// A managed checkout whose repository is still being typed: nothing matches it.
    RepositoryInput,
}

/// Where an earlier launch started, as it groups: a folder by its canonical path, or a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
enum LaunchDestination {
    Folder(String),
    Repository(String, String),
}

/// `entry`'s destination: its repository for a managed checkout, else its folder's canonical path (its spelling for an
/// older entry the helm kept no canonical path for).
fn launch_destination(entry: &LaunchHistoryEntry) -> LaunchDestination {
    match &entry.github_repo {
        Some(repo) => LaunchDestination::Repository(repo.owner.clone(), repo.name.clone()),
        None => LaunchDestination::Folder(
            entry
                .canonical_cwd
                .clone()
                .unwrap_or_else(|| entry.cwd.clone()),
        ),
    }
}

/// The canonical path the history knows for the folder spelled `cwd`, or that spelling when it knows none.
fn canonical_destination(history: &LaunchHistoryReply, cwd: &str) -> String {
    history
        .folders
        .iter()
        .find(|folder| folder.display_cwd == cwd)
        .map_or_else(|| cwd.to_owned(), |folder| folder.canonical_cwd.clone())
}

/// What makes launches one setup: their choices as the helm reads them, their destination, and the new environment's
/// recipe and place for a launch that made one, so the same choices in a container and on a host are two setups. The
/// strip keys a row's keyboard focus by it, so focus follows the setup when the rows change under it (a fill re-filters
/// them), and Enter never launches a setup the user did not move to.
#[derive(Clone, Debug, PartialEq)]
pub struct SetupKey {
    choices: LaunchSelection,
    destination: LaunchDestination,
    target: Option<LaunchTarget>,
}

/// `entry`'s setup.
pub fn setup_key(entry: &LaunchHistoryEntry) -> SetupKey {
    SetupKey {
        choices: normalized(&entry.selection),
        destination: launch_destination(entry),
        target: entry.target.clone(),
    }
}

/// `selection` as the helm reads it: the permission it would launch with, and no workspace trust (a recent setup's
/// historical trust is ignored: SPEC.md, "A recent setup's historical trust value is ignored").
fn normalized(selection: &LaunchSelection) -> LaunchSelection {
    LaunchSelection {
        permissions: selection.harness.request_permission(selection.permissions),
        workspace_trust: None,
        ..selection.clone()
    }
}

/// Whether `selection` passes `filter`: every set filter dimension must equal it; an unset one excludes nothing.
fn matches_filter(selection: &LaunchSelection, filter: &RecentFilter) -> bool {
    filter
        .harness
        .is_none_or(|harness| harness == selection.harness)
        && filter
            .model
            .as_ref()
            .is_none_or(|model| selection.model.as_ref() == Some(model))
        && filter
            .effort
            .is_none_or(|effort| selection.effort == Some(effort))
        && filter.permissions.is_none_or(|permissions| {
            selection.harness.request_permission(selection.permissions) == Some(permissions)
        })
}

/// The recent setups `history` offers for `filter` and `destination`, best first and uncapped: one per setup, the
/// newest launch of each, sorted by how many launches it holds, then by how recent its newest is. With no
/// `destination`, every destination counts.
pub fn ranked<'a>(
    history: &'a LaunchHistoryReply,
    filter: &RecentFilter,
    destination: Option<DraftDestination<'_>>,
) -> Vec<&'a LaunchHistoryEntry> {
    let has_repositories = history
        .launches
        .iter()
        .any(|entry| entry.github_repo.is_some());
    let wanted = |entry: &LaunchHistoryEntry| -> bool {
        match destination {
            None => true,
            Some(DraftDestination::RepositoryInput) => false,
            Some(DraftDestination::Repository(repo)) => entry.github_repo.as_ref() == Some(repo),
            // With repository launches in the history, a folder destination still offers them (the old
            // `destination_recents`), so a setup that launched into a checkout stays one press away. A new
            // environment's setup started in its own home, which says nothing about the folder the draft holds.
            Some(DraftDestination::Folder(cwd)) => {
                entry.target.is_some()
                    || (has_repositories && entry.github_repo.is_some())
                    || launch_destination(entry)
                        == LaunchDestination::Folder(canonical_destination(history, cwd))
            }
        }
    };
    // Grouped by setup; the first (newest) occurrence is the row, and its index breaks ties. The helm keeps the history
    // short, so a scan per entry is cheap.
    let mut groups: Vec<(&LaunchHistoryEntry, SetupKey, usize)> = Vec::new();
    for entry in history.launches.iter() {
        if !wanted(entry) || !matches_filter(&entry.selection, filter) {
            continue;
        }
        let key = setup_key(entry);
        match groups.iter_mut().find(|(_, seen, _)| *seen == key) {
            Some(group) => group.2 += 1,
            None => groups.push((entry, key, 1)),
        }
    }
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|&group| (Reverse(groups[group].2), group));
    order.into_iter().map(|group| groups[group].0).collect()
}

/// The strip's rows: [`ranked`], at most [`STRIP_ROWS`].
pub fn strip<'a>(
    history: &'a LaunchHistoryReply,
    filter: &RecentFilter,
    destination: DraftDestination<'_>,
) -> Vec<&'a LaunchHistoryEntry> {
    let mut rows = ranked(history, filter, Some(destination));
    rows.truncate(STRIP_ROWS);
    rows
}

/// The history recent setups are drawn from: the chosen host's (`ordinary`, while it is read and current) with the
/// helm's launches into new environments (`environment`) merged in, or `None` while there is neither. Only the
/// launches change: the saved folders and the checkout configuration's revision stay the host's, and with no host's
/// history there are none, so an environment's home never reads as a host's folder.
///
/// Each source keeps its own order, which is its own sequence and may disagree with its timestamps after a clock jump;
/// the timestamps only pick between the two sources' next launches, a tie going to the host's. This is the old
/// `merge_recent_launches`; the two clocks may be skewed, which the old launcher accepted too.
pub fn merged(
    ordinary: Option<&LaunchHistoryReply>,
    environment: &[LaunchHistoryEntry],
) -> Option<LaunchHistoryReply> {
    if ordinary.is_none() && environment.is_empty() {
        return None;
    }
    let mut reply = ordinary.cloned().unwrap_or(LaunchHistoryReply {
        launches: Vec::new(),
        folders: Vec::new(),
        checkout_config_revision: 0,
    });
    let mut ordinary = std::mem::take(&mut reply.launches).into_iter().peekable();
    let mut environment = environment.iter().peekable();
    loop {
        let from_environment = match (ordinary.peek(), environment.peek()) {
            (Some(host), Some(new)) => new.created_at > host.created_at,
            (None, Some(_)) => true,
            (_, None) => false,
        };
        let next = if from_environment {
            environment.next().cloned()
        } else {
            ordinary.next()
        };
        let Some(entry) = next else {
            return Some(reply);
        };
        reply.launches.push(entry);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// A row's words
// ---------------------------------------------------------------------------------------------------------------------

/// Where `entry` started, as its row names it: `gh:owner/name` for a managed checkout, else its folder as typed.
pub fn destination_label(entry: &LaunchHistoryEntry) -> String {
    match &entry.github_repo {
        Some(repo) => format!("gh:{}/{}", repo.owner, repo.name),
        None => entry.cwd.clone(),
    }
}

/// Where `entry` ran, as its row names it (the old `recent_placement_label`): a new environment by its recipe and place
/// as the footer names the one a draft would make, less its "new" ([`crate::environment_ui::launch_target_label`],
/// over the registry's `hosts`: "web container on host1"), so the row needs no host of the draft's; any other launch
/// by the chosen host's name, `host`, since only that host's history holds it. `None` for such a launch with no host
/// to name.
pub fn placement(
    entry: &LaunchHistoryEntry,
    host: Option<&HostText>,
    hosts: &[HostView],
) -> Option<HostText> {
    match &entry.target {
        Some(target) => Some(crate::environment_ui::launch_target_label(target, hosts)),
        None => host.cloned(),
    }
}

/// The folder `history` makes its host's default for New: where the newest launch into a plain folder started, else
/// `~` (SPEC.md, "Working directory"). A launch into a fresh managed checkout does not count, nor does one into a
/// disposable environment, whose folder belongs to that environment. The helm lists launches newest first.
pub fn remembered_folder(history: &LaunchHistoryReply) -> String {
    history
        .launches
        .iter()
        .find(|entry| entry.github_repo.is_none() && entry.target.is_none())
        .map_or_else(|| "~".to_owned(), |entry| entry.cwd.clone())
}

/// The permission `selection` launches with, in the launcher's words; "Agent's own setting" for none.
pub fn permission_value(selection: &LaunchSelection) -> &'static str {
    match selection.harness.request_permission(selection.permissions) {
        None => "Agent's own setting",
        permission => permission_display(permission),
    }
}

/// Whether `selection` launches with the agent's default Guarded mode, which a row leaves unsaid.
pub fn uses_guarded_default(selection: &LaunchSelection) -> bool {
    selection.harness.omitted_permission() == Some(LaunchPermission::Guarded)
        && selection.harness.request_permission(selection.permissions)
            == Some(LaunchPermission::Guarded)
}

/// The choices `selection` sets explicitly before its permission, for the strip's selection cell: its model, its
/// effort, a retired container; empty when all are the defaults.
pub fn explicit_before_permissions(selection: &LaunchSelection) -> Vec<String> {
    let mut words = Vec::new();
    if let Some(model) = &selection.model {
        words.push(format!("Model: {model}"));
    }
    if selection.effort.is_some() {
        words.push(format!("Effort: {}", effort_label(selection.effort)));
    }
    if selection.container.is_some() {
        words.push("container".to_owned());
    }
    words
}

/// `selection`'s model and effort as a search row's second line starts them: "Model: … · Effort: …", defaults named.
pub fn summary_before_permissions(selection: &LaunchSelection) -> String {
    let mut words = format!(
        "Model: {} · Effort: {}",
        selection.model.as_deref().unwrap_or("Default"),
        effort_label(selection.effort)
    );
    if selection.container.is_some() {
        words.push_str(" · container");
    }
    words
}

/// The wrapper as it appears in the editor, named in recents because it can distinguish otherwise identical
/// setups. The session card's agent summary has its own, narrower list of choices.
pub(super) fn wrapper_summary(selection: &LaunchSelection) -> Option<String> {
    let prefix = joined(selection.wrapper_prefix.as_deref());
    (!prefix.is_empty()).then(|| format!("Wrapper: {prefix}"))
}

/// A recent setup's complete choices for its accessible name and hover help, including any stored wrapper.
pub fn selection_summary(selection: &LaunchSelection) -> String {
    let mut words = summary_without_wrapper(selection);
    if let Some(wrapper) = wrapper_summary(selection) {
        words.push_str(" · ");
        words.push_str(&wrapper);
    }
    words
}

/// The choices before the wrapper, or just the harness for a shell. The search row draws the wrapper in its own space.
pub(super) fn summary_without_wrapper(selection: &LaunchSelection) -> String {
    if selection.harness.is_shell() {
        return format!("{} shell", harness_label(selection.harness));
    }
    let mut words = format!(
        "{} · {} · Permissions: {}",
        harness_label(selection.harness),
        summary_before_permissions(selection),
        permission_value(selection)
    );
    if let Some(flags) = &selection.permission_flags {
        words.push_str(" · ");
        words.push_str(&join_permission_flags(flags));
    }
    words
}

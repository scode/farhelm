//! Renderer-free rules for the structured launch composer.
//!
//! The component owns temporary input, focus, and request generations. This
//! module owns the choices that can be explained without a browser: whether a
//! known model still fits a harness, which stored setups match deliberate
//! fields, and what a click on one of those setups means. Keeping that split
//! matters because fetched history may refresh while the dialog is open, but
//! the form promotes it into offered rows only at a deliberate choice/search
//! boundary; neither refresh nor promotion may make a selection for the user.

use crate::api::{LaunchCatalogModel, LaunchHistory, LaunchHistoryEntry};
use crate::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection};

/// A stable display order prevents catalog entry order from moving buttons.
const EFFORT_ORDER: &[LaunchEffort] = &[
    LaunchEffort::Off,
    LaunchEffort::Minimal,
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
    LaunchEffort::Ultra,
];

/// Return the stable lowercase word used for an effort in the composer.
///
/// The wire enum and the visible control labels use the same six words. Keeping
/// that spelling here lets renderer-free search and the component controls share
/// one protocol vocabulary instead of drifting through separate `Debug` or
/// display conversions.
pub(crate) const fn effort_value(effort: LaunchEffort) -> &'static str {
    match effort {
        LaunchEffort::Off => "off",
        LaunchEffort::Minimal => "minimal",
        LaunchEffort::Low => "low",
        LaunchEffort::Medium => "medium",
        LaunchEffort::High => "high",
        LaunchEffort::Xhigh => "xhigh",
        LaunchEffort::Max => "max",
        LaunchEffort::Ultra => "ultra",
    }
}

/// Return the lowercase phrase used for a permission in visible summaries.
///
/// The wire spelling for `SmartApprove` uses an underscore, while people see
/// the same spaced phrase as Goose's segmented control. Keeping that choice
/// here prevents recent rows and the live summary from falling back to Rust's
/// `Debug` spelling.
pub(crate) const fn permission_value(permission: LaunchPermission) -> &'static str {
    match permission {
        LaunchPermission::Yolo => "yolo",
        LaunchPermission::Approve => "approve",
        LaunchPermission::SmartApprove => "smart approve",
        LaunchPermission::Chat => "chat",
    }
}

/// Normalize a permission to the modes the selected harness can represent.
///
/// Pi's missing tool gate is deliberately represented as YOLO even when an
/// older stored selection omitted the optional field. Goose owns the three
/// approval modes; moving one of them elsewhere clears it rather than letting
/// an unsupported hidden choice reach launch.
pub(crate) const fn normalized_permissions(
    harness: LaunchHarness,
    permissions: Option<LaunchPermission>,
) -> Option<LaunchPermission> {
    match harness {
        LaunchHarness::Pi => Some(LaunchPermission::Yolo),
        LaunchHarness::Goose => permissions,
        _ => match permissions {
            Some(
                LaunchPermission::Approve | LaunchPermission::SmartApprove | LaunchPermission::Chat,
            ) => None,
            permissions => permissions,
        },
    }
}

/// Return the permission phrase a complete selection presents to the user.
pub(crate) fn selection_permission_value(selection: &LaunchSelection) -> &'static str {
    normalized_permissions(selection.harness, selection.permissions)
        .map(permission_value)
        .unwrap_or("default")
}

/// Return the lowercase word search matches a harness by.
///
/// One spelling for the two places that compare a query against a harness —
/// the substring match that offers the row and the exact-word match that
/// preselects it — so they can never disagree about what "claude" names.
/// Derived from the `Debug` name rather than a second literal table because
/// that name is also what the row displays (`Harness: {harness:?}`).
pub(crate) fn harness_word(harness: LaunchHarness) -> String {
    format!("{harness:?}").to_ascii_lowercase()
}

/// A partial structured choice used to narrow suggestions without inventing
/// missing defaults.
///
/// `None` means the user has not selected that dimension. It does not mean a
/// candidate must also omit it: an explicit recent model remains relevant
/// until the user makes a conflicting explicit choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ComposerFilter {
    pub(crate) harness: Option<LaunchHarness>,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<LaunchEffort>,
    pub(crate) permissions: Option<LaunchPermission>,
}

/// One explicit choice shown by composer search.
///
/// Search never mutates selections. The component turns a clicked result into
/// its corresponding field update, which keeps a query from becoming a
/// hidden launch input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ComposerSearchResult {
    Harness(LaunchHarness),
    /// Switch the shared composer to its profile-or-raw-command controls.
    Command,
    Model {
        id: String,
        harness: LaunchHarness,
    },
    /// Apply one effort offered by the currently selected harness and model.
    Effort(LaunchEffort),
    /// Apply the path the person typed without changing their agent choices.
    UsePath(String),
    /// Open the explicit directory browser at the path the person typed.
    BrowsePath(String),
    /// Reuse a directory that has succeeded on this selected host before.
    Folder(String),
    /// Explicitly request a new checkout, preserving the agent selection.
    Github(crate::github_checkout::GithubRepo),
    Recent(LaunchHistoryEntry),
}

/// One actionable row in the model combobox's stable keyboard order.
///
/// The component renders rows from other harnesses with a " (Harness)"
/// suffix rather than under headings, so this flat sequence IS the
/// accessibility contract: Arrow keys and `aria-activedescendant` must agree
/// with pointer activation even when the chosen harness filters rows away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModelOption {
    HarnessDefault,
    Model {
        id: String,
        harness: LaunchHarness,
    },
    /// Toggle between the chosen harness's rows and every harness's rows.
    /// Offered only while a harness is chosen: with none chosen every row is
    /// already listed and the toggle would be a no-op with a lying label.
    ShowAll,
}

/// The model action Enter commits after navigation and draft interpretation.
///
/// Keeping this decision outside the renderer prevents keyboard handling from
/// silently treating the first visible row as selected. Custom ids retain the
/// exact bytes the person typed; only catalog ids are canonicalized. A custom
/// id carries the harness it was accepted under, so the renderer never has to
/// re-read the harness and trust that it is still there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ModelEnterTarget {
    Nothing,
    Option(ModelOption),
    Custom { id: String, harness: LaunchHarness },
    NeedsHarness(String),
}

/// Resolve Enter against the current options and catalog.
///
/// Arrow navigation takes precedence over text. Without navigation, an empty
/// draft does nothing, an exact known id carries its catalog owner, and only
/// an unknown id depends on an explicitly selected harness.
pub(crate) fn model_enter_target(
    options: &[ModelOption],
    active: Option<usize>,
    draft: &str,
    catalog: &[LaunchCatalogModel],
    harness: Option<LaunchHarness>,
) -> ModelEnterTarget {
    if let Some(index) = active {
        return options
            .get(index)
            .cloned()
            .map(ModelEnterTarget::Option)
            .unwrap_or(ModelEnterTarget::Nothing);
    }
    if draft.trim().is_empty() {
        return ModelEnterTarget::Nothing;
    }
    let owners = catalog
        .iter()
        .filter(|model| model.id.eq_ignore_ascii_case(draft))
        .collect::<Vec<_>>();
    if let Some(current) =
        harness.filter(|current| owners.iter().any(|model| model.harness == *current))
    {
        return ModelEnterTarget::Option(ModelOption::Model {
            id: owners[0].id.clone(),
            harness: current,
        });
    }
    if owners.len() == 1 {
        return ModelEnterTarget::Option(ModelOption::Model {
            id: owners[0].id.clone(),
            harness: owners[0].harness,
        });
    }
    if !owners.is_empty() {
        return ModelEnterTarget::NeedsHarness(draft.to_string());
    }
    match harness {
        Some(harness) => ModelEnterTarget::Custom {
            id: draft.to_string(),
            harness,
        },
        None => ModelEnterTarget::NeedsHarness(draft.to_string()),
    }
}

/// Return the bounded, filtered model choices for the launch combobox.
///
/// A known id carries its owning harness, so matching it never depends on a
/// browser-side guess. The catalog order remains stable within the fixed
/// harness order, making movement predictable while the list is open.
pub(crate) fn model_options(
    catalog: &[LaunchCatalogModel],
    harness: Option<LaunchHarness>,
    query: &str,
    show_all: bool,
) -> Vec<ModelOption> {
    let folded_query = query.to_ascii_lowercase();
    let mut options = Vec::new();
    if !matches!(
        harness,
        Some(LaunchHarness::OpenCode | LaunchHarness::Goose | LaunchHarness::Pi)
    ) {
        options.push(ModelOption::HarnessDefault);
    }
    for owner in [
        LaunchHarness::Codex,
        LaunchHarness::Claude,
        LaunchHarness::Muse,
        LaunchHarness::Goose,
        LaunchHarness::Pi,
        LaunchHarness::OpenCode,
    ] {
        if !show_all && harness.is_some_and(|selected| selected != owner) {
            continue;
        }
        options.extend(
            catalog
                .iter()
                .filter(|model| {
                    model.harness == owner && model.id.to_ascii_lowercase().contains(&folded_query)
                })
                .map(|model| ModelOption::Model {
                    id: model.id.clone(),
                    harness: model.harness,
                }),
        );
    }
    if harness.is_some() {
        options.push(ModelOption::ShowAll);
    }
    options
}

/// The visible and keyboard order of one kind of search result.
///
/// Grouping prevents a partial field action from resembling a complete saved
/// setup. Its fixed order also keeps Arrow-key positions predictable while a
/// query is open, irrespective of history or catalog ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerSearchGroup {
    Harnesses,
    Models,
    /// Reasoning-effort words valid for the selected harness and model.
    Efforts,
    Folders,
    Repositories,
    RecentSetups,
}

impl ComposerSearchGroup {
    /// Return the accessible heading for this stable result group.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Harnesses => "Harnesses",
            Self::Models => "Models",
            Self::Efforts => "Efforts",
            Self::Folders => "Folders",
            Self::Repositories => "GitHub repositories",
            Self::RecentSetups => "Recent setups",
        }
    }
}

/// Partition results into the groups rendered by the combobox.
///
/// Empty groups stay absent from the DOM, but the returned order is always
/// the same. Callers flatten this sequence for `aria-activedescendant` and
/// Arrow-key navigation, so pointer and keyboard activation share one order.
pub(crate) fn grouped_search_results(
    results: Vec<ComposerSearchResult>,
) -> Vec<(ComposerSearchGroup, Vec<ComposerSearchResult>)> {
    let mut harnesses = Vec::new();
    let mut models = Vec::new();
    let mut efforts = Vec::new();
    let mut folders = Vec::new();
    let mut repositories = Vec::new();
    let mut recents = Vec::new();
    for result in results {
        match result {
            ComposerSearchResult::Harness(_) => harnesses.push(result),
            ComposerSearchResult::Command => harnesses.push(result),
            ComposerSearchResult::Model { .. } => models.push(result),
            ComposerSearchResult::Effort(_) => efforts.push(result),
            ComposerSearchResult::UsePath(_)
            | ComposerSearchResult::BrowsePath(_)
            | ComposerSearchResult::Folder(_) => folders.push(result),
            ComposerSearchResult::Recent(_) => recents.push(result),
            ComposerSearchResult::Github(_) => repositories.push(result),
        }
    }
    [
        (ComposerSearchGroup::Harnesses, harnesses),
        (ComposerSearchGroup::Models, models),
        (ComposerSearchGroup::Efforts, efforts),
        (ComposerSearchGroup::Folders, folders),
        (ComposerSearchGroup::Repositories, repositories),
        (ComposerSearchGroup::RecentSetups, recents),
    ]
    .into_iter()
    .filter(|(_, results)| !results.is_empty())
    .collect()
}

/// Render every choice a recent setup will apply before an interaction changes
/// the form.
///
/// Omitted fields are named as defaults so absence is never mistaken for an
/// invisible retained value. Pi is the compatibility exception: an omitted
/// permission from an older snapshot is presented as its mandatory YOLO mode.
/// This is the complete accessible description; the compact row surface puts
/// its harness in a separately styled leading span.
pub(crate) fn selection_summary(selection: &LaunchSelection) -> String {
    format!(
        "{:?} · {}",
        selection.harness,
        selection_summary_without_harness(selection)
    )
}

/// Render the non-harness fields of a saved structured selection.
///
/// Recent rows put the harness first as their scan target, while titles and
/// accessible names still need the complete summary from [`selection_summary`].
/// Keeping one tail builder prevents those two representations from silently
/// disagreeing about which saved defaults a row will apply.
pub(crate) fn selection_summary_without_harness(selection: &LaunchSelection) -> String {
    format!(
        "{} · permissions: {}",
        selection_summary_before_permissions(selection),
        selection_permission_value(selection),
    )
}

/// Render the model and effort of a saved selection, stopping before the
/// permission.
///
/// The visible recent row spells the permission itself, so it can color
/// "yolo" as a warning without cutting that word back out of a formatted
/// string; this is the prefix it renders in front of that word. Every other
/// summary appends the permission through [`selection_summary_without_harness`],
/// so the two never disagree about the model or effort a row will apply.
pub(crate) fn selection_summary_before_permissions(selection: &LaunchSelection) -> String {
    format!(
        "model: {} · effort: {}",
        selection.model.as_deref().unwrap_or("default"),
        selection
            .effort
            .map(|effort| format!("{effort:?}"))
            .unwrap_or_else(|| "default".to_string()),
    )
}

/// The bounded kinds understood by composer search.
///
/// All preserves the existing unlabelled search surface. The GitHub scope
/// is recognized here so the renderer can reserve that query for independently
/// fetched repository suggestions without allowing local rows to leak into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchScope {
    All,
    Harness,
    Model,
    Effort,
    Folder,
    Recent,
    Github,
}

/// Trim a composer query and split one recognized leading scope label.
///
/// Only the first colon can be a delimiter. Unknown labels remain complete
/// ordinary queries, which preserves custom model identifiers containing
/// colons. The returned value is borrowed from the trimmed input.
pub(crate) fn scoped_query(query: &str) -> (SearchScope, &str) {
    let query = query.trim();
    let Some(colon) = query.find(':') else {
        return (SearchScope::All, query);
    };
    let (label, value) = query.split_at(colon);
    let scope = if label.eq_ignore_ascii_case("harness") {
        SearchScope::Harness
    } else if label.eq_ignore_ascii_case("model") {
        SearchScope::Model
    } else if label.eq_ignore_ascii_case("effort") {
        SearchScope::Effort
    } else if label.eq_ignore_ascii_case("folder") {
        SearchScope::Folder
    } else if label.eq_ignore_ascii_case("recent") {
        SearchScope::Recent
    } else if label.eq_ignore_ascii_case("gh") {
        SearchScope::Github
    } else {
        return (SearchScope::All, query);
    };
    (scope, value[1..].trim())
}

/// Find composer choices whose visible value matches a deliberate query.
///
/// The server owns history ordering, so the result preserves it. This does
/// not walk a filesystem; folder search is only a lookup through prior
/// successful destinations for the selected host. A path-shaped query also
/// exposes explicit use and browse actions, but does not invoke either. A
/// selected harness narrows catalog and custom-history models; absent a
/// harness, model ownership remains discoverable and effort words are absent.
/// A recognized leading label limits the result kinds to its scope, while
/// unlabelled input retains the combined search surface for compatibility.
pub(crate) fn search_results(
    history: &LaunchHistory,
    catalog: &[LaunchCatalogModel],
    query: &str,
    harness: Option<LaunchHarness>,
    model: Option<&str>,
) -> Vec<ComposerSearchResult> {
    let (scope, query) = scoped_query(query);
    if scope == SearchScope::Github || (scope == SearchScope::All && query.is_empty()) {
        return Vec::new();
    }
    let folded_query = query.to_ascii_lowercase();
    let mut results = Vec::new();

    if matches!(scope, SearchScope::All | SearchScope::Harness) {
        for harness in [
            LaunchHarness::Codex,
            LaunchHarness::Claude,
            LaunchHarness::Muse,
            LaunchHarness::Goose,
            LaunchHarness::Pi,
            LaunchHarness::OpenCode,
        ] {
            if query.is_empty() || harness_word(harness).contains(&folded_query) {
                results.push(ComposerSearchResult::Harness(harness));
            }
        }
        if query.is_empty() || "other / command".contains(&folded_query) {
            results.push(ComposerSearchResult::Command);
        }
    }

    if matches!(scope, SearchScope::All | SearchScope::Model) {
        for candidate in catalog {
            if harness.is_some_and(|selected| selected != candidate.harness) {
                continue;
            }
            if query.is_empty() || candidate.id.to_ascii_lowercase().contains(&folded_query) {
                results.push(ComposerSearchResult::Model {
                    id: candidate.id.clone(),
                    harness: candidate.harness,
                });
            }
        }
        // A custom model is a complete history fact, not a catalog omission.
        // Reuse ranked history so scoped and ordinary model search preserve
        // the same frequency and recency order.
        for launch in ranked_recents(history, &ComposerFilter::default(), None) {
            let Some(id) = launch.selection.model.as_ref() else {
                continue;
            };
            if harness.is_some_and(|selected| selected != launch.selection.harness) {
                continue;
            }
            if (!query.is_empty() && !id.to_ascii_lowercase().contains(&folded_query))
                || catalog.iter().any(|model| model.id == *id)
                || results.iter().any(|result| {
                    matches!(
                        result,
                        ComposerSearchResult::Model {
                            id: existing,
                            harness,
                        } if existing == id && *harness == launch.selection.harness
                    )
                })
            {
                continue;
            }
            results.push(ComposerSearchResult::Model {
                id: id.clone(),
                harness: launch.selection.harness,
            });
        }
    }

    if matches!(scope, SearchScope::All | SearchScope::Effort) {
        // Without a harness there is no safe effort vocabulary to offer.
        if let Some(harness) = harness {
            for effort in compatible_efforts(harness, model, catalog) {
                if query.is_empty() || effort_value(effort).contains(&folded_query) {
                    results.push(ComposerSearchResult::Effort(effort));
                }
            }
        }
    }

    if matches!(scope, SearchScope::All | SearchScope::Folder) {
        if is_path_query(query) {
            results.push(ComposerSearchResult::UsePath(query.to_string()));
            results.push(ComposerSearchResult::BrowsePath(query.to_string()));
        }
        for folder in &history.folders {
            if query.is_empty()
                || folder
                    .display_cwd
                    .to_ascii_lowercase()
                    .contains(&folded_query)
            {
                results.push(ComposerSearchResult::Folder(folder.display_cwd.clone()));
            }
        }
    }

    if matches!(scope, SearchScope::All | SearchScope::Recent) {
        // Search applies its text predicate after complete-selection ranking.
        for launch in ranked_recents(history, &ComposerFilter::default(), None) {
            let haystack = format!(
                "{} {:?} {}",
                recent_destination_label(launch),
                launch.selection.harness,
                launch.selection.model.as_deref().unwrap_or_default()
            )
            .to_ascii_lowercase();
            if query.is_empty() || haystack.contains(&folded_query) {
                results.push(ComposerSearchResult::Recent(launch.clone()));
            }
        }
    }
    results
}

/// Return the first flat result index whose visible word exactly matches the query.
///
/// Exact words win over broader substring matches without changing the fixed
/// group order. Efforts outrank exact model ids, which outrank exact harness
/// names; this keeps medium from landing on a model such as medium-context,
/// and keeps a model id from losing to a harness substring. A recognized label
/// narrows exact matching to its corresponding group; paths, recent setup
/// descriptions, and the GitHub scope do not participate.
pub(crate) fn default_search_index(
    grouped_results: &[(ComposerSearchGroup, Vec<ComposerSearchResult>)],
    query: &str,
) -> usize {
    let (scope, query) = scoped_query(query);
    let folded_query = query.to_ascii_lowercase();
    if folded_query.is_empty() {
        return 0;
    }
    let flat_results = grouped_results
        .iter()
        .flat_map(|(_, results)| results)
        .collect::<Vec<_>>();
    let kinds = match scope {
        SearchScope::All => vec![
            ComposerSearchGroup::Efforts,
            ComposerSearchGroup::Models,
            ComposerSearchGroup::Harnesses,
        ],
        SearchScope::Harness => vec![ComposerSearchGroup::Harnesses],
        SearchScope::Model => vec![ComposerSearchGroup::Models],
        SearchScope::Effort => vec![ComposerSearchGroup::Efforts],
        SearchScope::Folder | SearchScope::Recent | SearchScope::Github => Vec::new(),
    };
    for kind in kinds {
        if let Some(index) = flat_results
            .iter()
            .position(|result| exact_word_group(result, &folded_query) == Some(kind))
        {
            return index;
        }
    }
    0
}

/// Return the group of a result whose visible word IS the (lowercase,
/// trimmed) query, or `None` when the result is not an exact word match.
///
/// Only harness, model, and effort rows have a single word a person types
/// deliberately; paths, folders, and recent setups are descriptions, and an
/// exact match on those would be a coincidence rather than an intent. Each
/// kind compares by the same spelling the search offered it under —
/// [`harness_word`], the model id, [`effort_value`] — so a row that matched
/// as a substring can always also match exactly.
fn exact_word_group(
    result: &ComposerSearchResult,
    folded_query: &str,
) -> Option<ComposerSearchGroup> {
    match result {
        ComposerSearchResult::Harness(harness) if harness_word(*harness) == folded_query => {
            Some(ComposerSearchGroup::Harnesses)
        }
        ComposerSearchResult::Command if folded_query == "command" => {
            Some(ComposerSearchGroup::Harnesses)
        }
        ComposerSearchResult::Model { id, .. } if id.eq_ignore_ascii_case(folded_query) => {
            Some(ComposerSearchGroup::Models)
        }
        ComposerSearchResult::Effort(effort) if effort_value(*effort) == folded_query => {
            Some(ComposerSearchGroup::Efforts)
        }
        _ => None,
    }
}

/// Return whether a search term is an explicit filesystem path expression.
///
/// The composer intentionally accepts familiar absolute, home-relative, and
/// relative path spellings. A bare word remains a choice query so it cannot
/// accidentally turn a model or harness search into an attempted browse.
fn is_path_query(query: &str) -> bool {
    query.starts_with('/')
        || query.starts_with('~')
        || query.starts_with('.')
        || query.contains('/')
}

/// Return whether a saved setup agrees with every field the user selected.
///
/// This is deliberately a one-way filter. A saved explicit model or effort
/// is still offered when the corresponding filter is absent, because absence
/// represents a harness default rather than a claim that no explicit choice
/// is acceptable.
pub(crate) fn matches_filter(selection: &LaunchSelection, filter: &ComposerFilter) -> bool {
    filter
        .harness
        .is_none_or(|harness| selection.harness == harness)
        && filter
            .model
            .as_ref()
            .is_none_or(|model| selection.model.as_ref() == Some(model))
        && filter
            .effort
            .is_none_or(|effort| selection.effort == Some(effort))
        && filter
            .permissions
            .is_none_or(|permissions| selection.permissions == Some(permissions))
}

/// Rank every unique recent setup for the selected folder.
///
/// The helm supplies stable history order. This function aggregates
/// each complete setup over that bounded window, ranks frequent choices ahead
/// of one-off experiments, then uses the newest occurrence as a deterministic
/// tie-break. Repeats therefore improve a useful setup's rank without
/// crowding the list with duplicate buttons.
fn ranked_recents<'a>(
    history: &'a LaunchHistory,
    filter: &ComposerFilter,
    cwd: Option<&str>,
) -> Vec<&'a LaunchHistoryEntry> {
    let selected_destination =
        cwd.map(|cwd| RecentDestination::Existing(canonical_destination(history, cwd)));
    let candidates = history
        .launches
        .iter()
        .filter(|entry| {
            selected_destination.is_none_or(|destination| launch_destination(entry) == destination)
        })
        .filter(|entry| matches_filter(&entry.selection, filter))
        .collect::<Vec<_>>();
    let mut ranked = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            // The first matching item is the most recent one because the
            // helm's history order is stable. Later duplicates contribute
            // frequency but do not get their own row.
            if candidates[..index].iter().any(|prior| {
                prior.selection == entry.selection
                    && launch_destination(prior) == launch_destination(entry)
            }) {
                None
            } else {
                let frequency = candidates
                    .iter()
                    .filter(|candidate| {
                        candidate.selection == entry.selection
                            && launch_destination(candidate) == launch_destination(entry)
                    })
                    .count();
                Some((frequency, index, *entry))
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(frequency, recency, _)| (std::cmp::Reverse(*frequency), *recency));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

/// Return the three displayed recents after complete history aggregation.
///
/// Search deliberately calls [`ranked_recents`] directly before it applies
/// its own query policy. Keeping the cap here preserves the compact ordinary
/// surface without letting it change the ranking or candidates that search
/// evaluates.
pub(crate) fn matching_recents<'a>(
    history: &'a LaunchHistory,
    filter: &ComposerFilter,
    cwd: Option<&str>,
) -> Vec<&'a LaunchHistoryEntry> {
    ranked_recents(history, filter, cwd)
        .into_iter()
        .take(3)
        .collect()
}

/// Keep ordinary recents scoped to the selected directory while also offering
/// reusable repo setups, whose old allocation paths are irrelevant. Once a
/// repo is selected, only that repo's compatible setups belong in this strip.
pub(crate) fn destination_recents<'a>(
    history: &'a LaunchHistory,
    filter: &ComposerFilter,
    destination: &crate::github_checkout::DestinationDraft,
) -> Vec<&'a LaunchHistoryEntry> {
    use crate::github_checkout::DestinationDraft;
    // Old peers and ordinary-only histories keep the existing compact path.
    if let DestinationDraft::Existing { cwd } = destination
        && history
            .launches
            .iter()
            .all(|entry| entry.github_repo.is_none())
    {
        return matching_recents(history, filter, Some(cwd));
    }
    ranked_recents(history, filter, None)
        .into_iter()
        .filter(|entry| match destination {
            DestinationDraft::Existing { cwd } => {
                entry.github_repo.is_some()
                    || launch_destination(entry)
                        == RecentDestination::Existing(canonical_destination(history, cwd))
            }
            DestinationDraft::Github { repo, .. } => entry.github_repo.as_ref() == Some(repo),
        })
        .take(3)
        .collect()
}

/// Return the destination identity recorded for a displayed path.
///
/// Folder history is where the helm keeps canonical path identity while a
/// launch row preserves the submitted spelling. Falling back to the spelling
/// is deliberate for an old or partially populated reply: it avoids merging
/// two paths merely because the canonical fact was not supplied.
fn canonical_destination<'a>(history: &'a LaunchHistory, display_cwd: &'a str) -> &'a str {
    history
        .folders
        .iter()
        .find(|folder| folder.display_cwd == display_cwd)
        .map_or(display_cwd, |folder| folder.canonical_cwd.as_str())
}

/// Return the immutable accepted destination for one reusable launch.
///
/// Folder history deliberately remains a mutable, bounded browse suggestion.
/// It cannot reconstruct a launch's historical identity after another alias
/// or a repointed symlink uses the same spelling, so old rows fall back only
/// to their own display fact. Fresh launches instead group by accepted repo
/// intent; their actual cwd is diagnostic, never a reusable destination.
fn launch_destination(entry: &LaunchHistoryEntry) -> RecentDestination<'_> {
    match &entry.github_repo {
        Some(repo) => RecentDestination::Github(repo),
        None => RecentDestination::Existing(entry.canonical_cwd.as_deref().unwrap_or(&entry.cwd)),
    }
}

/// Name the destination a saved setup will request, rather than the path a
/// previous fresh allocation happened to receive.
pub(crate) fn recent_destination_label(entry: &LaunchHistoryEntry) -> String {
    entry.github_repo.as_ref().map_or_else(
        || entry.cwd.clone(),
        |repo| format!("gh:{}", repo.identifier()),
    )
}

/// Fresh intent and an existing path remain different setups even when their
/// last launches used the same directory. Repo identity also groups separate
/// fresh allocations without treating their numbered paths as new setups.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecentDestination<'a> {
    Existing(&'a str),
    Github(&'a crate::github_checkout::GithubRepo),
}

/// Apply a recent setup as a complete explicit selection.
///
/// This does not create a session, retain fields from the prior selection, or
/// infer a default for a field the saved setup omitted. The component uses the
/// returned value to update controls and waits for an explicit Launch click.
pub(crate) fn select_recent(entry: &LaunchHistoryEntry) -> LaunchSelection {
    entry.selection.clone()
}

/// Return whether a structured choice remains valid against the catalog the
/// helm build exposed.
///
/// Unknown model IDs are valid custom IDs once the person has selected their
/// owning harness. They cannot have model-specific effort constraints locally,
/// so this applies the harness-wide vocabulary and the required-model rules
/// for OpenCode, Goose, and Pi. It also rejects Goose-only permission modes on
/// every other harness while accepting an omitted Pi permission as the older
/// spelling of YOLO. The helm validates the final request again, including Zen
/// provider syntax for custom OpenCode IDs.
pub(crate) fn selection_is_compatible(
    selection: &LaunchSelection,
    catalog: &[LaunchCatalogModel],
) -> bool {
    if matches!(
        selection.harness,
        LaunchHarness::OpenCode | LaunchHarness::Goose | LaunchHarness::Pi
    ) && selection.model.is_none()
    {
        return false;
    }
    let known_models = selection.model.as_ref().map(|model| {
        catalog
            .iter()
            .filter(|candidate| candidate.id == *model)
            .collect::<Vec<_>>()
    });
    if known_models.as_ref().is_some_and(|models| {
        !models.is_empty()
            && !models
                .iter()
                .any(|model| model.harness == selection.harness)
    }) {
        return false;
    }
    let effort_is_compatible = selection.effort.is_none_or(|effort| {
        known_models
            .as_ref()
            .and_then(|models| {
                models
                    .iter()
                    .find(|model| model.harness == selection.harness)
            })
            .map_or_else(
                || compatible_efforts(selection.harness, None, catalog).contains(&effort),
                |model| model.efforts.contains(&effort),
            )
    });
    let permissions_are_compatible = match (selection.harness, selection.permissions) {
        (LaunchHarness::Goose, _)
        | (LaunchHarness::Pi, None | Some(LaunchPermission::Yolo))
        | (_, None | Some(LaunchPermission::Yolo)) => true,
        (
            _,
            Some(
                LaunchPermission::Approve | LaunchPermission::SmartApprove | LaunchPermission::Chat,
            ),
        ) => false,
    };
    effort_is_compatible && permissions_are_compatible
}

/// Move a structured choice to another harness without retaining impossible
/// dependent values.
///
/// A model is owned either by the release catalog or, for a custom id, by the
/// harness on which it was entered. The caller passes that ownership so a
/// harness click and a search result make exactly the same reconciliation.
/// Effort is independent when the new harness still offers it; only an effort
/// the new model or harness cannot accept is cleared. Goose-only permissions
/// clear elsewhere, while Pi always becomes YOLO because it has no tool gate.
pub(crate) fn reconcile_harness_selection(
    mut selection: LaunchSelection,
    model_owner: Option<LaunchHarness>,
    harness: LaunchHarness,
    catalog: &[LaunchCatalogModel],
) -> (LaunchSelection, Option<LaunchHarness>) {
    selection.harness = harness;
    let known_owners = selection.model.as_ref().map(|model| {
        catalog
            .iter()
            .filter(|candidate| candidate.id == *model)
            .map(|candidate| candidate.harness)
            .collect::<Vec<_>>()
    });
    let known_is_owned = known_owners
        .as_ref()
        .is_some_and(|owners| owners.contains(&harness));
    let owner = known_is_owned.then_some(harness).or(model_owner);
    let retained_owner = known_is_owned
        .then_some(harness)
        .or((model_owner == Some(harness)).then_some(harness));
    if (known_owners
        .as_ref()
        .is_some_and(|owners| !owners.is_empty())
        && !known_is_owned)
        || (known_owners.as_ref().is_none_or(|owners| owners.is_empty())
            && owner.is_some()
            && retained_owner.is_none())
    {
        selection.model = None;
    }
    if selection.effort.is_some_and(|effort| {
        !compatible_efforts(harness, selection.model.as_deref(), catalog).contains(&effort)
    }) {
        selection.effort = None;
    }
    selection.permissions = normalized_permissions(harness, selection.permissions);
    (selection, retained_owner)
}

/// Describe an explicit value cleared by a compatibility transition.
///
/// The initial default state is intentionally silent; this applies only when
/// a deliberate choice would otherwise disappear without explanation.
pub(crate) fn reconciliation_reset_reason(
    before: &LaunchSelection,
    after: &LaunchSelection,
) -> Option<String> {
    // Build the notice from the visible draft transition, not the
    // reconciliation branch: model and effort can disappear together, and a
    // notice must never report a choice the person did not make.
    let mut cleared = Vec::new();
    if before.model.is_some() && after.model.is_none() {
        cleared.push("the selected model is not in this harness's Farhelm offering");
    }
    if before.effort.is_some() && after.effort.is_none() {
        cleared.push("the selected effort is not in Farhelm's offering for that model or harness");
    }
    let mut notices = Vec::new();
    if !cleared.is_empty() {
        notices.push(format!("{}, so it was cleared", cleared.join("; ")));
    }
    match (before.permissions, after.harness, after.permissions) {
        (
            Some(
                LaunchPermission::Approve
                | LaunchPermission::SmartApprove
                | LaunchPermission::Chat,
            ),
            LaunchHarness::Pi,
            Some(LaunchPermission::Yolo),
        ) => notices.push(
            "the selected permission is unavailable because Pi supports only YOLO, so it was replaced"
                .to_string(),
        ),
        (Some(_), _, None) if before.permissions != after.permissions => notices.push(
            "the selected permission is unavailable for this harness, so it was cleared"
                .to_string(),
        ),
        _ => {}
    }
    (!notices.is_empty()).then(|| notices.join("; "))
}

/// Return the catalog-supported effort choices for a harness and optional
/// known model.
///
/// A custom model has no entry of its own, so it receives the released
/// harness vocabulary: the union of that harness's catalog entries. This is
/// presentation metadata only; the helm validates the final selection.
pub(crate) fn compatible_efforts(
    harness: LaunchHarness,
    model: Option<&str>,
    catalog: &[LaunchCatalogModel],
) -> Vec<LaunchEffort> {
    if let Some(model) = model
        && let Some(entry) = catalog
            .iter()
            .find(|entry| entry.harness == harness && entry.id == model)
    {
        return entry.efforts.clone();
    }

    EFFORT_ORDER
        .iter()
        .copied()
        .filter(|effort| {
            catalog
                .iter()
                .any(|entry| entry.harness == harness && entry.efforts.contains(effort))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{FolderHistoryEntry, LaunchCatalogModel};
    use crate::{HostId, LaunchEffort, LaunchHarness, LaunchPermission};

    /// Build an explicit structured setup; omitted optional choices stay
    /// omitted so grouping tests can distinguish defaults from explicit values.
    fn selection(
        harness: LaunchHarness,
        model: Option<&str>,
        effort: Option<LaunchEffort>,
    ) -> LaunchSelection {
        LaunchSelection {
            harness,
            model: model.map(str::to_string),
            effort,
            permissions: None,
        }
    }

    /// Repeated fresh allocations are one reusable repo setup. An explicit
    /// existing launch into its old cwd and a different permission choice are
    /// separate setups, and cwd filtering must never select fresh intent.
    #[test]
    fn repo_recents_group_intent_instead_of_previous_allocation_paths() {
        let fresh = LaunchHistoryEntry {
            host: HostId::default(),
            github_repo: Some(crate::github_checkout::GithubRepo::parse("acme/bar").unwrap()),
            canonical_cwd: Some("/work/bar-2".into()),
            cwd: "/work/bar-2".into(),
            selection: selection(LaunchHarness::Codex, None, None),
            created_at: 4,
            creation_seq: Some(4),
        };
        let previous = LaunchHistoryEntry {
            canonical_cwd: Some("/work/bar-1".into()),
            cwd: "/work/bar-1".into(),
            created_at: 1,
            creation_seq: Some(1),
            ..fresh.clone()
        };
        let existing = LaunchHistoryEntry {
            github_repo: None,
            created_at: 3,
            creation_seq: Some(3),
            ..fresh.clone()
        };
        let mut different_permissions = fresh.clone();
        different_permissions.selection.permissions = Some(LaunchPermission::Yolo);
        different_permissions.created_at = 2;
        different_permissions.creation_seq = Some(2);
        let history = LaunchHistory {
            checkout_config_revision: 0,
            launches: vec![
                fresh.clone(),
                existing.clone(),
                different_permissions.clone(),
                previous,
            ],
            folders: Vec::new(),
        };
        assert_eq!(
            ranked_recents(&history, &ComposerFilter::default(), None),
            vec![&fresh, &existing, &different_permissions]
        );
        assert_eq!(
            ranked_recents(&history, &ComposerFilter::default(), Some("/work/bar-2")),
            vec![&existing]
        );
        assert_eq!(
            destination_recents(
                &history,
                &ComposerFilter::default(),
                &crate::github_checkout::DestinationDraft::Existing {
                    cwd: "/work/bar-2".into()
                }
            ),
            vec![&fresh, &existing, &different_permissions]
        );
        assert_eq!(
            destination_recents(
                &history,
                &ComposerFilter::default(),
                &crate::github_checkout::DestinationDraft::github(
                    fresh.github_repo.clone().unwrap()
                )
            ),
            vec![&fresh, &different_permissions]
        );
        assert_eq!(recent_destination_label(&fresh), "gh:acme/bar");
        assert_eq!(recent_destination_label(&existing), "/work/bar-2");
    }

    /// The compact row presents its harness separately and spells the
    /// permission itself, while the complete accessible summary must still
    /// describe the exact same saved defaults. Specifically: `selection_summary`
    /// is exactly `"{harness:?} · "` followed by
    /// `selection_summary_without_harness`, which is exactly
    /// `selection_summary_before_permissions` followed by the readable
    /// permission phrase, so no two renderings of one saved
    /// setup can name different values.
    #[test]
    fn selection_summary_without_harness_preserves_the_saved_tail() {
        let selection = LaunchSelection {
            harness: LaunchHarness::Codex,
            model: Some("gpt-6-astra".into()),
            effort: Some(LaunchEffort::High),
            permissions: Some(LaunchPermission::Yolo),
        };

        assert_eq!(
            selection_summary_before_permissions(&selection),
            "model: gpt-6-astra · effort: High"
        );
        assert_eq!(
            selection_summary_without_harness(&selection),
            "model: gpt-6-astra · effort: High · permissions: yolo"
        );
        assert_eq!(
            selection_summary(&selection),
            "Codex · model: gpt-6-astra · effort: High · permissions: yolo"
        );
    }

    /// A partial composer selection must narrow only what the person chose;
    /// otherwise optional defaults would hide the very recent setups that
    /// let someone discover a prior explicit model or effort.
    #[test]
    fn absent_fields_do_not_filter_explicit_recent_choices() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                github_repo: None,
                canonical_cwd: None,
                cwd: "/work".into(),
                selection: selection(
                    LaunchHarness::Codex,
                    Some("gpt-5.6-sol"),
                    Some(LaunchEffort::High),
                ),
                created_at: 1,
                creation_seq: Some(1),
            }],
            folders: Vec::<FolderHistoryEntry>::new(),
        };
        let filter = ComposerFilter {
            harness: Some(LaunchHarness::Codex),
            ..ComposerFilter::default()
        };

        assert_eq!(matching_recents(&history, &filter, None).len(), 1);
    }

    /// Recents answer the practical question "what have I launched here?".
    /// A setup launched in another directory is not an answer, and repeating
    /// the newest setup must leave room for the next distinct choice.
    #[test]
    fn recents_are_unique_and_scoped_to_the_selected_folder() {
        let codex = selection(LaunchHarness::Codex, Some("gpt-6-astra"), None);
        let claude = selection(LaunchHarness::Claude, Some("claude-opus-4-6"), None);
        let history = LaunchHistory {
            checkout_config_revision: 0,
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/other".into(),
                    selection: codex.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: codex.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: codex.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: claude.clone(),
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
            folders: Vec::new(),
        };

        assert_eq!(
            matching_recents(&history, &ComposerFilter::default(), Some("/work"))
                .into_iter()
                .map(|entry| entry.selection.clone())
                .collect::<Vec<_>>(),
            vec![codex, claude]
        );
    }

    /// Frequency is a separate signal from recency: a one-off experiment
    /// should not hide the setup used repeatedly in the same destination.
    #[test]
    fn frequent_setup_outranks_a_newer_one_off() {
        let newest_one_off = selection(LaunchHarness::Claude, Some("claude-opus-4-6"), None);
        let frequent = selection(LaunchHarness::Codex, Some("gpt-6-astra"), None);
        let history = LaunchHistory {
            checkout_config_revision: 0,
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: newest_one_off.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: frequent.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: frequent.clone(),
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
            folders: Vec::new(),
        };

        assert_eq!(
            matching_recents(&history, &ComposerFilter::default(), Some("/work"))
                .into_iter()
                .map(|entry| entry.selection.clone())
                .collect::<Vec<_>>(),
            vec![frequent, newest_one_off]
        );
    }

    /// Equal frequencies use retained recency, and an omitted default field
    /// is not the same setup as a user-selected value. Distinct canonical
    /// destinations remain separate even when their display names resemble
    /// one another.
    #[test]
    fn recents_tie_break_defaults_and_keep_distinct_destinations_separate() {
        let default = selection(LaunchHarness::Codex, None, None);
        let explicit = selection(
            LaunchHarness::Codex,
            Some("gpt-6-astra"),
            Some(LaunchEffort::High),
        );
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![
                FolderHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: "/one/project".into(),
                    canonical_proven: true,
                    display_cwd: "/one-link".into(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                FolderHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: "/two/project".into(),
                    canonical_proven: true,
                    display_cwd: "/two-link".into(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
            ],
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one/project".into()),
                    cwd: "/one-link".into(),
                    selection: default.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one/project".into()),
                    cwd: "/one-link".into(),
                    selection: explicit.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/two/project".into()),
                    cwd: "/two-link".into(),
                    selection: default.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
            ],
        };
        assert_eq!(
            matching_recents(&history, &ComposerFilter::default(), Some("/one-link"))
                .into_iter()
                .map(|entry| entry.selection.clone())
                .collect::<Vec<_>>(),
            vec![default, explicit],
            "same-frequency choices use newest retained occurrence and do not merge defaults with explicit fields"
        );
        assert_eq!(
            matching_recents(&history, &ComposerFilter::default(), Some("/two-link")).len(),
            1,
            "a different canonical destination cannot borrow another folder's frequency"
        );
    }

    /// A known model belongs to one harness. Accepting it after a harness
    /// switch would let the dialog offer a create that the helm must refuse.
    #[test]
    fn known_model_cannot_cross_harnesses() {
        let catalog = vec![LaunchCatalogModel {
            id: "gpt-5.6-sol".into(),
            harness: LaunchHarness::Codex,
            efforts: vec![LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High],
        }];
        let wrong = selection(LaunchHarness::Claude, Some("gpt-5.6-sol"), None);

        assert!(!selection_is_compatible(&wrong, &catalog));
    }

    /// The model picker must keep its bounded default row, filter only ids,
    /// and omit an invalid OpenCode default before any renderer is involved.
    #[test]
    fn model_options_filter_and_respect_opencode_requirements() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "codex-fast".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![],
            },
            LaunchCatalogModel {
                id: "claude-fast".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![],
            },
        ];
        assert_eq!(
            model_options(&catalog, Some(LaunchHarness::Codex), "FAST", false),
            vec![
                ModelOption::HarnessDefault,
                ModelOption::Model {
                    id: "codex-fast".into(),
                    harness: LaunchHarness::Codex
                },
                ModelOption::ShowAll
            ]
        );
        assert_eq!(
            model_options(&catalog, Some(LaunchHarness::Codex), "", true),
            vec![
                ModelOption::HarnessDefault,
                ModelOption::Model {
                    id: "codex-fast".into(),
                    harness: LaunchHarness::Codex
                },
                ModelOption::Model {
                    id: "claude-fast".into(),
                    harness: LaunchHarness::Claude
                },
                ModelOption::ShowAll
            ]
        );
        assert!(matches!(
            model_options(&catalog, Some(LaunchHarness::OpenCode), "", false).first(),
            Some(ModelOption::ShowAll)
        ));
        // With no harness chosen every row is already listed, so the toggle
        // would flip its label without changing anything; it must be absent.
        assert_eq!(
            model_options(&catalog, None, "", false),
            vec![
                ModelOption::HarnessDefault,
                ModelOption::Model {
                    id: "codex-fast".into(),
                    harness: LaunchHarness::Codex
                },
                ModelOption::Model {
                    id: "claude-fast".into(),
                    harness: LaunchHarness::Claude
                },
            ]
        );
    }

    /// Enter must apply only an explicitly navigated row or the draft's own
    /// meaning, because defaulting to row zero can erase a valid selection.
    /// Exact catalog ids canonicalize and carry ownership, while unknown ids
    /// remain byte-preserving custom drafts that require a harness.
    #[test]
    fn model_enter_target_distinguishes_navigation_and_draft_semantics() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "codex-fast".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![],
            },
            LaunchCatalogModel {
                id: "claude-fast".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![],
            },
        ];
        let options = model_options(&catalog, Some(LaunchHarness::Codex), "fast", false);

        assert_eq!(
            model_enter_target(
                &options,
                Some(1),
                "unfinished-custom",
                &catalog,
                Some(LaunchHarness::Codex),
            ),
            ModelEnterTarget::Option(ModelOption::Model {
                id: "codex-fast".into(),
                harness: LaunchHarness::Codex,
            }),
            "navigation must win over the draft"
        );
        assert_eq!(
            model_enter_target(&options, None, "  ", &catalog, Some(LaunchHarness::Codex),),
            ModelEnterTarget::Nothing,
            "a whitespace-only draft must preserve the current selection"
        );
        assert_eq!(
            model_enter_target(
                &options,
                None,
                "CLAUDE-FAST",
                &catalog,
                Some(LaunchHarness::Codex),
            ),
            ModelEnterTarget::Option(ModelOption::Model {
                id: "claude-fast".into(),
                harness: LaunchHarness::Claude,
            }),
            "a known id must use its canonical catalog owner"
        );
        assert_eq!(
            model_enter_target(
                &options,
                None,
                " custom-id ",
                &catalog,
                Some(LaunchHarness::Codex),
            ),
            ModelEnterTarget::Custom {
                id: " custom-id ".into(),
                harness: LaunchHarness::Codex,
            },
            "a custom id must retain the submitted bytes and carry its harness"
        );
        assert_eq!(
            model_enter_target(&options, None, "custom-id", &catalog, None),
            ModelEnterTarget::NeedsHarness("custom-id".into()),
            "a custom id has no owner to infer"
        );
        assert_eq!(
            model_enter_target(
                &options,
                Some(options.len()),
                "custom-id",
                &catalog,
                Some(LaunchHarness::Codex),
            ),
            ModelEnterTarget::Nothing,
            "a stale active index must not fall through to draft application"
        );
    }

    /// Search and button transitions must not disagree about a model that
    /// belongs to the previous harness, while an effort the new harness does
    /// support remains an explicit user choice.
    #[test]
    fn harness_transition_clears_only_incompatible_dependencies() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "codex".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::High],
            },
            LaunchCatalogModel {
                id: "claude".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![LaunchEffort::High],
            },
        ];
        let (selection, owner) = reconcile_harness_selection(
            selection(
                LaunchHarness::Codex,
                Some("codex"),
                Some(LaunchEffort::High),
            ),
            None,
            LaunchHarness::Claude,
            &catalog,
        );
        assert_eq!(selection.harness, LaunchHarness::Claude);
        assert_eq!(selection.model, None);
        assert_eq!(selection.effort, Some(LaunchEffort::High));
        assert_eq!(owner, None);
    }

    /// A custom id has no catalog row to identify its provider. Its recorded
    /// owner supplies that missing boundary, so selecting another harness
    /// cannot make a custom value look portable merely because it is unknown.
    #[test]
    fn harness_transition_drops_a_custom_model_from_its_owner() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "codex".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::High],
            },
            LaunchCatalogModel {
                id: "claude".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![LaunchEffort::High],
            },
        ];
        let (selection, owner) = reconcile_harness_selection(
            selection(
                LaunchHarness::Codex,
                Some("private-codex-model"),
                Some(LaunchEffort::High),
            ),
            Some(LaunchHarness::Codex),
            LaunchHarness::Claude,
            &catalog,
        );
        assert_eq!(selection.model, None);
        assert_eq!(selection.effort, Some(LaunchEffort::High));
        assert_eq!(owner, None);
    }

    /// Permission reconciliation is part of the harness transition itself.
    /// Pi must never expose an omitted or Goose-only mode, while a portable
    /// YOLO choice remains selected when the user leaves Pi.
    #[test]
    fn harness_transition_normalizes_permissions_and_reports_replacements() {
        let goose = LaunchSelection {
            harness: LaunchHarness::Goose,
            model: None,
            effort: None,
            permissions: Some(LaunchPermission::SmartApprove),
        };
        let (pi, _) = reconcile_harness_selection(goose.clone(), None, LaunchHarness::Pi, &[]);
        assert_eq!(pi.permissions, Some(LaunchPermission::Yolo));
        assert_eq!(
            reconciliation_reset_reason(&goose, &pi).as_deref(),
            Some(
                "the selected permission is unavailable because Pi supports only YOLO, so it was replaced"
            )
        );

        let omitted = LaunchSelection {
            permissions: None,
            ..goose.clone()
        };
        let (pi_from_default, _) =
            reconcile_harness_selection(omitted.clone(), None, LaunchHarness::Pi, &[]);
        assert_eq!(pi_from_default.permissions, Some(LaunchPermission::Yolo));
        assert_eq!(
            reconciliation_reset_reason(&omitted, &pi_from_default),
            None
        );

        let (codex, _) = reconcile_harness_selection(pi, None, LaunchHarness::Codex, &[]);
        assert_eq!(codex.permissions, Some(LaunchPermission::Yolo));

        let (codex_from_goose, _) =
            reconcile_harness_selection(goose.clone(), None, LaunchHarness::Codex, &[]);
        assert_eq!(codex_from_goose.permissions, None);
        assert_eq!(
            reconciliation_reset_reason(&goose, &codex_from_goose).as_deref(),
            Some("the selected permission is unavailable for this harness, so it was cleared")
        );
    }

    /// Compatibility mirrors the helm boundary for all permission variants.
    /// An old Pi snapshot may omit its mode, but Goose approval modes cannot
    /// cross into any other harness.
    #[test]
    fn compatibility_rejects_goose_permissions_outside_goose() {
        for permission in [
            LaunchPermission::Approve,
            LaunchPermission::SmartApprove,
            LaunchPermission::Chat,
        ] {
            assert!(selection_is_compatible(
                &LaunchSelection {
                    harness: LaunchHarness::Goose,
                    model: Some("custom/provider-model".into()),
                    effort: None,
                    permissions: Some(permission),
                },
                &[],
            ));
            for harness in [
                LaunchHarness::Codex,
                LaunchHarness::Claude,
                LaunchHarness::Muse,
                LaunchHarness::OpenCode,
                LaunchHarness::Pi,
            ] {
                assert!(!selection_is_compatible(
                    &LaunchSelection {
                        harness,
                        model: Some("custom/provider-model".into()),
                        effort: None,
                        permissions: Some(permission),
                    },
                    &[],
                ));
            }
        }
        for permissions in [None, Some(LaunchPermission::Yolo)] {
            assert!(selection_is_compatible(
                &LaunchSelection {
                    harness: LaunchHarness::Pi,
                    model: Some("custom/provider-model".into()),
                    effort: None,
                    permissions,
                },
                &[],
            ));
        }
    }

    /// Older Pi snapshots omitted the optional field. Their summaries still
    /// have to name Pi's sole effective mode, while Goose modes use the same
    /// readable words as the controls.
    #[test]
    fn permission_summaries_are_readable_and_pi_omission_means_yolo() {
        let pi = LaunchSelection {
            harness: LaunchHarness::Pi,
            model: Some("x-ai/grok-4.6".into()),
            effort: None,
            permissions: None,
        };
        assert_eq!(selection_permission_value(&pi), "yolo");
        assert!(selection_summary(&pi).ends_with("permissions: yolo"));

        let goose = LaunchSelection {
            harness: LaunchHarness::Goose,
            permissions: Some(LaunchPermission::SmartApprove),
            ..pi
        };
        assert_eq!(selection_permission_value(&goose), "smart approve");
        assert!(selection_summary(&goose).ends_with("permissions: smart approve"));
    }

    /// A model-specific effort restriction must be reflected before submit.
    ///
    /// This keeps the picker from offering a known-invalid combination while
    /// still letting a custom model use the selected harness's released
    /// vocabulary.
    #[test]
    fn effort_options_follow_the_known_model_or_harness_catalog() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "gpt-6-astra".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::High],
            },
            LaunchCatalogModel {
                id: "gpt-5.6-sol".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High],
            },
        ];

        assert_eq!(
            compatible_efforts(LaunchHarness::Codex, Some("gpt-6-astra"), &catalog),
            vec![LaunchEffort::High]
        );
        assert_eq!(
            compatible_efforts(LaunchHarness::Codex, Some("custom"), &catalog),
            vec![LaunchEffort::Low, LaunchEffort::Medium, LaunchEffort::High]
        );
    }

    /// An exact shared model keeps a selected owner, but cannot guess between
    /// its owners while the composer still has no harness selection.
    #[test]
    fn shared_model_enter_requires_an_owner_only_when_ambiguous() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "x-ai/grok-4.6".into(),
                harness: LaunchHarness::Goose,
                efforts: vec![LaunchEffort::Low],
            },
            LaunchCatalogModel {
                id: "x-ai/grok-4.6".into(),
                harness: LaunchHarness::Pi,
                efforts: vec![LaunchEffort::Minimal],
            },
        ];
        assert_eq!(
            model_enter_target(&[], None, "x-ai/grok-4.6", &catalog, None),
            ModelEnterTarget::NeedsHarness("x-ai/grok-4.6".into())
        );
        assert_eq!(
            model_enter_target(
                &[],
                None,
                "x-ai/grok-4.6",
                &catalog,
                Some(LaunchHarness::Pi)
            ),
            ModelEnterTarget::Option(ModelOption::Model {
                id: "x-ai/grok-4.6".into(),
                harness: LaunchHarness::Pi
            })
        );
    }

    /// Effort search must expose only vocabulary that the selected choice can
    /// launch, while refusing to guess a vocabulary before a harness exists.
    #[test]
    fn search_offers_efforts_only_for_a_selected_compatible_harness() {
        let catalog = vec![
            LaunchCatalogModel {
                id: "claude-fable".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![LaunchEffort::Medium],
            },
            LaunchCatalogModel {
                id: "codex-basic".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![LaunchEffort::Low],
            },
            LaunchCatalogModel {
                id: "opencode-basic".into(),
                harness: LaunchHarness::OpenCode,
                efforts: vec![],
            },
        ];
        let history = LaunchHistory::default();

        assert!(
            search_results(
                &history,
                &catalog,
                "MED",
                Some(LaunchHarness::Claude),
                Some("claude-fable"),
            )
            .contains(&ComposerSearchResult::Effort(LaunchEffort::Medium))
        );
        assert!(
            !search_results(
                &history,
                &catalog,
                "low",
                Some(LaunchHarness::Claude),
                Some("claude-fable"),
            )
            .iter()
            .any(|result| matches!(result, ComposerSearchResult::Effort(_)))
        );
        assert!(
            !search_results(&history, &catalog, "medium", None, None)
                .iter()
                .any(|result| matches!(result, ComposerSearchResult::Effort(_)))
        );
        assert!(
            !search_results(
                &history,
                &catalog,
                "low",
                Some(LaunchHarness::OpenCode),
                Some("opencode-basic"),
            )
            .iter()
            .any(|result| matches!(result, ComposerSearchResult::Effort(_)))
        );
        // A model the catalog does not know (custom history, or typed) gets
        // the harness's released vocabulary — the union of that harness's
        // catalog efforts, the same rule the effort segment renders — never an
        // empty list. OpenCode's union is empty, so it still offers nothing.
        assert!(
            search_results(
                &history,
                &catalog,
                "med",
                Some(LaunchHarness::Claude),
                Some("claude-custom"),
            )
            .contains(&ComposerSearchResult::Effort(LaunchEffort::Medium))
        );
        assert!(
            !search_results(
                &history,
                &catalog,
                "low",
                Some(LaunchHarness::OpenCode),
                Some("opencode-custom"),
            )
            .iter()
            .any(|result| matches!(result, ComposerSearchResult::Effort(_)))
        );
    }

    /// Selecting a harness narrows both released and custom-history model
    /// results, while a fresh composer still searches every harness.
    #[test]
    fn search_models_follow_the_selected_harness() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: selection(LaunchHarness::Claude, Some("claude-private"), None),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: selection(LaunchHarness::Codex, Some("codex-private"), None),
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
            folders: Vec::new(),
        };
        let catalog = vec![
            LaunchCatalogModel {
                id: "claude-catalog".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![],
            },
            LaunchCatalogModel {
                id: "codex-catalog".into(),
                harness: LaunchHarness::Codex,
                efforts: vec![],
            },
        ];

        let claude_results = search_results(
            &history,
            &catalog,
            "catalog",
            Some(LaunchHarness::Claude),
            None,
        );
        assert_eq!(
            claude_results,
            vec![ComposerSearchResult::Model {
                id: "claude-catalog".into(),
                harness: LaunchHarness::Claude,
            }]
        );
        let selected_private = search_results(
            &history,
            &catalog,
            "private",
            Some(LaunchHarness::Claude),
            None,
        );
        assert!(selected_private.contains(&ComposerSearchResult::Model {
            id: "claude-private".into(),
            harness: LaunchHarness::Claude,
        }));
        assert!(!selected_private.contains(&ComposerSearchResult::Model {
            id: "codex-private".into(),
            harness: LaunchHarness::Codex,
        }));
        let all_results = search_results(&history, &catalog, "private", None, None);
        assert!(all_results.contains(&ComposerSearchResult::Model {
            id: "claude-private".into(),
            harness: LaunchHarness::Claude,
        }));
        assert!(all_results.contains(&ComposerSearchResult::Model {
            id: "codex-private".into(),
            harness: LaunchHarness::Codex,
        }));
    }

    /// Exact effort and model words must win over broader matches, while a
    /// query with no exact structured word keeps the first keyboard result.
    #[test]
    fn default_search_index_prefers_exact_specific_words() {
        let results = vec![
            ComposerSearchResult::Harness(LaunchHarness::Claude),
            ComposerSearchResult::Model {
                id: "claude".into(),
                harness: LaunchHarness::Claude,
            },
            ComposerSearchResult::Model {
                id: "medium-context".into(),
                harness: LaunchHarness::Claude,
            },
            ComposerSearchResult::Effort(LaunchEffort::Medium),
        ];
        let groups = grouped_search_results(results);

        assert_eq!(default_search_index(&groups, "medium"), 3);
        assert_eq!(default_search_index(&groups, "claude"), 1);
        assert_eq!(default_search_index(&groups, "partial"), 0);
    }

    /// The fixed group order places effort actions after model actions and
    /// before folder actions, so keyboard indexes stay stable as groups grow.
    #[test]
    fn grouped_search_results_places_efforts_between_models_and_folders() {
        let groups = grouped_search_results(vec![
            ComposerSearchResult::Folder("/work".into()),
            ComposerSearchResult::Effort(LaunchEffort::High),
            ComposerSearchResult::Model {
                id: "model".into(),
                harness: LaunchHarness::Claude,
            },
            ComposerSearchResult::Harness(LaunchHarness::Claude),
        ]);

        assert_eq!(
            groups.iter().map(|(group, _)| *group).collect::<Vec<_>>(),
            vec![
                ComposerSearchGroup::Harnesses,
                ComposerSearchGroup::Models,
                ComposerSearchGroup::Efforts,
                ComposerSearchGroup::Folders,
            ]
        );
    }

    /// Folder search must reuse only successful folder history, while a
    /// harness query distinguishes a partial harness choice from a saved
    /// complete setup that happens to mention it.
    #[test]
    fn search_keeps_choice_kinds_separate() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![FolderHistoryEntry {
                host: HostId::default(),
                canonical_cwd: "/work/helm".into(),
                canonical_proven: true,
                display_cwd: "~/work/helm".into(),
                created_at: 1,
                creation_seq: Some(1),
            }],
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                github_repo: None,
                canonical_cwd: None,
                cwd: "~/work/helm".into(),
                selection: selection(LaunchHarness::Claude, None, None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };
        let catalog = vec![LaunchCatalogModel {
            id: "claude-fable-5".into(),
            harness: LaunchHarness::Claude,
            efforts: vec![LaunchEffort::Low],
        }];

        assert_eq!(
            search_results(&history, &catalog, "clau", None, None),
            vec![
                ComposerSearchResult::Harness(LaunchHarness::Claude),
                ComposerSearchResult::Model {
                    id: "claude-fable-5".into(),
                    harness: LaunchHarness::Claude,
                },
                ComposerSearchResult::Recent(history.launches[0].clone()),
            ]
        );
        assert_eq!(
            search_results(&history, &catalog, "helm", None, None),
            vec![
                ComposerSearchResult::Folder("~/work/helm".into()),
                ComposerSearchResult::Recent(history.launches[0].clone()),
            ]
        );
    }

    /// Command mode must be an explicit picker result. An unmatched query is
    /// deliberately absent here: search text never becomes an invocation.
    #[test]
    fn search_offers_other_command_without_treating_text_as_a_command() {
        let history = LaunchHistory::default();

        assert_eq!(
            search_results(&history, &[], "command", None, None),
            vec![ComposerSearchResult::Command],
        );
        assert!(search_results(&history, &[], "run-this", None, None).is_empty());
    }

    /// A symlink spelling and its resolved spelling are one destination.
    ///
    /// Search must not offer both copies of the same complete setup while
    /// ordinary recents count them as one frequency group; otherwise the two
    /// surfaces would disagree about the history the helm retained.
    #[test]
    fn search_and_recents_group_aliases_by_canonical_destination() {
        let setup = selection(LaunchHarness::Codex, None, None);
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![
                FolderHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: "/real/project".into(),
                    canonical_proven: true,
                    display_cwd: "/alias/project".into(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                FolderHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: "/real/project".into(),
                    canonical_proven: true,
                    display_cwd: "/real/project".into(),
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/real/project".into()),
                    cwd: "/alias/project".into(),
                    selection: setup.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/real/project".into()),
                    cwd: "/real/project".into(),
                    selection: setup,
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
        };
        assert_eq!(
            matching_recents(&history, &ComposerFilter::default(), Some("/real/project")).len(),
            1
        );
        assert_eq!(
            search_results(&history, &[], "project", None, None)
                .into_iter()
                .filter(|result| matches!(result, ComposerSearchResult::Recent(_)))
                .count(),
            1
        );
    }

    /// Search is allowed to expose more than three results, but it must not
    /// derive a second ranking. This fixture puts frequency ahead of recency,
    /// retains default and explicit choices separately, and gives the same
    /// default selection two destinations so all aggregation identity fields
    /// participate in the comparison.
    #[test]
    fn search_and_ordinary_recents_share_complete_ranking_before_the_cap() {
        let default = selection(LaunchHarness::Codex, None, None);
        let explicit = selection(
            LaunchHarness::Codex,
            Some("gpt-6-astra"),
            Some(LaunchEffort::High),
        );
        let other = selection(LaunchHarness::Claude, None, None);
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: Vec::new(),
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: explicit.clone(),
                    created_at: 6,
                    creation_seq: Some(6),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/two".into()),
                    cwd: "/two/work".into(),
                    selection: default.clone(),
                    created_at: 5,
                    creation_seq: Some(5),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: default.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: explicit.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: default.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    github_repo: None,
                    canonical_cwd: Some("/three".into()),
                    cwd: "/three/work".into(),
                    selection: other.clone(),
                    created_at: 1,
                    creation_seq: Some(1),
                },
            ],
        };
        let ordinary = matching_recents(&history, &ComposerFilter::default(), None)
            .into_iter()
            .map(|entry| (entry.cwd.clone(), entry.selection.clone()))
            .collect::<Vec<_>>();
        let searched = search_results(&history, &[], "work", None, None)
            .into_iter()
            .filter_map(|result| match result {
                ComposerSearchResult::Recent(entry) => Some((entry.cwd, entry.selection)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            searched,
            vec![
                ("/one/work".into(), explicit.clone()),
                ("/one/work".into(), default.clone()),
                ("/two/work".into(), default.clone()),
                ("/three/work".into(), other),
            ],
            "search keeps the full frequency-and-recency ranking, including distinct destinations"
        );
        assert_eq!(ordinary, searched[..3]);
    }

    /// A path-shaped query offers deliberate actions before historical
    /// matches, while the grouped order keeps the keyboard's option indexes
    /// aligned with the headings the person can see.
    #[test]
    fn path_search_groups_explicit_actions_before_folder_history() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![FolderHistoryEntry {
                host: HostId::default(),
                canonical_cwd: "/work/helm".into(),
                canonical_proven: true,
                display_cwd: "~/work/helm".into(),
                created_at: 1,
                creation_seq: Some(1),
            }],
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                github_repo: None,
                canonical_cwd: None,
                cwd: "~/work/helm".into(),
                selection: selection(LaunchHarness::Claude, Some("claude-fable-5"), None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };

        let groups =
            grouped_search_results(search_results(&history, &[], "~/work/helm", None, None));
        assert_eq!(
            groups,
            vec![
                (
                    ComposerSearchGroup::Folders,
                    vec![
                        ComposerSearchResult::UsePath("~/work/helm".into()),
                        ComposerSearchResult::BrowsePath("~/work/helm".into()),
                        ComposerSearchResult::Folder("~/work/helm".into()),
                    ],
                ),
                (
                    ComposerSearchGroup::RecentSetups,
                    vec![ComposerSearchResult::Recent(history.launches[0].clone())],
                ),
            ],
            "a typed path is never an implicit filesystem search, but its two explicit actions and saved folder remain actionable in a stable order"
        );
    }

    /// A custom id remains selectable by search after its original session
    /// has become merely history; its recorded harness is the ownership fact
    /// that prevents an unknown model from crossing providers silently.
    #[test]
    fn search_offers_a_used_custom_model_with_its_harness() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: Vec::new(),
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                github_repo: None,
                canonical_cwd: None,
                cwd: "/work".into(),
                selection: selection(LaunchHarness::Codex, Some("release/candidate.42"), None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };
        assert!(
            search_results(&history, &[], "candidate", None, None).contains(
                &ComposerSearchResult::Model {
                    id: "release/candidate.42".into(),
                    harness: LaunchHarness::Codex,
                }
            )
        );
    }

    /// Known labels are case-insensitive and trim only the query boundaries;
    /// unknown labels remain ordinary text so provider IDs containing colons
    /// cannot be accidentally narrowed to a suffix.
    #[test]
    fn scoped_query_recognizes_known_labels_without_reparsing_unknown_ones() {
        assert_eq!(
            scoped_query("  MoDeL: provider/model:v2  "),
            (SearchScope::Model, "provider/model:v2")
        );
        assert_eq!(scoped_query("GH:"), (SearchScope::Github, ""));
        assert_eq!(
            scoped_query("folder : /tmp"),
            (SearchScope::All, "folder : /tmp")
        );
        assert_eq!(
            scoped_query("vendor:model:v2"),
            (SearchScope::All, "vendor:model:v2")
        );
    }

    /// Empty known scopes expose only their bounded kind, while GitHub queries
    /// never fall through to a local cwd or ordinary saved setup.
    #[test]
    fn scoped_search_empty_kinds_are_bounded_and_gh_is_local_free() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![FolderHistoryEntry {
                host: HostId::default(),
                canonical_cwd: "/tmp/gh:owner/repo".into(),
                canonical_proven: true,
                display_cwd: "/tmp/gh:owner/repo".into(),
                created_at: 1,
                creation_seq: Some(1),
            }],
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                canonical_cwd: None,
                github_repo: None,
                cwd: "/tmp/gh:owner/repo".into(),
                selection: selection(LaunchHarness::Claude, Some("claude-v2"), None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };
        let catalog = vec![LaunchCatalogModel {
            id: "claude-v2".into(),
            harness: LaunchHarness::Claude,
            efforts: vec![LaunchEffort::Medium],
        }];

        let harnesses = search_results(&history, &catalog, "harness:", None, None);
        assert_eq!(harnesses.len(), 7);
        assert!(harnesses.iter().all(|result| {
            matches!(
                result,
                ComposerSearchResult::Harness(_) | ComposerSearchResult::Command
            )
        }));
        assert!(harnesses.contains(&ComposerSearchResult::Command));
        assert_eq!(
            search_results(&history, &catalog, "harness:other", None, None),
            vec![ComposerSearchResult::Command]
        );
        let models = search_results(&history, &catalog, "model:", None, None);
        assert_eq!(models.len(), 1);
        assert!(models.contains(&ComposerSearchResult::Model {
            id: "claude-v2".into(),
            harness: LaunchHarness::Claude,
        }));
        assert!(
            models
                .iter()
                .all(|result| matches!(result, ComposerSearchResult::Model { .. }))
        );
        let efforts = search_results(
            &history,
            &catalog,
            "effort:",
            Some(LaunchHarness::Claude),
            Some("claude-v2"),
        );
        assert_eq!(efforts.len(), 1);
        assert!(efforts.contains(&ComposerSearchResult::Effort(LaunchEffort::Medium)));
        assert!(
            efforts
                .iter()
                .all(|result| matches!(result, ComposerSearchResult::Effort(_)))
        );
        let folders = search_results(&history, &catalog, "folder:", None, None);
        assert_eq!(folders.len(), 1);
        assert!(folders.contains(&ComposerSearchResult::Folder("/tmp/gh:owner/repo".into())));
        assert!(folders.iter().all(|result| matches!(
            result,
            ComposerSearchResult::UsePath(_)
                | ComposerSearchResult::BrowsePath(_)
                | ComposerSearchResult::Folder(_)
        )));
        let recents = search_results(&history, &catalog, "recent:", None, None);
        assert_eq!(recents.len(), 1);
        assert!(recents.contains(&ComposerSearchResult::Recent(history.launches[0].clone())));
        assert!(
            recents
                .iter()
                .all(|result| matches!(result, ComposerSearchResult::Recent(_)))
        );
        assert!(search_results(&history, &catalog, "gh:gh:owner/repo", None, None).is_empty());
    }

    /// Folder scope excludes complete setup rows, while unlabelled search
    /// retains both folder and recent matches for the same text.
    #[test]
    fn folder_scope_does_not_broaden_into_unlabelled_recent_search() {
        let history = LaunchHistory {
            checkout_config_revision: 0,
            folders: vec![FolderHistoryEntry {
                host: HostId::default(),
                canonical_cwd: "/work/project".into(),
                canonical_proven: true,
                display_cwd: "/work/project".into(),
                created_at: 1,
                creation_seq: Some(1),
            }],
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                canonical_cwd: None,
                github_repo: None,
                cwd: "/work/project".into(),
                selection: selection(LaunchHarness::Codex, None, None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };
        let scoped = search_results(&history, &[], "folder:project", None, None);
        assert_eq!(
            scoped,
            vec![ComposerSearchResult::Folder("/work/project".into())]
        );
        assert!(
            scoped
                .iter()
                .all(|result| matches!(result, ComposerSearchResult::Folder(_)))
        );
        assert!(
            search_results(&history, &[], "project", None, None)
                .iter()
                .any(|result| matches!(result, ComposerSearchResult::Recent(_)))
        );
    }

    /// Harness scope includes the legacy Other/Command action, and exact
    /// selection compares the value after a label, including model colons.
    #[test]
    fn scoped_harness_and_model_exact_words_select_the_matching_row() {
        let command = search_results(
            &LaunchHistory::default(),
            &[],
            "HARNESS:command",
            None,
            None,
        );
        let command_groups = grouped_search_results(command);
        assert_eq!(
            command_groups,
            vec![(
                ComposerSearchGroup::Harnesses,
                vec![ComposerSearchResult::Command]
            )]
        );
        assert_eq!(default_search_index(&command_groups, "harness:command"), 0);

        let catalog = vec![
            LaunchCatalogModel {
                id: "provider/model:v2-preview".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![],
            },
            LaunchCatalogModel {
                id: "provider/model:v2".into(),
                harness: LaunchHarness::Claude,
                efforts: vec![],
            },
        ];
        let model_groups = grouped_search_results(search_results(
            &LaunchHistory::default(),
            &catalog,
            "model:provider/model:v2",
            None,
            None,
        ));
        assert_eq!(model_groups[0].1.len(), 2);
        assert!(model_groups[0].1.contains(&ComposerSearchResult::Model {
            id: "provider/model:v2".into(),
            harness: LaunchHarness::Claude,
        }));
        assert_eq!(
            default_search_index(&model_groups, "MODEL: provider/model:v2"),
            1
        );

        let unknown_colon = vec![LaunchCatalogModel {
            id: "vendor:model:v2".into(),
            harness: LaunchHarness::Claude,
            efforts: vec![],
        }];
        assert!(
            search_results(
                &LaunchHistory::default(),
                &unknown_colon,
                "vendor:model:v2",
                None,
                None,
            )
            .contains(&ComposerSearchResult::Model {
                id: "vendor:model:v2".into(),
                harness: LaunchHarness::Claude,
            })
        );
    }
}

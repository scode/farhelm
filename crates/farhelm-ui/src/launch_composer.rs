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
    LaunchEffort::Low,
    LaunchEffort::Medium,
    LaunchEffort::High,
    LaunchEffort::Xhigh,
    LaunchEffort::Max,
    LaunchEffort::Ultra,
];

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
    Model {
        id: String,
        harness: LaunchHarness,
    },
    /// Apply the path the person typed without changing their agent choices.
    UsePath(String),
    /// Open the explicit directory browser at the path the person typed.
    BrowsePath(String),
    /// Reuse a directory that has succeeded on this selected host before.
    Folder(String),
    Recent(LaunchHistoryEntry),
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
    Folders,
    RecentSetups,
}

impl ComposerSearchGroup {
    /// Return the accessible heading for this stable result group.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Harnesses => "Harnesses",
            Self::Models => "Models",
            Self::Folders => "Folders",
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
    let mut folders = Vec::new();
    let mut recents = Vec::new();
    for result in results {
        match result {
            ComposerSearchResult::Harness(_) => harnesses.push(result),
            ComposerSearchResult::Model { .. } => models.push(result),
            ComposerSearchResult::UsePath(_)
            | ComposerSearchResult::BrowsePath(_)
            | ComposerSearchResult::Folder(_) => folders.push(result),
            ComposerSearchResult::Recent(_) => recents.push(result),
        }
    }
    [
        (ComposerSearchGroup::Harnesses, harnesses),
        (ComposerSearchGroup::Models, models),
        (ComposerSearchGroup::Folders, folders),
        (ComposerSearchGroup::RecentSetups, recents),
    ]
    .into_iter()
    .filter(|(_, results)| !results.is_empty())
    .collect()
}

/// Render every choice a recent setup will apply before a click changes the
/// form. Omitted fields are named as defaults so absence is never mistaken
/// for an invisible retained value.
pub(crate) fn selection_summary(selection: &LaunchSelection) -> String {
    format!(
        "{:?} · model: {} · effort: {} · permissions: {}",
        selection.harness,
        selection.model.as_deref().unwrap_or("default"),
        selection
            .effort
            .map(|effort| format!("{effort:?}"))
            .unwrap_or_else(|| "default".to_string()),
        selection
            .permissions
            .map(|permissions| format!("{permissions:?}"))
            .unwrap_or_else(|| "default".to_string()),
    )
}

/// Find composer choices whose visible value matches a deliberate query.
///
/// The server owns history ordering, so the result preserves it. This does
/// not walk a filesystem; folder search is only a lookup through prior
/// successful destinations for the selected host. A path-shaped query also
/// exposes explicit use and browse actions, but does not invoke either.
pub(crate) fn search_results(
    history: &LaunchHistory,
    catalog: &[LaunchCatalogModel],
    query: &str,
) -> Vec<ComposerSearchResult> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let folded_query = query.to_ascii_lowercase();
    let mut results = Vec::new();
    for harness in [
        LaunchHarness::Codex,
        LaunchHarness::Claude,
        LaunchHarness::Muse,
        LaunchHarness::OpenCode,
    ] {
        if format!("{harness:?}")
            .to_ascii_lowercase()
            .contains(&folded_query)
        {
            results.push(ComposerSearchResult::Harness(harness));
        }
    }
    for model in catalog {
        if model.id.to_ascii_lowercase().contains(&folded_query) {
            results.push(ComposerSearchResult::Model {
                id: model.id.clone(),
                harness: model.harness,
            });
        }
    }
    // A custom model is a complete history fact, not a catalog omission to
    // hide. Offer it as a model choice with the harness that actually ran
    // it, while keeping it out of the release-owned catalog list above.
    // Reuse the ordinary aggregation and ranking before search filters its
    // labels. Otherwise a text query quietly reordered the same saved setups
    // by raw arrival order instead of their advertised frequency/recency.
    for launch in ranked_recents(history, &ComposerFilter::default(), None) {
        let Some(id) = launch.selection.model.as_ref() else {
            continue;
        };
        if !id.to_ascii_lowercase().contains(&folded_query)
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
    // Paths are an explicit opt-in boundary. Plain language queries should
    // search the known choices only; a path-shaped query offers deliberate
    // use and browse actions without turning typing into filesystem work.
    if is_path_query(query) {
        results.push(ComposerSearchResult::UsePath(query.to_string()));
        results.push(ComposerSearchResult::BrowsePath(query.to_string()));
    }
    for folder in &history.folders {
        if folder
            .display_cwd
            .to_ascii_lowercase()
            .contains(&folded_query)
        {
            results.push(ComposerSearchResult::Folder(folder.display_cwd.clone()));
        }
    }
    // Search applies its text predicate after the same complete-selection
    // aggregation that feeds ordinary recents. The two surfaces therefore
    // keep one frequency and recency order for identical saved setups.
    for launch in ranked_recents(history, &ComposerFilter::default(), None) {
        let haystack = format!(
            "{} {:?} {}",
            launch.cwd,
            launch.selection.harness,
            launch.selection.model.as_deref().unwrap_or_default()
        )
        .to_ascii_lowercase();
        if haystack.contains(&folded_query) {
            results.push(ComposerSearchResult::Recent(launch.clone()));
        }
    }
    results
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
    let selected_destination = cwd.map(|cwd| canonical_destination(history, cwd));
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
/// to their own display fact.
fn launch_destination(entry: &LaunchHistoryEntry) -> &str {
    entry.canonical_cwd.as_deref().unwrap_or(&entry.cwd)
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
/// so this applies the harness-wide vocabulary and OpenCode's required-model
/// rule. The helm validates the final request again, including Zen provider
/// syntax for custom OpenCode IDs.
pub(crate) fn selection_is_compatible(
    selection: &LaunchSelection,
    catalog: &[LaunchCatalogModel],
) -> bool {
    if selection.harness == LaunchHarness::OpenCode && selection.model.is_none() {
        return false;
    }
    let known_model = selection
        .model
        .as_ref()
        .and_then(|model| catalog.iter().find(|candidate| candidate.id == *model));
    if known_model.is_some_and(|model| model.harness != selection.harness) {
        return false;
    }
    selection.effort.is_none_or(|effort| {
        known_model.map_or_else(
            || compatible_efforts(selection.harness, None, catalog).contains(&effort),
            |model| model.efforts.contains(&effort),
        )
    })
}

/// Move a structured choice to another harness without retaining impossible
/// dependent values.
///
/// A model is owned either by the release catalog or, for a custom id, by the
/// harness on which it was entered. The caller passes that ownership so a
/// harness click and a search result make exactly the same reconciliation.
/// Effort is independent when the new harness still offers it; only an effort
/// the new model or harness cannot accept is cleared.
pub(crate) fn reconcile_harness_selection(
    mut selection: LaunchSelection,
    model_owner: Option<LaunchHarness>,
    harness: LaunchHarness,
    catalog: &[LaunchCatalogModel],
) -> (LaunchSelection, Option<LaunchHarness>) {
    selection.harness = harness;
    let known_owner = selection.model.as_ref().and_then(|model| {
        catalog
            .iter()
            .find(|candidate| candidate.id == *model)
            .map(|candidate| candidate.harness)
    });
    let owner = known_owner.or(model_owner);
    let retained_owner = (owner == Some(harness)).then_some(harness);
    if owner.is_some() && retained_owner.is_none() {
        selection.model = None;
    }
    if selection.effort.is_some_and(|effort| {
        !compatible_efforts(harness, selection.model.as_deref(), catalog).contains(&effort)
    }) {
        selection.effort = None;
    }
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
    (!cleared.is_empty()).then(|| format!("{}, so it was cleared", cleared.join("; ")))
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
    use crate::{HostId, LaunchEffort, LaunchHarness};

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

    /// A partial composer selection must narrow only what the person chose;
    /// otherwise optional defaults would hide the very recent setups that
    /// let someone discover a prior explicit model or effort.
    #[test]
    fn absent_fields_do_not_filter_explicit_recent_choices() {
        let history = LaunchHistory {
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
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
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: None,
                    cwd: "/other".into(),
                    selection: codex.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: codex.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: codex.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
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
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: newest_one_off.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: None,
                    cwd: "/work".into(),
                    selection: frequent.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
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
                    canonical_cwd: Some("/one/project".into()),
                    cwd: "/one-link".into(),
                    selection: default.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/one/project".into()),
                    cwd: "/one-link".into(),
                    selection: explicit.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
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

    /// Folder search must reuse only successful folder history, while a
    /// harness query distinguishes a partial harness choice from a saved
    /// complete setup that happens to mention it.
    #[test]
    fn search_keeps_choice_kinds_separate() {
        let history = LaunchHistory {
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
            search_results(&history, &catalog, "clau"),
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
            search_results(&history, &catalog, "helm"),
            vec![
                ComposerSearchResult::Folder("~/work/helm".into()),
                ComposerSearchResult::Recent(history.launches[0].clone()),
            ]
        );
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
                    canonical_cwd: Some("/real/project".into()),
                    cwd: "/alias/project".into(),
                    selection: setup.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
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
            search_results(&history, &[], "project")
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
            folders: Vec::new(),
            launches: vec![
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: explicit.clone(),
                    created_at: 6,
                    creation_seq: Some(6),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/two".into()),
                    cwd: "/two/work".into(),
                    selection: default.clone(),
                    created_at: 5,
                    creation_seq: Some(5),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: default.clone(),
                    created_at: 4,
                    creation_seq: Some(4),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: explicit.clone(),
                    created_at: 3,
                    creation_seq: Some(3),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
                    canonical_cwd: Some("/one".into()),
                    cwd: "/one/work".into(),
                    selection: default.clone(),
                    created_at: 2,
                    creation_seq: Some(2),
                },
                LaunchHistoryEntry {
                    host: HostId::default(),
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
        let searched = search_results(&history, &[], "work")
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
                canonical_cwd: None,
                cwd: "~/work/helm".into(),
                selection: selection(LaunchHarness::Claude, Some("claude-fable-5"), None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };

        let groups = grouped_search_results(search_results(&history, &[], "~/work/helm"));
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
            folders: Vec::new(),
            launches: vec![LaunchHistoryEntry {
                host: HostId::default(),
                canonical_cwd: None,
                cwd: "/work".into(),
                selection: selection(LaunchHarness::Codex, Some("release/candidate.42"), None),
                created_at: 1,
                creation_seq: Some(1),
            }],
        };
        assert!(search_results(&history, &[], "candidate").contains(
            &ComposerSearchResult::Model {
                id: "release/candidate.42".into(),
                harness: LaunchHarness::Codex,
            }
        ));
    }
}

//! Tests of the recent setups' rules without a window: which earlier launches the strip offers for a draft, in what
//! order, and the words a row says. Ported from the old app's `launch_composer.rs` recent-setup tests at reference commit
//! `24bc0580` (named in each test's docs).

use farhelm_proto::session_requests::{FolderHistoryEntry, LaunchHistoryEntry, LaunchHistoryReply};
use farhelm_proto::{
    GithubRepo, LaunchEffort, LaunchHarness, LaunchPermission, LaunchSelection, LaunchTarget, Place,
};

use super::recents::{
    DraftDestination, RecentFilter, destination_label, explicit_before_permissions, merged,
    permission_value, ranked, remembered_folder, selection_summary, strip,
    summary_before_permissions, uses_guarded_default, wrapper_summary,
};

// ---- Fixtures ------------------------------------------------------------------------------------------------------

/// `harness` with every choice left to its default.
fn selection(harness: LaunchHarness) -> LaunchSelection {
    LaunchSelection {
        harness,
        model: None,
        effort: None,
        permissions: None,
        permission_flags: None,
        workspace_trust: None,
        wrapper_prefix: None,
        container: None,
    }
}

/// A launch of `selection` in the folder `cwd`, whose canonical path is the same spelling.
fn launch(cwd: &str, selection: LaunchSelection) -> LaunchHistoryEntry {
    LaunchHistoryEntry {
        host: 1,
        target: None,
        github_repo: None,
        canonical_cwd: Some(cwd.to_owned()),
        cwd: cwd.to_owned(),
        selection,
        created_at: 0,
        creation_seq: None,
    }
}

/// The history holding `launches`, newest first, and no folders.
fn history(launches: Vec<LaunchHistoryEntry>) -> LaunchHistoryReply {
    LaunchHistoryReply {
        launches,
        folders: Vec::new(),
        checkout_config_revision: 1,
    }
}

/// The harnesses of `rows`, in order.
fn harnesses(rows: &[&LaunchHistoryEntry]) -> Vec<LaunchHarness> {
    rows.iter().map(|row| row.selection.harness).collect()
}

fn repo(owner: &str, name: &str) -> GithubRepo {
    GithubRepo {
        owner: owner.into(),
        name: name.into(),
    }
}

// ---- Which setups, in what order -----------------------------------------------------------------------------------

/// A folder destination offers only the setups that launched there, one row per setup however often it launched.
/// Why: the strip offers what to launch here again, not every launch ever (old
/// `recents_are_unique_and_scoped_to_the_selected_folder`).
#[test]
fn recents_are_unique_and_scoped_to_the_folder() {
    let history = history(vec![
        launch("/work", selection(LaunchHarness::Codex)),
        launch("/other", selection(LaunchHarness::Muse)),
        launch("/work", selection(LaunchHarness::Claude)),
        launch("/work", selection(LaunchHarness::Codex)),
    ]);
    let rows = strip(
        &history,
        &RecentFilter::default(),
        DraftDestination::Folder("/work"),
    );
    assert_eq!(
        harnesses(&rows),
        [LaunchHarness::Codex, LaunchHarness::Claude]
    );
}

/// A setup launched more often outranks a newer one launched once; equally frequent setups go newest first; and the
/// strip shows three. Why: the setups the user keeps coming back to belong at the top (old
/// `frequent_setup_outranks_a_newer_one_off` and `recents_tie_break_defaults_and_keep_distinct_destinations_separate`).
#[test]
fn frequency_then_recency_and_three_at_most() {
    let history = history(vec![
        launch("/w", selection(LaunchHarness::Pi)),
        launch("/w", selection(LaunchHarness::Muse)),
        launch("/w", selection(LaunchHarness::Codex)),
        launch("/w", selection(LaunchHarness::Codex)),
        launch("/w", selection(LaunchHarness::Claude)),
    ]);
    let all = ranked(
        &history,
        &RecentFilter::default(),
        Some(DraftDestination::Folder("/w")),
    );
    assert_eq!(
        harnesses(&all),
        [
            LaunchHarness::Codex,
            LaunchHarness::Pi,
            LaunchHarness::Muse,
            LaunchHarness::Claude
        ]
    );
    let shown = strip(
        &history,
        &RecentFilter::default(),
        DraftDestination::Folder("/w"),
    );
    assert_eq!(harnesses(&shown), harnesses(&all[..3]));
}

/// A default model and an explicit one are different setups, and so are two folders. Why: each row must fill exactly
/// what it says (old `recents_tie_break_defaults_and_keep_distinct_destinations_separate`).
#[test]
fn default_and_explicit_models_and_two_folders_stay_apart() {
    let explicit = LaunchSelection {
        model: Some("gpt-5".into()),
        ..selection(LaunchHarness::Codex)
    };
    let history = history(vec![
        launch("/a", selection(LaunchHarness::Codex)),
        launch("/a", explicit),
        launch("/b", selection(LaunchHarness::Codex)),
    ]);
    assert_eq!(ranked(&history, &RecentFilter::default(), None).len(), 3);
}

/// A filter narrows only on what the draft has set: an agent type alone still offers a saved explicit model and
/// effort; a chosen model hides other models' setups. Why: the strip follows the draft without hiding setups for
/// choices the user has not made (old `absent_fields_do_not_filter_explicit_recent_choices`).
#[test]
fn only_set_choices_narrow_the_setups() {
    let saved = LaunchSelection {
        model: Some("gpt-5".into()),
        effort: Some(LaunchEffort::High),
        ..selection(LaunchHarness::Codex)
    };
    let history = history(vec![
        launch("/w", saved),
        launch("/w", selection(LaunchHarness::Claude)),
    ]);
    let codex = RecentFilter {
        harness: Some(LaunchHarness::Codex),
        ..RecentFilter::default()
    };
    assert_eq!(
        harnesses(&ranked(&history, &codex, None)),
        [LaunchHarness::Codex]
    );
    let other_model = RecentFilter {
        model: Some("o3".into()),
        ..codex
    };
    assert!(ranked(&history, &other_model, None).is_empty());
}

/// Pi's omitted permission is YOLO, so a launch that omitted it and one that chose YOLO are one setup, and a YOLO
/// filter finds it. Why: the helm launches both the same way (old `omitted_yolo_recents_group_and_filter_by_effective_mode`).
#[test]
fn an_omitted_default_and_its_explicit_value_are_one_setup() {
    let explicit = LaunchSelection {
        permissions: Some(LaunchPermission::Yolo),
        ..selection(LaunchHarness::Pi)
    };
    let history = history(vec![
        launch("/w", selection(LaunchHarness::Pi)),
        launch("/w", explicit),
    ]);
    let yolo = RecentFilter {
        permissions: Some(LaunchPermission::Yolo),
        ..RecentFilter::default()
    };
    assert_eq!(ranked(&history, &yolo, None).len(), 1);
}

/// Two spellings of one folder group by its canonical path, and the draft's spelling finds them through the history's
/// folders. Why: a symlinked alias is the same folder (old `search_and_recents_group_aliases_by_canonical_destination`).
#[test]
fn aliases_group_by_canonical_folder() {
    let mut alias = launch("~/w", selection(LaunchHarness::Codex));
    alias.canonical_cwd = Some("/home/u/w".into());
    let mut real = launch("/home/u/w", selection(LaunchHarness::Codex));
    real.canonical_cwd = Some("/home/u/w".into());
    let mut history = history(vec![alias, real]);
    history.folders.push(FolderHistoryEntry {
        host: 1,
        canonical_cwd: "/home/u/w".into(),
        canonical_proven: true,
        display_cwd: "~/w".into(),
        created_at: 0,
        creation_seq: None,
    });
    let rows = strip(
        &history,
        &RecentFilter::default(),
        DraftDestination::Folder("~/w"),
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cwd, "~/w", "the newest spelling is the row");
}

/// Repeated fresh checkouts of one repository are one setup; a folder destination still offers checkout setups; a
/// chosen repository offers only its own; a repository still being typed offers nothing. Why: a checkout setup is
/// the intent to check out that repository again, not the folder a past one happened to get (old
/// `repo_recents_group_intent_instead_of_previous_allocation_paths`).
#[test]
fn checkout_setups_group_by_repository() {
    let mut first = launch("/work/bar-1", selection(LaunchHarness::Codex));
    first.github_repo = Some(repo("acme", "bar"));
    let mut second = launch("/work/bar-2", selection(LaunchHarness::Codex));
    second.github_repo = Some(repo("acme", "bar"));
    let mut other = launch("/work/baz-1", selection(LaunchHarness::Codex));
    other.github_repo = Some(repo("acme", "baz"));
    let history = history(vec![
        first,
        second,
        other,
        launch("/w", selection(LaunchHarness::Claude)),
    ]);
    let bar = repo("acme", "bar");
    let rows = strip(
        &history,
        &RecentFilter::default(),
        DraftDestination::Repository(&bar),
    );
    assert_eq!(rows.len(), 1, "the two checkouts of acme/bar are one setup");
    assert_eq!(destination_label(rows[0]), "gh:acme/bar");
    let folder = strip(
        &history,
        &RecentFilter::default(),
        DraftDestination::Folder("/w"),
    );
    assert_eq!(
        folder.len(),
        3,
        "a folder still offers the checkout setups beside its own"
    );
    assert!(
        strip(
            &history,
            &RecentFilter::default(),
            DraftDestination::RepositoryInput
        )
        .is_empty()
    );
}

/// A Codex launch into a new container of `recipe` on host4, at `created_at`.
fn in_container(recipe: &str, created_at: i64) -> LaunchHistoryEntry {
    let mut entry = launch("~", selection(LaunchHarness::Codex));
    entry.target = Some(LaunchTarget::Environment {
        recipe: recipe.into(),
        place: Place::PodmanHost(4),
    });
    entry.created_at = created_at;
    entry
}

/// A launch into a new environment is offered whatever folder the draft holds, since it started in that environment's
/// own home, but under a chosen repository only with that repository; and its recipe and place group it as its choices
/// do, so the same choices in two recipes, or in a container and on a host, are separate setups. Why: SPEC.md,
/// "Creation", recent setups restore "recipe and place together", and one row for two placements would launch one of
/// them where the user did not pick it (old `ranked_recents`, which let a target through the folder match and grouped
/// by it).
#[test]
fn environment_launches_pass_the_folder_and_group_by_their_placement() {
    let mut checkout = in_container("rust", 0);
    checkout.github_repo = Some(repo("acme", "bar"));
    let history = history(vec![
        in_container("go", 0),
        in_container("rust", 0),
        in_container("rust", 0),
        launch("~", selection(LaunchHarness::Codex)),
        checkout,
    ]);
    let rows = |destination: DraftDestination<'_>| {
        ranked(&history, &RecentFilter::default(), Some(destination))
            .into_iter()
            .map(|row| {
                let recipe = row.target.as_ref().map(|target| target.recipe().to_owned());
                (recipe, row.github_repo.is_some())
            })
            .collect::<Vec<_>>()
    };
    let rust = || Some("rust".to_owned());
    assert_eq!(
        rows(DraftDestination::Folder("/elsewhere")),
        [
            (rust(), false),
            (Some("go".to_owned()), false),
            (rust(), true)
        ],
        "every environment setup, the more launched first; the host's `~` launch is another folder's"
    );
    assert_eq!(
        rows(DraftDestination::Folder("~")).len(),
        4,
        "the host's `~` launch is a setup of its own beside the environments'"
    );
    assert_eq!(
        rows(DraftDestination::Repository(&repo("acme", "bar"))),
        [(rust(), true)]
    );
    assert!(rows(DraftDestination::Repository(&repo("acme", "baz"))).is_empty());
}

/// The helm's launches into new environments merge into the chosen host's history by the time of each source's next
/// launch, a tie going to the host's, while each source keeps its own order; the host's saved folders stay its own, and
/// with no host's history the environments' launches stand alone, with no folders. Why: a newer environment setup must
/// not sink below every host setup because it came from another read, re-sorting either source by its clock would
/// reorder launches a clock jump made out of order, and an environment's home must never read as a host's folder (old
/// `merge_recent_launches` and `offered_recent_history`, with their tests).
#[test]
fn the_environments_launches_merge_by_each_sources_next_launch() {
    let mut ordinary = history(vec![
        launch("/a", selection(LaunchHarness::Claude)),
        launch("/b", selection(LaunchHarness::Claude)),
        launch("/c", selection(LaunchHarness::Claude)),
    ]);
    for (entry, at) in ordinary.launches.iter_mut().zip([10, 50, 5]) {
        entry.created_at = at;
    }
    ordinary.folders = vec![FolderHistoryEntry {
        host: 1,
        canonical_cwd: "/a".into(),
        canonical_proven: true,
        display_cwd: "/a".into(),
        created_at: 10,
        creation_seq: None,
    }];
    let environment = [
        in_container("x", 30),
        in_container("y", 5),
        in_container("z", 40),
    ];
    let both = merged(Some(&ordinary), &environment).expect("both histories");
    let order: Vec<String> = both
        .launches
        .iter()
        .map(|entry| match &entry.target {
            Some(target) => target.recipe().to_owned(),
            None => entry.cwd.clone(),
        })
        .collect();
    assert_eq!(order, ["x", "/a", "/b", "/c", "y", "z"]);
    assert_eq!(both.folders, ordinary.folders);

    let alone = merged(None, &environment).expect("the environments' launches");
    assert_eq!(alone.launches, environment);
    assert!(alone.folders.is_empty());
    assert_eq!(merged(None, &[]), None);
}

// ---- A row's words -------------------------------------------------------------------------------------------------

/// The full summary names every choice, defaults included, then Custom's flags; the strip's cell names only what was
/// set. Why: the accessible name and hover help say the whole setup, while the cell stays short (old
/// `selection_summary_without_harness_preserves_the_saved_tail` and `the_explicit_summary_names_only_what_the_selection_sets`).
#[test]
fn summaries_name_the_setup() {
    let saved = LaunchSelection {
        model: Some("gpt-6-astra".into()),
        effort: Some(LaunchEffort::High),
        permissions: Some(LaunchPermission::Yolo),
        ..selection(LaunchHarness::Codex)
    };
    assert_eq!(
        selection_summary(&saved),
        "Codex · Model: gpt-6-astra · Effort: High · Permissions: YOLO"
    );
    assert_eq!(
        summary_before_permissions(&saved),
        "Model: gpt-6-astra · Effort: High"
    );
    assert_eq!(
        explicit_before_permissions(&saved),
        ["Model: gpt-6-astra", "Effort: High"]
    );
    assert!(explicit_before_permissions(&selection(LaunchHarness::Codex)).is_empty());
    let custom = LaunchSelection {
        permissions: Some(LaunchPermission::Custom),
        permission_flags: Some(vec!["--yolo".into(), "a b".into()]),
        ..selection(LaunchHarness::Codex)
    };
    assert_eq!(
        selection_summary(&custom),
        "Codex · Model: Default · Effort: Default · Permissions: Custom · --yolo 'a b'"
    );
    // Two entries that restore different wrappers need distinct compact labels and complete hover names.
    for harness in [LaunchHarness::Codex, LaunchHarness::Bash] {
        for prefix in ["MODE=one", "MODE=two"] {
            let saved = LaunchSelection {
                wrapper_prefix: Some(vec!["env".into(), prefix.into()]),
                ..selection(harness)
            };
            let label = format!("Wrapper: env {prefix}");
            assert_eq!(wrapper_summary(&saved).as_deref(), Some(label.as_str()));
            assert!(selection_summary(&saved).ends_with(&label));
        }
    }
}

/// The agent's default Guarded mode goes unsaid in a row (it reads "Defaults" when nothing else was set), Cursor's
/// omitted permission is the agent's own setting, and OMP's explicit Guarded is its default too. Why: a row says what
/// differs from a fresh launch (old `compact recents distinguish guarded defaults and own-setting`).
#[test]
fn the_guarded_default_goes_unsaid() {
    assert!(uses_guarded_default(&selection(LaunchHarness::Claude)));
    let omp = LaunchSelection {
        permissions: Some(LaunchPermission::Guarded),
        ..selection(LaunchHarness::Omp)
    };
    assert!(uses_guarded_default(&omp));
    assert!(!uses_guarded_default(&selection(LaunchHarness::Cursor)));
    assert_eq!(
        permission_value(&selection(LaunchHarness::Cursor)),
        "Agent's own setting"
    );
    assert_eq!(permission_value(&selection(LaunchHarness::Pi)), "YOLO");
}

/// A setup's past workspace trust neither splits it into two rows nor shows in its words. Why: SPEC.md, "A recent
/// setup's historical trust value is ignored" (old `recent_trust_is_ignored_for_grouping_display_and_application`).
#[test]
fn trust_neither_splits_nor_shows() {
    let trusted = LaunchSelection {
        workspace_trust: Some(true),
        ..selection(LaunchHarness::Codex)
    };
    let history = history(vec![
        launch("/w", trusted.clone()),
        launch("/w", selection(LaunchHarness::Codex)),
    ]);
    assert_eq!(ranked(&history, &RecentFilter::default(), None).len(), 1);
    assert!(!selection_summary(&trusted).to_lowercase().contains("trust"));
}

// ---- New's folder default ------------------------------------------------------------------------------------------

/// The folder a host's history makes New's default is where its newest launch into a plain folder started: a newer
/// launch into a fresh managed checkout or a disposable environment does not count, and a history with neither, or
/// with no launches, gives `~`. Why: SPEC.md, "Working directory" ("the directory of the newest recent setup on that
/// installation that launched into a plain folder (a fresh managed checkout does not count), else `~`"); old
/// `remembered_folder_is_the_newest_plain_folder_launch`.
#[test]
fn the_remembered_folder_is_the_newest_plain_folder_launch() {
    let mut checkout = launch("/srv/checkouts/repo-1", selection(LaunchHarness::Codex));
    checkout.github_repo = Some(repo("acme", "repo"));
    let mut environment = launch("/home/sandbox", selection(LaunchHarness::Codex));
    environment.target = Some(LaunchTarget::Environment {
        recipe: "rust".into(),
        place: Place::PodmanHost(4),
    });
    let plain = launch("/srv/remote-work", selection(LaunchHarness::Claude));
    let older = launch("/srv/older", selection(LaunchHarness::Claude));
    assert_eq!(
        remembered_folder(&history(vec![
            checkout.clone(),
            environment.clone(),
            plain,
            older
        ])),
        "/srv/remote-work"
    );
    assert_eq!(
        remembered_folder(&history(vec![checkout, environment])),
        "~"
    );
    assert_eq!(remembered_folder(&history(Vec::new())), "~");
}

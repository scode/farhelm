//! The Tensorlake recipe list, for Settings → Tensorlake.
//!
//! One row per recipe: its name, a one-line summary of size, software and
//! tunnels, a status line, and Edit. "New recipe" and Edit swap the list for
//! the [`RecipeEditor`] in place (one focus owner inside the Settings modal,
//! like the templates dialog's list/editor swap), and the editor's Back
//! returns here.
//!
//! The list re-reads on every invalidation-feed notice, so another client's
//! edit appears without polling, and keeps showing the previous rows while a
//! re-read runs. A re-read that fails keeps those rows too, but says so with
//! a Retry: the feed notice that asked for the read is already spent, so
//! without the error and the button a stale list would stay stale, silently,
//! until some unrelated change bumped the feed again.
//!
//! Each managed recipe's status line reports its snapshot: when it was built
//! and with which agent versions, a build in progress with its step, a failed
//! or interrupted build with Retry, and staleness (packages changed, or a
//! disk now smaller than the snapshot's). Build, Rebuild and Retry are one
//! request to the helm, which starts the build or joins the running one;
//! progress then arrives through the same feed-driven re-read. A recipe that
//! reuses an image or snapshot is used as is and offers no build. The word is
//! always "snapshot", never "template", which means launch templates.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use dioxus::prelude::*;
use farhelm_proto::tensorlake::recipes::{BuildStep, BuildView, RecipeSource, RecipeView};

use super::recipe_editor::RecipeEditor;
use crate::ApiBase;
use crate::api::{build_recipe, fetch_recipes};

/// What the panel shows instead of the list.
#[derive(Clone, Debug, PartialEq)]
enum Editing {
    /// The editor for a recipe not saved yet.
    New,
    /// The editor for this recipe, as listed when Edit was clicked.
    Existing(Box<RecipeView>),
}

/// The one-line summary under a recipe's name, e.g. `4 CPU · 8 GB · 50 GB ·
/// 12 packages · 1 tunnel` or `2 CPU · 4 GB · 20 GB · uses image team-base`.
/// The tunnel count is left out when there are none.
pub(crate) fn recipe_summary(view: &RecipeView) -> String {
    let plural = |count: usize, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    let mut parts = vec![
        format!("{} CPU", view.size.cpus),
        format!("{} GB", view.size.memory_gb),
        format!("{} GB", view.size.disk_gb),
    ];
    parts.push(match &view.source {
        RecipeSource::Packages { packages } => plural(packages.len(), "package", "packages"),
        RecipeSource::Image { image } => format!("uses image {image}"),
        RecipeSource::Snapshot { snapshot_id } => format!("uses snapshot {snapshot_id}"),
    });
    if !view.tunnels.is_empty() {
        parts.push(plural(view.tunnels.len(), "tunnel", "tunnels"));
    }
    parts.join(" · ")
}

/// One line of a recipe's snapshot status, by tone.
#[derive(Clone, Debug, PartialEq, Eq)]
enum StatusLine {
    Plain(String),
    /// Something the user may want to act on (a stale snapshot).
    Warning(String),
    /// A failed build.
    Error(String),
    /// Background housekeeping the user need not act on.
    Muted(String),
}

/// The build button a row offers. All three send the same request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildAction {
    Build,
    Rebuild,
    Retry,
}

impl BuildAction {
    fn label(self) -> &'static str {
        match self {
            BuildAction::Build => "Build snapshot",
            BuildAction::Rebuild => "Rebuild snapshot",
            BuildAction::Retry => "Retry",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            BuildAction::Build => {
                "Build this recipe's snapshot in a temporary Tensorlake sandbox, which bills while it runs"
            }
            BuildAction::Rebuild => {
                "Build a fresh snapshot from this recipe, then delete the old one; existing hosts keep running"
            }
            BuildAction::Retry => "Start the snapshot build again",
        }
    }
}

/// What a recipe row says about its snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotStatus {
    lines: Vec<StatusLine>,
    /// A build is running: the row shows a busy indicator and no action.
    building: bool,
    action: Option<BuildAction>,
}

/// A UTC date and minute, `2026-11-03 14:02 UTC`. UTC and labelled, for the
/// reason `activity::utc_stamp` gives.
pub(super) fn utc_minute(secs: i64) -> String {
    let (year, month, day) = farhelm_proto::time::civil_from_days(secs.div_euclid(86_400));
    let time = secs.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}

/// "step 5 of 9".
fn step_of(step: BuildStep) -> String {
    format!("step {} of {}", step.number(), BuildStep::COUNT)
}

/// The status of a recipe's snapshot, from its view alone.
///
/// A built snapshot stays visible while a rebuild runs or after one failed,
/// because hosts keep starting from it until a new one replaces it. A build
/// that runs or was interrupted, and anything that still awaits deletion,
/// show whatever the recipe's source is now: a recipe switched to reusing an
/// image mid-build, or after a deletion failed, still has a builder or
/// snapshot that bills. Only the build actions depend on the source, since
/// the helm builds nothing for a reuse recipe.
fn snapshot_status(view: &RecipeView) -> SnapshotStatus {
    let mut lines = Vec::new();
    let packages = match &view.source {
        RecipeSource::Image { .. } => {
            lines.push(StatusLine::Plain("Uses the image as is".into()));
            None
        }
        RecipeSource::Snapshot { .. } => {
            lines.push(StatusLine::Plain("Uses the snapshot as is".into()));
            None
        }
        RecipeSource::Packages { packages } => Some(packages),
    };
    if let (Some(packages), Some(snapshot)) = (packages, &view.snapshot) {
        let mut built = format!("Snapshot built {}", utc_minute(snapshot.built_at));
        for version in [&snapshot.claude_version, &snapshot.codex_version]
            .into_iter()
            .flatten()
        {
            built.push_str(" · ");
            built.push_str(version);
        }
        lines.push(StatusLine::Plain(built));
        let mut wanted = packages.clone();
        let mut had = snapshot.packages.clone();
        wanted.sort();
        had.sort();
        if wanted != had {
            lines.push(StatusLine::Warning(
                "Packages changed since this snapshot was built; rebuild it to apply them".into(),
            ));
        }
        if view.size.disk_gb < snapshot.disk_gb {
            lines.push(StatusLine::Warning(format!(
                "The disk is now smaller than this snapshot's {} GB; rebuild before adding hosts",
                snapshot.disk_gb
            )));
        }
    }
    let (building, action) = match &view.build {
        BuildView::Building { step, .. } => {
            lines.push(StatusLine::Plain(format!(
                "Building snapshot: {} ({})",
                step.label(),
                step_of(*step)
            )));
            (true, None)
        }
        BuildView::Failed { step, message, .. } => {
            lines.push(StatusLine::Error(format!(
                "Snapshot build failed while {}: {message}",
                step.label()
            )));
            (false, Some(BuildAction::Retry))
        }
        BuildView::Interrupted { .. } => {
            lines.push(StatusLine::Warning(
                "The last snapshot build stopped when the helm stopped".into(),
            ));
            (false, Some(BuildAction::Retry))
        }
        BuildView::Idle if view.snapshot.is_some() => (false, Some(BuildAction::Rebuild)),
        BuildView::Idle if packages.is_some() => {
            lines.push(StatusLine::Plain("No snapshot yet".into()));
            (false, Some(BuildAction::Build))
        }
        BuildView::Idle => (false, None),
    };
    if view.cleanup_pending {
        lines.push(StatusLine::Muted(
            "Farhelm could not delete an old build sandbox or snapshot yet; it tries again at the next build and when the recipe is deleted"
                .into(),
        ));
    }
    SnapshotStatus {
        lines,
        building,
        action: action.filter(|_| packages.is_some()),
    }
}

/// The recipe list with its in-place editor.
///
/// `api_key_set` is whether the account section has a saved API key; build
/// actions need one and say so instead of offering a request the helm would
/// refuse.
#[component]
pub(crate) fn RecipeList(api_key_set: bool) -> Element {
    let base = use_context::<ApiBase>().0;
    let build_base = base.clone();
    // Per recipe: a build request in flight, and the helm's refusal of the
    // last one. A refusal stays until the next click or a re-read shows the
    // build running.
    let mut starting = use_signal(HashSet::<String>::new);
    let mut build_errors = use_signal(HashMap::<String, String>::new);
    let mounted = use_hook(|| Rc::new(Cell::new(true)));
    let drop_mounted = mounted.clone();
    use_drop(move || drop_mounted.set(false));
    let mut rows = use_signal(|| None::<Vec<RecipeView>>);
    let mut read_error = use_signal(|| None::<String>);
    // Only the newest read may land: a slow early read must not replace the
    // rows of a later one that already arrived.
    let mut generation = use_signal(|| 0_u64);
    let mut editing = use_signal(|| None::<Editing>);

    let reload = use_callback(move |_: ()| {
        let mine = *generation.peek() + 1;
        generation.set(mine);
        let base = base.clone();
        spawn(async move {
            let result = fetch_recipes(&base).await;
            if *generation.peek() != mine {
                return;
            }
            match result {
                Ok(recipes) => {
                    rows.set(Some(recipes));
                    read_error.set(None);
                }
                Err(reason) => read_error.set(Some(reason)),
            }
        });
    });
    use_hook(move || reload.call(()));
    crate::feed::use_feed_reader(move || reload.call(()));

    // One click of Build, Rebuild or Retry. The request outlives Settings
    // closing (a build the user asked for must still start); the row's
    // state is touched only while mounted.
    let start_build = use_callback(move |name: String| {
        if starting.peek().contains(&name) {
            return;
        }
        starting.write().insert(name.clone());
        build_errors.write().remove(&name);
        let base = build_base.clone();
        let mounted = mounted.clone();
        dioxus::core::spawn_forever(async move {
            let result = build_recipe(&base, &name).await;
            if !mounted.get() {
                return;
            }
            starting.write().remove(&name);
            match result {
                Ok(()) => reload.call(()),
                Err(reason) => {
                    build_errors.write().insert(name, reason);
                }
            }
        });
    });

    if let Some(target) = editing() {
        let (key, existing) = match target {
            Editing::New => (String::from("new"), None),
            Editing::Existing(view) => (format!("edit-{}", view.name), Some(*view)),
        };
        return rsx! {
            RecipeEditor {
                key: "{key}",
                existing,
                allow_delete: true,
                on_saved: move |_| { editing.set(None); reload.call(()); },
                on_cancel: move |_| editing.set(None),
                on_deleted: move |_| { editing.set(None); reload.call(()); },
            }
        };
    }

    let listed = rows();
    rsx! {
        section { class: "host-settings-section tensorlake-recipes", aria_label: "Recipes",
            h4 { class: "host-settings-section-title", "Recipes" }
            p { class: "host-settings-help",
                "A recipe describes the sandboxes Farhelm creates: their size, their software and the tunnels back to your side."
            }
            match (&listed, read_error()) {
                // The error and its Retry render below, rows or not.
                (None, Some(_)) => rsx! {},
                (None, None) => rsx! { p { class: "host-settings-help", role: "status", "Loading recipes…" } },
                (Some(recipes), _) if recipes.is_empty() => rsx! {
                    p { class: "host-settings-help recipe-list-empty", "No recipes yet." }
                },
                (Some(recipes), _) => rsx! {
                    ul { class: "recipe-list", aria_label: "Recipes",
                        for recipe in recipes.iter().cloned() {
                            li { key: "{recipe.name}", class: "recipe-row",
                                div { class: "recipe-row-main",
                                    span { class: "recipe-row-name font-mono", "{recipe.name}" }
                                    span { class: "recipe-row-summary", "{recipe_summary(&recipe)}" }
                                    {
                                        let status = snapshot_status(&recipe);
                                        let name = recipe.name.clone();
                                        let pending = starting.read().contains(&name);
                                        let error = build_errors.read().get(&name).cloned();
                                        rsx! {
                                            div { class: "recipe-row-snapshot", role: "status", aria_label: "Snapshot of {name}",
                                                for (index, line) in status.lines.iter().enumerate() {
                                                    match line {
                                                        StatusLine::Plain(text) => rsx! { span { key: "{index}", class: "recipe-row-status",
                                                            if status.building && index + 1 == status.lines.len() {
                                                                span { class: "delete-progress-dot", aria_hidden: "true" }
                                                            }
                                                            "{text}"
                                                        } },
                                                        StatusLine::Warning(text) => rsx! { span { key: "{index}", class: "recipe-row-status recipe-row-status-warning", "{text}" } },
                                                        StatusLine::Error(text) => rsx! { span { key: "{index}", class: "recipe-row-status recipe-row-status-error", "{text}" } },
                                                        StatusLine::Muted(text) => rsx! { span { key: "{index}", class: "recipe-row-status recipe-row-status-muted", "{text}" } },
                                                    }
                                                }
                                            }
                                            if let Some(action) = status.action {
                                                div { class: "recipe-row-actions",
                                                    button { r#type: "button", class: "btn btn-neutral recipe-row-build",
                                                        disabled: !api_key_set || pending,
                                                        aria_label: "{action.label()} for {name}",
                                                        "data-tooltip": action.tooltip(),
                                                        onclick: { let name = name.clone(); move |_| start_build.call(name.clone()) },
                                                        "{action.label()}"
                                                    }
                                                    if !api_key_set {
                                                        span { class: "recipe-row-status recipe-row-status-muted", "Save an API key above to build snapshots." }
                                                    }
                                                }
                                            }
                                            if let Some(error) = error {
                                                p { class: "checkout-root-error", role: "alert", "{error}" }
                                            }
                                        }
                                    }
                                }
                                button { r#type: "button", class: "btn btn-neutral recipe-row-edit",
                                    aria_label: "Edit {recipe.name}",
                                    "data-tooltip": "Edit this recipe",
                                    onclick: { let recipe = recipe.clone(); move |_| editing.set(Some(Editing::Existing(Box::new(recipe.clone())))) },
                                    "Edit"
                                }
                            }
                        }
                    }
                },
            }
            if let Some(reason) = read_error() {
                p { class: "checkout-root-error", role: "alert",
                    if listed.is_some() { "Could not refresh the recipes, so this list may be out of date: {reason}" } else { "{reason}" }
                }
                button { r#type: "button", class: "btn btn-neutral recipe-retry", "data-tooltip": "Read the recipes again",
                    onclick: move |_| reload.call(()), "Retry loading" }
            }
            button { r#type: "button", class: "btn btn-neutral recipe-new",
                "data-tooltip": "Write a new recipe",
                onclick: move |_| editing.set(Some(Editing::New)),
                "New recipe"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::tensorlake::recipes::{RecipeSize, SnapshotView, Tunnel};

    /// A 4 CPU / 8 GB / 50 GB recipe with `source` and `tunnels` tunnels, the
    /// mockup's first row.
    fn view(source: RecipeSource, tunnels: usize) -> RecipeView {
        RecipeView {
            name: "rust-dev".into(),
            size: RecipeSize {
                cpus: 4,
                memory_gb: 8,
                disk_gb: 50,
            },
            source,
            claude: None,
            codex: None,
            github_token_credential: None,
            claude_login_credential: None,
            codex_login_credential: None,
            tunnels: (0..tunnels)
                .map(|n| Tunnel {
                    sandbox_port: 8000 + n as u16,
                    host: "lb.internal".into(),
                    port: 8000,
                })
                .collect(),
            snapshot: None,
            build: BuildView::Idle,
            cleanup_pending: false,
        }
    }

    /// Spec: the summary reads size, then package count or the reused image or
    /// snapshot, then the tunnel count only when there are tunnels, with
    /// singular and plural counts.
    ///
    /// Why: this one line is how the list tells recipes apart at a glance; the
    /// mockup's wording is the contract the browser spec also checks.
    #[test]
    fn summaries_follow_the_mockup() {
        let packages = |n: usize| RecipeSource::Packages {
            packages: (0..n).map(|n| format!("pkg{n}")).collect(),
        };
        assert_eq!(
            recipe_summary(&view(packages(12), 1)),
            "4 CPU · 8 GB · 50 GB · 12 packages · 1 tunnel"
        );
        assert_eq!(
            recipe_summary(&view(packages(1), 2)),
            "4 CPU · 8 GB · 50 GB · 1 package · 2 tunnels"
        );
        assert_eq!(
            recipe_summary(&view(
                RecipeSource::Image {
                    image: "team-base".into()
                },
                0
            )),
            "4 CPU · 8 GB · 50 GB · uses image team-base"
        );
        assert_eq!(
            recipe_summary(&view(
                RecipeSource::Snapshot {
                    snapshot_id: "abc123".into()
                },
                0
            )),
            "4 CPU · 8 GB · 50 GB · uses snapshot abc123"
        );
    }

    /// A managed recipe with packages `git ripgrep`, 50 GB of disk, and the
    /// given snapshot and build.
    fn managed(snapshot: Option<SnapshotView>, build: BuildView) -> RecipeView {
        RecipeView {
            snapshot,
            build,
            ..view(
                RecipeSource::Packages {
                    packages: vec!["git".into(), "ripgrep".into()],
                },
                0,
            )
        }
    }

    /// A snapshot built 2026-11-03 14:02:05 UTC with `packages` on 50 GB.
    fn built(packages: &[&str]) -> SnapshotView {
        SnapshotView {
            id: "snap1".into(),
            built_at: 1_793_714_525,
            base_image: "tensorlake/ubuntu-systemd".into(),
            disk_gb: 50,
            packages: packages.iter().map(|p| p.to_string()).collect(),
            claude_version: Some("2.0.1 (Claude Code)".into()),
            codex_version: Some("codex-cli 0.40.0".into()),
        }
    }

    fn plain(text: &str) -> StatusLine {
        StatusLine::Plain(text.into())
    }

    /// Spec: a managed recipe without a snapshot offers Build; a built one
    /// says when and with which agents, and offers Rebuild; a running build
    /// shows its step as "step n of 9" with no action and keeps the built
    /// line; a failed one shows the step's phrase and the message with
    /// Retry; an interrupted one offers Retry; changed packages (in any
    /// order counts as unchanged) and a smaller disk warn; pending cleanup
    /// adds a muted line; a reused image or snapshot offers nothing.
    ///
    /// Why: this is the whole status vocabulary of the list, settled with the
    /// user; each state must offer exactly the action the helm accepts.
    #[test]
    fn snapshot_status_follows_the_build_state() {
        let idle = snapshot_status(&managed(None, BuildView::Idle));
        assert_eq!(
            (idle.lines, idle.building, idle.action),
            (
                vec![plain("No snapshot yet")],
                false,
                Some(BuildAction::Build)
            )
        );

        let built_line =
            "Snapshot built 2026-11-03 14:02 UTC · 2.0.1 (Claude Code) · codex-cli 0.40.0";
        let rebuilt = snapshot_status(&managed(Some(built(&["ripgrep", "git"])), BuildView::Idle));
        assert_eq!(
            (rebuilt.lines, rebuilt.action),
            (vec![plain(built_line)], Some(BuildAction::Rebuild))
        );

        let building = snapshot_status(&managed(
            Some(built(&["git", "ripgrep"])),
            BuildView::Building {
                step: BuildStep::Packages,
                started_at: 1,
            },
        ));
        assert_eq!(
            (building.lines, building.building, building.action),
            (
                vec![
                    plain(built_line),
                    plain("Building snapshot: installing packages (step 5 of 9)")
                ],
                true,
                None
            )
        );

        let failed = snapshot_status(&managed(
            None,
            BuildView::Failed {
                step: BuildStep::Packages,
                message: "E: Unable to locate package nope".into(),
                at: 1,
            },
        ));
        assert_eq!(
            (failed.lines, failed.action),
            (
                vec![StatusLine::Error(
                    "Snapshot build failed while installing packages: E: Unable to locate package nope".into()
                )],
                Some(BuildAction::Retry)
            )
        );

        let interrupted = snapshot_status(&managed(None, BuildView::Interrupted { started_at: 1 }));
        assert_eq!(interrupted.action, Some(BuildAction::Retry));

        let mut stale = managed(Some(built(&["git"])), BuildView::Idle);
        stale.size.disk_gb = 40;
        stale.cleanup_pending = true;
        let stale = snapshot_status(&stale);
        assert!(
            matches!(&stale.lines[1], StatusLine::Warning(text) if text.starts_with("Packages changed"))
        );
        assert!(
            matches!(&stale.lines[2], StatusLine::Warning(text) if text.starts_with("The disk is now smaller than this snapshot's 50 GB"))
        );
        assert!(matches!(&stale.lines[3], StatusLine::Muted(_)));

        for (source, text) in [
            (
                RecipeSource::Image {
                    image: "team-base".into(),
                },
                "Uses the image as is",
            ),
            (
                RecipeSource::Snapshot {
                    snapshot_id: "abc".into(),
                },
                "Uses the snapshot as is",
            ),
        ] {
            let reuse = snapshot_status(&view(source, 0));
            assert_eq!((reuse.lines, reuse.action), (vec![plain(text)], None));
        }
    }

    /// Spec: a reuse recipe shows a running or interrupted build and pending
    /// cleanup, but never offers a build action and never shows a snapshot
    /// line (its snapshot was retired when the source changed).
    ///
    /// Why: switching a recipe to an image mid-build, or after its old
    /// snapshot's deletion failed, leaves a builder or snapshot billing; the
    /// row is the only place the user can see that.
    #[test]
    fn a_reuse_recipe_still_shows_billing_build_activity_and_cleanup() {
        let image = || RecipeSource::Image {
            image: "team-base".into(),
        };
        let mut building = view(image(), 0);
        building.build = BuildView::Building {
            step: BuildStep::Codex,
            started_at: 1,
        };
        let status = snapshot_status(&building);
        assert_eq!(
            (status.lines, status.building, status.action),
            (
                vec![
                    plain("Uses the image as is"),
                    plain("Building snapshot: installing Codex (step 7 of 9)")
                ],
                true,
                None
            )
        );
        let mut interrupted = view(image(), 0);
        interrupted.build = BuildView::Interrupted { started_at: 1 };
        interrupted.cleanup_pending = true;
        interrupted.snapshot = Some(built(&["git"]));
        let status = snapshot_status(&interrupted);
        assert_eq!(status.action, None);
        assert_eq!(status.lines.len(), 3, "{:?}", status.lines);
        assert!(
            matches!(&status.lines[1], StatusLine::Warning(text) if text.starts_with("The last snapshot build stopped"))
        );
        assert!(matches!(&status.lines[2], StatusLine::Muted(_)));
    }
}

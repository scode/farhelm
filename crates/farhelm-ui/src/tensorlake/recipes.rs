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
//! Snapshot builds do not exist yet, so the status line says only whether a
//! managed recipe has a snapshot (it cannot yet) or that a reused image or
//! snapshot is used as is. The build feature replaces that line with build
//! status and Build/Rebuild actions.

use dioxus::prelude::*;
use farhelm_proto::tensorlake::recipes::{RecipeSource, RecipeView};

use super::recipe_editor::RecipeEditor;
use crate::ApiBase;
use crate::api::fetch_recipes;

/// What the panel shows instead of the list.
#[derive(Clone, Debug, PartialEq)]
enum Editing {
    /// The editor for a recipe not saved yet.
    New,
    /// The editor for this recipe, as listed when Edit was clicked.
    Existing(RecipeView),
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

/// The status line under a recipe's summary, until snapshot builds exist.
fn recipe_status(view: &RecipeView) -> &'static str {
    match view.source {
        RecipeSource::Packages { .. } => "Snapshot not built yet",
        RecipeSource::Image { .. } => "Uses the image as is",
        RecipeSource::Snapshot { .. } => "Uses the snapshot as is",
    }
}

/// The recipe list with its in-place editor.
#[component]
pub(crate) fn RecipeList() -> Element {
    let base = use_context::<ApiBase>().0;
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

    if let Some(target) = editing() {
        let (key, existing) = match target {
            Editing::New => (String::from("new"), None),
            Editing::Existing(view) => (format!("edit-{}", view.name), Some(view)),
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
                                    span { class: "recipe-row-status", "{recipe_status(&recipe)}" }
                                }
                                button { r#type: "button", class: "btn btn-neutral recipe-row-edit",
                                    aria_label: "Edit {recipe.name}",
                                    "data-tooltip": "Edit this recipe",
                                    onclick: { let recipe = recipe.clone(); move |_| editing.set(Some(Editing::Existing(recipe.clone()))) },
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
    use farhelm_proto::tensorlake::recipes::{RecipeSize, Tunnel};

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
            tunnels: (0..tunnels)
                .map(|n| Tunnel {
                    sandbox_port: 8000 + n as u16,
                    host: "lb.internal".into(),
                    port: 8000,
                })
                .collect(),
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
}

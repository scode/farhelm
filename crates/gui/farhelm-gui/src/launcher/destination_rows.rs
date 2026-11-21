//! The destination controls the launcher and the template editor both draw: a new environment's Recipe and Container
//! host ([`build_placement_controls`]), and the Folder or Managed checkout choice ([`destination_choice`]). The launcher
//! offers new environments in its host menu. The template editor offers them through Runs on. Both take their
//! defaults from [`environment_choices`].
//!
//! SPEC.md, "Creation" (the Runs on bullet) and "Container sessions". Ported from the old app's `PlacementPicker` and
//! `DestinationTypeControl`, at reference commit `24bc0580`. Neither control edits a draft itself: each reports the
//! user's choice, and the caller applies it to its own (the launcher's draft, or the template being edited). The
//! launcher's checks and notes around these rows are `crate::launcher::view::placement`'s. Here's the TLDR:
//!
//! - **New environment.** An offer needs a usable place. Taking it chooses the first recipe the provider can start
//!   from at its default
//!   place ([`ProviderUi::launch_place`]: the last container host used while it is usable, else the first usable one).
//! - **Template authoring** names a recipe and place for a future launch. It starts on the first listed container host
//!   and keeps unusable hosts selectable, each with its reason. It offers New sandbox without an account and checks
//!   only recipe base compatibility. Its Runs on switch keeps a chosen environment visible if its last place is gone.
//! - **Words per provider** come from [`ProviderUi`], whose methods are exhaustive over providers; nothing here names
//!   one. A place's own host is found through [`farhelm_proto::Place::container_host`], the place's exhaustive method.
//! - **Folder or Managed checkout** only reports the kind chosen; what changing it does is each caller's own.

use farhelm_proto::LaunchTarget;
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::tensorlake::Settings;
use farhelm_proto::ui_channel::HostView;
use gpui::{App, Window};

use super::draft::{DestinationKind, RunsOn};
use super::request::{container_reason, is_container_host};
use crate::assets::Icon;
use crate::components::{
    HoverHelp, MainAction, RadioGroup, RadioOption, Role, Select, SelectOption,
};
use crate::environment_ui::{ProviderUi, RUNS_ON};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;

/// Whether the picker names a future template destination or must offer places ready to launch now.
#[derive(Clone, Copy)]
pub(crate) enum PlacementMode<'a> {
    /// The template editor: names a recipe and place for later launches. The Runs on switch is always drawn, only a
    /// recipe whose base the provider cannot run is unavailable, and an unusable container host stays choosable with
    /// its reason.
    Authoring,
    /// The launcher: offers only places a launch can use now.
    Launch {
        /// The container host the user last launched a new container on, preferred as New container's default.
        last_container_host: Option<i64>,
        /// The account as the launcher read it, or why it could not. The provider uses it to decide what it offers
        /// and which recipes it refuses.
        account: Result<&'a Settings, &'a str>,
    },
}

/// Edits the owning draft applies synchronously so a launch on Enter can read the just-chosen value.
pub(crate) enum PlacementEdit {
    /// A new environment at its offered target, or Existing host with `None` (also when Existing host is already
    /// chosen).
    RunsOn(Option<LaunchTarget>),
    Recipe(String),
    ContainerHost(i64),
}

/// A default target for each supported provider, in menu order. `None` means its place is unavailable under this
/// caller's rules. A place with an empty recipe is still offered so the recipe control can explain what is missing.
pub(crate) fn environment_choices(
    mode: PlacementMode<'_>,
    recipes: &[RecipeView],
    hosts: &[HostView],
) -> Vec<(ProviderUi, Option<LaunchTarget>)> {
    let container_ids: Vec<_> = hosts
        .iter()
        .filter(|host| is_container_host(host))
        .filter(|host| matches!(mode, PlacementMode::Authoring) || container_reason(host).is_none())
        .map(|host| host.id)
        .collect();
    RUNS_ON
        .into_iter()
        .map(|provider| {
            let words = ProviderUi(provider);
            let place = match mode {
                PlacementMode::Authoring => words.template_place(&container_ids),
                PlacementMode::Launch {
                    last_container_host,
                    account,
                } => words.launch_place(&container_ids, last_container_host, account.ok()),
            };
            let target = place.map(|place| LaunchTarget::Environment {
                recipe: recipes
                    .iter()
                    .find(|recipe| match mode {
                        PlacementMode::Authoring => words.base_refusal(recipe).is_none(),
                        PlacementMode::Launch { .. } => words.recipe_pickable(recipe),
                    })
                    .map_or_else(String::new, |recipe| recipe.name.clone()),
                place,
            });
            (words, target)
        })
        .collect()
}

/// The shared placement controls before the caller gives each its label and position. The launcher joins Recipe
/// to the Host segment, and the template editor puts the controls on separate rows.
#[derive(Default)]
pub(crate) struct PlacementControls {
    pub runs_on: Option<RadioGroup<RunsOn>>,
    pub recipe: Option<Select>,
    pub container_host: Option<Select>,
}

/// Build Recipe and Container host and include Runs on when authoring a template.
/// Ordinary Host and launch-only notes belong to the caller. Recipes and hosts arrive in display order. An option
/// already chosen remains visible even if the backing entry disappears.
///
/// A chosen New segment can outlive its last usable place. A press on it then sends no edit at all, so
/// `PlacementEdit::RunsOn(None)` always means Existing host was pressed, and neither caller has to tell the two apart.
pub(crate) fn build_placement_controls(
    mode: PlacementMode<'_>,
    target: Option<&LaunchTarget>,
    recipes: &[RecipeView],
    hosts: &[HostView],
    on_enter: Option<MainAction>,
    edit: impl Fn(PlacementEdit, &mut Window, &mut App) + Clone + 'static,
) -> PlacementControls {
    let authoring = matches!(&mode, PlacementMode::Authoring);
    let prefix = if authoring { "templates" } else { "launcher" };
    let container_hosts: Vec<_> = hosts
        .iter()
        .filter(|host| is_container_host(host))
        .map(|host| (host, container_reason(host)))
        .collect();
    let mut controls = PlacementControls::default();
    if authoring {
        let chosen = RunsOn::of(target);
        let mut group = RadioGroup::new(
            "templates-runs-on",
            "Runs on",
            HoverHelp::tooltip("Runs on: an existing host, or a new disposable environment for each session from this template"),
            Role::Group,
        )
        .option(RadioOption::new(
            "Existing host",
            HoverHelp::tooltip("Launch on one of your existing hosts"),
            RunsOn::ExistingHost,
        ));
        let mut entering = Vec::new();
        for (words, default_target) in environment_choices(mode, recipes, hosts) {
            let segment = RunsOn::New(words.0);
            if default_target.is_some() || chosen == segment {
                group = group.option(RadioOption::new(
                    words.segment(),
                    HoverHelp::tooltip(words.segment_help()),
                    segment,
                ));
                entering.push((segment, default_target));
            }
        }
        let edit = edit.clone();
        group = group
            .chosen(Some(chosen))
            .on_change(move |segment, window, cx| {
                let target = entering
                    .iter()
                    .find(|(offered, _)| offered == segment)
                    .and_then(|(_, target)| target.clone());
                if target.is_none() && *segment != RunsOn::ExistingHost {
                    return;
                }
                edit(PlacementEdit::RunsOn(target), window, cx);
            });
        controls.runs_on = Some(group);
    }
    let Some(target) = target else {
        return controls;
    };
    let words = ProviderUi(target.provider());
    let help = HoverHelp::tooltip("Recipe: the software and credentials for the new environment");
    let options = recipes.iter().map(|recipe| {
        let (listing, refusal) = match mode {
            PlacementMode::Authoring => (None, words.base_refusal(recipe)),
            PlacementMode::Launch { account, .. } => (
                words.recipe_listing(recipe),
                words.recipe_refusal(recipe, account),
            ),
        };
        let mut label = HostSentence::new(format!("{prefix}-recipe-option"))
            .host(&HostText::from_host(recipe.name.as_str()));
        if let Some(listing) = &listing {
            label = label.text(format!(" · {}", listing.size));
        }
        let mut option =
            SelectOption::new(recipe.name.clone(), label.accessible_name(), help.clone());
        if let Some(listing) = listing {
            option = option.mark(listing.mark);
        }
        match refusal {
            Some(reason) => option.unavailable(reason),
            None => option,
        }
    });
    let edit_recipe = edit.clone();
    let mut recipe = Select::new(
        format!("{prefix}-recipe"),
        "Recipe",
        help.clone(),
        Role::ComboBox,
    )
    .options(options)
    .placeholder("Choose a recipe")
    .retained("Unavailable recipe (retained)", help)
    .on_choose(move |value, window, cx| {
        edit_recipe(PlacementEdit::Recipe(value.to_string()), window, cx);
    });
    if !target.recipe().is_empty() {
        recipe = recipe.chosen(target.recipe().to_owned());
    }
    if let Some(on_enter) = on_enter.clone() {
        recipe = recipe.on_enter(move |window, cx| on_enter(window, cx));
    }
    controls.recipe = Some(recipe.compact());

    if let Some(chosen) = target.place().container_host() {
        let help = HoverHelp::tooltip(
            "Container host: rootless Podman creates the container on this machine",
        );
        let options = container_hosts.into_iter().map(|(host, reason)| {
            let mut label = HostSentence::new(format!("{prefix}-container-host-option"))
                .host(&display_name(host));
            if let (PlacementMode::Authoring, Some(reason)) = (&mode, &reason) {
                label = label
                    .text(" · ")
                    .host(&HostText::from_host(reason.as_str()));
            }
            let option =
                SelectOption::new(host.id.to_string(), label.accessible_name(), help.clone());
            match (&mode, reason) {
                (PlacementMode::Launch { .. }, Some(reason)) => option.unavailable(reason),
                _ => option,
            }
        });
        let mut host = Select::new(
            format!("{prefix}-container-host"),
            "Container host",
            help.clone(),
            Role::ComboBox,
        )
        .options(options)
        .chosen(chosen.to_string())
        .retained("Unavailable host (retained)", help)
        .on_choose(move |value, window, cx| {
            if let Ok(host) = value.parse() {
                edit(PlacementEdit::ContainerHost(host), window, cx);
            }
        });
        if let Some(on_enter) = on_enter {
            host = host.on_enter(move |window, cx| on_enter(window, cx));
        }
        controls.container_host = Some(host.compact());
    }
    controls
}

/// The compact Folder or Managed checkout choice, with their glyphs and words, as the launcher and the template
/// editor's destination both draw it (the old app's shared `DestinationTypeControl`), under `id`, with `chosen` chosen
/// and `help` as the group's hover help. It only reports a choice through `on_change`: what changing the kind does (the
/// launcher restores a folder draft; the editor keeps the destination's text) is each caller's own. Enter on it only
/// chooses, as the old control's did.
pub(crate) fn destination_choice(
    id: &'static str,
    chosen: DestinationKind,
    help: HoverHelp,
    on_change: impl Fn(DestinationKind, &mut Window, &mut App) + 'static,
) -> RadioGroup<DestinationKind> {
    RadioGroup::new(id, "Destination type", help, Role::Group)
        .heading_segments()
        .option(
            RadioOption::new(
                "Folder",
                HoverHelp::tooltip("Folder: start in an existing folder on this host"),
                DestinationKind::Folder,
            )
            .icon(Icon::Folder),
        )
        .option(
            RadioOption::new(
                "Managed checkout",
                HoverHelp::tooltip(
                    "Managed checkout: Farhelm clones a repository into a new folder for this session",
                ),
                DestinationKind::Checkout,
            )
            .icon(Icon::Branch),
        )
        .chosen(Some(chosen))
        .on_change(move |kind, window, cx| on_change(*kind, window, cx))
}

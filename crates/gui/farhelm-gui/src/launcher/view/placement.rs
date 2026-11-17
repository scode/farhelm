//! Shared placement controls for the launcher and template editor, plus the launcher's readiness checks and notes.
//!
//! SPEC.md, "Creation" (the Runs on bullet) and "Container sessions". Ported from the old app's `PlacementPicker` and
//! `use_environment_places` (`environment_ui.rs`) with their use in `list/create_form.rs`, at reference commit
//! `24bc0580`. Here's the TLDR:
//!
//! - **The launch switch.** Existing host, then one segment per provider ([`RUNS_ON`]), as the old pressed-button group. A
//!   segment with no usable place is hidden, unless it is the one chosen, and the switch is hidden when only Existing
//!   host remains. Entering a segment chooses the first recipe the provider can start from at its default place
//!   ([`ProviderUi::launch_place`]: the last container host used while it is usable, else the first usable one); the
//!   chosen segment changes nothing, so it keeps the user's explicit recipe and place. Each is one of the draft's edits
//!   ([`crate::launcher::draft::LaunchDraft::choose_runs_on`] and its siblings), so their effects (Folder fixed to
//!   `~`) are the draft's.
//! - **Template authoring** names a recipe and place for a future launch. It starts on the first listed container host
//!   and keeps unusable hosts selectable. It offers New sandbox without an account and checks only recipe base
//!   compatibility.
//! - **Nothing copied.** The recipes, the recipe places and each host's Podman check are pushed, so the rows read the
//!   store while they draw. Besides the account read below, the one thing sent is a background Podman check of every
//!   container host as the launcher opens, or as a quick launch prepares one out of sight ([`check_container_hosts`],
//!   SPEC.md: "Opening the launcher refreshes probes in the background"), through the GUI's one way of asking for them
//!   (`crate::settings::check_container_hosts`), whose outcomes arrive through the push. `GET /api/environment-places`,
//!   whose read also schedules checks, is not read: a host's state is its pushed check, as in Settings → Containers
//!   once a host has a result. A host that can hold containers but has no result yet is listed, disabled with the
//!   helm's "needs a Podman check", until the check its opening asked for answers; one the helm cannot route a check to
//!   carries none, and says it cannot run containers.
//! - **Words per provider** come from [`ProviderUi`], whose methods are exhaustive over providers; nothing here names
//!   one. A place's own host is found through [`farhelm_proto::Place::container_host`], the place's exhaustive method.
//! - **New sandbox and the account.** The launcher reads the Tensorlake account once as it opens ([`read_account`]);
//!   New sandbox's segment, recipe options and registration note take their rules from [`ProviderUi`]'s
//!   `launch_place`, `recipe_pickable`, `recipe_listing`, `recipe_refusal` and `registration_note`.
//! - **The question before a billed create**, "Create a sandbox for this session?", is the sibling `sandbox_question`
//!   module's: a press on New sandbox asks it there, unless the account's "Create Tensorlake sandboxes without asking"
//!   is on.

use farhelm_helm_client::ApiPath;
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::tensorlake::Settings;
use farhelm_proto::ui_channel::{Availability, Collection, HostView, PREFERENCES_KEY, Preferences};
use farhelm_proto::{LaunchTarget, RecipePlaceView};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, px,
};
use std::rc::Rc;

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, after_reads, row};
use crate::components::{
    CommandWell, HoverHelp, MainAction, RadioGroup, RadioOption, Role, Select, SelectOption,
};
use crate::environment_ui::{ProviderUi, RUNS_ON};
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::launcher::draft::{Field, RunsOn};
use crate::launcher::request::{Account, Recipes, container_reason, is_container_host};
use crate::session;
use crate::theme::{self, metrics};

/// Ask the helm to check every host that can hold containers, in the background, as the launcher opens or a quick
/// launch prepares one out of sight (SPEC.md, "Container sessions": "Opening the launcher refreshes probes in the
/// background"). Each answer only says the check was asked for; its outcome reaches the Container host select through
/// the pushed hosts. A refusal is only logged: the host then keeps the reason of its last check, as Settings → Recipes
/// does.
pub(super) fn check_container_hosts(cx: &mut App) {
    let hosts = container_host_ids(cx);
    crate::settings::check_container_hosts(hosts, cx, |host, reason, _| {
        tracing::debug!(host, reason = %reason.raw(), "Podman check not asked");
    });
}

/// Read the Tensorlake account (`GET /api/tensorlake`) for `launcher`, as it opens or as a quick launch prepares it out
/// of sight: whether an API key is saved, which shows New sandbox, and Farhelm's SSH key, which its recipes need. The
/// answer, or the read's failure, becomes the launcher's one [`Account`], and a quick launch waiting for its reads goes
/// on if the catalog has answered too ([`after_reads`]). The callback holds the launcher weakly, so a launcher dropped
/// meanwhile (closed, or a reset) takes the answer nowhere. A read that cannot be sent leaves the account `Reading`:
/// that happens only without a UI session, when the catalog read cannot be sent either and a quick launch is abandoned.
pub(super) fn read_account(launcher: WeakEntity<Launcher>, cx: &mut App) {
    if let Ok(path) = ApiPath::api(&["tensorlake"]) {
        crate::operations::read::<Settings>(path, "GET /api/tensorlake", cx, move |answer, cx| {
            settle_account(&launcher, account_of(answer), cx);
        });
    }
}

/// The launcher's [`Account`] for the read's `answer`: the helm's failure words are host text, so they are kept
/// escaped, as a select option's label and the launch refusal show them.
fn account_of(answer: Result<Settings, HostText>) -> Account {
    match answer {
        Ok(settings) => Account::Read(settings),
        Err(error) => Account::Unreadable(
            HostSentence::new("launcher-account-error")
                .host(&error)
                .accessible_name()
                .to_string(),
        ),
    }
}

/// Give `launcher` its account and redraw it, then let a quick launch waiting for both reads go on.
fn settle_account(launcher: &WeakEntity<Launcher>, account: Account, cx: &mut App) {
    let Some(launcher) = launcher.upgrade() else {
        return;
    };
    launcher.update(cx, |launcher, cx| {
        launcher.account = account;
        cx.notify();
    });
    after_reads(launcher, cx);
}

/// The hosts whose Podman checks the launcher asks for: every listed host that can hold containers, whether or not it
/// has a result yet (the Container host select lists them all). None while the hosts collection is missing or
/// unavailable, because an unavailable list still names the last hosts it had, which may be gone (the shared check's
/// contract: the caller sends nothing then).
fn container_host_ids(cx: &App) -> Vec<i64> {
    let Some(store) = session::current_store(cx) else {
        return Vec::new();
    };
    let collections = store.read(cx).collections();
    match collections
        .meta(Collection::Hosts)
        .map(|meta| &meta.availability)
    {
        Some(Availability::Available) => collections
            .ordered::<HostView>()
            .into_iter()
            .filter(|host| is_container_host(host))
            .map(|host| host.id)
            .collect(),
        Some(Availability::Unavailable { .. }) | None => Vec::new(),
    }
}

/// Whether the picker names a future template destination or must offer places ready to launch now.
pub(crate) enum PlacementMode<'a> {
    /// The template editor: names a recipe and place for later launches. The Runs on switch is always drawn, only a
    /// recipe whose base the provider cannot run is unavailable, an unusable container host stays choosable with its
    /// reason, and Enter does nothing.
    Authoring,
    /// The launcher: offers only places a launch can use now, and hides the switch when only Existing host would show.
    Launch {
        /// The container host the user last launched a new container on, preferred as New container's default.
        last_container_host: Option<i64>,
        /// The Tensorlake account as the launcher read it, or why it could not; whether New sandbox is offered and
        /// which recipes it refuses depend on it, by [`ProviderUi`]'s rules.
        account: Result<&'a Settings, &'a str>,
        /// What Enter on any of the three controls does: the launcher's main action.
        on_enter: MainAction,
    },
}

/// Edits the owning draft applies synchronously so a launch on Enter can read the just-chosen value.
pub(crate) enum PlacementEdit {
    /// An existing host has no target. A new segment can also lack a target when its last usable place disappears.
    RunsOn(RunsOn, Option<LaunchTarget>),
    Recipe(String),
    ContainerHost(i64),
}

/// Draw Runs on, Recipe and Container host in the caller's existing row layout. Ordinary Host and launch-only notes
/// belong to the caller. The input catalog and hosts are ordered as displayed, with the launch and authoring rules
/// supplied by the mode. An option already chosen remains visible even if the backing entry disappears.
///
/// `edit` can receive `RunsOn(RunsOn::New(_), None)`: a press on a chosen segment whose last usable place has gone.
/// Each caller decides what that means (the launcher's draft ignores it; the template editor must drop it, or it would
/// read as Existing host). `wrap`'s first argument is the row's test selector, which a caller may ignore.
pub(crate) fn build_placement_rows(
    mode: PlacementMode<'_>,
    target: Option<&LaunchTarget>,
    recipes: &[RecipeView],
    hosts: &[HostView],
    edit: impl Fn(PlacementEdit, &mut Window, &mut App) + Clone + 'static,
    wrap: impl Fn(&'static str, &'static str, AnyElement) -> AnyElement,
) -> Vec<AnyElement> {
    let authoring = matches!(&mode, PlacementMode::Authoring);
    let (prefix, help, on_enter) = match &mode {
        PlacementMode::Authoring => (
            "templates",
            "Runs on: an existing host, or a new disposable environment for each session from this template",
            None,
        ),
        PlacementMode::Launch { on_enter, .. } => (
            "launcher",
            "Runs on: an existing host, or a new disposable environment for this session",
            Some(on_enter),
        ),
    };
    let container_hosts: Vec<_> = hosts
        .iter()
        .filter(|host| is_container_host(host))
        .map(|host| (host, container_reason(host)))
        .collect();
    let container_ids: Vec<_> = container_hosts
        .iter()
        .filter(|(_, reason)| authoring || reason.is_none())
        .map(|(host, _)| host.id)
        .collect();
    let chosen = RunsOn::of(target);
    let mut group = RadioGroup::new(
        format!("{prefix}-runs-on"),
        "Runs on",
        HoverHelp::tooltip(help),
        Role::Group,
    )
    .option(RadioOption::new(
        "Existing host",
        HoverHelp::tooltip("Launch on one of your existing hosts"),
        RunsOn::ExistingHost,
    ));
    let mut entering = Vec::new();
    for provider in RUNS_ON {
        let words = ProviderUi(provider);
        let place = match mode {
            PlacementMode::Authoring => words.template_place(&container_ids),
            PlacementMode::Launch {
                last_container_host,
                account,
                ..
            } => words.launch_place(&container_ids, last_container_host, account.ok()),
        };
        let default_target = place.map(|place| LaunchTarget::Environment {
            recipe: recipes
                .iter()
                .find(|recipe| match mode {
                    PlacementMode::Authoring => words.base_refusal(recipe).is_none(),
                    PlacementMode::Launch { .. } => words.recipe_pickable(recipe),
                })
                .map_or_else(String::new, |recipe| recipe.name.clone()),
            place,
        });
        let segment = RunsOn::New(provider);
        if default_target.is_some() || chosen == segment {
            group = group.option(RadioOption::new(
                words.segment(),
                HoverHelp::tooltip(words.segment_help()),
                segment,
            ));
            entering.push((segment, default_target));
        }
    }

    let mut rows = Vec::new();
    if authoring || !entering.is_empty() {
        let edit = edit.clone();
        group = group
            .chosen(Some(chosen))
            .on_change(move |segment, window, cx| {
                let target = entering
                    .iter()
                    .find(|(offered, _)| offered == segment)
                    .and_then(|(_, target)| target.clone());
                edit(PlacementEdit::RunsOn(*segment, target), window, cx);
            });
        if let Some(on_enter) = on_enter.cloned() {
            group = group.on_enter(move |window, cx| on_enter(window, cx));
        }
        rows.push(wrap(
            "launcher-runs-on-row",
            "Runs on",
            group.into_any_element(),
        ));
    }
    let Some(target) = target else {
        return rows;
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
    if let Some(on_enter) = on_enter.cloned() {
        recipe = recipe.on_enter(move |window, cx| on_enter(window, cx));
    }
    rows.push(wrap(
        "launcher-recipe-row",
        "Recipe",
        recipe.into_any_element(),
    ));

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
        if let Some(on_enter) = on_enter.cloned() {
            host = host.on_enter(move |window, cx| on_enter(window, cx));
        }
        rows.push(wrap(
            "launcher-container-host-row",
            "Container host",
            host.into_any_element(),
        ));
    }
    rows
}

impl Launcher {
    /// Draw the shared placement controls above Host, followed by the launcher's build note. Restart with and
    /// Replace with hold the destination, so these modes omit the controls entirely.
    pub(super) fn placement_rows(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.draft.holds(Field::Host) {
            return Vec::new();
        }
        let collections = self.store.read(cx).collections();
        let catalog = Recipes::of(collections);
        let recipes = match &catalog {
            Recipes::Listed(recipes) => recipes.as_slice(),
            Recipes::Loading | Recipes::Unreadable(_) => &[],
        };
        let last_container_host = collections
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.last_container_host);
        let (on_edit, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let target = self.draft.target.as_ref();
        let mut rows = build_placement_rows(
            PlacementMode::Launch {
                last_container_host,
                account: self.account.settings(),
                on_enter: Rc::new(move |window, cx| {
                    let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
                }),
            },
            target,
            recipes,
            &self.hosts(cx),
            move |edit, window, cx| {
                let _ = on_edit.update(cx, |launcher, cx| {
                    match edit {
                        PlacementEdit::RunsOn(segment, target) => {
                            launcher.draft.choose_runs_on(segment, target);
                            // The edit sets the folder to the environment's home, and leaving keeps it there, so the
                            // folder field must follow the draft, or Existing host would show the folder typed
                            // before while the footer and the launch use `~`.
                            launcher.sync_fields(window, cx);
                        }
                        PlacementEdit::Recipe(recipe) => launcher.draft.choose_recipe(recipe),
                        PlacementEdit::ContainerHost(host) => {
                            launcher.draft.choose_container_host(host)
                        }
                    }
                    launcher.changed(cx);
                });
            },
            |selector, label, control| row(selector, label, control, palette).into_any_element(),
        );
        if let Some(target) = target {
            rows.extend(self.place_note(target, palette, cx));
        }
        rows
    }

    /// The note under the selects, for a provider whose places build the recipe (SPEC.md: "the note says whether the
    /// recipe is ready there or builds on first launch"): the place's own reason first, then the recipe place's as the
    /// helm pushes it, building, ready on the host, or building on first launch, and "Checking build status…" before
    /// the place's state has arrived. Its words are the provider's ([`ProviderUi::launch_place_note`]).
    ///
    /// No note while the chosen container host is no longer listed, as the old `PlacementPicker` drew none without a
    /// listed place; otherwise it would say "Checking build status…" about a host that is gone. The Container host
    /// select keeps it as unavailable, and a press of Launch says why it cannot launch there.
    fn place_note(
        &self,
        target: &LaunchTarget,
        palette: &theme::Palette,
        cx: &App,
    ) -> Option<AnyElement> {
        let note = ProviderUi(target.provider()).launch_place_note()?;
        let host = target.place().container_host();
        let host_view = host.and_then(|id| self.hosts(cx).into_iter().find(|host| host.id == id));
        if host.is_some() && host_view.is_none() {
            return None;
        }
        let place_reason = host_view.as_ref().and_then(container_reason);
        let build = self
            .store
            .read(cx)
            .collections()
            .get::<RecipePlaceView>(&RecipePlaceView::key(target.recipe(), target.place()))
            .cloned();
        // The reasons are the helm's words, which can quote a host, so they are host values; the rest is the app's.
        let sentence = HostSentence::new("launcher-place-note-words");
        let sentence = match (place_reason, build) {
            (Some(reason), _) => sentence.host(&HostText::from_host(reason)),
            (None, None) => sentence.text(note.checking),
            (None, Some(build)) => match (&build.reason, build.building, build.ready) {
                (Some(reason), _, _) => sentence.host(&HostText::from_host(reason.as_str())),
                (None, true, _) => sentence.text(note.building),
                (None, false, true) => match &host_view {
                    Some(host) => sentence
                        .text(format!("{} ", note.ready_on))
                        .host(&display_name(host)),
                    None => sentence.text(note.ready_on),
                },
                (None, false, false) => sentence.text(note.first_launch),
            },
        };
        Some(
            div()
                .id("launcher-place-note")
                .debug_selector(|| "launcher-place-note".into())
                .role(Role::Status)
                .aria_label(sentence.accessible_name())
                .pl(px(LABEL_COLUMN + LABEL_GAP))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(sentence)
                .into_any_element(),
        )
    }

    /// The note under Folder while a new environment is chosen whose provider has an account to register Farhelm's SSH
    /// key with, once that key exists ([`ProviderUi::registration_note`]): the old launcher's two sentences, then the
    /// command as the helm wrote it in the shared [`CommandWell`], as Settings → Tensorlake shows it: host text shown
    /// escaped, with a Copy (`launcher-registration-command-copy`) that copies it byte for byte. The sentences are a
    /// labelled node, since plain text is not in the accessibility tree. Copy is an ordinary button, so it keeps the
    /// focus rules of every button (SPEC.md, "Keyboard focus"), and a copy moves no focus.
    ///
    /// It shows with no recipe chosen too (the catalog lists none New sandbox can start from, say), as SPEC.md
    /// says ("whether or not a recipe is chosen") and as the old launcher did: it drew it for any new environment,
    /// its `recipe().is_some()` being true for the empty name its switch entered with (`create_form.rs` at the
    /// reference commit), and the command does not depend on the recipe.
    pub(super) fn registration_note(&self, palette: &theme::Palette) -> Option<AnyElement> {
        let target = self.draft.target.as_ref()?;
        let note = ProviderUi(target.provider()).registration_note(self.account.settings().ok())?;
        let well = CommandWell::new(
            "launcher-registration-command",
            HostText::from_host(note.command),
            note.copy_label,
            HoverHelp::tooltip(note.copy_help),
        );
        Some(
            div()
                .debug_selector(|| "launcher-registration-note".into())
                .pl(px(LABEL_COLUMN + LABEL_GAP))
                .flex()
                .flex_col()
                .gap(px(6.))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_1))
                .child(
                    div()
                        .id("launcher-registration-words")
                        .role(Role::Label)
                        .aria_label(note.words)
                        .child(note.words),
                )
                .child(well)
                .into_any_element(),
        )
    }
}

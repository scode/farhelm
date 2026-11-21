//! The launcher's own side of a new environment: the checks it asks for as it opens, the Tensorlake account read, and
//! the notes it draws under the destination controls it shares with the template editor
//! (`crate::launcher::destination_rows`, which holds the rules for which environments to offer).
//!
//! SPEC.md, "Creation" (the Runs on bullet) and "Container sessions". Ported from the old app's
//! `use_environment_places` (`environment_ui.rs`) and the launcher's use of `PlacementPicker` in
//! `list/create_form.rs`, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **Edits are the draft's.** The host menu takes a default through
//!   [`crate::launcher::draft::LaunchDraft::choose_runs_on`]. Taking the same provider keeps the user's recipe and
//!   place. The draft fixes Folder to `~` when entering a new environment.
//! - **Nothing copied.** The recipes, the recipe places and each host's Podman check are pushed, so the rows read the
//!   store while they draw. Besides the account read below, the one thing sent is a background Podman check of every
//!   container host as the launcher opens, or as a quick launch prepares one out of sight ([`check_container_hosts`],
//!   SPEC.md: "Opening the launcher refreshes probes in the background"), through the GUI's one way of asking for them
//!   (`crate::settings::check_container_hosts`), whose outcomes arrive through the push. A host's state is its pushed
//!   check, as in Settings → Containers once a host has a result. A host that can hold containers but has no result yet
//!   is listed, disabled with the helm's "needs a Podman check", until the check its opening asked for answers; one the
//!   helm cannot route a check to carries none, and says it cannot run containers.
//! - **New sandbox and the account.** The launcher reads the Tensorlake account once as it opens ([`read_account`]);
//!   New sandbox's menu row, recipe options and registration note take their rules from [`ProviderUi`]'s
//!   `launch_place`, `recipe_pickable`, `recipe_listing`, `recipe_refusal` and `registration_note`.
//! - **The question before a billed create**, "Create a sandbox for this session?", is the sibling `sandbox_question`
//!   module's: attempting to launch into a new sandbox asks it there, unless "Create Tensorlake sandboxes without asking"
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

use super::{Launcher, after_reads};
use crate::components::{CommandWell, HoverHelp, Role};
use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::launcher::destination_rows::{
    PlacementControls, PlacementEdit, PlacementMode, build_placement_controls, environment_choices,
};
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

impl Launcher {
    /// The offer and recipe controls read the same catalog and account state as the host menu. The recipes' order
    /// also determines the first usable recipe a new environment defaults to.
    fn placement_inputs<'a>(
        &'a self,
        cx: &App,
    ) -> (PlacementMode<'a>, Vec<RecipeView>, Vec<HostView>) {
        let collections = self.store.read(cx).collections();
        let recipes = match Recipes::of(collections) {
            Recipes::Listed(recipes) => recipes,
            Recipes::Loading | Recipes::Unreadable(_) => Vec::new(),
        };
        let mode = PlacementMode::Launch {
            last_container_host: collections
                .get::<Preferences>(PREFERENCES_KEY)
                .and_then(|preferences| preferences.last_container_host),
            account: self.account.settings(),
        };
        (mode, recipes, self.hosts(cx))
    }

    /// The providers the host menu can enter now, with the target each would use on a first pick. Keep the selected
    /// environment visible after its place goes away so the menu can still show the draft's choice.
    pub(super) fn offered_environments(&self, cx: &App) -> Vec<LaunchTarget> {
        let (mode, recipes, hosts) = self.placement_inputs(cx);
        environment_choices(mode, &recipes, &hosts)
            .into_iter()
            .filter_map(|(provider, target)| {
                target.or_else(|| {
                    self.draft
                        .target
                        .as_ref()
                        .filter(|target| target.provider() == provider.0)
                        .cloned()
                })
            })
            .collect()
    }

    /// A real menu pick changes the draft first and then brings the text inputs into step with the environment's home.
    pub(super) fn pick_environment(
        &mut self,
        target: LaunchTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let choice = RunsOn::New(target.provider());
        if RunsOn::of(self.draft.target.as_ref()) == choice {
            // The retained choice can carry a missing place or an explicit recipe. Taking it again keeps both
            // the draft and any launch refusal explaining what is missing.
            return;
        }
        self.draft.choose_runs_on(choice, Some(target));
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// The shared recipe and container host controls for the selected environment. The address owns their layout.
    pub(super) fn placement_controls(
        &self,
        target: &LaunchTarget,
        cx: &mut Context<Self>,
    ) -> PlacementControls {
        let (mode, recipes, hosts) = self.placement_inputs(cx);
        let (on_edit, on_enter) = (cx.weak_entity(), cx.weak_entity());
        build_placement_controls(
            mode,
            Some(target),
            &recipes,
            &hosts,
            Some(Rc::new(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            })),
            move |edit, _, cx| {
                let _ = on_edit.update(cx, |launcher, cx| {
                    match edit {
                        PlacementEdit::RunsOn(_) => unreachable!("only templates draw Runs on"),
                        PlacementEdit::Recipe(recipe) => launcher.draft.choose_recipe(recipe),
                        PlacementEdit::ContainerHost(host) => {
                            launcher.draft.choose_container_host(host)
                        }
                    }
                    launcher.changed(cx);
                });
            },
        )
    }

    /// The quiet note under Container host, for a provider whose places build the recipe (SPEC.md: "the note says whether the
    /// recipe is ready there or builds on first launch"): the place's own reason first, then the recipe place's as the
    /// helm pushes it, building, ready on the host, or building on first launch, and "Checking build status…" before
    /// the place's state has arrived. Its words are the provider's ([`ProviderUi::launch_place_note`]).
    ///
    /// No note while the chosen container host is no longer listed, as the old `PlacementPicker` drew none without a
    /// listed place; otherwise it would say "Checking build status…" about a host that is gone. The Container host
    /// select keeps it as unavailable, and a press of Launch says why it cannot launch there.
    pub(super) fn place_note(
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
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.fg_2))
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

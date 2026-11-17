//! The launcher's Runs on switch, and the rows a new environment shows in place of the Host row: Recipe, Container
//! host, and the note on how the recipe stands there.
//!
//! SPEC.md, "Creation" (the Runs on bullet) and "Container sessions". Ported from the old app's `PlacementPicker` and
//! `use_environment_places` (`environment_ui.rs`) with their use in `list/create_form.rs`, at reference commit
//! `24bc0580`. Here's the TLDR:
//!
//! - **The switch.** Existing host, then one segment per provider ([`RUNS_ON`]), as the old pressed-button group. A
//!   segment with no usable place is hidden, unless it is the one chosen, and the switch is hidden when only Existing
//!   host remains. Entering a segment chooses the first recipe the provider can start from at its default place
//!   ([`ProviderUi::launch_place`]: the last container host used while it is usable, else the first usable one); the
//!   chosen segment changes nothing, so it keeps the user's explicit recipe and place. Each is one of the draft's edits
//!   ([`crate::launcher::draft::LaunchDraft::choose_runs_on`] and its siblings), so their effects (Folder fixed to
//!   `~`) are the draft's.
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
use farhelm_proto::{LaunchTarget, ProviderKind, RecipePlaceView};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, div, px,
};

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, after_reads, row};
use crate::components::{
    CommandWell, HoverHelp, RadioGroup, RadioOption, Role, Select, SelectOption,
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

/// A host's name as the selects and the note show it: the registry's display name, escaped, as an option's label takes
/// it (the template editor's host select does the same).
fn host_label(host: &HostView) -> String {
    HostSentence::new("launcher-container-host-option")
        .host(&display_name(host))
        .accessible_name()
        .to_string()
}

impl Launcher {
    /// The hosts that can hold containers, in the registry's order, with why each cannot now, if it cannot.
    fn container_hosts(&self, cx: &App) -> Vec<(HostView, Option<String>)> {
        self.hosts(cx)
            .into_iter()
            .filter(is_container_host)
            .map(|host| {
                let reason = container_reason(&host);
                (host, reason)
            })
            .collect()
    }

    /// The target entering `provider`'s segment chooses: the first recipe the provider can start from, whatever its
    /// account still lacks (none, an empty name, when the catalog lists none: [`ProviderUi::recipe_pickable`]), at the
    /// provider's default place; `None` when it has no usable place.
    fn entering(
        &self,
        provider: ProviderKind,
        recipes: &[RecipeView],
        cx: &App,
    ) -> Option<LaunchTarget> {
        let usable: Vec<i64> = self
            .container_hosts(cx)
            .into_iter()
            .filter(|(_, reason)| reason.is_none())
            .map(|(host, _)| host.id)
            .collect();
        let last = self
            .store
            .read(cx)
            .collections()
            .get::<Preferences>(PREFERENCES_KEY)
            .and_then(|preferences| preferences.last_container_host);
        let place =
            ProviderUi(provider).launch_place(&usable, last, self.account.settings().ok())?;
        let recipe = recipes
            .iter()
            .find(|recipe| ProviderUi(provider).recipe_pickable(recipe))
            .map(|recipe| recipe.name.clone())
            .unwrap_or_default();
        Some(LaunchTarget::Environment { recipe, place })
    }

    /// The Runs on switch and, while a new environment is chosen, its Recipe and Container host rows and the note, in
    /// that order, for the destination section above Host (see the module docs). Empty in a mode that holds the host
    /// (Restart with and Replace with keep the existing environment), and when only Existing host would show.
    pub(super) fn placement_rows(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if self.draft.holds(Field::Host) {
            return Vec::new();
        }
        let catalog = Recipes::of(self.store.read(cx).collections());
        let recipes: &[RecipeView] = match &catalog {
            Recipes::Listed(recipes) => recipes,
            Recipes::Loading | Recipes::Unreadable(_) => &[],
        };
        let chosen = RunsOn::of(self.draft.target.as_ref());
        let offered: Vec<(ProviderKind, Option<LaunchTarget>)> = RUNS_ON
            .iter()
            .map(|provider| (*provider, self.entering(*provider, recipes, cx)))
            .filter(|(provider, entering)| entering.is_some() || chosen == RunsOn::New(*provider))
            .collect();
        let mut rows = Vec::new();
        if !offered.is_empty() {
            rows.push(self.runs_on_row(offered, chosen, palette, cx));
        }
        if let Some(target) = self.draft.target.clone() {
            rows.push(self.recipe_row(&target, recipes, palette, cx));
            if let Some(host) = target.place().container_host() {
                rows.push(self.container_host_row(host, palette, cx));
            }
            rows.extend(self.place_note(&target, palette, cx));
        }
        rows
    }

    /// The switch itself: Existing host and the `offered` segments, each with the target entering it chooses.
    fn runs_on_row(
        &self,
        offered: Vec<(ProviderKind, Option<LaunchTarget>)>,
        chosen: RunsOn,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut group = RadioGroup::new(
            "launcher-runs-on",
            "Runs on",
            HoverHelp::tooltip(
                "Runs on: an existing host, or a new disposable environment for this session",
            ),
            Role::Group,
        )
        .option(RadioOption::new(
            "Existing host",
            HoverHelp::tooltip("Launch on one of your existing hosts"),
            RunsOn::ExistingHost,
        ));
        let mut entering = Vec::new();
        for (provider, target) in offered {
            let words = ProviderUi(provider);
            group = group.option(RadioOption::new(
                words.segment(),
                HoverHelp::tooltip(words.segment_help()),
                RunsOn::New(provider),
            ));
            entering.push((provider, target));
        }
        let (on_change, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let group = group
            .chosen(Some(chosen))
            .on_change(move |segment, window, cx| {
                let target = match segment {
                    RunsOn::ExistingHost => None,
                    RunsOn::New(provider) => entering
                        .iter()
                        .find(|(offered, _)| offered == provider)
                        .and_then(|(_, target)| target.clone()),
                };
                let _ = on_change.update(cx, |launcher, cx| {
                    launcher.draft.choose_runs_on(*segment, target);
                    // The edit sets the folder to the environment's home, and leaving keeps it there, so the folder
                    // field must follow the draft, or Existing host would show the folder typed before while the
                    // footer and the launch use `~`.
                    launcher.sync_fields(window, cx);
                    launcher.changed(cx);
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            });
        row("launcher-runs-on-row", "Runs on", group, palette).into_any_element()
    }

    /// The Recipe select: every recipe in the catalog, those the provider cannot launch from with the account as read
    /// unavailable with their reason, and a chosen recipe the catalog no longer lists kept as "Unavailable recipe
    /// (retained)". A provider that lists a recipe's size and mark (a sandbox's) has each option say "<name> · 2 CPUs ·
    /// 4 GB memory" after its mark, its reason after that.
    fn recipe_row(
        &self,
        target: &LaunchTarget,
        recipes: &[RecipeView],
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let words = ProviderUi(target.provider());
        let help =
            HoverHelp::tooltip("Recipe: the software and credentials for the new environment");
        let account = self.account.settings();
        let options = recipes.iter().map(|recipe| {
            let listing = words.recipe_listing(recipe);
            let mut label = HostSentence::new("launcher-recipe-option")
                .host(&HostText::from_host(recipe.name.as_str()));
            if let Some(listing) = &listing {
                label = label.text(format!(" · {}", listing.size));
            }
            let option = SelectOption::new(
                SharedString::from(recipe.name.clone()),
                label.accessible_name(),
                help.clone(),
            );
            let option = match listing {
                Some(listing) => option.mark(listing.mark),
                None => option,
            };
            match words.recipe_refusal(recipe, account) {
                Some(reason) => option.unavailable(reason),
                None => option,
            }
        });
        let (on_choose, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let mut select = Select::new("launcher-recipe", "Recipe", help.clone(), Role::ComboBox)
            .options(options)
            .placeholder("Choose a recipe")
            .retained("Unavailable recipe (retained)", help)
            .on_choose(move |recipe, _, cx| {
                let recipe = recipe.to_string();
                let _ = on_choose.update(cx, |launcher, cx| {
                    launcher.draft.choose_recipe(recipe);
                    launcher.changed(cx);
                });
            })
            .on_enter(move |window, cx| {
                let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
            });
        if !target.recipe().is_empty() {
            select = select.chosen(SharedString::from(target.recipe().to_owned()));
        }
        row("launcher-recipe-row", "Recipe", select, palette).into_any_element()
    }

    /// The Container host select: every host that can hold containers, those whose Podman is unusable now unavailable
    /// with their reason (SPEC.md: "Hosts with unusable Podman are disabled with their reason"), and a chosen host the
    /// registry no longer lists kept as "Unavailable host (retained)".
    fn container_host_row(
        &self,
        chosen: i64,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let help = HoverHelp::tooltip(
            "Container host: rootless Podman creates the container on this machine",
        );
        let options: Vec<SelectOption> = self
            .container_hosts(cx)
            .into_iter()
            .map(|(host, reason)| {
                let option =
                    SelectOption::new(host.id.to_string(), host_label(&host), help.clone());
                match reason {
                    Some(reason) => option.unavailable(reason),
                    None => option,
                }
            })
            .collect();
        let (on_choose, on_enter) = (cx.weak_entity(), cx.weak_entity());
        let select = Select::new(
            "launcher-container-host",
            "Container host",
            help.clone(),
            Role::ComboBox,
        )
        .options(options)
        .chosen(chosen.to_string())
        .retained("Unavailable host (retained)", help)
        .on_choose(move |host, _, cx| {
            let Ok(host) = host.parse::<i64>() else {
                return;
            };
            let _ = on_choose.update(cx, |launcher, cx| {
                launcher.draft.choose_container_host(host);
                launcher.changed(cx);
            });
        })
        .on_enter(move |window, cx| {
            let _ = on_enter.update(cx, |launcher, cx| launcher.press(window, cx));
        });
        row(
            "launcher-container-host-row",
            "Container host",
            select,
            palette,
        )
        .into_any_element()
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

//! The launcher's own side of a new environment: the checks it asks for as it opens, the Tensorlake account read, and
//! the notes it draws under the destination controls it shares with the template editor
//! (`crate::launcher::destination_rows`, which holds the Runs on switch's rules).
//!
//! SPEC.md, "Creation" (the Runs on bullet) and "Container sessions". Ported from the old app's
//! `use_environment_places` (`environment_ui.rs`) and the launcher's use of `PlacementPicker` in
//! `list/create_form.rs`, at reference commit `24bc0580`. Here's the TLDR:
//!
//! - **Edits are the draft's.** Each control's edit is one of the draft's
//!   ([`crate::launcher::draft::LaunchDraft::choose_runs_on`] and its siblings), so their effects (Folder fixed to `~`)
//!   are the draft's, and a press on the segment already chosen changes nothing, which keeps the user's explicit recipe
//!   and place.
//! - **Nothing copied.** The recipes, the recipe places and each host's Podman check are pushed, so the rows read the
//!   store while they draw. Besides the account read below, the one thing sent is a background Podman check of every
//!   container host as the launcher opens, or as a quick launch prepares one out of sight ([`check_container_hosts`],
//!   SPEC.md: "Opening the launcher refreshes probes in the background"), through the GUI's one way of asking for them
//!   (`crate::settings::check_container_hosts`), whose outcomes arrive through the push. A host's state is its pushed
//!   check, as in Settings → Containers once a host has a result. A host that can hold containers but has no result yet
//!   is listed, disabled with the helm's "needs a Podman check", until the check its opening asked for answers; one the
//!   helm cannot route a check to carries none, and says it cannot run containers.
//! - **New sandbox and the account.** The launcher reads the Tensorlake account once as it opens ([`read_account`]);
//!   New sandbox's segment, recipe options and registration note take their rules from [`ProviderUi`]'s
//!   `launch_place`, `recipe_pickable`, `recipe_listing`, `recipe_refusal` and `registration_note`.
//! - **The question before a billed create**, "Create a sandbox for this session?", is the sibling `sandbox_question`
//!   module's: a press on New sandbox asks it there, unless the account's "Create Tensorlake sandboxes without asking"
//!   is on.

use farhelm_helm_client::ApiPath;
use farhelm_proto::tensorlake::Settings;
use farhelm_proto::ui_channel::{Availability, Collection, HostView, PREFERENCES_KEY, Preferences};
use farhelm_proto::{LaunchTarget, RecipePlaceView};
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, div, px,
};
use std::rc::Rc;

use super::{LABEL_COLUMN, LABEL_GAP, Launcher, after_reads, row};
use crate::components::{CommandWell, HoverHelp, Role};
use crate::environment_ui::ProviderUi;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::launcher::destination_rows::{PlacementEdit, PlacementMode, build_placement_rows};
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
                        PlacementEdit::RunsOn(target) => {
                            launcher
                                .draft
                                .choose_runs_on(RunsOn::of(target.as_ref()), target);
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

//! The GUI's words for disposable-environment providers: one exhaustive answer per provider for each thing the GUI says
//! differently about a Tensorlake sandbox and a container.
//!
//! AGENTS.md ("Code rules") puts every provider's UI wording in one exhaustive UI module, so that adding a provider
//! fails to compile until it has chosen its own words, and no shared code compares providers. This is that module for
//! the GPUI app, ported from the old app's `environment_ui.rs` at the reference commit. It holds only what a GPUI
//! surface has needed so far, starting with an agent's environment-launch approval card; the launcher's and host
//! management's environment surfaces add their own methods here as they are ported, rather than starting a second
//! module. The Hosts panel's deletion of a sandbox or container and its details take theirs from here too
//! (`host_delete_label`, `delete_consequence`, `deleting_detail`, `locality_label`). The session list's and the session view's environment session rows, controls and suspended card take theirs
//! from the section on a session's own environment below. The launcher's refusals and its name for a new environment
//! take their words from here (`recipe_refusal`, [`target_label`]).

use farhelm_proto::recipes::{RecipeBase, RecipeView};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchTarget, Place, ProviderKind};

use crate::assets::Icon;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::local_time::{self, Zone};

/// One provider, seen through the GUI's vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProviderUi(pub(crate) ProviderKind);

/// The place row of an agent's environment-launch card, as [`ProviderUi::agent_launch_place`] words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentLaunchPlace {
    /// A labelled row whose value is the name of the host the place is on, which the card supplies as host text.
    NamedHost(&'static str),
    /// A label and this app's own words, for a place with no host of the user's to name.
    Note(&'static str, &'static str),
}

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderUi {
    /// What an agent's environment-launch card says will happen on Allow. That card is the creation confirmation
    /// (SPEC.md, "Agents and environments"), so it carries the consequence the launcher's own footer does, billing
    /// included, in this app's words rather than the helm's.
    pub(crate) fn agent_launch_consequence(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Allow creates a new Tensorlake sandbox for this session. It bills while running; Suspend stops CPU \
                 and memory billing, and deleting its last session removes the sandbox and its files."
            }
            ProviderKind::Podman => {
                "Allow creates a new container for this session on the container host. Deleting its last session \
                 removes the container and its files."
            }
        }
    }

    /// How an agent's environment-launch card names where the new environment goes. Decided here rather than from
    /// whether the card's place carries a host, so that a new provider has to choose its own words: a container lands
    /// on a container host the card names, while a Tensorlake sandbox has no host of the user's to name.
    pub(crate) fn agent_launch_place(self) -> AgentLaunchPlace {
        match self.0 {
            ProviderKind::Tensorlake => {
                AgentLaunchPlace::Note("creates", "a new Tensorlake sandbox")
            }
            ProviderKind::Podman => AgentLaunchPlace::NamedHost("new container on host"),
        }
    }

    /// The line a recipe's row in Settings → Recipes shows when the recipe's base cannot run on this provider, in place
    /// of the provider's build action. Tensorlake's is the old row's (`snapshot_status` in `tensorlake/recipes.rs` at
    /// the reference commit). Podman's follows the helm's own refusal of a container rebuild for such a base ("this
    /// recipe's base cannot run in a container"), since the old app drew no such line for a container place: its
    /// per-place rows listed only the places a base supports. Nothing asks for Podman's words yet: the row asks only
    /// for the Tensorlake account's, and its per-place container lines ([`ProviderUi::recipe_place`]) show the helm's
    /// own reason for a place whose base it cannot run, as the old lines did.
    pub(crate) fn recipe_base_unsupported(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "This recipe's base cannot run on Tensorlake",
            ProviderKind::Podman => "This recipe's base cannot run in a container",
        }
    }

    /// The words of a recipe's per-place line in Settings → Recipes for a place of this provider: what the line says
    /// while the place builds, once it is ready, before anything is built there, while an older artifact still awaits
    /// cleanup, and its Rebuild's hover help; `None` for a provider whose places get no such line. This is where the row
    /// decides which places it lists one by one. Podman's words are the old app's (`RecipePlaces` in
    /// `containers_settings.rs` at the reference commit). The Tensorlake account gets no line: the row's built-snapshot
    /// and prepared-snapshot lines already report that place.
    pub(crate) fn recipe_place(self) -> Option<RecipePlaceWords> {
        match self.0 {
            ProviderKind::Tensorlake => None,
            ProviderKind::Podman => Some(RecipePlaceWords {
                building: "Preparing…",
                ready: "Ready",
                not_built: "Not built; builds on first launch",
                cleanup_pending: "Previous image cleanup is unfinished",
                rebuild_help: "Rebuild this recipe at this place; existing environments retain their files",
            }),
        }
    }

    /// The Hosts panel's command that removes a host of this provider, which deletes its machine: the host menu's item
    /// and the label and answer of its question (`hosts::remove`). The old app's `host_delete_label`.
    pub(crate) fn host_delete_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Delete sandbox",
            ProviderKind::Podman => "Delete container",
        }
    }

    /// What that command removes, as its menu item's description and its answer's hover help (the old
    /// `host_delete_help`).
    pub(crate) fn host_delete_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Terminate this Tensorlake sandbox and delete its snapshots"
            }
            ProviderKind::Podman => {
                "Delete this container, all its sessions, running processes and files"
            }
        }
    }

    /// The consequence that command's question states after the machine's name: it always destroys the whole machine,
    /// and its consent covers work the helm's session list does not show, since nothing asks the machine's own Farhelm
    /// what it holds (the old `delete_consequence`).
    pub(crate) fn delete_consequence(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete the whole sandbox? Every session, running process, file, checkout, unpushed change and \
                 snapshot in it will be permanently removed. This cannot be undone."
            }
            ProviderKind::Podman => {
                "Delete the whole container? Every session, running process, file, checkout and unpushed change in \
                 it will be permanently removed. This cannot be undone."
            }
        }
    }

    /// What a host whose machine is being deleted says about it under its row; with `failed`, that the deletion failed
    /// and Retry finishes it (the old `deleting_detail`).
    pub(crate) fn deleting_detail(self, failed: bool) -> &'static str {
        match (self.0, failed) {
            (ProviderKind::Tensorlake, false) => {
                "Farhelm is deleting this sandbox, its files and its snapshots"
            }
            (ProviderKind::Podman, false) => "Farhelm is deleting this container and its files",
            (ProviderKind::Tensorlake, true) => {
                "Farhelm could not finish deleting this sandbox and its snapshots; some of them may remain and bill. \
                 Retry finishes the deletion"
            }
            (ProviderKind::Podman, true) => {
                "Farhelm could not finish deleting this container; it may remain. Retry finishes the deletion"
            }
        }
    }

    /// Why the launcher cannot create this provider's environment from `recipe`, or `None` when it can: a base the
    /// provider cannot run, then the provider's own prerequisites. The old app's `ProviderUi::recipe_refusal`, worded as
    /// it was. A Tensorlake recipe needs a built snapshot ("No snapshot yet"); its account prerequisites (an API key,
    /// Farhelm's local SSH key) need the Tensorlake settings read, which comes with the launcher's New sandbox. A
    /// container place builds a missing image on first launch, so Podman has none.
    pub(crate) fn recipe_refusal(self, recipe: &RecipeView) -> Option<String> {
        if !recipe.base.supports(self.0) {
            return Some(format!(
                "This recipe's base cannot run as a {}",
                self.noun()
            ));
        }
        match self.0 {
            ProviderKind::Tensorlake => recipe
                .snapshot
                .is_none()
                .then(|| "No snapshot yet".to_owned()),
            ProviderKind::Podman => None,
        }
    }

    /// What a build on a base only this provider runs leaves behind, said under the recipe editor's packages. Snapshot
    /// language must not imply cloud billing for a container image. The old app's words (`ProviderUi::recipe_build_help`
    /// in `environment_ui.rs` at the reference commit).
    fn recipe_build_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Farhelm then adds Claude Code and Codex with their official installers and takes a snapshot."
            }
            ProviderKind::Podman => {
                "Farhelm then adds Claude Code and Codex with their official installers and saves an image on the \
                 container host for launches there."
            }
        }
    }
}

// ---- A session's own environment: its marks, its lifecycle words and its suspended card -------------------------------
//
// The session list's and the session view's words for a session that runs in a disposable environment of its own
// (`session_list::environment`), including why its actions are unavailable while that environment is being deleted,
// and the two Deletes of such a session with the band's "stays" result (`session_list::environment_delete`): the old
// app's `ProviderUi` methods of the same names at the reference commit, word for word. The wordings that only
// name the environment (`suspend_question` and the rest) are the same sentence for every provider around its noun, as
// the old ones were, so they format [`ProviderUi::noun`] rather than matching.

#[warn(clippy::wildcard_enum_match_arm)]
impl ProviderUi {
    /// What the environment is called inside a sentence: the old `noun`.
    pub(crate) fn noun(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "sandbox",
            ProviderKind::Podman => "container",
        }
    }

    /// The word assistive technology reads for a session row's environment mark, in place of the host mark's `Local`
    /// or `Remote`: the old visually hidden `locality_label`. A sandbox's or container's details line in the Hosts
    /// panel starts with it too ("Sandbox: <name> · Recipe: …"), as the old details line did.
    pub(crate) fn locality_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Sandbox",
            ProviderKind::Podman => "Container",
        }
    }

    /// The mark a session in this provider's environment draws where its host mark would be: the cube for a sandbox,
    /// the shipping box for a container (SPEC.md: "a container mark distinct from the sandbox cube").
    pub(crate) fn mark(self) -> Icon {
        match self.0 {
            ProviderKind::Tensorlake => Icon::EnvironmentSandbox,
            ProviderKind::Podman => Icon::EnvironmentContainer,
        }
    }

    /// The environment mark's hover help, the old marks' `data-tooltip`.
    pub(crate) fn mark_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Sandbox: runs in its own disposable Tensorlake sandbox",
            ProviderKind::Podman => {
                "Container: runs in its own disposable container on one of your hosts"
            }
        }
    }

    /// What plain Stop leaves running, for an environment session's Stop help and its Stop question (SPEC.md: "Plain
    /// Stop ends the chosen agent and tabs but keeps its sandbox running and billing").
    pub(crate) fn stop_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "The sandbox keeps running and billing until you suspend or delete it."
            }
            ProviderKind::Podman => {
                "The container keeps running until you suspend or delete it. Suspending freezes its processes; its \
                 memory stays in use on its host."
            }
        }
    }

    /// Why every action of a session whose environment is being deleted is unavailable: the being-deleted badge's hover
    /// help and the reason the row menu's Clone and Delete and the header's Stop, Clone and Delete give. `failed` is a
    /// deletion that failed and waits for Retry in Hosts. The old `session_deleting_help`, word for word.
    pub(crate) fn session_deleting_help(self, failed: bool) -> &'static str {
        match (self.0, failed) {
            (ProviderKind::Tensorlake, false) => {
                "This session's sandbox is being deleted, so its actions are unavailable"
            }
            (ProviderKind::Podman, false) => {
                "This session's container is being deleted, so its actions are unavailable"
            }
            (ProviderKind::Tensorlake, true) => {
                "Deleting this session's sandbox failed, so its actions are unavailable. Retry in Hosts finishes the \
                 deletion"
            }
            (ProviderKind::Podman, true) => {
                "Deleting this session's container failed, so its actions are unavailable. Retry in Hosts finishes \
                 the deletion"
            }
        }
    }

    /// The consequence of a Delete that asks the environment's own Farhelm (`session_list::environment_delete`'s
    /// session lane). Nothing on this side knows whether the session is the environment's last, so it states both
    /// outcomes, and it is never skipped, since one of them destroys the environment. The old
    /// `session_delete_consequence`, word for word.
    pub(crate) fn session_delete_consequence(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete this session and its checkout? A checkout another session still uses stays. If this is the \
                 last session in its sandbox, the sandbox goes too, with every process and file in it and its \
                 snapshots. This cannot be undone."
            }
            ProviderKind::Podman => {
                "Delete this session and its checkout? A checkout another session still uses stays. If this is the \
                 last session in its container, the container goes too, with every process and file in it. This \
                 cannot be undone."
            }
        }
    }

    /// The session lane's answer. It names only the session, because only the environment's Farhelm decides whether
    /// the environment goes too, and the consequence says when it does. The same for every provider today, but decided
    /// per provider, as the old `session_delete_label` was.
    pub(crate) fn session_delete_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake | ProviderKind::Podman => "Delete session",
        }
    }

    /// The session lane's answer's hover help, and the row menu's and the header's Delete help while a click would ask
    /// in that lane: the old `session_delete_help`.
    pub(crate) fn session_delete_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete this session and its checkout; the sandbox goes too if this is its last session"
            }
            ProviderKind::Podman => {
                "Delete this session and its checkout; the container goes too if this is its last session"
            }
        }
    }

    /// The warning of the whole-environment Delete of one session, offered when Farhelm cannot ask the environment's
    /// own Farhelm. Its consent must cover sessions no list here shows, so it names them, and it says how to get the
    /// narrower Delete instead. The old `whole_delete_warning`, word for word.
    pub(crate) fn whole_delete_warning(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Farhelm cannot ask this sandbox's own Farhelm to delete just this session, so this deletes the whole \
                 sandbox: every session, running process, file, checkout, unpushed change and snapshot in it, \
                 including sessions Farhelm does not list because they were started while it was not connected. This \
                 cannot be undone. Resuming or updating the sandbox first lets Farhelm delete just this session."
            }
            ProviderKind::Podman => {
                "Farhelm cannot ask this container's own Farhelm to delete just this session, so this deletes the \
                 whole container: every session, running process, file, checkout and unpushed change in it, \
                 including sessions Farhelm does not list because they were started while it was not connected. This \
                 cannot be undone. Resuming or updating the container first lets Farhelm delete just this session."
            }
        }
    }

    /// The whole-environment Delete's answer, which names the resource it destroys with the session.
    pub(crate) fn whole_delete_label(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Delete session and sandbox",
            ProviderKind::Podman => "Delete session and container",
        }
    }

    /// The whole-environment Delete's answer's hover help, and the row menu's and the header's Delete help while a
    /// click would ask in that lane: the old `whole_delete_help`.
    pub(crate) fn whole_delete_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete this session together with its whole sandbox and everything in it"
            }
            ProviderKind::Podman => {
                "Delete this session together with its whole container and everything in it"
            }
        }
    }

    /// What the Delete notice band says when the environment stayed: `remaining` is the count the environment's own
    /// Farhelm reported, never one of this window's, since the list can miss sessions. The old `delete_kept`.
    pub(crate) fn delete_kept(self, remaining: u32) -> String {
        let noun = self.noun();
        match remaining {
            0 => format!("The {noun} stays: no other sessions are in it"),
            1 => format!("The {noun} stays: 1 other session is in it"),
            more => format!("The {noun} stays: {more} other sessions are in it"),
        }
    }

    /// `line` with the environment's own name appended, " sandbox: <name>" or " container: <name>", as both Delete
    /// questions name the resource apart from the session's title: the old `resource_label`. The name is the helm's,
    /// so it goes in as host text.
    pub(crate) fn resource_label(self, line: HostSentence, name: &str) -> HostSentence {
        line.text(format!(" {}: ", self.noun()))
            .host(&HostText::from_host(name))
    }

    /// Suspend's hover help, and the second half of its question: what freezing keeps and what it still costs.
    pub(crate) fn suspend_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Freeze the sandbox; its processes continue on Resume. Only snapshot storage bills while suspended."
            }
            ProviderKind::Podman => {
                "Freeze the container; its processes continue on Resume. Its memory stays in use on the container \
                 host."
            }
        }
    }

    /// The suspended card's explanation: the state, not the Suspend button's instruction again, with the provider's
    /// own cost caveat.
    pub(crate) fn suspended_explanation(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "This session's sandbox is suspended. Its processes are preserved, including the agent's running \
                 command, and continue when you resume it. Only snapshot storage bills while suspended."
            }
            ProviderKind::Podman => {
                "This session's container is suspended. Its processes are preserved, including the agent's running \
                 command, and continue when you resume it. Its memory stays in use on the container host."
            }
        }
    }

    /// The suspended card's diagnostics, appended to `line`: the provider's failed resume (`failed`, host text) first,
    /// then who suspended it and when, from the recorded completion (`by_farhelm`, unix ms) or else the observation
    /// (`observed`), never an inferred idle timeout. A sandbox suspended outside Farhelm adds Tensorlake's timeout for a
    /// session's own sandbox; a container's pause completion is not recorded, so a missing one names no outside actor.
    /// The time is the viewer's local time in `zone` ([`local_time::stamp`]), as every clock time the app shows, where
    /// the old app wrote UTC. The old `suspended_detail` for a session-owned host; the ordinary host's wording is the
    /// Hosts panel's.
    pub(crate) fn suspended_detail(
        self,
        line: HostSentence,
        observed: i64,
        by_farhelm: Option<i64>,
        failed: Option<&str>,
        zone: Zone,
    ) -> HostSentence {
        let line = match failed {
            Some(reason) => line
                .text(match self.0 {
                    ProviderKind::Tensorlake => "Tensorlake could not resume the sandbox: ",
                    ProviderKind::Podman => "Resume failed: ",
                })
                .host(&HostText::from_host(reason))
                .text(". "),
            None => line,
        };
        let attribution = match (self.0, by_farhelm.is_some()) {
            (ProviderKind::Tensorlake | ProviderKind::Podman, true) => "Suspended from Farhelm",
            (ProviderKind::Tensorlake, false) => "Suspended outside Farhelm, noticed",
            (ProviderKind::Podman, false) => "Suspension noticed",
        };
        let at = local_time::stamp(by_farhelm.unwrap_or(observed) / 1000, zone);
        let line = line.text(format!("{attribution} at {at}."));
        match (self.0, by_farhelm) {
            (ProviderKind::Tensorlake, None) => line.text(
                " Tensorlake suspends this sandbox after 30 minutes without a connection from Farhelm.",
            ),
            (ProviderKind::Tensorlake, Some(_)) | (ProviderKind::Podman, Some(_) | None) => line,
        }
    }

    /// What starting a stopped environment does, for the card while it starts and for Resume's help on a stopped one:
    /// it cannot promise that the old processes survived.
    pub(crate) fn starting_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Starting the sandbox; this view reconnects when it is ready."
            }
            ProviderKind::Podman => {
                "Starting the container again; its sessions return as interrupted, with Restart available."
            }
        }
    }

    /// Resume's hover help: the starting words for a `stopped` environment, else that preserved processes continue.
    pub(crate) fn resume_help(self, stopped: bool) -> &'static str {
        if stopped {
            self.starting_help()
        } else {
            "Resume this environment; preserved processes continue"
        }
    }

    /// The Suspend question's first sentence, before [`ProviderUi::suspend_help`]: Stop and suspend's (`stop`) keeps
    /// apart the agent it ends and every other process it freezes.
    pub(crate) fn suspend_question(self, stop: bool) -> String {
        let noun = self.noun();
        if stop {
            format!(
                "Stop this session's agent and close its terminal tabs, then suspend the whole {noun}? Every other \
                 process freezes and continues on Resume. The stopped agent and closed tabs do not continue. "
            )
        } else {
            format!(
                "Suspend the whole {noun}? Every process in it freezes, including any other sessions, and continues \
                 on Resume. "
            )
        }
    }

    /// The Suspend question's title, its accessible name: "Suspend sandbox?" or "Stop and suspend sandbox?".
    pub(crate) fn suspend_title(self, stop: bool) -> String {
        format!(
            "{} {}?",
            if stop { "Stop and suspend" } else { "Suspend" },
            self.noun()
        )
    }

    /// The Suspend question's Cancel help: what Cancel keeps running.
    pub(crate) fn suspend_cancel(self, stop: bool) -> String {
        if stop {
            format!(
                "Cancel: keep the agent, its terminal tabs and the {} running",
                self.noun()
            )
        } else {
            format!("Cancel: keep the {} running", self.noun())
        }
    }

    /// The Suspend question's answer help, and Stop and suspend's: the scope includes every other session in the
    /// environment, listed or not.
    pub(crate) fn suspend_action(self, stop: bool) -> String {
        if stop {
            format!(
                "Stop this session's agent and close its terminal tabs, then suspend the whole {}",
                self.noun()
            )
        } else {
            format!(
                "Suspend the whole {}, freezing every process in it until Resume",
                self.noun()
            )
        }
    }
}

/// How a session's own environment is named where its host's name would be (SPEC.md: "<recipe> sandbox", and "<recipe>
/// container on <host>"): the old `target_label`. `place_name` is the helm's name for a container's place host, and a
/// place the helm could not name reads "host <id>". Raw host text (the recipe and the place's name are the helm's), for
/// the caller to escape. Decided here, over the place, so shared code names no provider.
pub(crate) fn target_label(target: &LaunchTarget, place_name: Option<&str>) -> String {
    let noun = ProviderUi(target.provider()).noun();
    match target.place() {
        Place::TensorlakeAccount => format!("{} {noun}", target.recipe()),
        Place::PodmanHost(host) => {
            let name = place_name.map_or_else(|| format!("host {host}"), str::to_owned);
            format!("{} {noun} on {name}", target.recipe())
        }
    }
}

/// The recipe editor's help under its packages for a recipe on `base`: what Farhelm adds and what the build leaves at
/// each place that can run the base. Managed Ubuntu runs everywhere, so its words name both artifacts; a base only one
/// provider runs takes that provider's words ([`ProviderUi::recipe_build_help`]). Decided here, with the provider
/// wording, so the editor names no provider (AGENTS.md, "Code rules"). The old editor's match, at the reference commit.
pub(crate) fn recipe_build_help(base: &RecipeBase) -> &'static str {
    match base {
        RecipeBase::ManagedUbuntu => {
            "Farhelm then adds Claude Code and Codex with their official installers. On Tensorlake it takes a \
             snapshot; on a container host it saves an image for launches there."
        }
        RecipeBase::TensorlakeImage { .. } | RecipeBase::TensorlakeSnapshot { .. } => {
            ProviderUi(ProviderKind::Tensorlake).recipe_build_help()
        }
        RecipeBase::ContainerImage { .. } => ProviderUi(ProviderKind::Podman).recipe_build_help(),
    }
}

/// What the launcher's footer and progress pill name a new environment by: [`target_label`], with a container host
/// named as everywhere else ([`display_name`]) from the pushed hosts list, or "host <id>" once the registry no longer
/// lists it. The recipe and the host's name are host values, so the whole label is host text.
pub(crate) fn launch_target_label(target: &LaunchTarget, hosts: &[HostView]) -> HostText {
    let place_name = target
        .place()
        .container_host()
        .and_then(|id| hosts.iter().find(|host| host.id == id))
        .map(|host| display_name(host).raw().to_owned());
    HostText::from_host(target_label(target, place_name.as_deref()))
}

/// A provider's words for a recipe's per-place line, as [`ProviderUi::recipe_place`] chooses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecipePlaceWords {
    /// While the place builds or prepares the recipe's artifact.
    pub(crate) building: &'static str,
    /// Once the place has an artifact ready for launches.
    pub(crate) ready: &'static str,
    /// Before anything is built at the place.
    pub(crate) not_built: &'static str,
    /// While an artifact the place replaced still awaits deletion.
    pub(crate) cleanup_pending: &'static str,
    /// The place's Rebuild button's hover help.
    pub(crate) rebuild_help: &'static str,
}

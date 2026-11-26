//! The GUI's words for disposable-environment providers: one exhaustive answer per provider for each thing the GUI says
//! differently about a Tensorlake sandbox and a container.
//!
//! AGENTS.md ("Code rules") puts every provider's UI wording in one exhaustive UI module, so that adding a provider
//! fails to compile until it has chosen its own words, and no shared code compares providers. This is that module for
//! the GPUI app, ported from the old app's `environment_ui.rs` at the reference commit. It holds only what a GPUI
//! surface has needed so far, starting with an agent's environment-launch approval card; the launcher's and host
//! management's environment surfaces add their own methods here as they are ported, rather than starting a second
//! module. The Hosts panel's deletion of a sandbox or container and its details take theirs from here too
//! (`host_delete_label`, `delete_consequence`, `deleting_detail`, `locality_label`). The session list's and the session
//! view's environment session rows, controls and suspended card take theirs from the section on a session's own
//! environment below. The launcher's Runs on switch, its note and its refusals take their words from here (`segment`,
//! `launch_help`, `recipe_refusal`, [`target_label`]), and so do New sandbox's account prerequisites, its recipes' mark
//! and size and its SSH-key registration note (`launch_place`, `recipe_listing`, `registration_note`), the question it
//! asks before creating a billed sandbox (`creation_question`), and the template editor's copy of the switch, with its
//! own authoring rules (`template_place`, `base_refusal`).

use farhelm_proto::recipes::{RecipeBase, RecipeView};
use farhelm_proto::tensorlake::{Settings, SshKeyStatus};
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchTarget, Place, ProviderKind};

use crate::assets::Icon;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::local_time::{self, Zone};

/// One provider, seen through the GUI's vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProviderUi(pub(crate) ProviderKind);

/// What Stop and suspend closes before freezing the environment, as shown by the session row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopTarget {
    /// The main program has ended and there are no open tabs. Stop can still clean up its remaining processes.
    EndedWithoutTabs,
    /// The session's main program has already ended. Open terminal tabs would close.
    Tabs,
    /// The main program is running. Stop ends it and closes its terminal tabs.
    Program(&'static str),
}

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
    /// per-place rows listed only the places a base supports. A historical container place can still show its build
    /// status after a base change. Its disabled Rebuild uses these words to explain why it cannot run again there.
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

    /// The hover help of a sandbox's or container's own mark in its Hosts row, which names the machine itself, not a
    /// session in it ([`ProviderUi::mark_help`] is the session row's). True for every way such a host comes to be
    /// listed: a sandbox added from Add host or made for a session is reached over SSH like any remote host, and a
    /// container Farhelm made for a session runs on one of the user's container hosts, this machine included.
    pub(crate) fn host_mark_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Sandbox: a Tensorlake sandbox, reached over SSH",
            ProviderKind::Podman => "Container: a container on one of your container hosts",
        }
    }

    /// What a recipe's row in Settings → Recipes calls this provider's place when the place is the provider's account
    /// rather than a host: the name ahead of the row's built-snapshot and prepared-snapshot lines, which report that
    /// place, as a container place's line starts with its host's name ("Local (this machine) · Not built"). Without
    /// it, "No snapshot yet" and Build snapshot read as the whole recipe's state on a recipe used only for containers,
    /// as a real container run found. `None` for a provider whose places are all hosts, which their lines name.
    pub(crate) fn account_place(self) -> Option<&'static str> {
        match self.0 {
            ProviderKind::Tensorlake => Some("Tensorlake"),
            ProviderKind::Podman => None,
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

    /// The hover help of the Suspend answer of the Hosts panel's Suspend question, the old question's words for this
    /// provider's machine.
    pub(crate) fn host_suspend_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Suspend: pause this sandbox until it is resumed",
            ProviderKind::Podman => "Suspend: pause this container until it is resumed",
        }
    }

    /// Whom Farhelm asked to suspend or resume this provider's machine, as a changing host's details name it ("Asked
    /// Tensorlake to resume at …; waiting for confirmation"): the old `lifecycle_request_detail`'s `asked`.
    pub(crate) fn lifecycle_asked(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "Tensorlake",
            ProviderKind::Podman => "the container host",
        }
    }

    /// The provider diagnosed a new failure while a launch waited for its existing host to wake. Match the remedy
    /// for the initiating phase so starting a stopped container never says it failed to resume frozen processes.
    pub(crate) fn wake_failed(self, stopped: bool) -> &'static str {
        match (self.0, stopped) {
            (ProviderKind::Tensorlake, _) => {
                "Tensorlake could not resume this host; check its details"
            }
            (ProviderKind::Podman, true) => {
                "The container host could not start this container; check its details"
            }
            (ProviderKind::Podman, false) => {
                "The container host could not resume this container; check its details"
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

    /// Name the resource the launcher will create. The host menu and Templates' Runs on use the same words.
    pub(crate) fn segment(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => "New sandbox",
            ProviderKind::Podman => "New container",
        }
    }

    /// The segment's hover help, the old segment buttons' tooltips.
    pub(crate) fn segment_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Create a disposable Tensorlake sandbox from a built recipe"
            }
            ProviderKind::Podman => {
                "Create a disposable container from a recipe on one of your hosts"
            }
        }
    }

    /// A short qualification beside the launcher menu's action to start a new environment.
    pub(crate) fn launch_offer_tag(self) -> Option<&'static str> {
        match self.0 {
            ProviderKind::Podman => Some("experimental"),
            ProviderKind::Tensorlake => None,
        }
    }

    /// Where the launcher's Runs on switch creates this provider's environment when its segment is entered, or `None`
    /// when it has no usable place, which hides the segment (SPEC.md: "A segment with no usable place is hidden").
    /// `container_hosts` are the hosts whose Podman is usable now, in the registry's order, and `last` the container host
    /// last launched on (the pushed preferences' `last_container_host`). A container goes to the last container host
    /// while it is usable, else the first usable one (the old `PlacementPicker`'s default).
    ///
    /// `account` is the Tensorlake account as the launcher read it (`GET /api/tensorlake`), `None` while that read is
    /// out or after it failed. The account is a usable place once it has an API key, the old switch's whole rule for
    /// showing New sandbox: a missing SSH key or snapshot still shows the segment, and the recipe's refusal then says
    /// what to fix ([`ProviderUi::recipe_refusal`]). A container place needs no account.
    pub(crate) fn launch_place(
        self,
        container_hosts: &[i64],
        last: Option<i64>,
        account: Option<&Settings>,
    ) -> Option<Place> {
        match self.0 {
            ProviderKind::Tensorlake => account
                .filter(|account| account.api_key_set)
                .map(|_| Place::TensorlakeAccount),
            ProviderKind::Podman => last
                .filter(|last| container_hosts.contains(last))
                .or_else(|| container_hosts.first().copied())
                .map(Place::PodmanHost),
        }
    }

    /// The words of the launcher's note on how the chosen recipe stands at the chosen place, for a provider whose
    /// places each build the recipe themselves (the old `PlacementPicker`'s status line); `None` for one whose built
    /// recipe the account holds, which the recipe's own refusal covers ("No snapshot yet").
    pub(crate) fn launch_place_note(self) -> Option<LaunchPlaceNote> {
        match self.0 {
            ProviderKind::Tensorlake => None,
            ProviderKind::Podman => Some(LaunchPlaceNote {
                building: "Preparing…",
                ready_on: "Ready on",
                first_launch: "Builds on first launch here (a few minutes)",
                checking: "Checking build status…",
            }),
        }
    }

    /// Detail for the chosen environment in the Host picker's help. The billing and deletion facts that must be
    /// visible before launch also appear in [`Self::launch_notice`].
    pub(crate) fn launch_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "A new Tensorlake sandbox for this session. It bills while running. Suspend stops CPU and memory \
                 billing; snapshot storage still bills. Deleting its last session removes the sandbox and its files. \
                 After Farhelm disconnects, Tensorlake suspends it after 30 minutes without activity."
            }
            ProviderKind::Podman => {
                "A new container for this session, built from the recipe on its container host. The checkout and files \
                 live inside it. Suspend freezes processes while keeping memory in use; deleting its last session \
                 removes the container and its files."
            }
        }
    }

    /// Billing and deletion information the launcher shows even when the user skips the creation confirmation.
    pub(crate) fn launch_notice(self) -> Option<&'static str> {
        match self.0 {
            ProviderKind::Tensorlake => Some(
                "This session's sandbox bills while running. Suspend stops CPU and memory billing. Deleting its last session removes the sandbox and its files.",
            ),
            ProviderKind::Podman => {
                Some("Deleting the last session removes this container and its files.")
            }
        }
    }

    /// Why the launcher cannot create this provider's environment from `recipe`, or `None` when it can, in the old
    /// app's order (`ProviderUi::recipe_refusal` and `sandbox_sessions::recipe_refusal` at the reference commit, worded
    /// as they were): a base the provider cannot run, then what the provider's account still lacks, then what the
    /// recipe itself lacks. A Tensorlake sandbox needs an API key, Farhelm's local SSH key and a built snapshot ("No
    /// snapshot yet"); a container place builds a missing image on first launch, so Podman needs nothing more.
    ///
    /// `account` is the Tensorlake account as the launcher read it, or the words for why it has none yet (its read is
    /// still out, or the read's own failure), which a provider that needs it gives as its reason. Whether the user has
    /// registered the key with the account cannot be read from here, so it never refuses (SPEC.md: "Farhelm cannot
    /// check account registration"); the launcher's note says so instead ([`ProviderUi::registration_note`]).
    pub(crate) fn recipe_refusal(
        self,
        recipe: &RecipeView,
        account: Result<&Settings, &str>,
    ) -> Option<String> {
        self.base_refusal(recipe)
            .or_else(|| self.account_refusal(account))
            .or_else(|| self.artifact_refusal(recipe))
    }

    /// Whether entering this provider's Runs on segment may choose `recipe`: what [`ProviderUi::recipe_refusal`] says
    /// of the recipe itself, its account left out. The old switch entered New sandbox on the first recipe with a built
    /// snapshot even while Farhelm's SSH key was missing, so the chosen recipe's refusal then names the key to create
    /// rather than leaving no recipe chosen (kept by the launcher area on 2026-11-16).
    pub(crate) fn recipe_pickable(self, recipe: &RecipeView) -> bool {
        self.base_refusal(recipe).is_none() && self.artifact_refusal(recipe).is_none()
    }

    /// What this provider's account must hold before the launcher creates anything there. A sandbox needs the
    /// account's API key and Farhelm's own SSH key, in that order; the key's unavailable reason is the helm's, so it is
    /// written escaped, as a host value. A container needs no account, so it never waits for the account's read.
    fn account_refusal(self, account: Result<&Settings, &str>) -> Option<String> {
        match self.0 {
            ProviderKind::Tensorlake => {
                let account = match account {
                    Ok(account) => account,
                    Err(reason) => return Some(reason.to_owned()),
                };
                if !account.api_key_set {
                    return Some("Set a Tensorlake API key in Settings".to_owned());
                }
                match &account.ssh_key {
                    SshKeyStatus::NotCreated => {
                        Some("Create Farhelm's SSH key in Settings → Tensorlake".to_owned())
                    }
                    SshKeyStatus::Unavailable { reason } => Some(
                        HostSentence::new("launcher-ssh-key-unavailable")
                            .text("Farhelm's local SSH key is unavailable: ")
                            .host(&HostText::from_host(reason.as_str()))
                            .accessible_name()
                            .to_string(),
                    ),
                    SshKeyStatus::Ready { .. } => None,
                }
            }
            ProviderKind::Podman => None,
        }
    }

    /// What `recipe` must have built before this provider can start from it: a sandbox starts from the recipe's
    /// snapshot, which the account holds; a container's place builds its own image on first launch.
    fn artifact_refusal(self, recipe: &RecipeView) -> Option<String> {
        match self.0 {
            ProviderKind::Tensorlake => recipe
                .snapshot
                .is_none()
                .then(|| "No snapshot yet".to_owned()),
            ProviderKind::Podman => None,
        }
    }

    /// Why `recipe` cannot make this provider's environment at all, whatever the account or place: its base is one
    /// the provider cannot run. The first of [`ProviderUi::recipe_refusal`]'s reasons, and the only one the template
    /// editor's Recipe select disables an option for, as the old `PlacementPicker` did in its authoring mode: a
    /// template stores a recipe's name for later launches, which check the rest when they are made.
    pub(crate) fn base_refusal(self, recipe: &RecipeView) -> Option<String> {
        (!recipe.base.supports(self.0))
            .then(|| format!("This recipe's base cannot run as a {}", self.noun()))
    }

    /// Where the template editor's Runs on switch puts this provider's environment when its segment is entered, or
    /// `None` when there is nowhere to name one, which hides the segment. `container_hosts` are every host that can
    /// hold containers, in the registry's order, usable now or not. The old `PlacementPicker`'s authoring mode, which
    /// counted places by name, not by readiness ("Authoring needs names, not account readiness", its editor said): New
    /// container shows while any container host is listed and enters the first (the editor passed no last container
    /// host), and New sandbox always shows, entering the Tensorlake account with its prerequisites unchecked. The
    /// launcher's own rule, which counts only usable places, is [`ProviderUi::launch_place`].
    pub(crate) fn template_place(self, container_hosts: &[i64]) -> Option<Place> {
        match self.0 {
            ProviderKind::Tensorlake => Some(Place::TensorlakeAccount),
            ProviderKind::Podman => container_hosts.first().copied().map(Place::PodmanHost),
        }
    }

    /// How the launcher's Recipe select lists a recipe for this provider: with the provider's mark and the recipe's
    /// CPU and memory, or by name alone (`None`). SPEC.md, "Sandbox sessions": New sandbox "lists recipes with a cube
    /// mark and CPU and memory size", which the old select did not draw; SPEC.md names neither for a container's
    /// recipes, so they keep the old names alone. The size uses the words a sandbox host's details line uses.
    pub(crate) fn recipe_listing(self, recipe: &RecipeView) -> Option<RecipeListing> {
        match self.0 {
            ProviderKind::Tensorlake => Some(RecipeListing {
                mark: self.mark(),
                size: format!(
                    "{} CPUs · {} GB memory",
                    recipe.size.cpus, recipe.size.memory_gb
                ),
            }),
            ProviderKind::Podman => None,
        }
    }

    /// What the launcher says about registering Farhelm's SSH key with the provider's account, once that key exists
    /// (`account`, as the launcher read it; `None` while it is out or failed): the old launcher's note, its command and
    /// its copy button's words. SPEC.md, "Sandbox sessions": "Once Farhelm's SSH key exists, New sandbox shows the
    /// existing copyable SSH-key registration command, whether or not a recipe is chosen".
    /// A container has no account to register with.
    pub(crate) fn registration_note(
        self,
        account: Option<&Settings>,
    ) -> Option<RegistrationNote<'_>> {
        match self.0 {
            ProviderKind::Tensorlake => match &account?.ssh_key {
                SshKeyStatus::Ready {
                    register_command, ..
                } => Some(RegistrationNote {
                    words: "Register Farhelm's SSH key with Tensorlake before launching. Farhelm cannot check \
                            account registration.",
                    command: register_command,
                    copy_label: "Copy registration command",
                    copy_help: "Copy the command that registers Farhelm's SSH key with your Tensorlake account",
                }),
                SshKeyStatus::NotCreated | SshKeyStatus::Unavailable { .. } => None,
            },
            ProviderKind::Podman => None,
        }
    }

    /// What the launcher asks before it creates this provider's environment from `recipe`, or `None` when it creates
    /// one without asking. SPEC.md, "Sandbox sessions": launching "asks Create a sandbox for this session?, naming
    /// recipe, CPU, memory and disk, linking Tensorlake pricing, and explaining suspension, the idle timeout and
    /// irreversible file deletion", and "Only Create Tensorlake sandboxes without asking skips this question". The
    /// words are the old launcher's question at the reference commit (`create_form.rs`), sentence for sentence.
    ///
    /// A sandbox bills from the moment it exists, so its question is asked unless `account`, as the launcher read it,
    /// has that setting on. The launcher may have read it a while ago: a setting turned off since is caught by the
    /// helm, which re-checks it and refuses a create sent without consent. An account the launcher could not read asks,
    /// since only a setting the user turned on may skip the question. A container bills nothing per create, so it is
    /// never asked about.
    pub(crate) fn creation_question(
        self,
        recipe: &RecipeView,
        account: Result<&Settings, &str>,
    ) -> Option<CreationQuestion> {
        match self.0 {
            ProviderKind::Tensorlake => {
                if account.is_ok_and(|account| account.create_without_asking) {
                    return None;
                }
                Some(CreationQuestion {
                    title: "Create a sandbox for this session?",
                    size: format!(
                        "{} CPU · {} GB memory · {} GB disk",
                        recipe.size.cpus, recipe.size.memory_gb, recipe.size.disk_gb
                    ),
                    consequence: "The sandbox bills while running. Suspend preserves its processes and files and \
                                  stops CPU and memory billing; snapshot storage still bills. Tensorlake suspends it \
                                  after 30 minutes without a connection from Farhelm. Deleting its last session \
                                  irreversibly removes the sandbox, every process, its files and snapshots.",
                    link_label: "Tensorlake pricing",
                    link_url: "https://www.tensorlake.ai/pricing",
                    link_help: "Open Tensorlake's pricing page in your browser",
                    cancel_help: "Cancel: create no sandbox and keep the launcher open",
                    confirm_label: "Create and launch",
                    confirm_help: "Create a billed Tensorlake sandbox from this recipe and launch the session in it",
                })
            }
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
    /// outcomes, and it is never skipped, since one of them destroys the environment. Both outcomes stop any
    /// processes and close any terminals in the selected session, even when another session keeps the environment.
    pub(crate) fn session_delete_consequence(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Delete this session and its checkout? This stops any processes still running in the session and \
                 closes its terminal tabs. A checkout another session still uses stays. If this is the last session \
                 in its sandbox, the sandbox goes too, with every process and file in it and its snapshots. This \
                 cannot be undone."
            }
            ProviderKind::Podman => {
                "Delete this session and its checkout? This stops any processes still running in the session and \
                 closes its terminal tabs. A checkout another session still uses stays. If this is the last session \
                 in its container, the container goes too, with every process and file in it. This cannot be undone."
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
                "This session's sandbox is suspended. Its processes are preserved and continue when you resume it. \
                 Only snapshot storage bills while suspended."
            }
            ProviderKind::Podman => {
                "This session's container is suspended. Its processes are preserved and continue when you resume it. \
                 Its memory stays in use on the container host."
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
    ///
    /// A container's words follow SPEC.md ("Container sessions"), which has two cases the card cannot tell apart: "A
    /// host reboot interrupts sessions last known live", and "A container stopped and started on the same host boot
    /// returns ended sessions through ordinary exit classification", with Restart for a captured conversation either
    /// way. Sessions that had already exited keep their outcome, so only the running ones are promised a change. The old
    /// app said its sessions "return as interrupted, with Restart available", true only after a reboot; the GPUI
    /// migration's orchestrator decided on 2026-11-16 to word it to SPEC.md instead.
    pub(crate) fn starting_help(self) -> &'static str {
        match self.0 {
            ProviderKind::Tensorlake => {
                "Starting the sandbox; this view reconnects when it is ready."
            }
            ProviderKind::Podman => {
                "Starting the container again; sessions that were running come back ended, or interrupted if its \
                 host restarted, with Restart where a conversation was captured."
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

    /// The Suspend question's consequence before [`ProviderUi::suspend_help`]. Stop and suspend distinguishes the
    /// session's closed tabs and program (if still running) from the processes that Resume can continue.
    pub(crate) fn suspend_question(self, stops: Option<StopTarget>) -> String {
        let noun = self.noun();
        match stops {
            Some(StopTarget::Program(program)) => format!(
                "Stop this session's {program} and close its terminal tabs, then suspend the whole {noun}? Every other \
                 process freezes and continues on Resume. The stopped {program} and closed tabs do not continue. "
            ),
            Some(StopTarget::Tabs) => format!(
                "Close this session's terminal tabs, then suspend the whole {noun}? The remaining processes freeze and \
                 continue on Resume. The closed tabs stay closed. "
            ),
            Some(StopTarget::EndedWithoutTabs) => format!(
                "Finish stopping this session, then suspend the whole {noun}? The remaining processes freeze and \
                 continue on Resume. "
            ),
            None => format!(
                "Suspend the whole {noun}? Every process in it freezes, including any other sessions, and continues \
                 on Resume. "
            ),
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
    pub(crate) fn suspend_cancel(self, stops: Option<StopTarget>) -> String {
        match stops {
            Some(StopTarget::Program(program)) => format!(
                "Cancel: keep the {program}, its terminal tabs and the {} running",
                self.noun()
            ),
            Some(StopTarget::Tabs) => format!(
                "Cancel: leave the terminal tabs open and keep the {} running",
                self.noun()
            ),
            Some(StopTarget::EndedWithoutTabs) | None => {
                format!("Cancel: keep the {} running", self.noun())
            }
        }
    }

    /// The Suspend question's answer help, and Stop and suspend's: the scope includes every other session in the
    /// environment, listed or not.
    pub(crate) fn suspend_action(self, stops: Option<StopTarget>) -> String {
        match stops {
            Some(StopTarget::Program(program)) => format!(
                "Stop this session's {program} and close its terminal tabs, then suspend the whole {}",
                self.noun()
            ),
            Some(StopTarget::Tabs) => format!(
                "Close this session's terminal tabs, then suspend the whole {}",
                self.noun()
            ),
            Some(StopTarget::EndedWithoutTabs) => format!(
                "Finish stopping this session, then suspend the whole {}",
                self.noun()
            ),
            None => format!(
                "Suspend the whole {}, freezing every process in it until Resume",
                self.noun()
            ),
        }
    }
}

/// How a session's own environment is named where its host's name would be (SPEC.md: "<recipe> sandbox", and "<recipe>
/// container on <host>"): the old `target_label`. `place_name` is the helm's name for a container's place host, and a
/// place the helm could not name reads "host <id>". Raw host text (the recipe and the place's name are the helm's), for
/// the caller to escape. Decided here, over the place, so shared code names no provider.
///
/// A target with no recipe chosen (an empty name, which the launcher's Runs on switch enters when the catalog offers
/// the provider none) names the bare environment, "sandbox" or "container on <host>", so the footer reads "new
/// sandbox". The old label had no such case and read "new  sandbox" there.
pub(crate) fn target_label(target: &LaunchTarget, place_name: Option<&str>) -> String {
    let noun = ProviderUi(target.provider()).noun();
    let named = match target.recipe() {
        "" => noun.to_owned(),
        recipe => format!("{recipe} {noun}"),
    };
    match target.place() {
        Place::TensorlakeAccount => named,
        Place::PodmanHost(host) => {
            let name = place_name.map_or_else(|| format!("host {host}"), str::to_owned);
            format!("{named} on {name}")
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

/// The Runs on switch's environment segments, in the old switch's order (after Existing host).
pub(crate) const RUNS_ON: [ProviderKind; 2] = [ProviderKind::Podman, ProviderKind::Tensorlake];

// `RUNS_ON` is a list, which the compiler does not hold to every provider, and a provider missing from it never gets a
// segment. This match has no `_` arm, so a new provider stops the build here until someone places it in `RUNS_ON` (or
// decides it gets no segment) and adds it to this match (AGENTS.md, "Code rules").
const _: () = match ProviderKind::Podman {
    ProviderKind::Podman | ProviderKind::Tensorlake => (),
};

/// A provider's words for the launcher's note on the chosen recipe at the chosen place, as
/// [`ProviderUi::launch_place_note`] chooses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaunchPlaceNote {
    /// While the place builds or prepares the recipe.
    pub(crate) building: &'static str,
    /// Before the place's name, once it has the recipe ready ("Ready on <name>").
    pub(crate) ready_on: &'static str,
    /// Before anything is built there.
    pub(crate) first_launch: &'static str,
    /// While the place's state for the recipe has not been pushed.
    pub(crate) checking: &'static str,
}

/// How the launcher's Recipe select lists one recipe, as [`ProviderUi::recipe_listing`] chooses it: the mark drawn
/// before the option and the size words after its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecipeListing {
    pub(crate) mark: Icon,
    /// "2 CPUs · 4 GB memory".
    pub(crate) size: String,
}

/// The launcher's note on registering Farhelm's SSH key with the provider's account, as
/// [`ProviderUi::registration_note`] words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RegistrationNote<'a> {
    /// The note's sentences, the app's own.
    pub(crate) words: &'static str,
    /// The command that registers the key, as the helm wrote it (host text, copied byte for byte).
    pub(crate) command: &'a str,
    /// The copy button's label and hover help.
    pub(crate) copy_label: &'static str,
    pub(crate) copy_help: &'static str,
}

/// The question the launcher asks before creating a billed environment, as [`ProviderUi::creation_question`] words it
/// for one recipe. All of it is the app's own wording; the recipe's name, which the launcher names in the question too,
/// is host text and so is not part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CreationQuestion {
    /// The question's title and accessible name.
    pub(crate) title: &'static str,
    /// The recipe's size, "2 CPU · 4 GB memory · 20 GB disk", as the recipe stood when the question was worded.
    pub(crate) size: String,
    /// What creating it means: billing, suspension, the idle timeout and what deleting removes.
    pub(crate) consequence: &'static str,
    /// The pricing link's label, the page it opens and its hover help.
    pub(crate) link_label: &'static str,
    pub(crate) link_url: &'static str,
    pub(crate) link_help: &'static str,
    /// Cancel's hover help, saying what Cancel keeps.
    pub(crate) cancel_help: &'static str,
    /// The confirming answer's label and hover help.
    pub(crate) confirm_label: &'static str,
    pub(crate) confirm_help: &'static str,
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

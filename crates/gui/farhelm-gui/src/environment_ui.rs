//! The GUI's words for disposable-environment providers: one exhaustive answer per provider for each thing the GUI says
//! differently about a Tensorlake sandbox and a container.
//!
//! AGENTS.md ("Code rules") puts every provider's UI wording in one exhaustive UI module, so that adding a provider
//! fails to compile until it has chosen its own words, and no shared code compares providers. This is that module for
//! the GPUI app, ported from the old app's `environment_ui.rs` at the reference commit. It holds only what a GPUI
//! surface has needed so far, starting with an agent's environment-launch approval card; the launcher's and host
//! management's environment surfaces add their own methods here as they are ported, rather than starting a second
//! module.

use farhelm_proto::ProviderKind;

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

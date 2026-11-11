//! Shared recipe definitions preserve the catalog's existing stored shape.
//!
//! User choices and credential names live in spec_json. Build engines own
//! state_json independently, so a concurrent save cannot discard resources.
//! Tensorlake's routes keep their compatibility names while both providers
//! consume the same Recipe definition.

use farhelm_proto::recipes::{
    BuildView, EndpointSave, EndpointView, PrepareView, RecipeBase, RecipeSave, RecipeSize,
    RecipeView, Tunnel, check_recipe,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    store::HostId,
    tensorlake::{
        build::{BuiltSnapshot, FailureRecord, PrepareFailureRecord},
        prepared::PreparedSnapshot,
    },
};

/// Coordinates from the actual install plan, retained for clone admission.
/// They carry no supervisor identity. Each environment learns it at hello.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PreparedInstallFacts {
    pub(crate) home: std::path::PathBuf,
    pub(crate) farhelm_path: std::path::PathBuf,
    pub(crate) state_dir: std::path::PathBuf,
    pub(crate) farhelm_version: String,
}

/// One place host's owned Podman artifacts, stored in the recipe's state_json.
/// Empty state loads from older records. Resource IDs remain recorded through
/// failed cleanup, independently of saves to the recipe definition. The provider
/// must recheck image existence on the place host before reusing either image,
/// because a host reinstall can preserve its registry ID while losing storage.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct PodmanRecipeState {
    pub(crate) image: Option<BuiltImage>,
    pub(crate) prepared: Option<PreparedImage>,
    pub(crate) builder_container_id: Option<String>,
    pub(crate) preparer_container_id: Option<String>,
    /// As with Tensorlake's starts, a durable start without a live task is an
    /// interrupted attempt. The next explicit build or lazy preparation owns
    /// its cleanup, with no work scheduled merely by loading the catalog.
    pub(crate) build_started_at: Option<i64>,
    pub(crate) prepare_started_at: Option<i64>,
    /// Replacements are recorded before their predecessors are retired. Only
    /// Farhelm-owned images belong here, never a user-supplied base reference.
    pub(crate) retired_images: Vec<String>,
}

impl PodmanRecipeState {
    /// A recipe cannot be forgotten while a place still records resources or
    /// interrupted work. The provider clears ownership after confirmed cleanup.
    pub(crate) fn needs_cleanup(&self) -> bool {
        self.image.is_some()
            || self.prepared.is_some()
            || self.builder_container_id.is_some()
            || self.preparer_container_id.is_some()
            || self.build_started_at.is_some()
            || self.prepare_started_at.is_some()
            || !self.retired_images.is_empty()
    }
}

/// The software image actually built at one place, with its frozen inputs.
/// Tensorlake's disk floor belongs to its snapshot record alone. Podman applies
/// no resource limits and records no recipe size in its image provenance.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct BuiltImage {
    pub(crate) id: String,
    pub(crate) built_at: i64,
    pub(crate) base: RecipeBase,
    pub(crate) packages: Vec<String>,
    pub(crate) claude_version: Option<String>,
    pub(crate) codex_version: Option<String>,
}

/// A Podman installation checkpoint and the software image it was prepared from.
/// Freshness uses the same three facts as Tensorlake preparation, age, Farhelm
/// version and built-source identity. Credentials remain per-environment work.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct PreparedImage {
    pub(crate) id: String,
    pub(crate) prepared_at: i64,
    pub(crate) farhelm_version: String,
    pub(crate) source_image: String,
    pub(crate) facts: PreparedInstallFacts,
}

/// An endpoint's persisted selection; the value belongs to the credential table.
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct StoredEndpoint {
    pub(crate) base_url: String,
    pub(crate) token_credential: Option<String>,
}

/// Secret-bearing input to the pure settings merges, never persisted as a recipe
/// or returned by a route. Only delivery resolves a named value into this shape.
pub(crate) struct ResolvedEndpoint {
    pub(crate) base_url: String,
    pub(crate) token: Option<String>,
    /// Bound to the token by the same catalog read; never inferred from time.
    pub(crate) revision: Option<u64>,
    /// A deleted catalog name may be reused at revision1. Delivery compares
    /// this hidden identity too, so the new value cannot match an old stamp.
    pub(crate) incarnation: Option<String>,
}

/// A recipe's `spec_json`: user choices and credential names, without values.
///
/// Never crosses the HTTP boundary; [`Self::view`] is the only way out.
/// Rows are written only through [`Self::saved`], so a stored recipe has
/// passed `check_recipe` with this build's or an older build's rules.
#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Recipe {
    /// Tensorlake alone uses these limits. A Podman build keeps the choice
    /// intact for a later Tensorlake launch of the same shared recipe.
    pub(crate) size: RecipeSize,
    pub(crate) base: RecipeBase,
    pub(crate) packages: Vec<String>,
    pub(crate) claude: Option<StoredEndpoint>,
    pub(crate) codex: Option<StoredEndpoint>,
    pub(crate) claude_login_credential: Option<String>,
    pub(crate) codex_login_credential: Option<String>,
    pub(crate) github_token_credential: Option<String>,
    pub(crate) tunnels: Vec<Tunnel>,
}

impl Recipe {
    /// Validate and normalize choices without resolving secret values. The caller
    /// checks credential references inside its write transaction. Packages keep
    /// their given order with exact duplicates removed.
    pub(crate) fn saved(mut save: RecipeSave) -> Result<Self, String> {
        check_recipe(&save)?;
        let mut seen = std::collections::HashSet::new();
        save.packages.retain(|package| seen.insert(package.clone()));
        let claude =
            resolve_endpoint(save.claude).map_err(|reason| format!("Claude Code: {reason}"))?;
        let codex = resolve_endpoint(save.codex).map_err(|reason| format!("Codex: {reason}"))?;
        Ok(Self {
            size: save.size,
            base: save.base,
            packages: save.packages,
            claude,
            codex,
            claude_login_credential: save.claude_login_credential,
            codex_login_credential: save.codex_login_credential,
            github_token_credential: save.github_token_credential,
            tunnels: save.tunnels,
        })
    }

    /// The secret-free read shape of this recipe.
    pub(crate) fn view(&self, name: &str) -> RecipeView {
        let endpoint = |endpoint: &Option<StoredEndpoint>| {
            endpoint.as_ref().map(|endpoint| EndpointView {
                base_url: endpoint.base_url.clone(),
                token_credential: endpoint.token_credential.clone(),
            })
        };
        RecipeView {
            name: name.to_string(),
            size: self.size,
            base: self.base.clone(),
            packages: self.packages.clone(),
            claude: endpoint(&self.claude),
            codex: endpoint(&self.codex),
            claude_login_credential: self.claude_login_credential.clone(),
            codex_login_credential: self.codex_login_credential.clone(),
            github_token_credential: self.github_token_credential.clone(),
            tunnels: self.tunnels.clone(),
            snapshot: None,
            build: BuildView::Idle,
            prepared: None,
            preparation: PrepareView::Idle,
            cleanup_pending: false,
        }
    }
}

/// Parse the URL with the server's full parser after the shared structural
/// check. The selected credential name survives a URL change deliberately:
/// the editor selects an identity, not an unseen token-preservation action.
fn resolve_endpoint(save: Option<EndpointSave>) -> Result<Option<StoredEndpoint>, &'static str> {
    let Some(save) = save else {
        return Ok(None);
    };
    reqwest::Url::parse(&save.base_url)
        .map_err(|_| "use an http or https base URL without credentials, query or fragment")?;
    Ok(Some(StoredEndpoint {
        base_url: save.base_url,
        token_credential: save.token_credential,
    }))
}

/// A recipe's state_json owns artifacts independently at every place.
/// Tensorlake's historical fields retain their flat stored shape, while
/// Podman records are keyed by place host. Saves never retire artifacts.
/// A successful replacement build records the replacement first. `{}` decodes to the
/// default; unknown fields are ignored so an older helm can read a newer
/// state.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct RecipeState {
    /// Podman storage is scoped to a place host's Unix account. Tensorlake's
    /// existing fields stay flat and unchanged so every stored snapshot and
    /// pending-create marker retains its historical meaning. Empty maps are
    /// omitted, preserving Tensorlake-only writes byte-for-field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) podman: BTreeMap<HostId, PodmanRecipeState>,
    /// The built software layer from which preparation starts.
    pub(crate) snapshot: Option<BuiltSnapshot>,
    /// Current installation checkpoint; failures preserve its provenance even
    /// when age, Farhelm version or a newer built source makes it unusable.
    pub(crate) prepared: Option<PreparedSnapshot>,
    /// Preparation owns a distinct sandbox and pending request, so a rebuild
    /// cannot mistake installation work for its own interrupted builder.
    pub(crate) preparer_sandbox_id: Option<String>,
    pub(crate) prepare_pending_create_at: Option<i64>,
    pub(crate) prepare_started_at: Option<i64>,
    pub(crate) last_prepare_failure: Option<PrepareFailureRecord>,
    /// The builder sandbox, from right after create (or from when one is
    /// found by its name) until it is terminated and its snapshots are
    /// deleted.
    pub(crate) builder_sandbox_id: Option<String>,
    /// When a builder create was sent whose outcome Farhelm does not know
    /// yet: written before the request goes out, cleared when the builder's
    /// ID is recorded, when Tensorlake refused the create outright, or when
    /// a lookup of the builder's name finds nothing at least
    /// [`crate::tensorlake::build::CREATE_SETTLE_SECS`] after the create was sent. While set, the next
    /// build and the recipe's deletion must look the name up.
    pub(crate) pending_create_at: Option<i64>,
    /// Set when a build starts and cleared when it ends either way; set with
    /// no live build means the helm stopped mid-build.
    pub(crate) build_started_at: Option<i64>,
    pub(crate) last_failure: Option<FailureRecord>,
    /// Snapshots this recipe no longer uses whose deletion has not succeeded
    /// yet: superseded by a rebuild, or retained from an older helm's
    /// save-time retirement rule.
    pub(crate) retired_snapshots: Vec<String>,
}

impl RecipeState {
    /// Whether billed resources or an interrupted build still need settling
    /// before the recipe can be forgotten. A build start keeps deletion on
    /// the cleanup path, but does not authorize adopting a sandbox by name;
    /// that needs the pending-create marker written before the request.
    pub(crate) fn needs_cleanup(&self) -> bool {
        self.snapshot.is_some()
            || self.prepared.is_some()
            || self.preparer_sandbox_id.is_some()
            || self.prepare_pending_create_at.is_some()
            || self.prepare_started_at.is_some()
            || self.builder_sandbox_id.is_some()
            || self.pending_create_at.is_some()
            || self.build_started_at.is_some()
            || !self.retired_snapshots.is_empty()
            || self.podman.values().any(PodmanRecipeState::needs_cleanup)
    }
}

#[cfg(test)]
#[path = "recipes_tests.rs"]
mod tests;

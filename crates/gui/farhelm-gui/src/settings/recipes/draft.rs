//! The recipe editor's draft: what the form holds, as text, and how it becomes the save request.
//!
//! Plain Rust over the shared recipe types, ported nearly verbatim from the old editor's `Draft`, `endpoint_save` and
//! their helpers (`tensorlake/recipe_editor.rs` at reference commit `24bc0580`). The rules it applies are the helm's
//! own, shared through `farhelm_proto::recipes` ([`check_recipe`], [`parse_packages`]; the editor checks a new name with
//! `check_recipe_name` before this), so a refusal the form can explain is said at once rather than after a round trip,
//! and the helm checks everything again.
//!
//! Here's the TLDR of its rules:
//!
//! - **Unsaved means different from the baseline.** The editor keeps the draft it opened as its baseline, and the form
//!   has unsaved changes exactly when the draft differs from it, so an edit typed back to its original is not one, and a
//!   different credential selection is.
//! - **Every base keeps its own text.** Switching bases never loses what was typed for another base; only the selected
//!   base is sent. Packages belong to every base.
//! - **Each agent keeps every mode's draft.** An agent's endpoint, its subscription login and its mode are separate
//!   fields, and only the selected mode's choice is sent (SPEC.md: "Switching modes retains each draft but saves only the
//!   selected choice"). Credentials are names from the catalog; no secret is ever part of a draft.
//! - **Reopening is stable.** A saved recipe loads into a draft that saves back unchanged, so Edit then Save changes
//!   nothing the user did not touch, the credential choices included.

use farhelm_proto::recipes::{
    EndpointSave, EndpointView, RecipeBase, RecipeSave, RecipeSize, RecipeView, Tunnel,
    check_recipe, parse_packages,
};

/// The base Farhelm builds on. Kept apart from the per-base text fields so switching bases never loses what was typed;
/// packages apply to every base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::settings) enum Base {
    TensorlakeUbuntu,
    Image,
    Snapshot,
    /// An existing OCI image and the login account prepared inside it.
    ContainerImage,
}

/// One endpoint's URL and named credential selection, with no secret draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::settings) struct EndpointDraft {
    pub(in crate::settings) base_url: String,
    pub(in crate::settings) token_credential: String,
}

/// How one agent authenticates on hosts made from the recipe. Each agent keeps every mode's draft while only the
/// selected mode's choice reaches the recipe, so a mode switch never submits a hidden endpoint or login name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::settings) enum AgentMode {
    None,
    Endpoint,
    Subscription,
}

/// One tunnel row as typed. Ports stay text until save so a half-typed row does not fight the input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::settings) struct TunnelDraft {
    pub(in crate::settings) sandbox_port: String,
    pub(in crate::settings) host: String,
    pub(in crate::settings) port: String,
}

/// The editor's whole form state (see the module docs for what equality with the baseline means).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::settings) struct Draft {
    pub(in crate::settings) name: String,
    pub(in crate::settings) cpus: String,
    pub(in crate::settings) memory_gb: String,
    pub(in crate::settings) disk_gb: String,
    pub(in crate::settings) base: Base,
    pub(in crate::settings) packages: String,
    pub(in crate::settings) image: String,
    pub(in crate::settings) snapshot_id: String,
    pub(in crate::settings) container_reference: String,
    pub(in crate::settings) container_user: String,
    pub(in crate::settings) claude: EndpointDraft,
    pub(in crate::settings) codex: EndpointDraft,
    pub(in crate::settings) claude_mode: AgentMode,
    pub(in crate::settings) claude_login_credential: String,
    pub(in crate::settings) codex_mode: AgentMode,
    pub(in crate::settings) codex_login_credential: String,
    pub(in crate::settings) github_token_credential: String,
    pub(in crate::settings) tunnels: Vec<TunnelDraft>,
}

impl Draft {
    /// A new recipe's starting values: a small size inside the Usage Credits plan's limits, and Tensorlake Ubuntu with
    /// no extra packages.
    pub(in crate::settings) fn new_recipe() -> Self {
        Self {
            name: String::new(),
            cpus: "2".into(),
            memory_gb: "4".into(),
            disk_gb: "20".into(),
            base: Base::TensorlakeUbuntu,
            packages: String::new(),
            image: String::new(),
            snapshot_id: String::new(),
            container_reference: String::new(),
            container_user: String::new(),
            claude: EndpointDraft::default(),
            codex: EndpointDraft::default(),
            claude_mode: AgentMode::None,
            claude_login_credential: String::new(),
            codex_mode: AgentMode::None,
            codex_login_credential: String::new(),
            github_token_credential: String::new(),
            tunnels: Vec::new(),
        }
    }

    /// The form for an existing recipe. Packages show one per line; whatever separators were typed originally are not
    /// stored. An agent with an endpoint opens in endpoint mode, one with a login in subscription mode, and the other
    /// mode's field starts empty.
    pub(in crate::settings) fn from_view(view: &RecipeView) -> Self {
        let mut draft = Self::new_recipe();
        draft.name = view.name.clone();
        draft.cpus = view.size.cpus.to_string();
        draft.memory_gb = view.size.memory_gb.to_string();
        draft.disk_gb = view.size.disk_gb.to_string();
        draft.packages = view.packages.join("\n");
        match &view.base {
            RecipeBase::ManagedUbuntu => {
                draft.base = Base::TensorlakeUbuntu;
            }
            RecipeBase::TensorlakeImage { image } => {
                draft.base = Base::Image;
                draft.image = image.clone();
            }
            RecipeBase::TensorlakeSnapshot { snapshot_id } => {
                draft.base = Base::Snapshot;
                draft.snapshot_id = snapshot_id.clone();
            }
            RecipeBase::ContainerImage { reference, user } => {
                draft.base = Base::ContainerImage;
                draft.container_reference = reference.clone();
                draft.container_user = user.clone();
            }
        }
        let endpoint = |view: &Option<EndpointView>| EndpointDraft {
            base_url: view
                .as_ref()
                .map(|endpoint| endpoint.base_url.clone())
                .unwrap_or_default(),
            token_credential: view
                .as_ref()
                .and_then(|endpoint| endpoint.token_credential.clone())
                .unwrap_or_default(),
        };
        draft.claude = endpoint(&view.claude);
        draft.codex = endpoint(&view.codex);
        draft.claude_mode = mode(
            view.claude.is_some(),
            view.claude_login_credential.is_some(),
        );
        draft.claude_login_credential = view.claude_login_credential.clone().unwrap_or_default();
        draft.codex_mode = mode(view.codex.is_some(), view.codex_login_credential.is_some());
        draft.codex_login_credential = view.codex_login_credential.clone().unwrap_or_default();
        draft.github_token_credential = view.github_token_credential.clone().unwrap_or_default();
        draft.tunnels = view
            .tunnels
            .iter()
            .map(|tunnel| TunnelDraft {
                sandbox_port: tunnel.sandbox_port.to_string(),
                host: tunnel.host.clone(),
                port: tunnel.port.to_string(),
            })
            .collect();
        draft
    }

    /// The save request this draft describes, or the first problem a user can fix in the form, checked with the same
    /// rules the helm applies. Only the selected base is sent; package text belongs to every base, while hidden image
    /// and snapshot text stays local to the editing session. The name is not checked here: an existing recipe's never
    /// changes, and the editor checks a new one first. Messages are lowercase like the helm's; the caller capitalizes
    /// them.
    pub(in crate::settings) fn to_save(&self) -> Result<RecipeSave, String> {
        let number = |text: &str, what: &str| {
            text.trim()
                .parse::<u32>()
                .map_err(|_| format!("{what} must be a whole number"))
        };
        let port = |text: &str| {
            text.trim()
                .parse::<u16>()
                .map_err(|_| "tunnel ports must be from 1 to 65535".to_string())
        };
        let size = RecipeSize {
            cpus: number(&self.cpus, "CPUs")?,
            memory_gb: number(&self.memory_gb, "memory")?,
            disk_gb: number(&self.disk_gb, "disk")?,
        };
        let base = self.recipe_base();
        let tunnels = self
            .tunnels
            .iter()
            .map(|row| {
                Ok(Tunnel {
                    sandbox_port: port(&row.sandbox_port)?,
                    host: row.host.trim().to_string(),
                    port: port(&row.port)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let claude = if self.claude_mode == AgentMode::Endpoint {
            Some(endpoint_save(&self.claude).ok_or("Claude API endpoint needs a base URL")?)
        } else {
            None
        };
        let claude_login_credential = if self.claude_mode == AgentMode::Subscription {
            if self.claude_login_credential.is_empty() {
                return Err("choose a Claude login in Subscription mode".into());
            }
            Some(self.claude_login_credential.clone())
        } else {
            None
        };
        let codex = if self.codex_mode == AgentMode::Endpoint {
            Some(endpoint_save(&self.codex).ok_or("Codex API endpoint needs a base URL")?)
        } else {
            None
        };
        let codex_login_credential = if self.codex_mode == AgentMode::Subscription {
            if self.codex_login_credential.is_empty() {
                return Err("choose a Codex login in Subscription mode".into());
            }
            Some(self.codex_login_credential.clone())
        } else {
            None
        };
        let save = RecipeSave {
            size,
            base,
            packages: parse_packages(&self.packages),
            claude,
            codex,
            claude_login_credential,
            codex_login_credential,
            github_token_credential: (!self.github_token_credential.is_empty())
                .then(|| self.github_token_credential.clone()),
            tunnels,
        };
        check_recipe(&save)?;
        Ok(save)
    }

    /// The selected base with its own text, as a save sends it: an image or snapshot name trimmed, a container's
    /// reference and user as typed (the helm judges those whole). The other bases' text is left out.
    pub(in crate::settings) fn recipe_base(&self) -> RecipeBase {
        match self.base {
            Base::TensorlakeUbuntu => RecipeBase::ManagedUbuntu,
            Base::Image => RecipeBase::TensorlakeImage {
                image: self.image.trim().to_string(),
            },
            Base::Snapshot => RecipeBase::TensorlakeSnapshot {
                snapshot_id: self.snapshot_id.trim().to_string(),
            },
            Base::ContainerImage => RecipeBase::ContainerImage {
                reference: self.container_reference.clone(),
                user: self.container_user.clone(),
            },
        }
    }
}

/// The mode a saved agent opens in: an endpoint wins over a login, which the helm never stores together anyway.
fn mode(endpoint: bool, login: bool) -> AgentMode {
    if endpoint {
        AgentMode::Endpoint
    } else if login {
        AgentMode::Subscription
    } else {
        AgentMode::None
    }
}

/// The save shape of one endpoint draft.
///
/// A blank base URL removes the endpoint's reference; the catalog value stays saved. Unlike an unsaved secret, a name
/// needs no discard warning of its own.
fn endpoint_save(draft: &EndpointDraft) -> Option<EndpointSave> {
    let base_url = draft.base_url.trim();
    if base_url.is_empty() {
        return None;
    }
    Some(EndpointSave {
        base_url: base_url.to_string(),
        token_credential: (!draft.token_credential.is_empty())
            .then(|| draft.token_credential.clone()),
    })
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;

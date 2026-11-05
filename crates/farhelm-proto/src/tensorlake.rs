//! The helm's Tensorlake account settings as the UI reads and writes them.
//!
//! Tensorlake sandboxes are hosts Farhelm creates and manages through
//! Tensorlake's REST API, with an API key the user pastes into Settings. The
//! key follows the ntfy token's write-only rule: the read shape carries only
//! whether one is saved, so a client can replace or clear the key without
//! ever reading it back.
//!
//! The same reply also carries the two per-helm facts the section shows: the
//! helm tag that prefixes the names of sandboxes this helm creates, and the
//! ssh key Farhelm generated for reaching them, with the one-time `tl`
//! command that registers it. An API key cannot register ssh keys, so Farhelm
//! cannot do that step itself, and cannot check that it was done.

/// Recipes: the named descriptions Farhelm creates sandboxes from (see the
/// module's own docs).
pub mod recipes;

use serde::{Deserialize, Serialize};

/// The explicit lifecycle operation Farhelm is still handling under a hold.
/// This is a display direction, not the vendor's sandbox status: a suspended
/// sandbox stays suspended until Farhelm asks to resume it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleDirection {
    Suspending,
    Resuming,
}

/// The non-secret snapshot returned after loading or saving the settings.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Settings {
    /// Whether an API key is saved. The key itself never leaves the helm.
    pub api_key_set: bool,
    /// Whether adding a Tensorlake host skips its billing confirmation. Off by
    /// default, and separate from the general "set up new hosts without
    /// asking" preference, which never covers creating a sandbox.
    pub create_without_asking: bool,
    /// A short lowercase alphanumeric tag, stable for this helm, that the
    /// names of the sandboxes it creates carry. It tells this helm's
    /// sandboxes apart from another helm's on the same Tensorlake account.
    pub helm_tag: String,
    /// Farhelm's own ssh key for Tensorlake sandboxes.
    pub ssh_key: SshKeyStatus,
}

/// Farhelm's Tensorlake ssh key, or why the helm has none to show.
///
/// Reading the settings never creates the key; the user asks for it with a
/// separate request (`POST /api/tensorlake/ssh-key`). A key that exists but
/// cannot be read is a state the section shows, never a failed read of the
/// rest of the settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SshKeyStatus {
    /// The key exists in the helm's state directory.
    Ready {
        /// The public key line, as `ssh-keygen` wrote it.
        public_key: String,
        /// The public key file's absolute path on the helm's machine.
        public_key_path: String,
        /// The name the registration command gives the key in Tensorlake.
        name: String,
        /// The one-time command that registers the key with the user's
        /// Tensorlake account: `tl sbx ssh keys add --name <name> '<key>'`. It
        /// carries the public key's text, so it runs anywhere `tl` is signed
        /// in, not only on the helm's machine.
        register_command: String,
    },
    /// This helm has no key yet. Creating one is an explicit action.
    NotCreated,
    /// The helm could not create or read the key; the reason is for display.
    Unavailable { reason: String },
}

impl Default for SshKeyStatus {
    fn default() -> Self {
        Self::Unavailable {
            reason: String::new(),
        }
    }
}

/// One explicit save. Every field is optional, so a control sends only what
/// it owns and a stale form cannot overwrite a value it did not edit.
///
/// An absent or blank `api_key` keeps the saved key; anything else replaces
/// it. `clear_api_key` removes it, and asking for both is refused. An absent
/// `create_without_asking` keeps the saved choice. Debug is deliberately
/// absent: this type can carry the key. Unknown fields are refused, because
/// with every field optional a misspelled one would otherwise save nothing
/// and still succeed.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SaveSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub clear_api_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_without_asking: Option<bool>,
}

/// A successful Test: the saved key listed the account's sandboxes.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TestResult {
    /// How many sandboxes (of any kind, not only Farhelm's) the key can see.
    pub sandboxes: u64,
}

/// Recipe forwards sharing one SSH connection and its health.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TunnelView {
    pub tunnels: Vec<recipes::Tunnel>,
    pub state: TunnelState,
}

/// Live forwarding health, independent of the supervisor's connection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum TunnelState {
    Up,
    Failed {
        reason: String,
    },
    Retrying {
        reason: Option<String>,
        delay_secs: u64,
    },
}

/// A reserved-name sandbox no host or recipe on this helm owns. IDs keep
/// deletion retries possible after termination releases the name.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrphanSandbox {
    pub id: String,
    pub name: String,
    pub created_at: Option<i64>,
    pub state: String,
    pub from_another_helm: bool,
    /// Explicit cleanup survives closing Settings, including after the
    /// sandbox has terminated and disappeared from the vendor's listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<OrphanCleanup>,
}

/// Helm-owned, in-memory progress for an explicitly requested deletion.
/// It is retained until the snapshot sweep succeeds; nothing retries it
/// automatically and a helm restart discards it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum OrphanCleanup {
    Running,
    Failed { reason: String },
}

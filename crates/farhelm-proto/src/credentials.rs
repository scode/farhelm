//! Named, write-only credentials owned by the helm.
//!
//! Requests may contain secrets; snapshots contain only identity, timestamps
//! and recipe references. Keeping the wire shapes separate prevents an ordinary
//! list or save response from accidentally serializing a stored credential.

use serde::{Deserialize, Serialize};

/// The consumer fixes a credential's interpretation for its entire lifetime.
/// Names are globally unique, so a recipe cannot silently start using another
/// kind when someone replaces a value under the same name.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    GithubToken,
    ClaudeLogin,
    CodexLogin,
    EndpointToken,
}

/// Saved metadata safe to return to any authenticated Settings client.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Credential {
    pub name: String,
    pub kind: CredentialKind,
    pub created_at: i64,
    pub updated_at: i64,
    /// Replacements advance this independently of timestamp resolution.
    pub revision: u64,
    /// Recipe names, rather than secret values or host configuration.
    pub used_by: Vec<String>,
}

/// Create one fixed identity. No Debug implementation: the request is secret.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCredential {
    pub name: String,
    pub kind: CredentialKind,
    pub secret: String,
}

/// Replace only the value; absent or blank text keeps the saved credential.
/// Neither its name nor its consumer can be changed by a stale editor.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SaveCredential {
    pub secret: Option<String>,
}

/// A committed save may leave a host's previous value in place. Reasons carry
/// no submitted bytes or transport captures, and identify the affected host.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeliveryFailure {
    pub host: String,
    pub reason: String,
}

/// Delivery failure does not undo the saved credential. Callers acknowledge
/// storage and display these failures separately instead of offering a retry
/// that would unnecessarily replace the value again.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedCredential {
    pub credential: Credential,
    pub delivery_failures: Vec<DeliveryFailure>,
}

/// Authentication files are intentionally opaque beyond being bounded objects.
/// Vendor-added fields must survive a paste or upload without a Farhelm update.
pub const CODEX_LOGIN_CAP: usize = 64 * 1024;

/// Validate a replacement without returning any part of its contents in errors.
/// Surrounding whitespace is a copy/paste artifact and is removed before use.
pub fn check_secret(kind: CredentialKind, secret: &str) -> Result<(), &'static str> {
    let secret = secret.trim();
    if secret.is_empty() {
        return Err("a credential value is required");
    }
    if kind == CredentialKind::CodexLogin {
        if secret.len() > CODEX_LOGIN_CAP {
            return Err("a Codex login must be a JSON object of at most 64 KiB");
        }
        match serde_json::from_str::<serde_json::Value>(secret) {
            Ok(value) if value.is_object() => Ok(()),
            _ => Err("a Codex login must be a JSON object of at most 64 KiB"),
        }
    } else {
        crate::tensorlake::recipes::check_token(secret)
    }
}

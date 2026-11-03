//! The helm's Tensorlake account configuration: the API key and the
//! "create sandboxes without asking" choice, stored as one JSON singleton row
//! in `helm.db` (`tensorlake_account`, schema 70).
//!
//! This follows the ntfy settings' shape: dedicated helm configuration kept
//! apart from the replayed preferences row, explicit saves that validate, and
//! a secret that no read ever returns. The key is stored unencrypted, like
//! the ntfy token; the Settings wording says so plainly. Later PRs add their
//! own tables (recipes) rather than growing this row.

use super::api::ApiKey;
use crate::store::HelmStore;
use farhelm_proto::tensorlake::SaveSettings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The stored row. Never crosses the API boundary as this type; the routes
/// build [`farhelm_proto::tensorlake::Settings`] from it, which carries only
/// whether a key is saved.
///
/// Missing fields decode as their defaults, so a field added here later
/// needs no migration.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Config {
    pub(crate) api_key: Option<String>,
    pub(crate) create_without_asking: bool,
}

/// The longest key Farhelm accepts. Tensorlake's keys are well under 100
/// characters; this bounds what a paste can store, not what a key may be.
const MAX_KEY_LEN: usize = 4096;

impl Config {
    /// The saved key, ready to pass to a call.
    pub(crate) fn api_key(&self) -> Option<ApiKey> {
        self.api_key.clone().map(ApiKey::new)
    }

    /// Apply one save to the stored row. Resolve this against the stored row
    /// inside the save's transaction, so two windows saving different fields
    /// cannot undo each other.
    ///
    /// A pasted key is trimmed, because a copy from a dashboard often brings
    /// a trailing newline; what remains must be visible ASCII, which every
    /// bearer token is. Refusals use fixed messages so a mistyped key never
    /// reaches an error string.
    pub(crate) fn saved(previous: Self, save: SaveSettings) -> Result<Self, &'static str> {
        let replacement = save
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty());
        if replacement.is_some() && save.clear_api_key {
            return Err("paste a new API key or clear the saved one, not both");
        }
        let api_key = match replacement {
            Some(key) => {
                if key.len() > MAX_KEY_LEN || !key.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
                    return Err(
                        "an API key must be at most 4096 visible ASCII characters with no spaces",
                    );
                }
                Some(key.to_string())
            }
            None if save.clear_api_key => None,
            None => previous.api_key,
        };
        Ok(Self {
            api_key,
            create_without_asking: save
                .create_without_asking
                .unwrap_or(previous.create_without_asking),
        })
    }
}

/// How many characters the helm tag has. Four base-36 characters give about
/// 1.7 million values: enough that two helms sharing one Tensorlake account
/// are very unlikely to collide, and short enough that the sandbox names it
/// prefixes stay readable.
const HELM_TAG_LEN: usize = 4;

/// This helm's tag for the sandboxes it creates, derived from the registry
/// namespace, which is minted once with `helm.db` and never changes.
///
/// Hashed rather than cut from the namespace: the namespace is part of agent
/// retry keys, and a hash keeps sandbox names, which Tensorlake and anyone
/// with access to the account can see, from carrying any of it. The tag is
/// lowercase letters and digits only, so it fits inside a Tensorlake sandbox
/// name (lowercase letters, digits and hyphens).
pub(crate) fn helm_tag(namespace: &str) -> String {
    let digest = Sha256::new()
        .chain_update(b"farhelm tensorlake helm tag\0")
        .chain_update(namespace.as_bytes())
        .finalize();
    let mut value = u64::from_be_bytes(digest[..8].try_into().expect("a digest has 8 bytes"));
    let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut tag = String::with_capacity(HELM_TAG_LEN);
    for _ in 0..HELM_TAG_LEN {
        tag.push(alphabet[(value % 36) as usize] as char);
        value /= 36;
    }
    tag
}

/// Validation and storage refusals stay distinct, without secret data in
/// either. The routes use only these fixed messages.
pub(crate) enum SaveError {
    Invalid(&'static str),
    Storage,
}

impl HelmStore {
    /// Read the account row, never the shared preference snapshot.
    pub(crate) async fn tensorlake_config(&self) -> anyhow::Result<Config> {
        self.conn()
            .call("tensorlake settings read", |conn| {
                let json: String = conn.query_row(
                    "SELECT config FROM tensorlake_account WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )?;
                Ok(serde_json::from_str(&json)?)
            })
            .await
    }

    /// Apply one save to the row inside one database operation and return
    /// the row as stored.
    pub(crate) async fn save_tensorlake_config(
        &self,
        save: SaveSettings,
    ) -> Result<Config, SaveError> {
        let result = self
            .conn()
            .call("tensorlake settings save", move |conn| {
                let tx = conn.transaction()?;
                let previous: String = tx.query_row(
                    "SELECT config FROM tensorlake_account WHERE singleton = 1",
                    [],
                    |row| row.get(0),
                )?;
                let config = match Config::saved(serde_json::from_str(&previous)?, save) {
                    Ok(config) => config,
                    Err(reason) => return Ok(Err(reason)),
                };
                tx.execute(
                    "UPDATE tensorlake_account SET config = ?1 WHERE singleton = 1",
                    [serde_json::to_string(&config)?],
                )?;
                tx.commit()?;
                Ok(Ok(config))
            })
            .await
            .map_err(|_| SaveError::Storage)?;
        result.map_err(SaveError::Invalid)
    }
}

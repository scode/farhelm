//! Shared notices: persistent conditions the helm records about a host, as `GET /api/notices` serves them and the UI
//! channel pushes them.
//!
//! The helm owns these conditions (its notices module records and resolves them); this module only holds their shape,
//! so the helm's serializer and the GUI's decoder are the same type rather than two copies that can drift.

use serde::{Deserialize, Serialize};

/// A condition's last-known explanation. Bytes describe the last successful measurement, which can outlive a later
/// unknown or unrequested measurement.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoticeContent {
    StuckTrash {
        checkout_id: String,
        name: String,
        repository: String,
        bytes: Option<u64>,
        reason: String,
        at: i64,
    },
}

/// Stable identity and first observation belong to the notice, not to each removal attempt. Resolved notices are
/// always served as read.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Notice {
    pub id: i64,
    /// The registry id of the host the condition is about.
    pub host: i64,
    pub raised_at: i64,
    pub read: bool,
    pub resolved: bool,
    pub content: NoticeContent,
}

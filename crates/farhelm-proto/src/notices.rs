//! Shared notices: persistent conditions about a host or Farhelm, as `GET /api/notices` serves them and the UI
//! channel pushes them.
//!
//! The helm owns these conditions (its notices module records and resolves them); this module only holds their shape,
//! so the helm's serializer and the GUI's decoder are the same type rather than two copies that can drift.

use serde::{Deserialize, Serialize};

/// The last-known condition. Stuck trash keeps the last measured size even if a later measurement is missing.
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
    HostSessionLimit,
    FleetSessionLimit,
}

/// Stable identity and first observation belong to the notice, not to each removal attempt. Resolved notices are
/// always served as read.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Notice {
    pub id: i64,
    /// The registry id for a host condition. An app-wide condition has no host.
    pub host: Option<i64>,
    pub raised_at: i64,
    pub read: bool,
    pub resolved: bool,
    pub content: NoticeContent,
}

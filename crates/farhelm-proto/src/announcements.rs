//! The helm's announcement queue, as `GET /api/announcements` serves it and the UI channel pushes it.
//!
//! The helm owns the queue (its announcements module stores, orders and dismisses entries); this module only holds
//! the shape, so the helm's serializer and the GUI's decoder are one type.

use serde::{Deserialize, Serialize};

/// Display-ready content, kept unchanged for the lifetime of its id. Optional detail and link have no source-specific
/// meaning to the queue: producers supply their wording and destination, and clients render them.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
    pub link: Option<AnnouncementLink>,
}

/// One optional, labelled destination supplied by the announcement's producer.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnnouncementLink {
    pub label: String,
    pub url: String,
}

/// One consistent queue snapshot. Pending runs oldest insertion first; history runs newest shown time first, then
/// newest insertion first.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct AnnouncementQueue {
    pub pending: Vec<Announcement>,
    pub history: Vec<Announcement>,
}

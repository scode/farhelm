//! Explicitly saved helm configuration for ntfy alerts. The read shape contains
//! only a token-presence bit; a client can replace a secret without ever reading it.

use serde::{Deserialize, Serialize};

/// ntfy's five urgency levels, shared so the form and sender use the same names.
#[derive(Clone, Copy, Default, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Min,
    Low,
    #[default]
    Default,
    High,
    Urgent,
}

impl Priority {
    /// Header spelling accepted by ntfy, independent of display formatting.
    pub fn header(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Low => "low",
            Self::Default => "default",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
}

/// The non-secret snapshot returned after loading or saving the helm's settings.
/// Topic is editable configuration, but must never be included in diagnostics.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub priority: Priority,
    pub token_set: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "https://ntfy.sh".into(),
            topic: String::new(),
            priority: Priority::Default,
            token_set: false,
        }
    }
}

/// One explicit save. Null/omitted token preserves it; empty clears it; any
/// other string replaces it. Debug is deliberately absent from secret-bearing types.
#[derive(Clone, Deserialize, Serialize)]
pub struct SaveSettings {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub priority: Priority,
    pub token: Option<String>,
}

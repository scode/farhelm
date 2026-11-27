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
    /// The settings label, independent of ntfy's lowercase HTTP header vocabulary.
    ///
    /// Sending an alert must use `header`, even when the form shows this label.
    pub fn display_label(self) -> &'static str {
        match self {
            Self::Min => "Min",
            Self::Low => "Low",
            Self::Default => "Default",
            Self::High => "High",
            Self::Urgent => "Urgent",
        }
    }

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
///
/// `enabled` is the master switch: nothing is sent while it is off. The event
/// switches below it choose which alerts the master switch lets through.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub priority: Priority,
    pub token_set: bool,
    /// Alert when an agent waits for an answer. Defaults on when absent, so a
    /// configuration saved before the switch existed keeps sending the waiting
    /// alerts it always sent.
    #[serde(default = "waiting_default")]
    pub notify_waiting: bool,
}

/// What an absent `notify_waiting` means: on, the only alert there used to be.
fn waiting_default() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "https://ntfy.sh".into(),
            topic: String::new(),
            priority: Priority::Default,
            token_set: false,
            notify_waiting: true,
        }
    }
}

impl Settings {
    /// Whether the helm should alert when an agent waits for an answer: the
    /// master switch, the waiting switch and a topic to send to. The waiting
    /// policy reads this to decide whether to track waiting spells at all.
    pub fn sends_waiting_alerts(&self) -> bool {
        self.enabled && self.notify_waiting && !self.topic.is_empty()
    }
}

/// One explicit save. A null/omitted token preserves it only when the server's
/// scheme, host and effective port stay the same; changing origin clears it.
/// Empty clears it and any other string replaces it regardless of origin.
/// Debug is deliberately absent from secret-bearing types.
#[derive(Clone, Deserialize, Serialize)]
pub struct SaveSettings {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub priority: Priority,
    pub token: Option<String>,
    /// See [`Settings::notify_waiting`]. Absent keeps the stored value, the
    /// way an absent token keeps the saved token: a client older than the
    /// switch saves the whole form it knows, and must not turn waiting alerts
    /// back on that a newer client turned off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_waiting: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Configurations written before the waiting switch existed must keep the waiting alerts they always had, and a
    /// save from a client that predates the switch must leave it as stored. Specified: stored settings JSON without
    /// `notify_waiting` decodes with it on, the defaults have it on, and a save body without it decodes as none.
    #[test]
    fn a_missing_waiting_switch_reads_as_on() {
        let stored: Settings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token_set":false}"#,
        )
        .expect("old settings decode");
        assert!(stored.notify_waiting);
        let save: SaveSettings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token":null}"#,
        )
        .expect("old save decodes");
        assert_eq!(save.notify_waiting, None);
        assert!(Settings::default().notify_waiting);
    }

    /// The waiting policy tracks spells only while alerts can actually go out. Specified: on only with the master
    /// switch on, the waiting switch on and a topic set.
    #[test]
    fn waiting_alerts_need_the_master_switch_the_waiting_switch_and_a_topic() {
        let on = Settings {
            enabled: true,
            topic: "t".into(),
            ..Settings::default()
        };
        assert!(on.sends_waiting_alerts());
        for off in [
            Settings {
                enabled: false,
                ..on.clone()
            },
            Settings {
                notify_waiting: false,
                ..on.clone()
            },
            Settings {
                topic: String::new(),
                ..on.clone()
            },
        ] {
            assert!(!off.sends_waiting_alerts());
        }
    }
}

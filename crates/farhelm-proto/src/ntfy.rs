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
    /// Alert when an agent says it is blocked (`farhelm agent notify`). Off by default, including for configurations
    /// saved before it existed.
    #[serde(default)]
    pub notify_blocked: bool,
    /// What counts as blocked, in the user's words, appended to the blocked-agent instruction; `None` uses
    /// [`DEFAULT_BLOCKED_CRITERIA`]. Tracked as a choice rather than as text, so a configuration that never chose its
    /// own criteria follows every later change to Farhelm's default wording instead of keeping a copy of an old one.
    #[serde(default)]
    pub custom_blocked_criteria: Option<String>,
    /// Whether the agents Farhelm starts are asked to run `farhelm agent notify` when they are blocked (the
    /// blocked-agent instruction), independently of ntfy: the session list shows a blocked session as waiting whether
    /// or not any alert goes out. `None` is a configuration saved before the choice had its own switch, which asked
    /// exactly while ntfy alerts and their blocked switch were both on; [`Settings::asks_agents_when_blocked`] keeps
    /// that meaning, so an upgrade changes no running behavior. To decide whether to ask, read it through that method;
    /// only a save, which must keep an absent value absent, reads the field itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_agents_when_blocked: Option<bool>,
}

/// The blocked-agent instruction's first part, always given while agents are asked to notify when blocked
/// ([`Settings::asks_agents_when_blocked`]): what to run and the intent, which is not to make the agent try harder,
/// only to keep it from stopping silently. The maintainer settled this wording in the plan
/// `agent-blocked-notifications`; change it deliberately.
pub const BLOCKED_INSTRUCTION_FRAMING: &str = "farhelm: If you are going to stop without completing what the user \
     asked because you are blocked, do not stop silently \u{2014} run `farhelm agent notify \"<what is blocked \
     (briefly)>: <why is it blocked (briefly)>\"` before ending your turn so the user is notified right away. Do not \
     treat this as a request to try harder or work around blockers; only make sure you notify instead of stopping \
     quietly. Keep the reason to one short sentence saying what blocked you and what you need.";

/// Farhelm's own answer to what counts as blocked, used while the user has not written their own
/// ([`Settings::custom_blocked_criteria`] is `None`). Settings shows it read-only beside the empty custom field.
pub const DEFAULT_BLOCKED_CRITERIA: &str = "you cannot finish what the user asked because of missing credentials, \
     permissions, tools, or access; a decision or clarification only the user can make; an external failure or \
     environment problem you cannot resolve; or any case where you are leaving requested work unfinished or skipping \
     part of the request.";

/// The most characters custom criteria may have once normalized ([`normalize_blocked_criteria`]).
pub const BLOCKED_CRITERIA_CAP: usize = 1000;

/// The blocked-agent instruction for the agents Farhelm starts under `settings`, or `None` while agents are not asked
/// to notify ([`Settings::asks_agents_when_blocked`]): [`BLOCKED_INSTRUCTION_FRAMING`], then "What counts as blocked: "
/// and the user's criteria or [`DEFAULT_BLOCKED_CRITERIA`]. The one source of the instruction the helm sends with each
/// create and Restart; the desktop shows the same default criteria constant.
pub fn blocked_instructions(settings: &Settings) -> Option<String> {
    settings.asks_agents_when_blocked().then(|| {
        let criteria = settings
            .custom_blocked_criteria
            .as_deref()
            .unwrap_or(DEFAULT_BLOCKED_CRITERIA);
        format!("{BLOCKED_INSTRUCTION_FRAMING} What counts as blocked: {criteria}")
    })
}

/// Custom criteria as they are stored: whitespace runs (line breaks included) one space, characters that must not be
/// shown as themselves removed, trimmed; empty means Farhelm's default (`None`). Refused, with a reason a form can
/// show, when longer than [`BLOCKED_CRITERIA_CAP`] characters. The helm applies it to every save.
pub fn normalize_blocked_criteria(text: &str) -> Result<Option<String>, &'static str> {
    let safe: String = text
        .chars()
        .filter(|c| c.is_whitespace() || !crate::text::is_presentation_unsafe(*c))
        .collect();
    let normalized = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() > BLOCKED_CRITERIA_CAP {
        return Err("what counts as blocked must be at most 1000 characters");
    }
    Ok((!normalized.is_empty()).then_some(normalized))
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
            notify_blocked: false,
            custom_blocked_criteria: None,
            ask_agents_when_blocked: None,
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

    /// Whether the helm should send an ntfy alert when an agent says it is blocked: the master switch, the blocked
    /// switch and a topic. The session's blocked mark does not depend on this.
    pub fn sends_blocked_alerts(&self) -> bool {
        self.enabled && self.notify_blocked && !self.topic.is_empty()
    }

    /// Whether the agents Farhelm starts are asked to notify when blocked: the stored choice, or, for a configuration
    /// saved before the choice existed, what it did then, while ntfy alerts and their blocked switch were both on.
    pub fn asks_agents_when_blocked(&self) -> bool {
        self.ask_agents_when_blocked
            .unwrap_or(self.enabled && self.notify_blocked)
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
    /// See [`Settings::notify_blocked`]; absent keeps the stored value, as for `notify_waiting`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_blocked: Option<bool>,
    /// See [`Settings::custom_blocked_criteria`]. Absent keeps the stored choice, as for `notify_waiting`; text that
    /// normalizes to nothing ([`normalize_blocked_criteria`]), the empty string included, goes back to Farhelm's
    /// default; other text is the user's own criteria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_blocked_criteria: Option<String>,
    /// See [`Settings::ask_agents_when_blocked`]. Absent keeps the stored choice, as for `notify_waiting`, so a client
    /// older than the switch neither turns the instruction on nor off; a client that has the switch always sends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_agents_when_blocked: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Configurations written before the waiting switch existed must keep the waiting alerts they always had, and a
    /// save from a client that predates the switch must leave it as stored; the blocked switch, newer still, starts off.
    /// Specified: stored settings JSON without either switch decodes with waiting on and blocked off, the defaults have
    /// waiting on, and a save body without them decodes both as none.
    #[test]
    fn a_missing_waiting_switch_reads_as_on() {
        let stored: Settings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token_set":false}"#,
        )
        .expect("old settings decode");
        assert!(stored.notify_waiting);
        assert!(!stored.notify_blocked, "blocked alerts start off");
        let save: SaveSettings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token":null}"#,
        )
        .expect("old save decodes");
        assert_eq!(save.notify_waiting, None);
        assert_eq!(save.notify_blocked, None);
        assert!(Settings::default().notify_waiting);
    }

    /// The blocked-agent instruction is the maintainer's settled wording, and a configuration that never chose its
    /// own criteria must follow Farhelm's default rather than keep a copy. Specified: with no stored choice, off while
    /// either the master or the blocked switch is off; a stored choice decides on its own, whatever those switches say;
    /// on, the framing and "What counts as blocked: " followed by the default criteria when none are chosen, or by the
    /// chosen text; the framing names `farhelm agent notify` and says not to try harder.
    #[test]
    fn the_blocked_instruction_is_the_framing_and_the_chosen_criteria() {
        let on = Settings {
            enabled: true,
            notify_blocked: true,
            ..Settings::default()
        };
        assert_eq!(
            blocked_instructions(&on).as_deref(),
            Some(
                format!("{BLOCKED_INSTRUCTION_FRAMING} What counts as blocked: {DEFAULT_BLOCKED_CRITERIA}")
                    .as_str()
            )
        );
        let custom = Settings {
            custom_blocked_criteria: Some("the build is red".into()),
            ..on.clone()
        };
        assert!(
            blocked_instructions(&custom)
                .unwrap()
                .ends_with(" What counts as blocked: the build is red")
        );
        for off in [
            Settings {
                enabled: false,
                ..on.clone()
            },
            Settings {
                notify_blocked: false,
                ..on.clone()
            },
        ] {
            assert_eq!(blocked_instructions(&off), None);
        }
        // The explicit choice decides, whatever ntfy's own switches say.
        assert!(
            blocked_instructions(&Settings {
                enabled: false,
                notify_blocked: false,
                ask_agents_when_blocked: Some(true),
                ..on.clone()
            })
            .is_some()
        );
        assert_eq!(
            blocked_instructions(&Settings {
                ask_agents_when_blocked: Some(false),
                ..on.clone()
            }),
            None
        );
        // The maintainer's settled wording (the plan `agent-blocked-notifications`), spelled out in full so an edit to
        // either constant fails here rather than reaching agents unnoticed.
        assert_eq!(
            BLOCKED_INSTRUCTION_FRAMING,
            "farhelm: If you are going to stop without completing what the user asked because you are blocked, do \
             not stop silently \u{2014} run `farhelm agent notify \"<what is blocked (briefly)>: <why is it blocked \
             (briefly)>\"` before ending your turn so the user is notified right away. Do not treat this as a \
             request to try harder or work around blockers; only make sure you notify instead of stopping quietly. \
             Keep the reason to one short sentence saying what blocked you and what you need."
        );
        assert_eq!(
            DEFAULT_BLOCKED_CRITERIA,
            "you cannot finish what the user asked because of missing credentials, permissions, tools, or access; a \
             decision or clarification only the user can make; an external failure or environment problem you \
             cannot resolve; or any case where you are leaving requested work unfinished or skipping part of the \
             request."
        );
        assert!(!BLOCKED_INSTRUCTION_FRAMING.contains('\n'));
    }

    /// Custom criteria are stored the way they are sent to agents, and the empty choice must mean the default, never
    /// an empty criteria line. Specified: whitespace runs (line breaks included) become one space and the ends are
    /// trimmed; a presentation-unsafe character is removed; empty or whitespace-only text is the default (`None`);
    /// text at the cap is kept and one character more is refused.
    #[test]
    fn custom_criteria_are_normalized_and_bounded() {
        assert_eq!(
            normalize_blocked_criteria("  red\n  build\t ")
                .unwrap()
                .as_deref(),
            Some("red build")
        );
        assert_eq!(
            normalize_blocked_criteria("a\u{202e}b").unwrap().as_deref(),
            Some("ab")
        );
        assert_eq!(normalize_blocked_criteria(" \n ").unwrap(), None);
        assert!(normalize_blocked_criteria(&"x".repeat(BLOCKED_CRITERIA_CAP)).is_ok());
        assert!(normalize_blocked_criteria(&"x".repeat(BLOCKED_CRITERIA_CAP + 1)).is_err());
    }

    /// A configuration saved before "Ask agents to notify when blocked" had its own switch must keep asking exactly
    /// when it did, so an upgrade changes nothing, and a stored choice must win over ntfy's switches either way, so
    /// the instruction works without ntfy. Specified: absent, the choice reads as on only with ntfy alerts and their
    /// blocked switch both on; stored, it reads as stored whatever those switches are; it is left off the wire while
    /// absent, and a save without it decodes as none.
    #[test]
    fn asking_agents_when_blocked_upgrades_from_the_ntfy_switches() {
        for (enabled, notify_blocked) in
            [(true, true), (true, false), (false, true), (false, false)]
        {
            let old = Settings {
                enabled,
                notify_blocked,
                ..Settings::default()
            };
            assert_eq!(
                old.asks_agents_when_blocked(),
                enabled && notify_blocked,
                "{enabled} {notify_blocked}"
            );
            for chosen in [true, false] {
                let new = Settings {
                    ask_agents_when_blocked: Some(chosen),
                    ..old.clone()
                };
                assert_eq!(new.asks_agents_when_blocked(), chosen);
            }
        }
        let wire = serde_json::to_value(Settings::default()).unwrap();
        assert!(wire.get("ask_agents_when_blocked").is_none(), "{wire}");
        let save: SaveSettings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token":null}"#,
        )
        .unwrap();
        assert_eq!(save.ask_agents_when_blocked, None);
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

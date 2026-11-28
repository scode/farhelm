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
    /// How many seconds an alert that is ready to go out (a session settled into waiting, or an agent said it is
    /// blocked) is held first, so a user who is already attending to that session sends none: switching to it,
    /// typing into it, or interacting with it while it is open in an active window withdraws it (SPEC.md, "Ntfy
    /// alerts"). `0` sends as soon as the alert's own rules allow. Configurations saved before the setting existed
    /// read as [`DEFAULT_GRACE_PERIOD_SECS`]; at most [`MAX_GRACE_PERIOD_SECS`].
    #[serde(default = "grace_period_default")]
    pub grace_period_secs: u32,
    /// A daily range of local hours during which alerts go out at another priority, or not at all. Configurations
    /// saved before it existed read as [`QuietHours::default`], which is off.
    #[serde(default)]
    pub quiet_hours: QuietHours,
}

/// Quiet hours for ntfy alerts: from `start_hour` up to, not including, `end_hour`, in the local time of the machine
/// the helm runs on, every alert the helm sends goes out at `priority` instead of the saved one, or is not sent at all
/// when `priority` is `None` (shown as Off).
///
/// The range is half-open and may wrap past midnight: 22 to 7 covers 22:00 through 06:59, and 9 to 17 covers 09:00
/// through 16:59. Hours are whole hours (0 to 23), and the start and end must differ, which the helm checks on every
/// save; a range that breaks either rule is never active ([`Self::contains_hour`]), so a malformed stored value can
/// silence nothing.
///
/// Send test does not follow these rules: it always sends, so a user can check the connection during quiet hours that
/// silence alerts ([`Settings::test_outcome`]).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct QuietHours {
    pub enabled: bool,
    pub start_hour: u8,
    pub end_hour: u8,
    /// The priority alerts go out at during quiet hours; `None` (JSON `null`) sends none. An absent field reads as
    /// [`Priority::Low`], the default, so that only an explicit `null` silences alerts.
    #[serde(default = "quiet_priority_default")]
    pub priority: Option<Priority>,
}

impl Default for QuietHours {
    /// Off, with the range and priority the form starts from when the user turns it on: 22:00 to 07:00 at Low.
    fn default() -> Self {
        Self {
            enabled: false,
            start_hour: 22,
            end_hour: 7,
            priority: quiet_priority_default(),
        }
    }
}

/// What an absent quiet-hours priority means: Low.
fn quiet_priority_default() -> Option<Priority> {
    Some(Priority::Low)
}

impl QuietHours {
    /// Whether quiet hours are on and `hour` (0 to 23, local time) falls in their range. A range whose hours are out of
    /// bounds or equal is never active.
    pub fn contains_hour(&self, hour: u8) -> bool {
        let (start, end) = (self.start_hour, self.end_hour);
        if !self.enabled || start > 23 || end > 23 || start == end {
            return false;
        }
        if start < end {
            hour >= start && hour < end
        } else {
            hour >= start || hour < end
        }
    }
}

/// What a Send test did, so the form can say what priority the test arrived at and why (`POST /api/ntfy-alerts/test`
/// answers with it).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TestOutcome {
    /// The priority the test was sent at.
    pub priority: Priority,
    pub quiet_hours: QuietHoursNow,
}

/// Whether quiet hours were in force when a test was sent, and what that meant for it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuietHoursNow {
    /// Outside quiet hours, or quiet hours are off: the test went out at the saved priority.
    Inactive,
    /// Inside quiet hours: the test went out at their priority, as alerts do now.
    Active,
    /// Inside quiet hours that send no alerts: the test still went out, at the saved priority, while real alerts are
    /// silenced.
    Silenced,
}

/// The grace period a configuration has when it never chose one, which the maintainer settled at 10 seconds.
pub const DEFAULT_GRACE_PERIOD_SECS: u32 = 10;

/// The longest grace period a save may store: ten minutes, the bound the plan `blocked-notify-attention-and-debounce`
/// suggested. The Settings card offers up to two minutes and shows any longer stored value as an extra choice.
pub const MAX_GRACE_PERIOD_SECS: u32 = 600;

/// What an absent `grace_period_secs` means: [`DEFAULT_GRACE_PERIOD_SECS`].
fn grace_period_default() -> u32 {
    DEFAULT_GRACE_PERIOD_SECS
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
            grace_period_secs: DEFAULT_GRACE_PERIOD_SECS,
            quiet_hours: QuietHours::default(),
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

    /// The priority an alert sent at `local_hour` (0 to 23) goes out at: the quiet-hours priority inside quiet hours,
    /// else the saved one. `None` means quiet hours send no alerts right now. Says nothing about whether an alert is
    /// sent at all otherwise; the switches above decide that.
    pub fn effective_priority(&self, local_hour: u8) -> Option<Priority> {
        if self.quiet_hours.contains_hour(local_hour) {
            self.quiet_hours.priority
        } else {
            Some(self.priority)
        }
    }

    /// What a Send test at `local_hour` does: the priority alerts go out at now, except that quiet hours sending no
    /// alerts do not stop the test, which then goes at the saved priority so it can still prove the connection
    /// (SPEC.md, "Settings → Notifications").
    pub fn test_outcome(&self, local_hour: u8) -> TestOutcome {
        let (priority, quiet_hours) = if !self.quiet_hours.contains_hour(local_hour) {
            (self.priority, QuietHoursNow::Inactive)
        } else {
            match self.quiet_hours.priority {
                Some(priority) => (priority, QuietHoursNow::Active),
                None => (self.priority, QuietHoursNow::Silenced),
            }
        };
        TestOutcome {
            priority,
            quiet_hours,
        }
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
    /// See [`Settings::grace_period_secs`]. Absent keeps the stored value, as for `notify_waiting`; the helm refuses
    /// one above [`MAX_GRACE_PERIOD_SECS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_period_secs: Option<u32>,
    /// See [`Settings::quiet_hours`]. Absent keeps the stored value, as for `notify_waiting`; the helm refuses hours
    /// above 23 or a start equal to the end, whether or not quiet hours are on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_hours: Option<QuietHours>,
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

    /// Every configuration saved before the grace period existed must get the default, not an immediate send, and an
    /// older client's save must not reset a chosen one. Specified: stored settings without the field read 10 seconds,
    /// as the defaults do; a save body without it decodes as none; a chosen value round-trips.
    #[test]
    fn a_missing_grace_period_reads_as_the_default() {
        let stored: Settings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token_set":false}"#,
        )
        .unwrap();
        assert_eq!(stored.grace_period_secs, DEFAULT_GRACE_PERIOD_SECS);
        assert_eq!(Settings::default().grace_period_secs, 10);
        let save: SaveSettings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token":null}"#,
        )
        .unwrap();
        assert_eq!(save.grace_period_secs, None);
        let chosen = Settings {
            grace_period_secs: 0,
            ..Settings::default()
        };
        let back: Settings =
            serde_json::from_value(serde_json::to_value(&chosen).unwrap()).unwrap();
        assert_eq!(back.grace_period_secs, 0);
    }

    /// Quiet hours decide whether an alert at a given hour is quiet, and an off-by-one at either end, or a wrap past
    /// midnight read the wrong way round, would quiet the wrong hours. Specified: the range is half-open, so 22 to 7
    /// covers 22:00 through 06:59 across midnight and 9 to 17 covers 09:00 through 16:59; quiet hours that are off,
    /// whose start equals their end, or whose hours exceed 23 contain no hour.
    #[test]
    fn quiet_hours_cover_their_half_open_range_and_wrap_past_midnight() {
        let covered = |start_hour, end_hour| -> Vec<u8> {
            let quiet = QuietHours {
                enabled: true,
                start_hour,
                end_hour,
                ..QuietHours::default()
            };
            (0..24).filter(|hour| quiet.contains_hour(*hour)).collect()
        };
        assert_eq!(covered(22, 7), [0, 1, 2, 3, 4, 5, 6, 22, 23]);
        assert_eq!(covered(9, 17), [9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(covered(23, 0), [23]);
        assert_eq!(covered(0, 23), (0..23).collect::<Vec<_>>());
        assert!(covered(8, 8).is_empty());
        assert!(covered(24, 7).is_empty());
        assert!(covered(22, 30).is_empty());
        let off = QuietHours {
            enabled: false,
            ..QuietHours::default()
        };
        assert!((0..24).all(|hour| !off.contains_hour(hour)));
    }

    /// The priority an alert goes out at, and what Send test reports, follow from the hour and the quiet-hours
    /// setting, and the test must still go out when quiet hours silence alerts, so a user can check the connection
    /// then. Specified: outside quiet hours the saved priority, reported inactive; inside, the quiet-hours priority,
    /// reported active; inside quiet hours set to Off, no alert priority, and a test at the saved priority reported
    /// silenced.
    #[test]
    fn the_effective_priority_and_the_test_outcome_follow_quiet_hours() {
        let settings = |quiet: Option<Priority>| Settings {
            priority: Priority::High,
            quiet_hours: QuietHours {
                enabled: true,
                start_hour: 22,
                end_hour: 7,
                priority: quiet,
            },
            ..Settings::default()
        };
        let lowered = settings(Some(Priority::Min));
        assert_eq!(lowered.effective_priority(12), Some(Priority::High));
        assert_eq!(lowered.effective_priority(23), Some(Priority::Min));
        assert_eq!(
            lowered.test_outcome(12),
            TestOutcome {
                priority: Priority::High,
                quiet_hours: QuietHoursNow::Inactive,
            }
        );
        assert_eq!(
            lowered.test_outcome(3),
            TestOutcome {
                priority: Priority::Min,
                quiet_hours: QuietHoursNow::Active,
            }
        );
        let silenced = settings(None);
        assert_eq!(silenced.effective_priority(12), Some(Priority::High));
        assert_eq!(silenced.effective_priority(6), None);
        assert_eq!(
            silenced.test_outcome(6),
            TestOutcome {
                priority: Priority::High,
                quiet_hours: QuietHoursNow::Silenced,
            }
        );
    }

    /// Every configuration saved before quiet hours existed must keep its alerts as they were, an older client's save
    /// must not reset them, and Off must survive storage as something other than the default. Specified: stored
    /// settings without the field read as quiet hours off from 22:00 to 07:00 at Low; a save without it decodes as
    /// none; quiet hours without a priority field read Low, while an explicit null is Off and round-trips as Off; the
    /// test outcome's wire names are snake case.
    #[test]
    fn missing_quiet_hours_read_as_off_and_off_round_trips() {
        let stored: Settings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token_set":false}"#,
        )
        .unwrap();
        assert_eq!(stored.quiet_hours, QuietHours::default());
        assert_eq!(
            QuietHours::default(),
            QuietHours {
                enabled: false,
                start_hour: 22,
                end_hour: 7,
                priority: Some(Priority::Low),
            }
        );
        let save: SaveSettings = serde_json::from_str(
            r#"{"enabled":true,"server_url":"https://ntfy.sh","topic":"t","priority":"default","token":null}"#,
        )
        .unwrap();
        assert_eq!(save.quiet_hours, None);
        let absent: QuietHours =
            serde_json::from_str(r#"{"enabled":true,"start_hour":1,"end_hour":2}"#).unwrap();
        assert_eq!(absent.priority, Some(Priority::Low));
        let off: QuietHours =
            serde_json::from_str(r#"{"enabled":true,"start_hour":1,"end_hour":2,"priority":null}"#)
                .unwrap();
        assert_eq!(off.priority, None);
        let back: QuietHours = serde_json::from_value(serde_json::to_value(off).unwrap()).unwrap();
        assert_eq!(back, off);
        assert_eq!(
            serde_json::to_value(TestOutcome {
                priority: Priority::Low,
                quiet_hours: QuietHoursNow::Silenced,
            })
            .unwrap(),
            serde_json::json!({"priority": "low", "quiet_hours": "silenced"})
        );
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

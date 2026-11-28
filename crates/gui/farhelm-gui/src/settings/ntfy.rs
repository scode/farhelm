//! Settings → Notifications, its two helm-configured cards: **Blocked agents** (whether the agents Farhelm starts are
//! asked to run `farhelm agent notify` when they are blocked, and what counts as blocked) and **ntfy alerts** (whether
//! the helm sends an alert when a session waits for the user or says it is blocked, how long it holds an alert back in
//! case the user attends to the session first, the ntfy server, topic, access token and priority it sends with, and
//! quiet hours, a daily range of hours with another priority or none).
//!
//! Both cards are one form over one helm configuration: the instruction choice is stored with the ntfy settings
//! (`farhelm_proto::ntfy::Settings::ask_agents_when_blocked`), so it shares their read, their one Save and their
//! routes, and needs no second form or stored row. It is drawn as a card of its own because it works with or without
//! ntfy: a blocked agent's session shows as waiting in the session list whether or not any alert goes out.
//!
//! SPEC.md, "Session list" (the Notifications sentences) and "Status" (Ntfy alerts); SPEC_impl.md, "Ntfy waiting
//! alerts". Ported from the old app's `NtfySettings` (`ntfy_settings.rs`) with its words and its look from the
//! `.ntfy-settings` and `.checkout-root-*` rules in `app.css`, at reference commit `24bc0580`. Here's the TLDR of its
//! rules:
//!
//! - **Helm configuration, read once.** The settings live behind `/api/ntfy-alerts`, outside the preference row and
//!   not pushed, so the form reads them when it is made (the first time the section shows in an opening of the dialog,
//!   the pattern Agent trust follows), and a reply reaches only that form. Until the read answers, the form shows the
//!   helm's defaults, as the old one did, with every control unavailable and the fields read-only, so a slow read
//!   cannot turn into a save of the defaults; a failed read shows the helm's reason with "Retry loading".
//! - **One explicit Save of the whole form.** The route takes the whole configuration in one `PUT`, and its answer is
//!   what the helm stored (the server address normalized, whether a token is saved), which the form then shows, as the
//!   old one did. So the form is one [`Draft`] saved with [`drafts::save`], not a [`crate::drafts::DraftInput`] per
//!   field: `DraftInput::save` sends and settles only its own field, so one whole-form save cannot be built from three
//!   of them. That is a deliberate exception to the rule of one `DraftInput` per explicitly saved field, decided by the
//!   migration's settings-and-hosts area on 2026-11-14, until a second whole-form save makes it worth lifting into
//!   `drafts.rs`. The text fields are bound to the draft here instead: an edit goes into the draft (refused, and
//!   the field put back, while the save is in flight), and the fields are read-only while it is. The token field is
//!   also read-only while Clear saved token is on, as the old one was.
//! - **Quiet hours are the helm's clock.** The helm decides them when it sends, by the local time of the machine it runs
//!   on, so the form shows no clock of its own and says whose time it is ([`QUIET_NOTE`]). Start and end are hour
//!   selects with all 24 hours, and the form does not refuse a start equal to the end: the helm's Save refusal says so,
//!   which keeps swapping 22:00–07:00 to 07:00–22:00 from passing through a refused state.
//! - **The token is write-only.** The helm never returns it; the form shows whether one is saved (`token_set`) and
//!   offers to replace it (type one) or clear it ("Clear saved token"). A blank token field sends `null`, which the helm
//!   reads as "keep the saved token", but only on the same server: changing to another server clears it unless a new
//!   one is typed, and the form says so before saving ([`server_changed`]).
//! - **Saves are the operation store's work** under [`OpKey::preferences`], the preference row's key, although this is
//!   helm configuration, not a preference: helm configuration has no key of its own, so a Save here is refused while
//!   any other Settings save is in flight, and the other way round ([`super::agent_trust`] has the reasoning, accepted
//!   by this area on 2026-11-14). A Save the key refuses sends nothing and says so under the form ([`Refused`]) until
//!   the next save starts. A done save shows "ntfy alert settings saved" until the next edit and fills the form from
//!   the helm's answer, emptying the token field; a refusal keeps the form as typed with the helm's words under it,
//!   until the next save starts.
//! - **Send test is not a save.** It asks the helm to send one alert with the settings it stores, even while alerts are
//!   off, and sends nothing of the form (SPEC.md: "Send test uses the saved configuration"). It changes nothing, so it
//!   is no operation on a key: it goes out with [`crate::operations::send`] and its answer shows in place, "Test alert
//!   sent" with the priority it went at and whether quiet hours decided it ([`tested_line`]), or the helm's reason,
//!   until the next edit, save or test. A refused save's words stay above it until the next
//!   save, as a draft's refusal does, where the old form showed one outcome at a time. While it is out the form is
//!   locked as for a save, as the old one was, so a Save cannot overtake it. It moves no focus and opens nothing.
//!
//! The Sounds card above this form has four switches. Each writes its own preference immediately.

use farhelm_helm_client::{ApiPath, Method, PathError};
use farhelm_proto::ntfy::{
    DEFAULT_BLOCKED_CRITERIA, Priority, QuietHours, QuietHoursNow, SaveSettings, Settings,
    TestOutcome,
};
use farhelm_proto::{LaunchHarness, WaitingAlertSupport};
use gpui::{
    AnyWindowHandle, App, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px,
};

use super::choices;
use super::{Refused, card};
use crate::components::{
    Button, Checkbox, HoverHelp, RadioGroup, RadioOption, Select, SelectOption, TextInput,
    TextInputEvent, TextInputState, Tier,
};
use crate::drafts::{self, Draft};
use crate::host_text::HostText;
use crate::operations::{Command, OpKey, Outcome, Work};
use crate::theme;

/// The fields share two columns. Their help and outcomes can grow below them as needed.
const FIELD_GAP: f32 = 10.;
/// The form's name for [`Refused`]: its Save is the one control here whose press the key can refuse, and the line saying
/// so is drawn under the id [`Refused::line`] makes of it, `ntfy-busy`.
const REFUSED: &str = "ntfy";
/// The route the form reads and saves, as a refusal without words names it.
const ROUTE: &str = "/api/ntfy-alerts";
/// The priority segments' hover help, the old buttons' `data-tooltip`, which the group repeats as its own.
pub(super) const PRIORITY_HELP: &str =
    "Priority used for session alerts; Save applies the selected priority";
pub(super) const PRIORITY_LABEL: &str = "Priority";
pub(super) const INTRO: &str = "Send a session's title and host when it waits for you. In the desktop app, keep the window open.";
pub(super) const TOPIC_HELP: &str =
    "Anyone who knows an ntfy.sh topic can read its alerts. Choose a hard-to-guess topic.";
pub(super) const TOKEN_HELP: &str =
    "Optional server access token; leave blank to preserve the saved token";
pub(super) const TOKEN_CLEAR_HELP: &str = "The saved access token will be cleared when you save";
pub(super) const TOKEN_CHANGED_HELP: &str =
    "Optional token for this server; leaving it blank clears the previous server's saved token";
/// The grace period segments' id, shared with Settings search; each segment is drawn under `ntfy-grace-<its label>`.
pub(super) const GRACE_ID: &str = "ntfy-grace";
/// The grace period segments' caption.
pub(super) const GRACE_LABEL: &str = "Grace period";
/// The grace period segments' hover help, which every segment repeats, as the priority segments do.
pub(super) const GRACE_HELP: &str = "How long an alert waits before it is sent; Save applies it";
/// What the grace period does, under its segments: the per-session awareness rule (SPEC.md, "Ntfy alerts") in the
/// user's terms. Window focus is deliberately not in it, since focus alone withdraws nothing.
pub(super) const GRACE_NOTE: &str = "An alert waits this long before it is sent. It is not sent if you switch to the \
     session, type, click or scroll in it, or it stops waiting first. Off sends alerts at once.";
/// The grace periods offered, in seconds, shortest first: Off, then a few steps up to two minutes. The helm takes up
/// to ten minutes (`farhelm_proto::ntfy::MAX_GRACE_PERIOD_SECS`); a stored value not in this list is offered as an
/// extra segment ([`grace_choices`]), so the form never shows nothing chosen.
pub(super) const GRACE_CHOICES: [u32; 6] = [0, 5, 10, 30, 60, 120];

/// The segment label for a grace period of `secs`: "Off" for none, whole minutes as minutes, anything else in seconds.
pub(super) fn grace_label(secs: u32) -> String {
    match secs {
        0 => "Off".to_owned(),
        secs if secs % 60 == 0 => format!("{} min", secs / 60),
        secs => format!("{secs} s"),
    }
}

/// The grace periods the segments offer for a form holding `current`: [`GRACE_CHOICES`], with `current` added in order
/// when it is not one of them (a value saved through the helm's route by another client).
fn grace_choices(current: u32) -> Vec<u32> {
    let mut choices = GRACE_CHOICES.to_vec();
    if !choices.contains(&current) {
        choices.push(current);
        choices.sort_unstable();
    }
    choices
}

/// The quiet hours note, under their switch: whose clock they follow, how the range reads, and that Send test still
/// proves the connection when they send nothing (SPEC.md, "Settings → Notifications").
pub(super) const QUIET_NOTE: &str = "Uses the clock of the machine this helm runs on. From the start hour up to the \
     end hour, which can run past midnight, alerts go out at the priority below, or not at all with Off. Send test \
     still sends.";
/// The quiet hours selects' and segments' ids, shared with Settings search.
pub(super) const QUIET_START_ID: &str = "ntfy-quiet-start";
pub(super) const QUIET_END_ID: &str = "ntfy-quiet-end";
pub(super) const QUIET_PRIORITY_ID: &str = "ntfy-quiet-priority";
/// The quiet hours start select's accessible name; the form captions it "Start".
pub(super) const QUIET_START_LABEL: &str = "Quiet hours start";
/// The quiet hours end select's accessible name; the form captions it "End".
pub(super) const QUIET_END_LABEL: &str = "Quiet hours end";
/// The hour selects' hover help.
pub(super) const QUIET_HOURS_HELP: &str =
    "An hour on the clock of the machine this helm runs on; Save applies it";
/// The quiet hours priority segments' caption and accessible name.
pub(super) const QUIET_PRIORITY_LABEL: &str = "Quiet hours priority";
/// The quiet hours priority segments' hover help.
pub(super) const QUIET_PRIORITY_HELP: &str =
    "Priority for alerts during quiet hours, or Off to send none; Save applies it";

/// An hour as the selects show it: `07:00`.
pub(super) fn hour_label(hour: u8) -> String {
    format!("{hour:02}:00")
}

/// The quiet hours priorities offered, in segment order: Off (no alerts), then the five ntfy priorities.
pub(super) fn quiet_priorities() -> impl Iterator<Item = Option<Priority>> {
    std::iter::once(None).chain(PRIORITIES.into_iter().map(Some))
}

/// A quiet hours priority's segment label: "Off" for none, else the priority's own.
pub(super) fn quiet_priority_label(priority: Option<Priority>) -> &'static str {
    priority.map_or("Off", Priority::display_label)
}

/// The line a sent test shows: the priority it went at and, inside quiet hours, that they decided it. A helm that
/// answers without an outcome (one older than quiet hours) gets the plain line.
pub(super) fn tested_line(outcome: Option<TestOutcome>) -> String {
    let Some(TestOutcome {
        priority,
        quiet_hours,
    }) = outcome
    else {
        return "Test alert sent".to_owned();
    };
    let priority = priority.display_label();
    match quiet_hours {
        QuietHoursNow::Inactive => format!("Test alert sent ({priority})"),
        QuietHoursNow::Active => format!("Test alert sent ({priority} \u{2014} quiet hours)"),
        QuietHoursNow::Silenced => format!(
            "Test alert sent ({priority} \u{2014} quiet hours active and set to Off; real alerts are silenced)"
        ),
    }
}

/// Every priority, in the order the old segments showed them.
pub(super) const PRIORITIES: [Priority; 5] = [
    Priority::Min,
    Priority::Low,
    Priority::Default,
    Priority::High,
    Priority::Urgent,
];

/// What the form holds: the stored settings as edited, the token typed (empty: none typed) and whether Clear saved
/// token is on.
#[derive(Clone, PartialEq)]
struct Form {
    enabled: bool,
    notify_waiting: bool,
    notify_blocked: bool,
    /// Whether agents are asked to notify when blocked, as the form shows it: the stored choice, or, for a
    /// configuration saved before the choice existed, what that configuration did
    /// ([`Settings::asks_agents_when_blocked`]).
    ask_agents: bool,
    /// What counts as blocked, as typed; empty means Farhelm's default ([`DEFAULT_BLOCKED_CRITERIA`]).
    criteria: String,
    server_url: String,
    topic: String,
    priority: Priority,
    /// How long the helm holds an alert back, in seconds ([`Settings::grace_period_secs`]).
    grace_period_secs: u32,
    /// Quiet hours as edited ([`Settings::quiet_hours`]), sent whole with every save.
    quiet_hours: QuietHours,
    token: String,
    clear: bool,
}

impl Form {
    /// The form over `stored`, with no token typed and nothing to clear.
    fn over(stored: &Settings) -> Form {
        Form {
            enabled: stored.enabled,
            notify_waiting: stored.notify_waiting,
            notify_blocked: stored.notify_blocked,
            ask_agents: stored.asks_agents_when_blocked(),
            criteria: stored.custom_blocked_criteria.clone().unwrap_or_default(),
            server_url: stored.server_url.clone(),
            topic: stored.topic.clone(),
            priority: stored.priority,
            grace_period_secs: stored.grace_period_secs,
            quiet_hours: stored.quiet_hours,
            token: String::new(),
            clear: false,
        }
    }

    /// The save the helm takes: an explicit clear sends `""`, a typed token replaces the saved one, and a blank field
    /// sends none, which the helm applies its same-server rule to (the module docs).
    fn save(&self) -> SaveSettings {
        let token = if self.clear {
            Some(String::new())
        } else {
            Some(self.token.clone()).filter(|token| !token.is_empty())
        };
        SaveSettings {
            enabled: self.enabled,
            server_url: self.server_url.clone(),
            topic: self.topic.clone(),
            priority: self.priority,
            token,
            notify_waiting: Some(self.notify_waiting),
            notify_blocked: Some(self.notify_blocked),
            // Always sent, so emptying the field is how Save goes back to the default: the helm reads text that
            // normalizes to nothing as "use Farhelm's".
            custom_blocked_criteria: Some(self.criteria.clone()),
            // Always sent too, so the first save from this form turns an upgraded configuration's implied choice
            // into a stored one, and later changes to the ntfy switches no longer move it.
            ask_agents_when_blocked: Some(self.ask_agents),
            grace_period_secs: Some(self.grace_period_secs),
            quiet_hours: Some(self.quiet_hours),
        }
    }

    /// `PUT /api/ntfy-alerts` with [`Form::save`]. Turning it into JSON cannot fail (plain strings, a bool and an
    /// enum); if it ever did, the request would go without a body and the helm would refuse it with its own words.
    fn command(&self) -> Result<Command, PathError> {
        Ok(Command::new(
            Work::Save,
            Method::Put,
            path(&[])?,
            serde_json::to_value(self.save()).ok(),
            format!("PUT {ROUTE}"),
        ))
    }
}

/// The route the form reads and saves ([`ROUTE`]), or with `tail` a route under it (Send test's `test`). `ApiPath`
/// builds a path from all its segments at once, so the route's own segment is spelled here only.
fn path(tail: &[&str]) -> Result<ApiPath, PathError> {
    let segments: Vec<&str> = std::iter::once("ntfy-alerts")
        .chain(tail.iter().copied())
        .collect();
    ApiPath::api(&segments)
}

/// Whether `draft` names another server than `saved`, the way the helm decides whether a saved token goes with it:
/// scheme, host and effective port, default ports normalized. An address that does not parse yet (half typed) names no
/// server, so it promises nothing about the token; the helm decides for real when the form is saved.
fn server_changed(saved: &str, draft: &str) -> bool {
    let origin = |value: &str| {
        url::Url::parse(value)
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
            .map(|url| url.origin())
    };
    match (origin(saved), origin(draft)) {
        (Some(saved), Some(draft)) => saved != draft,
        _ => false,
    }
}

/// What the waiting switch covers, worded from each built-in agent's [`WaitingAlertSupport`]: an alert follows a
/// session's Waiting status, which only an agent's own screen reader produces, so the wording comes from the same answer
/// the supervisor's readers are pinned to rather than from a list kept here (the plan `agent-blocked-notifications`
/// asked for exactly that, so a new agent cannot be left out or wrongly promised alerts).
pub(super) fn waiting_agents() -> String {
    let named = |support: WaitingAlertSupport| -> Vec<&'static str> {
        LaunchHarness::BUILT_INS
            .iter()
            .filter(|harness| harness.agent_kind().waiting_alert_support() == support)
            .map(|harness| harness.display_name())
            .collect()
    };
    let mut words = Vec::new();
    let questions = named(WaitingAlertSupport::QuestionsAndApprovals);
    if !questions.is_empty() {
        words.push(format!(
            "Covers questions and approvals in {}.",
            and_list(&questions)
        ));
    }
    let trust = named(WaitingAlertSupport::WorkspaceTrustOnly);
    if !trust.is_empty() {
        words.push(format!(
            "In {}, only the question about trusting a folder.",
            and_list(&trust)
        ));
    }
    words.push("Other agents never show as waiting.".to_owned());
    words.join(" ")
}

/// `names` as English joins them: "A", "A and B", "A, B and C".
fn and_list(names: &[&str]) -> String {
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// What the ntfy blocked switch covers, shown under it: the alert an agent asks for with `farhelm agent notify`,
/// while the session list shows that session as waiting whatever the switch says. It points at the Blocked agents
/// card, since this switch alone no longer asks Farhelm's agents to run the command.
pub(super) const BLOCKED_HELP: &str = "Sent when an agent runs farhelm agent notify. Ask agents to notify when \
     blocked, above, decides whether Farhelm asks its agents to. The session list shows that session as waiting \
     whether or not this is on.";

/// The Blocked agents card's note, under its heading: what a blocked agent's report does in the app, which is the same
/// with or without ntfy.
pub(super) const BLOCKED_INTRO: &str = "When an agent says it is blocked, its session shows as waiting until you \
     switch to it or type into it.";

/// The criteria field's help: what it is for and what leaving it empty does.
pub(super) const CRITERIA_HELP: &str = "Your own description of when an agent should tell you it is blocked; leave it \
     empty to use Farhelm's, shown below";

/// What "Ask agents to notify when blocked" asks of the agents Farhelm starts, worded from each built-in agent's
/// `AgentKind::supports_instruction_injection` (pinned by the supervisor against what each launch actually carries), so
/// the list cannot drift from the agents that really get the instruction.
pub(super) fn blocked_agents() -> String {
    let named: Vec<&'static str> = LaunchHarness::BUILT_INS
        .iter()
        .filter(|harness| harness.agent_kind().supports_instruction_injection())
        .map(|harness| harness.display_name())
        .collect();
    format!(
        "Farhelm asks {} to run farhelm agent notify when they are blocked, in sessions started or restarted after \
         you save. Other agents only do so if you tell them to.",
        and_list(&named)
    )
}

/// The line beside Send test when the saved settings would make a test mislead: one that arrives proves the server and
/// topic, not that alerts are on. A maintainer's test passed while no real alert ever came (the plan
/// `agent-blocked-notifications`), so the form now says what the saved settings do.
pub(super) const UNSAVED_LINE: &str =
    "Unsaved changes: alerts and Send test use the saved settings until you Save.";
/// [`UNSAVED_LINE`]'s counterpart for saved settings with the master switch off.
pub(super) const OFF_LINE: &str = "ntfy alerts are off, so none are sent. Send test still sends one: a test arriving does not mean alerts are on.";
/// [`UNSAVED_LINE`]'s counterpart for saved settings with both event switches off, the waiting one and the blocked one:
/// then no session sends an alert, whatever the master switch says.
pub(super) const EVENTS_OFF_LINE: &str = "Waiting and blocked alerts are both off. Send test still sends one, but no \
     session sends an alert.";

/// The form's text fields, each bound to its part of the [`Form`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Field {
    Server,
    Topic,
    Token,
    /// What counts as blocked, drawn in the Blocked agents card under its switch rather than with the connection
    /// fields.
    Criteria,
}

impl Field {
    /// In drawing order: the criteria field, in the Blocked agents card above the ntfy alerts card, then the connection
    /// fields. Settings search lists the criteria with its card's switch.
    pub(super) const ALL: [Field; 4] = [Field::Criteria, Field::Server, Field::Topic, Field::Token];

    /// The text input drawn for this field.
    pub(super) fn input(self, alerts: &NtfyAlerts) -> &TextInputState {
        match self {
            Field::Server => &alerts.server,
            Field::Topic => &alerts.topic,
            Field::Token => &alerts.token,
            Field::Criteria => &alerts.criteria,
        }
    }

    /// The id, label and ordinary help shared by the form and Settings search.
    pub(super) fn words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Server => (
                "ntfy-server",
                "Server URL",
                "ntfy server that receives session alerts",
            ),
            Self::Topic => (
                "ntfy-topic",
                "Topic",
                "Topic to publish alerts to; subscribe to the same topic in your ntfy client",
            ),
            Self::Token => ("ntfy-token", "Access token (optional)", TOKEN_HELP),
            Self::Criteria => (
                "ntfy-blocked-criteria",
                "What counts as blocked (optional)",
                CRITERIA_HELP,
            ),
        }
    }

    /// The part of `form` this field edits. Mutable, so one match serves both typing into the form and reading it
    /// ([`NtfyAlerts::fill`] reads through a copy).
    fn value(self, form: &mut Form) -> &mut String {
        match self {
            Field::Server => &mut form.server_url,
            Field::Topic => &mut form.topic,
            Field::Token => &mut form.token,
            Field::Criteria => &mut form.criteria,
        }
    }
}

/// The ntfy switches need handles outside their paint pass when a search result opens the form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Switch {
    /// The master switch: nothing is sent while it is off.
    Enabled,
    /// Alerts when an agent waits for an answer, under the master switch.
    Waiting,
    /// Alerts when an agent says it is blocked (`farhelm agent notify`), under the master switch.
    Blocked,
    ClearToken,
    /// Whether the agents Farhelm starts are asked to notify when blocked, in the Blocked agents card: independent of
    /// the master switch.
    AskAgents,
    /// Quiet hours, above their hours and priority.
    QuietHours,
}

impl Switch {
    /// The form and the catalog use the same id, label and help for a switch.
    pub(super) fn words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Enabled => (
                "ntfy-enabled",
                "Enable ntfy alerts",
                "Send ntfy alerts from this helm, for the events switched on below; Save applies it",
            ),
            Self::Waiting => (
                "ntfy-waiting",
                "Agent is waiting for an answer",
                "Alert when a session waits for your answer; Save applies it",
            ),
            Self::Blocked => (
                "ntfy-blocked",
                "Agent is blocked",
                "Alert when an agent says it cannot finish what you asked; Save applies it",
            ),
            Self::ClearToken => (
                "ntfy-clear-token",
                "Clear saved token",
                "Remove the saved access token when you Save",
            ),
            Self::AskAgents => (
                "blocked-ask-agents",
                "Ask agents to notify when blocked",
                "Ask the agents Farhelm starts to run farhelm agent notify instead of stopping silently; Save \
                 applies it",
            ),
            Self::QuietHours => (
                "ntfy-quiet-hours",
                "Quiet hours",
                "During a daily range of hours, send alerts at another priority or not at all; Save applies it",
            ),
        }
    }
}

/// The ntfy alerts form, a view of its own that Settings makes the first time Notifications shows in an opening and
/// keeps until the dialog closes (see the module docs).
pub(super) struct NtfyAlerts {
    pub(super) save_receipt: super::editing::SaveReceipt,
    enabled_focus: gpui::FocusHandle,
    waiting_focus: gpui::FocusHandle,
    blocked_focus: gpui::FocusHandle,
    clear_focus: gpui::FocusHandle,
    ask_focus: gpui::FocusHandle,
    quiet_focus: gpui::FocusHandle,
    /// What the helm last said it stores: the read's answer, then each done save's. `None` until the read answers.
    stored: Option<Settings>,
    /// Why the last read failed, until a read answers.
    unreadable: Option<HostText>,
    form: Draft<Form>,
    /// Save and Close may be waiting on another form while this draft is still editable.
    edit_revision: u64,
    server: TextInputState,
    topic: TextInputState,
    token: TextInputState,
    /// What counts as blocked: a growing box, since criteria can run to a few sentences.
    criteria: TextInputState,
    /// Whether the last save was done; its line shows until the next edit touches the form, or a test starts.
    saved: bool,
    /// Whether Send test is out; the form is locked meanwhile.
    testing: bool,
    /// How the last Send test went, until the next edit, save or test: what the helm says it did (`None` from a helm
    /// that answers with no outcome), or its reason.
    tested: Option<Result<Option<TestOutcome>, HostText>>,
    /// Whether the last Save sent nothing because another Settings save held the key, which the form says until its
    /// next save starts.
    refused: Refused,
    /// The window the form is drawn in, where answers that arrive later set the fields' text.
    window: AnyWindowHandle,
    _edits: Vec<Subscription>,
}

impl NtfyAlerts {
    /// The form, reading the helm's settings now.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server = TextInputState::new(window, cx);
        let topic = TextInputState::new(window, cx);
        let token = TextInputState::new_masked(window, cx);
        let criteria = TextInputState::new_growing(4, window, cx);
        let mut alerts = NtfyAlerts {
            save_receipt: super::editing::SaveReceipt::default(),
            enabled_focus: cx.focus_handle().tab_stop(true),
            waiting_focus: cx.focus_handle().tab_stop(true),
            blocked_focus: cx.focus_handle().tab_stop(true),
            clear_focus: cx.focus_handle().tab_stop(true),
            ask_focus: cx.focus_handle().tab_stop(true),
            quiet_focus: cx.focus_handle().tab_stop(true),
            stored: None,
            unreadable: None,
            form: Draft::load(Form::over(&Settings::default())),
            edit_revision: 0,
            server,
            topic,
            token,
            criteria,
            saved: false,
            testing: false,
            tested: None,
            refused: Refused::default(),
            window: window.window_handle(),
            _edits: Vec::new(),
        };
        alerts._edits = Field::ALL
            .iter()
            .map(|&field| {
                field
                    .input(&alerts)
                    .subscribe(window, cx, move |alerts, event, window, cx| {
                        if let TextInputEvent::Changed = event {
                            alerts.typed(field, window, cx);
                        }
                    })
            })
            .collect();
        // The fields show the loaded defaults, locked, until the read answers, as the old form's did.
        alerts.fill(window, cx);
        alerts.hold(cx);
        alerts.sync(window, cx);
        Self::read(alerts.window, cx);
        alerts
    }

    /// Read the helm's settings, the answer going to this form if it is still open, under [`choices::read_route`]'s
    /// contract (once, on its own, after the calling update has returned).
    fn read(window: AnyWindowHandle, cx: &mut Context<Self>) {
        let alerts = cx.weak_entity();
        let answered = move |answer: Result<Settings, HostText>, cx: &mut App| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = alerts.update(cx, |alerts, cx| alerts.answered(answer, window, cx));
            });
        };
        choices::read_route(path(&[]), format!("GET {ROUTE}"), cx, answered);
    }

    /// A read, or a done save, told what the helm stores: an untouched form takes it (emptying the token field), and
    /// the controls become available; or the read's reason shows.
    fn answered(
        &mut self,
        answer: Result<Settings, HostText>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(stored) => {
                self.unreadable = None;
                if self.form.follow(Form::over(&stored)) {
                    self.fill(window, cx);
                }
                self.stored = Some(stored);
                self.hold(cx);
            }
            Err(reason) => self.unreadable = Some(reason),
        }
        self.sync(window, cx);
        cx.notify();
    }

    /// Set the text fields to the draft's values. Setting a field raises no change event, so this is not an edit.
    fn fill(&self, window: &mut Window, cx: &mut App) {
        let mut form = self.form.value().clone();
        for field in Field::ALL {
            field
                .input(self)
                .set_text(field.value(&mut form).clone(), window, cx);
        }
    }

    /// The topic field, for tests that must reach past its read-only hold to show what the form does with an edit
    /// that arrives anyway.
    #[cfg(test)]
    pub(super) fn topic_input(&self) -> TextInputState {
        self.topic.clone()
    }

    /// Whether the controls take no input: before the read has answered, and while a save or Send test is out.
    fn locked(&self) -> bool {
        self.stored.is_none() || self.form.is_saving() || self.testing
    }

    /// The switch can take focus only while its form is loaded and unlocked.
    pub(super) fn search_focus(&self, switch: Switch) -> Option<gpui::FocusHandle> {
        (!self.locked()).then(|| self.switch_handle(switch).clone())
    }

    fn switch_handle(&self, switch: Switch) -> &gpui::FocusHandle {
        match switch {
            Switch::Enabled => &self.enabled_focus,
            Switch::Waiting => &self.waiting_focus,
            Switch::Blocked => &self.blocked_focus,
            Switch::ClearToken => &self.clear_focus,
            Switch::AskAgents => &self.ask_focus,
            Switch::QuietHours => &self.quiet_focus,
        }
    }

    /// Hold the text fields read-only while the form is locked, and the token field also while Clear saved token is
    /// on; free them otherwise. Called whenever either changes.
    fn hold(&self, cx: &mut App) {
        for field in Field::ALL {
            let held = match field {
                Field::Server | Field::Topic | Field::Criteria => self.locked(),
                Field::Token => self.locked() || self.form.value().clear,
            };
            field.input(self).set_read_only(held, cx);
        }
    }

    /// Change the form as `change` says, unless it is locked. Returns whether the draft took it.
    fn edit(&mut self, change: impl FnOnce(&mut Form), cx: &mut Context<Self>) -> bool {
        if self.locked() {
            return false;
        }
        let mut form = self.form.value().clone();
        change(&mut form);
        let taken = self.form.edit(form);
        if taken {
            self.edit_revision = self.edit_revision.wrapping_add(1);
            self.tested = None;
        }
        cx.notify();
        taken
    }

    /// The user typed into `field`: the draft takes the text, or, refused, the field shows the draft's again.
    fn typed(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        let text = field.input(self).text(cx).to_string();
        if !self.edit(|form| *field.value(form) = text, cx) {
            let mut form = self.form.value().clone();
            field
                .input(self)
                .set_text(field.value(&mut form).clone(), window, cx);
        }
        self.sync(window, cx);
    }

    /// The token field's placeholder, which says what Save does with a blank field: the old field's words.
    fn sync(&self, window: &mut Window, cx: &mut App) {
        let placeholder = if self.token_will_clear() {
            "Saved token will be cleared"
        } else if self.token_saved() {
            "Token saved; leave blank to keep it"
        } else {
            "No token saved"
        };
        self.token.set_placeholder(placeholder, window, cx);
    }

    fn token_saved(&self) -> bool {
        self.stored.as_ref().is_some_and(|stored| stored.token_set)
    }

    /// Whether the form names another server than the one the helm stores.
    fn server_changed(&self) -> bool {
        self.stored
            .as_ref()
            .is_some_and(|stored| server_changed(&stored.server_url, &self.form.value().server_url))
    }

    /// Whether Save would clear the saved token, as far as the form can tell: asked to, or a blank field on another
    /// server.
    fn token_will_clear(&self) -> bool {
        let form = self.form.value();
        self.token_saved() && (form.clear || (self.server_changed() && form.token.is_empty()))
    }

    /// The line beside Send test, if the saved settings or unsaved edits would make a test mislead (see
    /// [`UNSAVED_LINE`]). Unsaved edits come first: until they are saved, the other lines would describe settings the
    /// form no longer shows. Nothing until the read answers.
    fn saved_state(&self) -> Option<&'static str> {
        let stored = self.stored.as_ref()?;
        if self.has_unsaved_edits() {
            Some(UNSAVED_LINE)
        } else if !stored.enabled {
            Some(OFF_LINE)
        } else if !stored.notify_waiting && !stored.notify_blocked {
            // Either switch alone still lets its alerts go out, so only both off means nothing is sent.
            Some(EVENTS_OFF_LINE)
        } else {
            None
        }
    }

    /// Whether the form differs from the last read, including typed credentials and the explicit clear answer.
    fn has_unsaved_edits(&self) -> bool {
        self.form.is_touched()
            && self
                .stored
                .as_ref()
                .is_some_and(|stored| Form::over(stored) != *self.form.value())
    }

    /// The form's edit state for Settings' footer and close: saving, or [`Self::has_unsaved_edits`].
    pub(super) fn edit_state(&self) -> super::editing::EditState {
        super::editing::EditState::of(self.form.is_saving(), self.has_unsaved_edits())
    }

    /// Include whether this draft would clear the saved token, which also depends on the last loaded settings.
    pub(super) fn edit_revision(&self) -> (u64, bool) {
        (self.edit_revision, self.token_will_clear())
    }

    /// Send test uses the saved configuration and holds Save until its reply.
    pub(super) fn holds_save(&self) -> bool {
        self.testing
    }

    /// Whether the last Send test failed, which a close waiting on it shows instead of closing.
    pub(super) fn test_failed(&self) -> bool {
        matches!(self.tested, Some(Err(_)))
    }

    /// Reveal either an answered refusal or the existing message for a save that met a busy preference key.
    pub(super) fn save_refused(&self) -> bool {
        !self.testing
            && (self.form.refusal().is_some() || self.refused.line(REFUSED, |_, _| ()).is_some())
    }

    /// The fixture supplies the helm's validation refusal for the invalid topic it staged.
    #[cfg(test)]
    pub(super) fn refuse_topic_for_golden(&mut self, cx: &mut Context<Self>) {
        self.form.refuse_for_golden(HostText::from_host(
            "topic must contain at most 64 letters, numbers, underscores or dashes",
        ));
        cx.notify();
    }

    /// Restore the stored alert configuration without issuing a request or changing the saved token.
    pub(super) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(stored) = &self.stored {
            self.form = Draft::load(Form::over(stored));
            self.fill(window, cx);
            self.hold(cx);
            self.sync(window, cx);
            cx.notify();
        }
    }

    /// Save the form and report whether the operation was admitted. A busy preference key is explained under the form.
    pub(super) fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.locked() {
            return false;
        }
        let window = self.window;
        let started = drafts::save(
            self,
            |alerts: &mut NtfyAlerts| &mut alerts.form,
            &OpKey::preferences(),
            Form::command,
            cx,
            move |alerts, settled, cx| {
                alerts.hold(cx);
                let Outcome::Done(body) = &settled.outcome else {
                    alerts.save_receipt.settle(&settled.outcome, cx);
                    return;
                };
                // The answer is what the helm stored, which the form shows as the old one did. The fields need the
                // window, which this completion does not have, so that waits until it has returned. An answer that
                // does not read as the settings (never sent by the helm) is read again instead, with no saved line,
                // so the form goes on to show what the helm stores.
                let Some(Ok(stored)) = body.clone().map(serde_json::from_value::<Settings>) else {
                    Self::read(window, cx);
                    alerts.save_receipt.settle(&settled.outcome, cx);
                    return;
                };
                alerts.saved = true;
                let view = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ =
                            view.update(cx, |alerts, cx| alerts.answered(Ok(stored), window, cx));
                    });
                });
                alerts.save_receipt.settle(&settled.outcome, cx);
            },
        );
        // Another Settings save may hold the key, which the form then says.
        self.refused.record(REFUSED, &started);
        match &started {
            Ok(()) => {
                self.saved = false;
                self.tested = None;
                self.hold(cx);
            }
            Err(not_started) => tracing::debug!(?not_started, "ntfy alert settings not saved"),
        }
        cx.notify();
        started.is_ok()
    }

    /// Send test: ask the helm to send one alert with the settings it stores (the module docs). Does nothing while the
    /// form is locked, or when there is no UI session to send through.
    fn send_test(&mut self, cx: &mut Context<Self>) {
        if self.locked() {
            return;
        }
        let path = match path(&["test"]) {
            Ok(path) => path,
            Err(error) => {
                tracing::debug!(?error, "ntfy test path not built");
                return;
            }
        };
        let shown = format!("POST {}", path.as_str());
        let alerts = cx.weak_entity();
        let sent =
            crate::operations::send(Method::Post, path, None, shown, cx, move |answer, cx| {
                let _ = alerts.update(cx, |alerts, cx| {
                    alerts.testing = false;
                    // A body that is not an outcome (none, from an older helm) still means the test was sent.
                    alerts.tested = Some(answer.map(|body| {
                        body.and_then(|body| serde_json::from_value::<TestOutcome>(body).ok())
                    }));
                    alerts.hold(cx);
                    let failed = alerts.test_failed();
                    alerts.save_receipt.settled_other(failed, cx);
                    cx.notify();
                });
            });
        if sent {
            self.testing = true;
            self.tested = None;
            self.saved = false;
            self.hold(cx);
            cx.notify();
        }
    }

    /// One field with its words above it, as wide as the section (`.checkout-root-label`).
    fn field(
        &self,
        field: Field,
        help_override: Option<&'static str>,
        palette: &theme::Palette,
    ) -> Div {
        let (id, label, help) = field.words();
        let input = TextInput::new(
            id,
            field.input(self),
            label,
            HoverHelp::tooltip(help_override.unwrap_or(help)),
        )
        .height(px(choices::FIELD_HEIGHT));
        let input = match field {
            Field::Server => input.monospace(),
            Field::Topic | Field::Token | Field::Criteria => input,
        };
        let form = self.form.value();
        let edited = self.stored.as_ref().is_some_and(|stored| match field {
            Field::Server => form.server_url != stored.server_url,
            Field::Topic => form.topic != stored.topic,
            Field::Token => !form.token.is_empty(),
            // Compared as the helm will store it, so retyped spacing is not an edit.
            Field::Criteria => farhelm_proto::ntfy::normalize_blocked_criteria(&form.criteria)
                .map_or(true, |normalized| {
                    normalized != stored.custom_blocked_criteria
                }),
        });
        choices::field_with_caption(id, input, card::caption(label, edited, palette))
    }

    /// What counts as blocked, under the blocked switch: the field, then Farhelm's own wording read-only while the
    /// field is empty, so the user sees what agents are told without the box being filled with a copy of it (the
    /// stored choice stays "Farhelm's default", which follows later changes to that wording); or, once they have typed
    /// their own, a Reset to default that empties the field.
    fn criteria_row(&self, palette: &theme::Palette, cx: &Context<Self>) -> Div {
        let row =
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(self.field(Field::Criteria, None, palette));
        if self.form.value().criteria.trim().is_empty() {
            return row.child(
                card::help(
                    "ntfy-default-criteria",
                    format!("Farhelm's default: {DEFAULT_BLOCKED_CRITERIA}"),
                    cx,
                )
                .min_w_0(),
            );
        }
        let weak = cx.weak_entity();
        row.child(
            div().flex().child(
                Button::new(
                    "ntfy-criteria-reset",
                    "Reset to default",
                    HoverHelp::tooltip(
                        "Empty the field so agents get Farhelm's own wording; Save applies it",
                    ),
                    Role::Button,
                )
                .tier(Tier::Neutral)
                .disabled(self.locked())
                .on_click(move |_, window, cx| {
                    let _ = weak.update(cx, |alerts, cx| {
                        if alerts.edit(|form| form.criteria.clear(), cx) {
                            alerts.criteria.set_text(String::new(), window, cx);
                        }
                    });
                }),
            ),
        )
    }

    /// The grace period row, under the event switches: its caption, the segments ([`grace_choices`]) and what the
    /// period does ([`GRACE_NOTE`]). A press chooses into the form and saves nothing, as the priority segments do.
    fn grace_row(&self, palette: &theme::Palette, cx: &Context<Self>) -> Div {
        let form = self.form.value();
        let edited = self
            .stored
            .as_ref()
            .is_some_and(|stored| form.grace_period_secs != stored.grace_period_secs);
        let choose = cx.weak_entity();
        let group = grace_choices(form.grace_period_secs).into_iter().fold(
            RadioGroup::new(
                GRACE_ID,
                GRACE_LABEL,
                HoverHelp::tooltip(GRACE_HELP),
                Role::Group,
            )
            // Sized from its words, not stretched across the card: six short choices in a full-width well read as
            // scattered words.
            .compact_segments(),
            |group, secs| {
                group.option(RadioOption::new(
                    grace_label(secs),
                    HoverHelp::tooltip(GRACE_HELP),
                    secs,
                ))
            },
        );
        div()
            .flex()
            .flex_col()
            .gap(px(choices::LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(card::caption(GRACE_LABEL, edited, palette))
            .child(
                div().flex().min_w_0().child(
                    group
                        .chosen(Some(form.grace_period_secs))
                        .disabled(self.locked())
                        .on_change(move |&chosen, _, cx| {
                            let _ = choose.update(cx, |alerts, cx| {
                                let _ = alerts.edit(|form| form.grace_period_secs = chosen, cx);
                            });
                        }),
                ),
            )
            .child(card::help("ntfy-grace-note", GRACE_NOTE, cx).min_w_0())
    }

    /// The quiet hours row, after the connection settings: the switch with its note, the start and end hours side by
    /// side, and the priority segments across the card's width, since six choices do not fit the half-width column
    /// the saved priority sits in. A choice goes into the form and saves nothing, as every control here does.
    fn quiet_hours_row(&self, palette: &theme::Palette, cx: &Context<Self>) -> Div {
        let quiet = self.form.value().quiet_hours;
        let stored = self.stored.as_ref().map(|stored| stored.quiet_hours);
        let edited = |differs: fn(&QuietHours, &QuietHours) -> bool| {
            stored.is_some_and(|stored| differs(&quiet, &stored))
        };
        // `wrapper` names the box around the select (`choices::field_with_caption`), apart from the select's own id,
        // which the select's control carries for tests and search.
        let hour = |id: &'static str,
                    wrapper: &'static str,
                    label: &'static str,
                    caption: &'static str,
                    chosen: u8,
                    edited: bool,
                    set: fn(&mut QuietHours, u8)| {
            let weak = cx.weak_entity();
            let select = Select::new(
                id,
                label,
                HoverHelp::tooltip(QUIET_HOURS_HELP),
                Role::ComboBox,
            )
            .options((0..24u8).map(|hour| {
                SelectOption::new(
                    hour.to_string(),
                    hour_label(hour),
                    HoverHelp::tooltip(QUIET_HOURS_HELP),
                )
            }))
            .chosen(chosen.to_string())
            .disabled(self.locked())
            .on_choose(move |value, _, cx| {
                let Ok(hour) = value.parse::<u8>() else {
                    return;
                };
                let _ = weak.update(cx, |alerts, cx| {
                    let _ = alerts.edit(|form| set(&mut form.quiet_hours, hour), cx);
                });
            });
            choices::field_with_caption(wrapper, select, card::caption(caption, edited, palette))
        };
        let choose = cx.weak_entity();
        let priority = div()
            .flex()
            .flex_col()
            .gap(px(choices::LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(card::caption(
                QUIET_PRIORITY_LABEL,
                edited(|form, stored| form.priority != stored.priority),
                palette,
            ))
            .child(
                div().flex().min_w_0().child(
                    quiet_priorities()
                        .fold(
                            RadioGroup::new(
                                QUIET_PRIORITY_ID,
                                QUIET_PRIORITY_LABEL,
                                HoverHelp::tooltip(QUIET_PRIORITY_HELP),
                                Role::Group,
                            )
                            .small_segments()
                            .fit_words(true),
                            |group, priority| {
                                group.option(RadioOption::new(
                                    quiet_priority_label(priority),
                                    HoverHelp::tooltip(QUIET_PRIORITY_HELP),
                                    priority,
                                ))
                            },
                        )
                        .chosen(Some(quiet.priority))
                        .disabled(self.locked())
                        .on_change(move |&chosen, _, cx| {
                            let _ = choose.update(cx, |alerts, cx| {
                                let _ = alerts.edit(|form| form.quiet_hours.priority = chosen, cx);
                            });
                        }),
                ),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(FIELD_GAP))
            .w_full()
            .min_w_0()
            .child(self.switch(
                Switch::QuietHours,
                quiet.enabled,
                |form, on| form.quiet_hours.enabled = on,
                cx,
            ))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(FIELD_GAP))
                    .child(hour(
                        QUIET_START_ID,
                        "ntfy-quiet-start-field",
                        QUIET_START_LABEL,
                        "Start",
                        quiet.start_hour,
                        edited(|form, stored| form.start_hour != stored.start_hour),
                        |quiet, hour| quiet.start_hour = hour,
                    ))
                    .child(hour(
                        QUIET_END_ID,
                        "ntfy-quiet-end-field",
                        QUIET_END_LABEL,
                        "End",
                        quiet.end_hour,
                        edited(|form, stored| form.end_hour != stored.end_hour),
                        |quiet, hour| quiet.end_hour = hour,
                    )),
            )
            .child(priority)
    }

    /// A switch with its words, toggling through this form.
    fn switch(
        &self,
        switch: Switch,
        on: bool,
        change: fn(&mut Form, bool),
        cx: &Context<Self>,
    ) -> Div {
        let (id, label, help) = switch.words();
        let weak = cx.weak_entity();
        let edited = self.stored.as_ref().is_some_and(|stored| match switch {
            Switch::Enabled => on != stored.enabled,
            Switch::Waiting => on != stored.notify_waiting,
            Switch::Blocked => on != stored.notify_blocked,
            Switch::ClearToken => on,
            Switch::AskAgents => on != stored.asks_agents_when_blocked(),
            Switch::QuietHours => on != stored.quiet_hours.enabled,
        });
        let caption = super::editing::caption(label, edited.then(|| theme::palette(cx)));
        let copy = match switch {
            Switch::ClearToken => card::switch_copy(caption, help, cx),
            Switch::Enabled => card::switch_copy(caption, "", cx),
            Switch::Waiting => card::switch_copy(caption, "", cx).child(card::help(
                "ntfy-waiting-agents",
                waiting_agents(),
                cx,
            )),
            Switch::Blocked => card::switch_copy(caption, BLOCKED_HELP, cx),
            Switch::AskAgents => card::switch_copy(caption, "", cx).child(card::help(
                "blocked-agents-named",
                blocked_agents(),
                cx,
            )),
            Switch::QuietHours => card::switch_copy(caption, QUIET_NOTE, cx),
        };
        div().child(
            Checkbox::new(id, label, HoverHelp::tooltip(help), Role::Switch)
                .switch()
                .content(copy)
                .focus(self.switch_handle(switch))
                .checked(on)
                .disabled(self.locked())
                .on_toggle(move |on, window, cx| {
                    let _ = weak.update(cx, |alerts, cx| {
                        let _ = alerts.edit(|form| change(form, on), cx);
                        alerts.hold(cx);
                        alerts.sync(window, cx);
                    });
                }),
        )
    }
}

impl Render for NtfyAlerts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let form = self.form.value().clone();
        let weak = cx.weak_entity();
        let failure =
            |id: SharedString, reason: &HostText| choices::failure_outcome(id, reason, cx);

        let token_help = if form.clear {
            TOKEN_CLEAR_HELP
        } else if self.server_changed() && self.token_saved() {
            TOKEN_CHANGED_HELP
        } else {
            TOKEN_HELP
        };
        let origin_hint =
            (self.token_saved() && self.server_changed() && form.token.is_empty() && !form.clear)
                .then(|| {
                    choices::help(
                        "Changing the server clears the saved token when you save",
                        cx,
                    )
                    .id("ntfy-token-origin-hint")
                    .debug_selector(|| "ntfy-token-origin-hint".to_owned())
                });

        // `.segmented-control` of `aria-pressed` buttons, as the old form drew the five priorities: each its own tab
        // stop, a press choosing it into the form and saving nothing.
        let choose = weak.clone();
        let priority = div()
            .flex()
            .flex_col()
            .gap(px(choices::LABEL_GAP))
            .w_full()
            .min_w_0()
            .child(card::caption(
                PRIORITY_LABEL,
                self.stored
                    .as_ref()
                    .is_some_and(|stored| form.priority != stored.priority),
                palette,
            ))
            // Small segments take their basis along a row. Keep that basis horizontal in this field column.
            .child(
                div().flex().min_w_0().child(
                    PRIORITIES
                        .into_iter()
                        .fold(
                            RadioGroup::new(
                                "ntfy-priority",
                                PRIORITY_LABEL,
                                HoverHelp::tooltip(PRIORITY_HELP),
                                Role::Group,
                            )
                            .small_segments()
                            .fit_words(true),
                            |group, priority| {
                                group.option(RadioOption::new(
                                    priority.display_label(),
                                    HoverHelp::tooltip(PRIORITY_HELP),
                                    priority,
                                ))
                            },
                        )
                        .chosen(Some(form.priority))
                        .disabled(self.locked())
                        .on_change(move |&chosen, _, cx| {
                            let _ = choose.update(cx, |alerts, cx| {
                                let _ = alerts.edit(|form| form.priority = chosen, cx);
                            });
                        }),
                ),
            );

        // "Retry loading" while the read has failed, as in the old form.
        let retry = self.unreadable.is_some().then(|| {
            let weak = weak.clone();
            choices::retry_loading(
                "ntfy-retry",
                "Retry loading",
                "Load the saved ntfy alert settings again",
                move |cx| {
                    let _ = weak.update(cx, |alerts, cx| {
                        alerts.unreadable = None;
                        Self::read(alerts.window, cx);
                        cx.notify();
                    });
                },
            )
        });
        let send_test = Button::new(
            "ntfy-send-test",
            "Send test",
            HoverHelp::tooltip(
                "Send one alert using the saved settings, including while alerts are switched off",
            ),
            Role::Button,
        )
        .tier(Tier::Neutral)
        .disabled(self.stored.is_none())
        .busy(self.form.is_saving() || self.testing)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |alerts, cx| alerts.send_test(cx));
        });

        let saving = self.form.is_saving();
        let tested = self.tested.as_ref().map(|tested| match tested {
            Ok(outcome) => {
                let line = tested_line(*outcome);
                choices::outcome_line("ntfy-tested", Role::Status, theme::color(palette.fg_1))
                    .aria_label(line.clone())
                    .child(line)
            }
            Err(reason) => failure("ntfy-tested".into(), reason),
        });
        let saved = (self.saved && !saving && !self.form.is_touched()).then(|| {
            choices::outcome_line("ntfy-saved", Role::Status, theme::color(palette.fg_1))
                .aria_label("ntfy alert settings saved")
                .child("ntfy alert settings saved")
        });

        let fields = div()
            .flex()
            .flex_col()
            .gap(px(FIELD_GAP))
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(FIELD_GAP))
                    .child(self.field(Field::Server, None, palette))
                    .child(self.field(Field::Topic, None, palette)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .rounded(theme::metrics::RADIUS_SM)
                    .border_l_2()
                    .border_color(theme::color(palette.warn))
                    .bg(theme::color(palette.warn_bg_dim))
                    .px(px(8.))
                    .py(px(5.))
                    .child(
                        card::help_style(cx)
                            .flex_none()
                            .text_color(theme::color(palette.warn))
                            .child("⚠"),
                    )
                    .child(card::help("notification-topic-help", TOPIC_HELP, cx).min_w_0()),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap(px(FIELD_GAP))
                    .child(self.field(Field::Token, Some(token_help), palette))
                    .child(priority),
            )
            .children(origin_hint)
            .children(
                self.unreadable
                    .as_ref()
                    .map(|reason| failure("ntfy-load-error".into(), reason)),
            )
            .children(retry);
        let saved_state = self.saved_state().map(|line| {
            choices::outcome_line("ntfy-saved-state", Role::Status, theme::color(palette.warn))
                .aria_label(line)
                .child(line)
        });
        let test = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(saved_state)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(FIELD_GAP))
                    .child(
                        choices::help("Send test sends one alert using the saved settings.", cx)
                            .min_w_0(),
                    )
                    .child(div().flex_none().child(send_test)),
            )
            .children((saving || self.testing).then(|| {
                choices::outcome_line("ntfy-working", Role::Status, theme::color(palette.fg_0))
                    .aria_label("Working…")
                    .child("Working…")
            }))
            .children(
                self.form
                    .refusal()
                    .map(|reason| failure("ntfy-error".into(), reason)),
            )
            .children(self.refused.line(REFUSED, |id, text| {
                failure(id.into(), &HostText::from_host(text))
            }))
            .children(saved)
            .children(tested);
        // The Blocked agents card: what the agents Farhelm starts are told, which holds with or without ntfy.
        let blocked_agents = div()
            .id("blocked-agents")
            .role(Role::Region)
            .aria_label("Blocked agents")
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .min_w_0()
            .child(
                card::heading("Blocked agents", Some(BLOCKED_INTRO), None, cx)
                    .id("blocked-agents-heading")
                    .role(Role::Heading)
                    .aria_label("Blocked agents"),
            )
            .child(card::rows(
                [
                    self.switch(
                        Switch::AskAgents,
                        form.ask_agents,
                        |form, on| form.ask_agents = on,
                        cx,
                    ),
                    self.criteria_row(palette, cx),
                ],
                cx,
            ));
        let ntfy_alerts = div()
            .id("ntfy-alerts")
            .role(Role::Region)
            .aria_label("ntfy alerts")
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .min_w_0()
            .child(
                card::heading("ntfy alerts", Some(INTRO), None, cx)
                    .id("ntfy-heading")
                    .role(Role::Heading)
                    .aria_label("ntfy alerts"),
            )
            .child(card::rows(
                [
                    self.switch(
                        Switch::Enabled,
                        form.enabled,
                        |form, on| form.enabled = on,
                        cx,
                    ),
                    self.switch(
                        Switch::Waiting,
                        form.notify_waiting,
                        |form, on| form.notify_waiting = on,
                        cx,
                    ),
                    self.switch(
                        Switch::Blocked,
                        form.notify_blocked,
                        |form, on| form.notify_blocked = on,
                        cx,
                    ),
                    self.grace_row(palette, cx),
                    fields,
                    self.switch(
                        Switch::ClearToken,
                        form.clear,
                        |form, on| form.clear = on,
                        cx,
                    ),
                    self.quiet_hours_row(palette, cx),
                    test,
                ],
                cx,
            ));
        // Two cards, spaced as the section spaces its other groups, though one view draws both.
        div()
            .flex()
            .flex_col()
            .gap(px(super::SECTION_GAP))
            .w_full()
            .min_w_0()
            .child(blocked_agents)
            .child(ntfy_alerts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unrelated saves must keep a token the form never received, and an explicit replacement or clear must not be
    /// taken for that keeping: the old form's `secret_updates_are_explicit`. Specified: a blank token field sends
    /// none, a typed one sends it, and Clear saved token sends `""` whatever was typed.
    #[test]
    fn a_blank_token_keeps_the_saved_one_and_a_typed_or_cleared_one_does_not() {
        let mut form = Form::over(&Settings::default());
        assert_eq!(form.save().token, None);
        form.token = "replacement".into();
        assert_eq!(form.save().token.as_deref(), Some("replacement"));
        form.clear = true;
        assert_eq!(form.save().token.as_deref(), Some(""));
    }

    /// The waiting switch's wording must name exactly the agents whose waits Farhelm can see, from the same answer the
    /// supervisor's readers are pinned to, and must not promise alerts for an agent read by change counting alone.
    /// Specified: Claude Code and Codex are named for questions and approvals, Muse Code and Antigravity for the
    /// folder-trust question only, and Goose, Pi, Cursor and the shells are not named.
    #[test]
    fn the_waiting_switch_names_the_agents_whose_waits_are_read() {
        let words = waiting_agents();
        assert!(
            words.starts_with("Covers questions and approvals in ")
                && words.contains("Claude Code")
                && words.contains("Codex"),
            "{words}"
        );
        assert!(
            words.contains(
                "In Muse Code and Antigravity, only the question about trusting a folder."
            ) || words.contains(
                "In Antigravity and Muse Code, only the question about trusting a folder."
            ),
            "{words}"
        );
        let named: Vec<&str> = words
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        for absent in [
            "Goose", "Pi", "Cursor", "Bash", "Zsh", "OMP", "Grok", "OpenCode",
        ] {
            assert!(!named.contains(&absent), "{absent} in {words}");
        }
    }

    /// The blocked switch says which agents Farhelm asks to notify, from the same answer the supervisor's delivery is
    /// pinned to. Specified: Claude Code, Codex, Muse Code, Goose, Pi and OMP are named; Cursor, Grok, Antigravity,
    /// OpenCode and the shells are not.
    #[test]
    fn the_blocked_switch_names_the_agents_that_get_the_instruction() {
        let words = blocked_agents();
        for named in ["Claude Code", "Codex", "Muse Code", "Goose", "Pi", "OMP"] {
            assert!(words.contains(named), "{named} missing from {words}");
        }
        let listed: Vec<&str> = words
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        for absent in ["Cursor", "Grok", "Antigravity", "OpenCode", "Bash", "Zsh"] {
            assert!(!listed.contains(&absent), "{absent} in {words}");
        }
    }

    /// The segments must offer whatever the helm stores, or a value saved by another client would show nothing chosen
    /// and the next Save would still send it unseen. Specified: the presets are Off, 5 s, 10 s, 30 s, 1 min and 2 min;
    /// a stored value among them adds nothing, and one outside them (45, 600) is added in order and labelled in
    /// seconds or whole minutes.
    #[test]
    fn the_grace_segments_offer_the_presets_and_any_stored_value() {
        let labels: Vec<String> = GRACE_CHOICES.into_iter().map(grace_label).collect();
        assert_eq!(labels, ["Off", "5 s", "10 s", "30 s", "1 min", "2 min"]);
        assert_eq!(grace_choices(10), GRACE_CHOICES.to_vec());
        assert_eq!(grace_choices(45), [0, 5, 10, 30, 45, 60, 120]);
        assert_eq!(grace_choices(600), [0, 5, 10, 30, 60, 120, 600]);
        assert_eq!(grace_label(45), "45 s");
        assert_eq!(grace_label(600), "10 min");
    }

    /// The session pane throttles its interaction signal, which withdraws a held alert, so the throttle's gap must stay
    /// well inside the shortest grace period offered here, or a withdrawal could wait past the alert's due time.
    /// Specified: twice the gap is at most the shortest preset above Off.
    #[test]
    fn the_interaction_throttle_fits_inside_the_shortest_grace_period() {
        let shortest = GRACE_CHOICES
            .into_iter()
            .filter(|secs| *secs > 0)
            .min()
            .expect("a grace period above Off is offered");
        assert!(
            crate::session_pane::INTERACTION_GAP * 2
                <= std::time::Duration::from_secs(u64::from(shortest))
        );
    }

    /// The grace period is saved with the rest of the form, or a Save would silently put an older client's default
    /// back. Specified: the form over the stored settings sends their grace period, and an edited one sends the edit.
    #[test]
    fn the_grace_period_is_sent_with_the_form() {
        let stored = Settings {
            grace_period_secs: 30,
            ..Settings::default()
        };
        let mut form = Form::over(&stored);
        assert_eq!(form.save().grace_period_secs, Some(30));
        form.grace_period_secs = 0;
        assert_eq!(form.save().grace_period_secs, Some(0));
    }

    /// Quiet hours are saved whole with the rest of the form, or a Save would put an older client's quiet hours back or
    /// drop the user's Off. Specified: the form over the stored settings sends their quiet hours, Off included, and an
    /// edit sends the edit.
    #[test]
    fn quiet_hours_are_sent_with_the_form() {
        let stored = Settings {
            quiet_hours: QuietHours {
                enabled: true,
                start_hour: 23,
                end_hour: 6,
                priority: None,
            },
            ..Settings::default()
        };
        let mut form = Form::over(&stored);
        assert_eq!(form.save().quiet_hours, Some(stored.quiet_hours));
        form.quiet_hours.start_hour = 21;
        form.quiet_hours.priority = Some(Priority::Min);
        assert_eq!(
            form.save().quiet_hours,
            Some(QuietHours {
                start_hour: 21,
                priority: Some(Priority::Min),
                ..stored.quiet_hours
            })
        );
    }

    /// The hour selects and the priority segments are how the user reads quiet hours, and the test line is how they
    /// learn what a test did, so their words are pinned. Specified: hours read `00:00` to `23:00`; the priorities are
    /// Off then Min to Urgent; a test line names the priority, adds "quiet hours" inside them and the silenced wording
    /// when they are set to Off, and is the plain "Test alert sent" from a helm that answers with no outcome.
    #[test]
    fn quiet_hours_words_and_the_test_line() {
        assert_eq!(hour_label(0), "00:00");
        assert_eq!(hour_label(7), "07:00");
        assert_eq!(hour_label(23), "23:00");
        let labels: Vec<&str> = quiet_priorities().map(quiet_priority_label).collect();
        assert_eq!(labels, ["Off", "Min", "Low", "Default", "High", "Urgent"]);
        let line = |priority, quiet_hours| {
            tested_line(Some(TestOutcome {
                priority,
                quiet_hours,
            }))
        };
        assert_eq!(tested_line(None), "Test alert sent");
        assert_eq!(
            line(Priority::Default, QuietHoursNow::Inactive),
            "Test alert sent (Default)"
        );
        assert_eq!(
            line(Priority::Low, QuietHoursNow::Active),
            "Test alert sent (Low \u{2014} quiet hours)"
        );
        assert_eq!(
            line(Priority::High, QuietHoursNow::Silenced),
            "Test alert sent (High \u{2014} quiet hours active and set to Off; real alerts are silenced)"
        );
    }

    /// English lists read "A", "A and B", "A, B and C".
    #[test]
    fn names_join_as_an_english_list() {
        assert_eq!(and_list(&["A"]), "A");
        assert_eq!(and_list(&["A", "B"]), "A and B");
        assert_eq!(and_list(&["A", "B", "C"]), "A, B and C");
    }

    /// The warning that Save clears the token must follow the helm's own rule, which compares servers, not spellings:
    /// equal addresses must not promise a removal, and a half-typed one has no server yet. The old form's
    /// `token_warning_compares_normalized_server_origins`. Specified: default ports, a path, a trailing slash and a
    /// capitalised host name are the same server; another scheme, host or port is another; an unfinished address is
    /// neither.
    #[test]
    fn the_server_changes_only_with_its_scheme_host_or_port() {
        for same in [
            "https://example.test",
            "https://example.test:443/prefix",
            "https://EXAMPLE.test/",
        ] {
            assert!(!server_changed("https://example.test", same), "{same}");
        }
        for other in [
            "http://example.test",
            "https://other.test",
            "https://example.test:8443",
        ] {
            assert!(server_changed("https://example.test", other), "{other}");
        }
        assert!(!server_changed("https://example.test", "https://"));
    }
}

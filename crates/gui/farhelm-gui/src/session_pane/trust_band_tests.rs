//! Tests of the trust hint band (`trust_band.rs`): when it shows, where it sits, what it reads and when, its three
//! buttons, its failure line, and which agent it speaks for. They port the old `claude-trust.spec.ts` and the band's
//! half of `workspace-trust.spec.ts` (reference commit `24bc0580`), whose cases are named on the tests that replace
//! them. The rules every agent's band shares are tested on Claude's; Codex's and Muse's tests cover what is the agent's
//! own (its words, its switch, its dismissal and its occurrences). Their fixtures and Antigravity's use ordinary
//! directories so the hint cannot silently depend on checkout ownership.
//!
//! The window tests run the real main view over the helm client's fake, whose terminal streams the connection serves,
//! and read what the window drew from GPUI's driver record of the frame. The open session is `a`, at the question of
//! the agent under test ([`Agent`]; Claude unless a test names another) in the helm fixture's managed checkout. The
//! fake answers commands only in the order they were sent, as the helm does, so the tests keep what the window sent
//! ([`Trust::pending`]) and answer up to the one they mean, answering everything before it as an ordinary success. The
//! reads of the agent under test's trust and the band's dismissals are never answered that way: a test answers each
//! one itself.

use farhelm_helm_client::Method;
use farhelm_helm_client::fake::FakeCommand;
use farhelm_proto::LaunchHarness;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, KeyedEntry, PREFERENCES_KEY, Preferences, SessionRow,
};
use gpui::{Bounds, Pixels, Role, TestAppContext, px, size};
use serde_json::{Value, json};

use super::trust_band::{
    BAND_ID, COPY_ID, DISMISS_ID, ERROR, ERROR_ID, Facts, HIDE_ID, SETTINGS_ID,
};
use crate::focus::Mark;
use crate::operations::OpKey;
use crate::session_list::menu_tests::{push, snapshot};
use crate::session_list::tests::{Fixture, session};
use crate::settings::{self, TrustAgent};
use crate::terminal::set::AGENT_TAB_ID;
use crate::test_support::{self, MainWindow, any_open, click, frame, keys, meta, settle};
use crate::ticker;

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// Claude's trust route, which the band and Settings both read.
const CLAUDE_TRUST: &str = "/api/checkout-claude-trust";
/// The shared trust route of the other agents, which ends in the agent's word.
const AGENT_TRUST: &str = "/api/checkout-agent-trust/";

/// An agent the band speaks for, as these tests see it from outside: where its trust is read, where its dismissal is
/// stored, its switch in Settings, and a session at its question.
#[derive(Clone, Copy)]
struct Agent {
    route: &'static str,
    field: &'static str,
    switch: &'static str,
    asking: fn(&'static str) -> Fixture,
}

const CLAUDE: Agent = Agent {
    route: CLAUDE_TRUST,
    field: "claude_trust_hint_dismissed",
    switch: "settings-claude-trust",
    asking: claude_asking,
};
const CODEX: Agent = Agent {
    route: "/api/checkout-agent-trust/codex",
    field: "codex_trust_hint_dismissed",
    switch: "settings-codex-trust",
    asking: codex_asking,
};
const MUSE: Agent = Agent {
    route: "/api/checkout-agent-trust/muse",
    field: "muse_trust_hint_dismissed",
    switch: "settings-muse-trust",
    asking: muse_asking,
};
const ANTIGRAVITY: Agent = Agent {
    route: "/api/checkout-agent-trust/antigravity",
    field: "antigravity_trust_hint_dismissed",
    switch: "settings-antigravity-trust",
    asking: antigravity_asking,
};

/// The session filed under `key` at Claude's question, in the helm fixture's managed checkout, whatever its launch.
fn claude_asking(key: &'static str) -> Fixture {
    session(key).at_claude_trust_question()
}

/// An ordinary directory at Codex's own workspace-trust question.
fn codex_asking(key: &'static str) -> Fixture {
    session(key)
        .launch(launched("codex"))
        .at_workspace_trust_question()
        .unmanaged()
        .not_launched_as_checkout()
}

/// The same for Muse.
fn muse_asking(key: &'static str) -> Fixture {
    session(key)
        .launch(launched("muse"))
        .at_workspace_trust_question()
        .unmanaged()
        .not_launched_as_checkout()
}

/// Antigravity's question in an ordinary directory. Clear both checkout ownership and launch provenance so neither
/// can accidentally qualify the session for the managed-checkout hints.
fn antigravity_asking(key: &'static str) -> Fixture {
    session(key)
        .launch(launched("antigravity"))
        .at_workspace_trust_question()
        .unmanaged()
        .not_launched_as_checkout()
}

/// A structured agent launch of the harness whose wire name is `harness`, in the helm's wire form.
fn launched(harness: &str) -> Value {
    json!({
        "kind": "agent",
        "selection": {"harness": harness, "permissions": null},
        "start": [harness, "{farhelm_args}"],
        "resume": null,
    })
}

/// A command launch of `codex` that declares Codex, which names no structured harness.
fn command_launch() -> Value {
    json!({"kind": "command", "command": "codex", "yolo": false, "agent": "codex", "resume": null})
}

/// The open session: at Claude's question, in the helm fixture's managed checkout.
fn a() -> Fixture {
    claude_asking("a")
}

/// The main window over `a` (open) and `b`, the agent under test, and the commands the window sent that the test has
/// not answered yet.
struct Trust<'a> {
    main: MainWindow<'a>,
    agent: Agent,
    pending: Vec<FakeCommand>,
}

/// The window with `a` open at Claude's question, its terminal live and focused, and the band's first read sent.
fn open(cx: &mut TestAppContext) -> Trust<'_> {
    open_as(cx, CLAUDE)
}

/// The window with `a` open at `agent`'s question, its terminal live and focused, and the band's first read of
/// `agent`'s trust sent.
fn open_as(cx: &mut TestAppContext, agent: Agent) -> Trust<'_> {
    let fleet = [((agent.asking)("a"), 0), (session("b"), 0)];
    let mut main = test_support::open_main(cx, snapshot(&fleet, false));
    // Tall enough for Settings at Agent trust to show Claude's hint switch without scrolling.
    main.cx.simulate_resize(size(px(1600.), px(1000.)));
    main.terminal_live();
    Trust {
        main,
        agent,
        pending: Vec::new(),
    }
}

/// Whether `command` reads any agent's trust.
fn any_trust_read(command: &FakeCommand) -> bool {
    command.method == Method::Get
        && (command.path == CLAUDE_TRUST || command.path.starts_with(AGENT_TRUST))
}

impl Trust<'_> {
    /// Whether `command` is a read of the agent under test's trust.
    fn is_read(&self, command: &FakeCommand) -> bool {
        command.method == Method::Get && command.path == self.agent.route
    }

    /// Whether `command` saves the shared dismissal of the agent under test's hint.
    fn is_dismissal(&self, command: &FakeCommand) -> bool {
        command.method == Method::Put
            && command.path == "/api/preferences"
            && command
                .body
                .as_ref()
                .is_some_and(|body| body.get(self.agent.field).is_some())
    }

    /// Take what the window sent since the last call into [`Trust::pending`].
    fn take(&mut self) {
        let sent = self.main.helm.commands();
        self.pending.extend(sent);
    }

    /// The reads of the agent under test's trust waiting for an answer, in order: the band's and Settings' alike.
    fn reads(&mut self) -> Vec<u64> {
        self.take();
        self.pending
            .iter()
            .filter(|command| self.is_read(command))
            .map(|command| command.id)
            .collect()
    }

    /// The dismissal saves waiting for an answer, in order, with their bodies.
    fn dismissals(&mut self) -> Vec<(u64, Value)> {
        self.take();
        self.pending
            .iter()
            .filter(|command| self.is_dismissal(command))
            .map(|command| (command.id, command.body.clone().unwrap_or_default()))
            .collect()
    }

    /// Answer command `id` with `status` and `body`, after answering every command sent before it: another agent's
    /// trust read as off, and anything else as a success with no body. Panics when one of those is a read of the agent
    /// under test's trust or a dismissal, which a test must answer itself.
    fn answer(&mut self, id: u64, status: u16, body: Option<Value>) {
        self.take();
        let at = self
            .pending
            .iter()
            .position(|command| command.id == id)
            .unwrap_or_else(|| panic!("command {id} is not waiting"));
        let before: Vec<FakeCommand> = self.pending.drain(..=at).collect();
        for command in &before[..at] {
            self.ordinary(command);
        }
        self.main.helm.reply(id, status, body);
        settle(self.main.cx);
    }

    /// Answer every command waiting, as [`Trust::answer`] answers those before the one it targets.
    fn answer_rest(&mut self) {
        self.take();
        for command in std::mem::take(&mut self.pending) {
            self.ordinary(&command);
        }
        settle(self.main.cx);
    }

    /// Answer `command` the ordinary way: another agent's trust read as off, anything else as a success with no body.
    /// Panics on a read of the agent under test's trust or a dismissal, which a test must answer itself.
    fn ordinary(&mut self, command: &FakeCommand) {
        assert!(
            !self.is_read(command) && !self.is_dismissal(command),
            "answer {command:?} first"
        );
        let reply =
            any_trust_read(command).then(|| json!({ "enabled": false, "hint_dismissed": false }));
        self.main.helm.reply(command.id, 200, reply);
    }

    /// Answer the read of the agent under test's trust `id`: on as `enabled` says.
    fn trust_is(&mut self, id: u64, enabled: bool) {
        self.answer(
            id,
            200,
            Some(json!({ "enabled": enabled, "hint_dismissed": false })),
        );
    }

    /// Refuse command `id` with the helm's words.
    fn refuse(&mut self, id: u64) {
        self.answer(id, 503, Some(json!("fixture refusal")));
    }

    /// The one read of the agent under test's trust waiting, which must be the only one.
    fn one_read(&mut self) -> u64 {
        let reads = self.reads();
        assert_eq!(
            reads.len(),
            1,
            "one read of the agent's trust is out: {reads:?}"
        );
        reads[0]
    }

    /// Where the element with id `id` was drawn in a fresh frame, if it was.
    fn drawn(&mut self, id: &str) -> Option<Bounds<Pixels>> {
        frame(self.main.cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .map(|node| node.bounds)
    }

    /// Whether the band shows at all.
    fn shows(&mut self) -> bool {
        self.drawn(BAND_ID).is_some()
    }

    /// Whether the band shows its explanation and buttons, and whether it shows its failure line.
    fn parts(&mut self) -> (bool, bool) {
        (
            self.drawn(COPY_ID).is_some() && self.drawn(DISMISS_ID).is_some(),
            self.drawn(ERROR_ID).is_some(),
        )
    }

    /// The helm pushes the preference row with the agent under test's dismissal stored as `dismissed`, and no other.
    fn dismissed_is(&mut self, dismissed: Option<bool>) {
        let preferences = Preferences {
            visit_history: vec!["a".into()],
            welcome_seen: Some(true),
            ..Preferences::default()
        };
        let mut value = serde_json::to_value(preferences).unwrap();
        value[self.agent.field] = json!(dismissed);
        self.main.helm.push(CollectionChanges {
            collection: Collection::Preferences,
            meta: meta(&[PREFERENCES_KEY]),
            upsert: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value,
            }],
            remove: Vec::new(),
        });
        settle(self.main.cx);
    }

    /// The helm pushes `a` as `fixture`, listing no terminal tabs.
    fn a_is(&mut self, fixture: Fixture) {
        push(
            &mut self.main.helm,
            &["a", "b"],
            &[(fixture, 0)],
            self.main.cx,
        );
    }

    /// Open Settings at Sessions as the gear does, from a control of the test's own.
    fn open_settings(&mut self) {
        let opener = test_support::handle(self.main.cx);
        self.main.cx.update(|window, cx| {
            let mark = Mark::take(cx);
            settings::open(&opener, &mark, window, cx);
        });
        settle(self.main.cx);
        assert!(any_open(self.main.cx), "the premise: Settings opened");
    }

    /// Close Settings with its footer's Close.
    fn close_settings(&mut self) {
        click(self.main.cx, "settings-footer-close");
        assert!(!any_open(self.main.cx), "the premise: Settings closed");
    }

    /// The band shows for `a`: its first read answered off.
    fn shown(&mut self) {
        let read = self.one_read();
        self.trust_is(read, false);
        assert!(self.shows(), "the premise: the band shows");
    }
}

// ---- When it shows -------------------------------------------------------------------------------------------------

/// A failed Settings read leaves trust unknown, so the band must keep the
/// save error without advertising a switch whose state it cannot confirm.
/// The agent tab and the current question gate both lines independently of
/// the folder. Dismissal and the occurrence's × are separate choices.
#[test]
fn every_condition_withdraws_the_band_on_its_own() {
    let all = Facts {
        question: true,
        agent_shown: true,
        trust: Some(false),
        dismissed: false,
        hidden: false,
    };
    assert!(all.shows() && all.shows_failure(true) && !all.shows_failure(false));
    // Each withdrawn condition, and whether the failure line still shows without it.
    type Withdraw = fn(&mut Facts);
    let withdrawn: [(Withdraw, bool); 6] = [
        (|facts| facts.question = false, false),
        (|facts| facts.agent_shown = false, false),
        (|facts| facts.trust = Some(true), false),
        (|facts| facts.trust = None, true),
        (|facts| facts.dismissed = true, true),
        (|facts| facts.hidden = true, false),
    ];
    for (change, failure) in withdrawn {
        let mut facts = all;
        change(&mut facts);
        assert!(!facts.shows(), "{facts:?}");
        assert_eq!(facts.shows_failure(true), failure, "{facts:?}");
    }
}

/// SPEC.md: the band "takes layout space rather than covering the question, is announced without taking focus", and
/// says why Farhelm does not answer for the user; and it waits for trust known off (the old `claude-trust.spec.ts`,
/// "Claude trust hint preserves the terminal and opens the opt-in caveats", its first part). Specified: with `a` at the
/// question, the band reads Claude's trust once and shows nothing until the answer; answered off, it shows as a status
/// above the terminal's grid, which loses height to it, with the old words and buttons, while the terminal keeps
/// focus.
#[gpui::test]
fn the_band_reads_trust_then_shows_above_the_terminal_without_taking_focus(
    cx: &mut TestAppContext,
) {
    let mut trust = open(cx);
    let read = trust.one_read();
    assert!(!trust.shows(), "nothing until trust is known off");
    let before = trust
        .drawn("terminal-grid")
        .expect("the premise: the grid is drawn");

    trust.trust_is(read, false);
    let band = frame(trust.main.cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(BAND_ID))
        .expect("the band shows");
    assert_eq!(band.role, Some(Role::Status));
    let after = trust.drawn("terminal-grid").unwrap();
    assert!(
        band.bounds.bottom() <= after.top(),
        "{:?} above {after:?}",
        band.bounds
    );
    assert!(
        after.size.height < before.size.height,
        "the band took height from the terminal"
    );
    assert!(
        test_support::terminal_focused(trust.main.cx),
        "focus stayed in the terminal"
    );
    assert_eq!(
        test_support::label(trust.main.cx, COPY_ID).as_deref(),
        Some(
            "Claude asks this for every new folder. Farhelm can answer it for you, but it's off by default: \
             there's no supported way to do it, so Farhelm would have to edit Claude's own settings file, which \
             comes with caveats."
        )
    );
    assert_eq!(
        test_support::label(trust.main.cx, SETTINGS_ID).as_deref(),
        Some("Settings and caveats")
    );
    assert!(trust.reads().is_empty(), "one read");
}

/// Claude shows the hint in ordinary folders. Selecting a shell tab hides
/// the agent's question and its hint until the agent terminal returns.
#[gpui::test]
fn a_plain_folders_agent_terminal_shows_the_claude_band(cx: &mut TestAppContext) {
    let agent = Agent {
        asking: |key| claude_asking(key).unmanaged().not_launched_as_checkout(),
        ..CLAUDE
    };
    let plain = (agent.asking)("a");
    let row: SessionRow = serde_json::from_value(plain.entry().value).unwrap();
    assert!(
        row.info.working_copy.is_none(),
        "premise: ordinary directory"
    );
    let mut trust = open_as(cx, agent);
    trust.shown();
    push(
        &mut trust.main.helm,
        &["a", "b"],
        &[(plain, 1)],
        trust.main.cx,
    );
    assert!(
        trust.shows(),
        "adding a tab leaves the current hint visible"
    );
    click(trust.main.cx, "session-terminal-tab-tab-0");
    assert!(!trust.shows(), "not over a tab");
    click(trust.main.cx, AGENT_TAB_ID);
    assert!(trust.shows(), "back over the agent terminal");
}

// ---- Settings and caveats ------------------------------------------------------------------------------------------

/// Trust is not pushed, so the band reads it again when Settings closes, whichever entry point opened Settings: the old
/// band did, and without it a user who turned trust on in Settings would still be told it is off (the old
/// `claude-trust.spec.ts`, "Claude trust hint preserves the terminal and opens the opt-in caveats", its Settings part).
/// Specified: Settings and caveats opens Settings (whose Agent trust reads are its own tests'); closing it sends one
/// more read of Claude's trust, and an answer of on takes the band away; Settings opened as the gear opens it, closed,
/// reads again, and an answer of off brings the band back.
#[gpui::test]
fn closing_settings_reads_trust_again(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    click(trust.main.cx, SETTINGS_ID);
    assert!(any_open(trust.main.cx), "Settings opened");
    let settings_read = trust.one_read();
    trust.trust_is(settings_read, true);

    trust.close_settings();
    let read = trust.one_read();
    trust.trust_is(read, true);
    assert!(!trust.shows(), "trust is on now");

    trust.open_settings();
    trust.close_settings();
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(trust.shows(), "trust is off again");
}

/// SPEC.md: "A change made elsewhere updates the hint band as it happens". Trust is not pushed, so the band reads it
/// again on the ticker's 30-second clock while it shows (the migration's orchestrator's decision of 2026-11-16), and
/// only then, so a hidden band costs no reads. Specified: with the band showing, one clock period later it reads
/// Claude's trust with no Settings close; an answer of on, as if turned on elsewhere, takes it away; and with the band
/// gone, another period sends no read.
#[gpui::test]
fn a_tick_reads_trust_again_while_the_band_shows(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    assert!(trust.reads().is_empty(), "the premise: no read is out");
    test_support::advance(trust.main.cx, ticker::CLOCK_PERIOD);
    let read = trust.one_read();
    trust.trust_is(read, true);
    assert!(!trust.shows(), "turned on elsewhere");
    test_support::advance(trust.main.cx, ticker::CLOCK_PERIOD);
    assert!(trust.reads().is_empty(), "no tick while the band is away");
}

/// A tick must not pile reads on a helm slower than the clock: the helm answers in order, so each late answer would be
/// dropped by the newer tick's read, and the band would never learn trust while it showed. Specified: with the band
/// showing, a first period sends one read; with it unanswered, a second period sends none, so exactly one is out.
#[gpui::test]
fn a_tick_sends_nothing_while_a_read_is_out(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    test_support::advance(trust.main.cx, ticker::CLOCK_PERIOD);
    test_support::advance(trust.main.cx, ticker::CLOCK_PERIOD);
    trust.one_read();
}

/// A read sent before Settings closed can answer while the newer read sent at the close is still out; the old band's
/// `use_hint_read_reconciliation` existed so such a stale answer never showed (the old `claude-trust.spec.ts`, "Claude
/// trust band repairs an idle read crossing a Settings dismissal save"). The helm answers in order, so the older read
/// never answers after the newer one; what matters is the gap between the two answers. Specified: with the band's first
/// read unanswered, Settings opened and closed sends a second; the first answering off shows nothing, and the second
/// answering on leaves the band away.
#[gpui::test]
fn only_the_newest_trust_read_counts(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    let first = trust.one_read();
    trust.open_settings();
    trust.close_settings();
    let reads = trust.reads();
    assert_eq!(reads.len(), 2, "a second read: {reads:?}");

    trust.trust_is(first, false);
    assert!(!trust.shows(), "the older answer is dropped");
    trust.trust_is(reads[1], true);
    assert!(!trust.shows());
}

// ---- × and Don't show again ----------------------------------------------------------------------------------------

/// SPEC.md: "× hides only the current occurrence, until the question leaves the screen" (the old `claude-trust.spec.ts`,
/// "Claude trust hint preserves the terminal and opens the opt-in caveats", its × part). Switching sessions must not
/// bring a hidden band back, and a new occurrence must not stay hidden. Specified: × hides the band and sends nothing;
/// opening `b` and `a` again keeps it hidden; a push of `a` with the question gone, then back, shows the band again.
#[gpui::test]
fn hide_lasts_until_the_question_leaves(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    click(trust.main.cx, HIDE_ID);
    assert!(!trust.shows());
    assert!(trust.dismissals().is_empty(), "× stores nothing");

    click(trust.main.cx, "session-row-b");
    click(trust.main.cx, "session-row-a");
    settle(trust.main.cx);
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(!trust.shows(), "still hidden after switching");

    trust.a_is(session("a"));
    trust.a_is(a());
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(trust.shows(), "a new occurrence");
}

/// SPEC.md: "Don't show again stores a shared dismissal, reversible beside the switch" (the old `claude-trust.spec.ts`,
/// "Claude trust hint preserves the terminal and opens the opt-in caveats", its last part). Specified: Don't show again
/// hides the band at once and sends `claude_trust_hint_dismissed: true` alone, with a read of Claude's trust; with the
/// dismissal stored the band stays away, and the helm storing it unset again (Settings' switch) brings the band back.
#[gpui::test]
fn dont_show_again_stores_the_shared_dismissal(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    click(trust.main.cx, DISMISS_ID);
    assert!(!trust.shows(), "hidden while the save is out");
    let dismissals = trust.dismissals();
    assert_eq!(dismissals.len(), 1);
    assert_eq!(
        dismissals[0].1,
        json!({ "claude_trust_hint_dismissed": true })
    );
    let read = trust.one_read();

    // The helm pushes a change before it answers the command that caused it.
    trust.dismissed_is(Some(true));
    trust.answer(dismissals[0].0, 204, None);
    trust.trust_is(read, false);
    assert!(!trust.shows(), "dismissed");

    trust.dismissed_is(Some(false));
    assert!(trust.shows(), "shown again");
}

/// The band and Settings save the same field under the preference row's key, which admits one save at a time, and the
/// band shows only what the helm stores: the old band had to discard a Settings choice the helm then refused (the old
/// `claude-trust.spec.ts`, "Claude trust band discards a newer Settings overlay after its origin closes"; a press of
/// the switch while the band's save is out is the operation store's and Settings' own tests'). Specified: with the
/// band's dismissal stored, Settings' hint switch saves "show", and after Settings closes, the helm refusing it leaves
/// the band away with no failure line.
#[gpui::test]
fn the_band_follows_only_the_stored_dismissal(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    click(trust.main.cx, DISMISS_ID);
    let band_save = trust.dismissals()[0].0;
    let band_read = trust.one_read();
    trust.dismissed_is(Some(true));
    trust.answer(band_save, 204, None);
    trust.trust_is(band_read, false);

    trust.main.cx.update(|window, cx| {
        let opener = cx.focus_handle();
        let mark = Mark::take(cx);
        settings::open_at_agent_trust(TrustAgent::Claude, &opener, &mark, window, cx);
    });
    settle(trust.main.cx);
    let settings_read = trust.one_read();
    trust.trust_is(settings_read, false);
    trust.answer_rest();
    // From Claude's trust switch, where the handover put focus, to its hint switch, as a keyboard user goes.
    keys(trust.main.cx, "tab");
    assert_eq!(
        test_support::focused_id(trust.main.cx).as_deref(),
        Some("settings-claude-trust-hint")
    );
    keys(trust.main.cx, "space");
    let dismissals = trust.dismissals();
    assert_eq!(dismissals.len(), 1);
    assert_eq!(
        dismissals[0].1,
        json!({ "claude_trust_hint_dismissed": false })
    );

    trust.close_settings();
    assert!(!trust.shows(), "not before the helm stores it");
    trust.refuse(dismissals[0].0);
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(!trust.shows(), "the refused choice never shows");
    assert_eq!(trust.parts(), (false, false));
}

/// SPEC.md: "A failed Don't show again save restores the band with an adjacent error while that session is still
/// shown; leaving the session drops the error. If the corrective settings read also fails, only the save error appears
/// until trust is known to be off again" (the old `claude-trust.spec.ts`, "Claude trust hint reports refused dismissal
/// only while its session is shown"). Specified: a refused save with its read refused too shows only the failure line,
/// with the old words; a read answering off brings back the explanation beside the line; opening `b` and `a` again
/// drops the line; a second refusal brings it back, and a press that the helm stores takes it away with the band; a
/// refusal that lands after the user opened `b` shows on neither session, and `a` opened again shows the band without
/// the line.
#[gpui::test]
fn a_refused_dont_show_again_says_so_only_while_its_session_is_open(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    click(trust.main.cx, DISMISS_ID);
    let save = trust.dismissals()[0].0;
    let read = trust.one_read();
    trust.refuse(save);
    trust.refuse(read);
    assert_eq!(trust.parts(), (false, true), "only the failure line");
    assert_eq!(
        test_support::label(trust.main.cx, ERROR_ID).as_deref(),
        Some(ERROR)
    );

    trust.open_settings();
    trust.close_settings();
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert_eq!(trust.parts(), (true, true), "trust known off again");

    click(trust.main.cx, "session-row-b");
    click(trust.main.cx, "session-row-a");
    settle(trust.main.cx);
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert_eq!(trust.parts(), (true, false), "leaving dropped the line");

    click(trust.main.cx, DISMISS_ID);
    let save = trust.dismissals()[0].0;
    let read = trust.one_read();
    trust.refuse(save);
    trust.trust_is(read, false);
    assert_eq!(trust.parts(), (true, true), "refused again");
    click(trust.main.cx, DISMISS_ID);
    let save = trust.dismissals()[0].0;
    let read = trust.one_read();
    trust.dismissed_is(Some(true));
    trust.answer(save, 204, None);
    trust.trust_is(read, false);
    assert!(
        !trust.shows(),
        "dismissed, and the line went with the press"
    );
    trust.dismissed_is(Some(false));

    click(trust.main.cx, DISMISS_ID);
    let save = trust.dismissals()[0].0;
    let read = trust.one_read();
    click(trust.main.cx, "session-row-b");
    trust.refuse(save);
    trust.trust_is(read, false);
    assert!(!trust.shows(), "b has no band");
    click(trust.main.cx, "session-row-a");
    settle(trust.main.cx);
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert_eq!(
        trust.parts(),
        (true, false),
        "a fresh band, without the line"
    );
}

/// A Don't show again pressed while another preference save holds the row's key sends nothing, and says so in the words
/// every refused press in the app uses (the operation store's `NotStarted::BUSY_REASON` after "Not saved:", the
/// migration's orchestrator's decision of 2026-11-14), not the helm-refusal line, since nothing was saved or refused.
/// Specified: with a preference save out, Don't show again sends no dismissal and the band keeps its explanation with
/// the busy line under it.
#[gpui::test]
fn a_dont_show_again_while_another_preference_save_is_out_says_why(cx: &mut TestAppContext) {
    let mut trust = open(cx);
    trust.shown();
    trust.main.cx.update(|_, cx| {
        let operations = crate::session::current_operations(cx).expect("the premise: a UI session");
        let command = settings::auto_update_command().unwrap();
        operations
            .update(cx, |operations, cx| {
                operations.start(&OpKey::preferences(), command, cx, |_, _| {})
            })
            .expect("the premise: the key was free");
    });
    settle(trust.main.cx);
    click(trust.main.cx, DISMISS_ID);
    assert!(trust.dismissals().is_empty(), "nothing was sent");
    assert_eq!(trust.parts(), (true, true));
    let busy = super::trust_band::busy();
    assert_eq!(
        test_support::label(trust.main.cx, ERROR_ID).as_deref(),
        Some(&*busy)
    );
}

// ---- Which agent asks -----------------------------------------------------------------------------------------------

/// The flag-based agents' explanation after their lead (the old `AgentTrustBand`'s).
const FLAG: &str = "Farhelm can answer it through a command-line flag. It's off by default \
                    because repository hooks, settings and commands are then trusted.";

/// The agent the band names decides whose switch it advertises and whose dismissal it stores, so a session must never
/// borrow another agent's: the harness-neutral question flag says only that some agent asks (the old
/// `TrustAgent::for_session`). Specified (`TrustAgent::asking`): Claude's own flag names Claude whatever the launch;
/// without it, the neutral flag names Codex, Muse or Antigravity for a structured launch of that harness, and no agent for every
/// other harness (Pi, whose approval is not a recognized question, and Claude, whose question has its own flag), for a
/// command launch declaring Codex, or for a legacy session stored as Codex; with neither flag, no agent.
#[test]
fn only_a_sessions_own_launch_names_the_agent_asking() {
    let flagged = [
        (LaunchHarness::Codex, TrustAgent::Codex),
        (LaunchHarness::Muse, TrustAgent::Muse),
        (LaunchHarness::Antigravity, TrustAgent::Antigravity),
    ];
    let mut launches: Vec<(Value, Option<TrustAgent>)> = LaunchHarness::ALL
        .iter()
        .map(|harness| {
            let name = serde_json::to_value(harness).unwrap();
            let named = flagged.iter().find(|(flag, _)| flag == harness);
            (
                launched(name.as_str().unwrap()),
                named.map(|(_, agent)| *agent),
            )
        })
        .collect();
    launches.push((command_launch(), None));
    launches.push((
        json!({"kind": "command", "command": "agy", "yolo": false, "agent": "antigravity", "resume": null}),
        None,
    ));
    let legacy = json!({
        "kind": "legacy", "invocation": "codex", "agent_kind": "codex", "resume_template": null,
    });
    launches.push((legacy, None));
    for (launch, named) in launches {
        for (claude, workspace) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut fixture = session("a").launch(launch.clone());
            if claude {
                fixture = fixture.at_claude_trust_question();
            }
            if workspace {
                fixture = fixture.at_workspace_trust_question();
            }
            let row: SessionRow = serde_json::from_value(fixture.entry().value).unwrap();
            let expected = match (claude, workspace) {
                (true, _) => Some(TrustAgent::Claude),
                (false, true) => named,
                (false, false) => None,
            };
            assert_eq!(
                TrustAgent::asking(&row.info),
                expected,
                "{launch} with the Claude flag {claude} and the neutral flag {workspace}"
            );
        }
    }
}

/// SPEC.md: "Codex and Muse show the same nonmodal discovery hint ... The hint names the agent, offers Settings at its
/// switch in Agent trust and Don't show again, and has an occurrence-only ×. Each shared dismissal is independent" (the
/// old `workspace-trust.spec.ts`, "${name} trust hint stays scoped to its managed agent terminal and own dismissal", its
/// band half; the gates every agent's band shares are tested on Claude's above). Specified for `agent`,
/// with `other` the other flag-based agent: with `a` at `agent`'s question and its trust read off, the band shows the
/// expected explanation and a Settings button; Settings opens with focus on `agent`'s switch once its read
/// answers, and closing it reads `agent`'s trust again; × hides the band and stores nothing, `a` pushed at `other`'s
/// question shows `other`'s band, and `a` pushed at `agent`'s question again shows the band, a new occurrence; Don't
/// show again sends `agent`'s dismissal field alone, and with it stored the band stays away while `a` pushed at
/// `other`'s question shows `other`'s.
fn a_flag_agents_band_is_its_own(cx: &mut TestAppContext, agent: Agent, other: Agent, copy: &str) {
    let row: SessionRow = serde_json::from_value((agent.asking)("a").entry().value).unwrap();
    assert!(row.info.working_copy.is_none(), "premise: plain directory");
    let mut trust = open_as(cx, agent);
    trust.shown();
    assert_eq!(
        test_support::label(trust.main.cx, COPY_ID).as_deref(),
        Some(copy)
    );
    assert_eq!(
        test_support::label(trust.main.cx, SETTINGS_ID).as_deref(),
        Some("Settings")
    );

    click(trust.main.cx, SETTINGS_ID);
    assert!(any_open(trust.main.cx), "Settings opened");
    let settings_read = trust.one_read();
    trust.trust_is(settings_read, false);
    trust.answer_rest();
    assert_eq!(
        test_support::focused_id(trust.main.cx).as_deref(),
        Some(agent.switch)
    );
    trust.close_settings();
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(trust.shows(), "back after Settings closed");

    click(trust.main.cx, HIDE_ID);
    assert!(!trust.shows());
    assert!(trust.dismissals().is_empty(), "× stores nothing");
    trust.a_is((other.asking)("a"));
    trust.answer_rest();
    assert!(
        trust.shows(),
        "another agent's question is not the hidden one"
    );
    trust.a_is((agent.asking)("a"));
    let read = trust.one_read();
    trust.trust_is(read, false);
    assert!(trust.shows(), "a new occurrence");

    click(trust.main.cx, DISMISS_ID);
    assert!(!trust.shows(), "hidden while the save is out");
    let dismissals = trust.dismissals();
    assert_eq!(dismissals.len(), 1);
    let mut alone = json!({});
    alone[agent.field] = json!(true);
    assert_eq!(dismissals[0].1, alone);
    let read = trust.one_read();
    trust.dismissed_is(Some(true));
    trust.answer(dismissals[0].0, 204, None);
    trust.trust_is(read, false);
    assert!(!trust.shows(), "dismissed");
    trust.a_is((other.asking)("a"));
    trust.answer_rest();
    assert!(trust.shows(), "the other agent's hint is not dismissed");
}

/// Codex's band, as [`a_flag_agents_band_is_its_own`] specifies it, with Muse as the other agent.
#[gpui::test]
fn codexs_band_is_its_own(cx: &mut TestAppContext) {
    a_flag_agents_band_is_its_own(
        cx,
        CODEX,
        MUSE,
        &format!("Codex asks about trust for this folder. {FLAG}"),
    );
}

/// Muse's band, as [`a_flag_agents_band_is_its_own`] specifies it, with Codex as the other agent.
#[gpui::test]
fn muses_band_is_its_own(cx: &mut TestAppContext) {
    a_flag_agents_band_is_its_own(
        cx,
        MUSE,
        CODEX,
        &format!("Muse asks about trust for this folder. {FLAG}"),
    );
}

/// An ordinary directory is enough to show Antigravity's hint. Its Settings action focuses the Antigravity switch,
/// × lasts for the current question, and the saved dismissal leaves Codex's hint available.
#[gpui::test]
fn antigravitys_band_shows_in_an_ordinary_directory(cx: &mut TestAppContext) {
    a_flag_agents_band_is_its_own(
        cx,
        ANTIGRAVITY,
        CODEX,
        "Antigravity asks whether to trust this folder. Farhelm can skip this question for plain folders and managed \
         checkouts. It's off by default because repository hooks, settings and commands are then trusted.",
    );
}

/// SPEC.md: "Pi has a switch but no hint band", and a session whose launch names no harness must not borrow Codex's
/// switch or dismissal from the neutral question flag (the old `workspace-trust.spec.ts`, "Pi and command launches do
/// not borrow a workspace trust hint"). Specified: with `a` a Pi launch and `b` a command launch declaring Codex, both
/// at the workspace-trust question in a managed checkout, opening either sends no read of any agent's trust and `a`,
/// whose agent terminal is live, shows no band; `b` pushed as the fixture's Codex launch at the same question then
/// sends a read, so the silence was the launch's doing.
#[gpui::test]
fn pi_and_command_launches_borrow_no_band(cx: &mut TestAppContext) {
    let pi = session("a")
        .launch(launched("pi"))
        .at_workspace_trust_question();
    let command = session("b")
        .launch(command_launch())
        .at_workspace_trust_question();
    let mut main = test_support::open_main(cx, snapshot(&[(pi, 0), (command, 0)], false));
    main.terminal_live();
    assert!(!main.helm.commands().iter().any(any_trust_read));
    // A consistency check only: no read was answered, so trust stays unknown and no band would show for any agent. The
    // read assertions are what tell the mechanisms apart.
    let band = frame(main.cx)
        .into_iter()
        .find(|node| node.element_id.as_deref() == Some(BAND_ID));
    assert!(band.is_none(), "no band for Pi");

    click(main.cx, "session-row-b");
    settle(main.cx);
    assert!(!main.helm.commands().iter().any(any_trust_read));
    push(
        &mut main.helm,
        &["a", "b"],
        &[(session("b").at_workspace_trust_question(), 0)],
        main.cx,
    );
    let reads: Vec<String> = main
        .helm
        .commands()
        .into_iter()
        .filter(any_trust_read)
        .map(|command| command.path)
        .collect();
    assert_eq!(reads, [CODEX.route], "the premise: a Codex launch reads");
}

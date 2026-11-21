//! The trust hint band above the agent terminal offers Settings while a workspace-trust question is visible. Claude,
//! Codex and Muse show it in managed checkouts. Antigravity also shows it in ordinary directories.
//!
//! SPEC.md, "Managed checkouts" (the paragraphs that start "While Claude's trust question is visible" and "Codex and
//! Muse show the same nonmodal discovery hint"). Ported from the old app's `ClaudeTrustBand` (`claude_trust.rs`) and
//! `AgentTrustBand` (`workspace_trust.rs`) at reference commit `24bc0580`, with their words, buttons, hover help and
//! failure line, and their shared look from `.claude-trust-band` in `app.css`. The orchestrator assigned the band to the
//! session view on 2026-11-14, and decided on 2026-11-16 that Codex's and Muse's are built as this one. Here's the TLDR
//! of its rules:
//!
//! - **When it shows** ([`Facts::shows`]): the agent's question is on the open session's agent terminal in a directory
//!   eligible for its trust setting, that trust is known off, its hint is not dismissed, and × has not hidden this
//!   occurrence.
//! - **Which agent asks** ([`TrustAgent::asking`]): Claude by its own flag, Codex, Muse and Antigravity by the harness-neutral flag
//!   and the session's structured launch. The band shows one agent's note at a time and takes its read, its Settings
//!   entry point, its dismissal and its words ([`TrustAgent::band_words`]) from that agent; it never branches on the
//!   agent itself.
//! - **What it reads, and when.** The question and the dismissal are pushed and read while drawing. The agent's trust
//!   is not pushed, so the band reads its route ([`TrustAgent::read`]) when the open session reaches the question (or
//!   another agent asks there), when any Settings dialog closes ([`settings::Closings`]), when Don't show again is
//!   pressed (the old corrective read), and on the ticker's 30-second clock while the band shows
//!   ([`TrustBand::tick_while`]), so a change made elsewhere reaches the band within one tick, as the old band's refresh
//!   brought it (the orchestrator's decision of 2026-11-16; the old refresh followed the session feed and named no
//!   interval, so the tick is the ticker's age clock). A tick sends nothing while a read is still out ([`Known::out`]),
//!   so a slow helm gets no pile of reads. The helm answers in the order it was asked, so the last answer is the newest;
//!   an answer that arrives while a newer read is still out is dropped, so a stale off does not show in that gap
//!   ([`TrustBand::reads`]).
//! - **× and Don't show again**: [`TrustBand::hidden`] and [`TrustBand::hide_everywhere`].
//!
//! NOTE: Three deviations, recorded in the parity file
//! (`docs/gpui/parity/sidebar-and-session-view.md`, "Deviations from the old app"): no band under the unavailable
//! card; only the stored dismissal shows, never a Settings switch's pending one; and a Don't show again pressed while
//! another preference save is out is refused with the shared busy line, where the old app queued it.

use std::collections::HashSet;

use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, ParentElement as _, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Subscription, WeakEntity, div,
    prelude::FluentBuilder as _, px,
};

use crate::components::{Button, HoverHelp, Tier};
use crate::focus::Mark;
use crate::local_state::{self, LocalState};
use crate::operations::{NotStarted, OpKey, Outcome};
use crate::session::{self, ObservingCurrent, PerSession};
use crate::settings::{self, BandWords, TrustAgent};
use crate::theme::{self, metrics};
use crate::ticker;

// The element ids tests and the test bridge find the band and its parts by, whichever agent asks (named for Claude's
// band, which came first): the explanation, Settings, Don't show again, ×, and the failure line of a refused Don't show
// again.
pub(crate) const BAND_ID: &str = "claude-trust-band";
pub(crate) const COPY_ID: &str = "claude-trust-band-copy";
pub(crate) const SETTINGS_ID: &str = "claude-trust-band-settings";
pub(crate) const DISMISS_ID: &str = "claude-trust-band-dismiss";
pub(crate) const HIDE_ID: &str = "claude-trust-band-hide";
pub(crate) const ERROR_ID: &str = "claude-trust-band-error";

/// The failure line's words after the helm refused Don't show again (a press refused because another preference save
/// was out says [`busy`] instead).
pub(crate) const ERROR: &str = "Could not save hiding this trust hint. Try again.";

/// `.claude-trust-band`: 10 by 14 pixels of padding, 12-pixel text at `line-height: 1.5`, the actions 8 pixels apart and
/// 8 below the explanation, and the 3-pixel info rule down the left edge (an inset `box-shadow` there).
const PAD: (f32, f32) = (10., 14.);
const LINE_HEIGHT: f32 = 18.;
const ACTIONS_GAP: f32 = 8.;
const RULE: f32 = 3.;

// ---- When the band shows --------------------------------------------------------------------------------------------

/// What decides the band for the open session, each read where it lives: the pushed row, the drawn terminal, the read
/// of the asking agent's trust (`None` until one answers, or when it failed), the pushed dismissal of that agent's hint
/// (or the band's own save of it in flight), and the occurrence's ×.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Facts {
    pub(crate) question: bool,
    pub(crate) eligible: bool,
    pub(crate) agent_shown: bool,
    pub(crate) trust: Option<bool>,
    pub(crate) dismissed: bool,
    pub(crate) hidden: bool,
}

impl Facts {
    /// The explanation and its buttons: every condition holds, trust read as off (the old `show_hint`).
    pub(crate) fn shows(self) -> bool {
        self.question
            && self.eligible
            && self.agent_shown
            && self.trust == Some(false)
            && !self.dismissed
            && !self.hidden
    }

    /// The failure line of a refused Don't show again, which `failed` says the open session has: while the question is
    /// up in its agent terminal and the occurrence is not hidden, unless trust is known to be on. An unknown trust keeps
    /// the line, without the explanation, which needs trust known off (the old `error_visible`).
    pub(crate) fn shows_failure(self, failed: bool) -> bool {
        failed
            && self.question
            && self.eligible
            && self.agent_shown
            && !self.hidden
            && self.trust != Some(true)
    }
}

/// Read the asking agent's setting only for a reachable session in that
/// agent's supported folder scope. A stale row carries its last known screen.
fn at_question(row: &SessionRow) -> Option<TrustAgent> {
    TrustAgent::asking(&row.info).filter(|&agent| eligible(agent, row) && !row.stale)
}

/// The same folder rule governs both the setting read and the painted hint.
fn eligible(agent: TrustAgent, row: &SessionRow) -> bool {
    !agent.managed_checkouts_only() || row.info.working_copy.is_some()
}

// ---- State ----------------------------------------------------------------------------------------------------------

/// What the band knows for one UI session. A reset starts it over: its reads and its save die with the session.
#[derive(Default)]
struct Known {
    /// The open session the trust read is for, while it is at a question ([`at_question`]), and the agent asking there,
    /// whose trust is read.
    read_for: Option<(String, TrustAgent)>,
    /// The last answer for `read_for`: `Some` when the helm said, `None` before any answer or after a failed one.
    trust: Option<bool>,
    /// The agent whose hint the band's Don't show again is storing, while the save is in flight. The preference row's
    /// key admits one save at a time, so there is never more than one.
    hiding: Option<TrustAgent>,
    /// The session whose Don't show again was refused while it was open, with the line's words: [`ERROR`] for the
    /// helm's refusal, [`busy`] for a press the operation store refused because another preference save was out.
    /// Dropped when another session opens.
    failed: Option<(String, SharedString)>,
    /// How many trust reads are out (sent, not yet answered), whether or not their answers will apply. A tick reads
    /// only while none is.
    out: u32,
}

/// The line for a Don't show again the operation store refused because another preference save was out: the band's
/// lead-in and the store's shared reason, as every area words such a press (the orchestrator's decision of
/// 2026-11-14, [`NotStarted::BUSY_REASON`]).
pub(crate) fn busy() -> SharedString {
    format!("Not saved: {}", NotStarted::BUSY_REASON).into()
}

/// The band's state, held by the session pane for the window's life.
pub(crate) struct TrustBand {
    known: PerSession<Known>,
    /// The sessions whose current occurrence the user hid with ×, each with the agent that asked (SPEC.md: "× hides
    /// only the current occurrence, until the question leaves the screen"). Kept for the window's life, as the old page
    /// kept it, across switches between sessions, and cleared only by a pushed row showing that agent's question gone,
    /// another agent asking there included (the old `OccurrenceDismissals`): a row the store does not hold (a cut or
    /// filtered list) is not that evidence.
    hidden: HashSet<(String, TrustAgent)>,
    /// Counts the reads sent and the changes of the session (or agent) read for, so an answer applies only if neither
    /// happened since its read was sent. Outside [`Known`] so it never starts over.
    reads: u64,
    /// The Settings button's focus handle: where focus returns when the Settings it opened closes.
    settings: FocusHandle,
    /// Holds the ticker's clock while the band shows ([`TrustBand::tick_while`]); the band observes it.
    tick: Entity<Tick>,
    _store: ObservingCurrent,
    _observing: Vec<Subscription>,
}

impl TrustBand {
    /// The band's state, following the current store, the selection and Settings' closings from the next update on.
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let tick = cx.new(|_| Tick(None));
        TrustBand {
            known: PerSession::default(),
            hidden: HashSet::new(),
            reads: 0,
            settings: cx.focus_handle().tab_stop(true),
            _store: session::observe_current(cx, |band: &mut Self, cx| band.follow(cx)),
            _observing: vec![
                cx.observe_global::<LocalState>(|band, cx| band.follow(cx)),
                // Settings closed, and the user may have changed the agent's trust there.
                cx.observe_global::<settings::Closings>(|band, cx| band.read(cx)),
                // A tick while the band shows: someone may have changed the agent's trust elsewhere.
                cx.observe(&tick, |band, _, cx| {
                    if band.known.current(cx).out == 0 {
                        band.read(cx);
                    }
                }),
            ],
            tick,
        }
    }

    /// Hold the ticker's 30-second clock exactly while `shown`, which the pane works out in each frame from whether it
    /// drew the band, as it takes and drops its other clocks. Taking or dropping the clock notifies nothing,
    /// so the frame that calls this does not draw again because of it.
    pub(crate) fn tick_while(&mut self, shown: bool, cx: &mut Context<Self>) {
        self.tick.update(cx, |tick, cx| {
            if tick.0.is_some() != shown {
                tick.0 = shown.then(|| ticker::clock(cx));
            }
        });
    }

    /// Bring the band up to the store and the selection: end the occurrences whose question a pushed row shows gone,
    /// drop a failure line that belongs to a session no longer open, and read trust when the open session reaches a
    /// question.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let store = session::current_store(cx);
        // The agent the store's row for `key` shows asking, and the one at the question in the band's sense.
        let question = |key: &str, cx: &App| {
            let row = store
                .as_ref()?
                .read(cx)
                .collections()
                .get::<SessionRow>(key)?;
            Some((TrustAgent::asking(&row.info), at_question(row)))
        };
        let before = self.hidden.len();
        self.hidden.retain(|(key, agent)| {
            question(key, cx).is_none_or(|(asking, _)| asking == Some(*agent))
        });
        let mut changed = self.hidden.len() != before;
        let open = local_state::selected(cx);
        let wanted = open.clone().and_then(|key| {
            let agent = question(&key, cx)?.1?;
            Some((key, agent))
        });
        let known = self.known.current(cx);
        if known
            .failed
            .as_ref()
            .is_some_and(|(key, _)| Some(key) != open.as_ref())
        {
            known.failed = None;
            changed = true;
        }
        if known.read_for != wanted {
            known.read_for = wanted;
            known.trust = None;
            changed = true;
            // Retires the old session's read, if one is out, and sends the new one's.
            self.reads += 1;
            self.read(cx);
        }
        // Only on a change: this runs on every push, and the pane redraws whenever the band notifies.
        if changed {
            cx.notify();
        }
    }

    /// Send a read of the asking agent's trust for the session the band reads for, nothing while it reads for none.
    /// The answer applies only if [`TrustBand::reads`] has not moved since.
    fn read(&mut self, cx: &mut Context<Self>) {
        let Some((_, agent)) = self.known.current(cx).read_for.clone() else {
            return;
        };
        self.reads += 1;
        self.known.current(cx).out += 1;
        let sent = self.reads;
        let band = cx.weak_entity();
        agent.read(cx, move |answer, cx| {
            let _ = band.update(cx, |band, cx| {
                let applies = band.reads == sent;
                let known = band.known.current(cx);
                known.out = known.out.saturating_sub(1);
                if applies {
                    known.trust = answer.ok();
                    cx.notify();
                }
            });
        });
    }

    /// Don't show again for the open session, whose `agent` asks: store that agent's shared dismissal through the
    /// operation store under the preference row's key, as its Settings switch does, and read trust again. The band
    /// hides while the save is out and then follows the stored value. A refusal, or a press refused because another
    /// preference save holds the key, shows its line while the session pressed for stays open ([`Known::failed`]);
    /// while trust is not known off (the corrective read failed), only that line shows ([`Facts::shows_failure`]).
    fn hide_everywhere(&mut self, agent: TrustAgent, cx: &mut Context<Self>) {
        let Some(pressed) = local_state::selected(cx) else {
            return;
        };
        let Some(operations) = session::current_operations(cx) else {
            return;
        };
        let command = match settings::hide_trust_hint_command(agent) {
            Some(Ok(command)) => command,
            Some(Err(error)) => {
                tracing::debug!(?error, "the trust hint's dismissal not built");
                return;
            }
            // An agent without a hint has no band, so no button to press.
            None => return,
        };
        self.known.current(cx).failed = None;
        let band = cx.weak_entity();
        let settling = pressed.clone();
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, move |settled, cx| {
                let refused = matches!(settled.outcome, Outcome::Refused(_));
                let _ = band.update(cx, |band, cx| band.hide_settled(&settling, refused, cx));
            })
        });
        match started {
            Ok(()) => {
                self.known.current(cx).hiding = Some(agent);
                self.read(cx);
            }
            Err(NotStarted::Busy(_)) => {
                self.known.current(cx).failed = Some((pressed, busy()));
            }
            Err(not_started) => {
                tracing::debug!(?not_started, "the trust hint's dismissal not sent")
            }
        }
        cx.notify();
    }

    /// Don't show again, pressed for `pressed`, settled: a refusal shows only while `pressed` is still open.
    fn hide_settled(&mut self, pressed: &str, refused: bool, cx: &mut Context<Self>) {
        let open = local_state::selected(cx);
        let known = self.known.current(cx);
        known.hiding = None;
        if refused && open.as_deref() == Some(pressed) {
            known.failed = Some((pressed.to_owned(), ERROR.into()));
        }
        cx.notify();
    }
}

/// The holder of the band's [`ticker::Clock`]: the clock notifies the view that holds it, and the band reads on each
/// notification, so the clock gets an entity of its own rather than notifying the band, whose every change would then
/// look like a tick.
struct Tick(Option<ticker::Clock>);

// ---- Drawing --------------------------------------------------------------------------------------------------------

/// The band for the open session filed under `key`, drawn from `row` (the store's) and `agent_shown` (whether its agent
/// terminal is the one drawn, live: no tab chosen, and no card over it or in its place), or `None` when it shows
/// nothing.
pub(crate) fn draw(
    band: &Entity<TrustBand>,
    key: &str,
    row: &SessionRow,
    agent_shown: bool,
    cx: &App,
) -> Option<AnyElement> {
    // Every fact below is about the asking agent's question; while no agent with a hint asks, no question is up.
    let agent = TrustAgent::asking(&row.info)?;
    let words = agent.band_words()?;
    let state = band.read(cx);
    let known = state.known.read(cx);
    let facts = Facts {
        question: true,
        eligible: eligible(agent, row),
        agent_shown,
        trust: known.and_then(|known| known.trust),
        dismissed: !settings::trust_hint_shown(agent, cx)
            || known.is_some_and(|known| known.hiding == Some(agent)),
        hidden: state.hidden.contains(&(key.to_owned(), agent)),
    };
    let failed = known
        .and_then(|known| known.failed.as_ref())
        .filter(|(failed, _)| failed.as_str() == key)
        .map(|(_, words)| words.clone());
    let (shows, failure) = (facts.shows(), facts.shows_failure(failed.is_some()));
    if !shows && !failure {
        return None;
    }
    let palette = theme::palette(cx);
    let weak = band.downgrade();
    Some(
        div()
            .id(BAND_ID)
            .debug_selector(|| BAND_ID.into())
            // Announced as it appears, without taking focus (SPEC.md: "is announced without taking focus").
            .role(Role::Status)
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .py(px(PAD.0))
            .px(px(PAD.1))
            .border_b_1()
            .border_color(theme::color(palette.border))
            .bg(theme::color(palette.bg_2))
            .text_color(theme::color(palette.fg_0))
            .text_size(metrics::TEXT_SM)
            .line_height(px(LINE_HEIGHT))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(px(RULE))
                    .bg(theme::color(palette.info)),
            )
            .when(shows, |band| {
                band.child(copy(&words, palette))
                    .child(actions(&weak, key, agent, &words, state))
            })
            .children(failed.filter(|_| failure).map(|words| {
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.into())
                    .role(Role::Alert)
                    .aria_label(words.clone())
                    .when(shows, |line| line.mt(px(ACTIONS_GAP)))
                    .text_color(theme::color(palette.danger))
                    .child(words)
            }))
            .into_any_element(),
    )
}

/// The explanation, its lead in bold info ink (`.claude-trust-band-copy strong`).
fn copy(words: &BandWords, palette: &theme::Palette) -> impl IntoElement {
    let lead = HighlightStyle {
        color: Some(theme::color(palette.info)),
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let text = format!("{}{}", words.lead, words.rest);
    div()
        .id(COPY_ID)
        .debug_selector(|| COPY_ID.into())
        .role(Role::Label)
        .aria_label(SharedString::from(text.clone()))
        .child(StyledText::new(text).with_highlights([(0..words.lead.len(), lead)]))
}

/// Settings, Don't show again and ×, each acting through `band` for `agent`'s question in the session filed under
/// `key`.
fn actions(
    band: &WeakEntity<TrustBand>,
    key: &str,
    agent: TrustAgent,
    words: &BandWords,
    state: &TrustBand,
) -> impl IntoElement {
    let opener = state.settings.clone();
    let settings = Button::new(
        SETTINGS_ID,
        words.settings,
        HoverHelp::tooltip(words.settings_help).below(),
        Role::Button,
    )
    .tier(Tier::Primary)
    .focus(&state.settings)
    .on_click(move |_, window, cx| {
        let mark = Mark::take(cx);
        settings::open_at_agent_trust(agent, &opener, &mark, window, cx);
    });
    let dismissing = band.clone();
    let dismiss = Button::new(
        DISMISS_ID,
        "Don't show again",
        HoverHelp::tooltip(words.dismiss_help).below(),
        Role::Button,
    )
    .tier(Tier::Neutral)
    .on_click(move |_, _, cx| {
        let _ = dismissing.update(cx, |band, cx| band.hide_everywhere(agent, cx));
    });
    let (hiding, occurrence) = (band.clone(), key.to_owned());
    let hide = Button::new(
        HIDE_ID,
        "×",
        HoverHelp::tooltip("Hide this note for this question only").below(),
        Role::Button,
    )
    .accessible_name("Hide this trust hint for this occurrence")
    .tier(Tier::Neutral)
    .on_click(move |_, _, cx| {
        let _ = hiding.update(cx, |band, cx| {
            band.hidden.insert((occurrence.clone(), agent));
            cx.notify();
        });
    });
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(ACTIONS_GAP))
        .mt(px(ACTIONS_GAP))
        .child(settings)
        .child(dismiss)
        .child(hide)
}

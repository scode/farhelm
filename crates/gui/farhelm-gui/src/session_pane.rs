//! The main pane beside the session list: the selected session's top row and live terminal, or what the window says
//! with no session open.
//!
//! SPEC.md, "Session view": the agent's real TUI is live. The pane owns the selected session's
//! terminals, the agent terminal and one per shell tab (the terminal area's [`TerminalSet`], which draws whichever the
//! user chose), and draws the session's top row above them ([`header`]: the lifecycle actions, and the left region
//! holding the set's tab strip and the open-tunnels pill), with the lines a long Restart or Replace, a refusal and a
//! header Replace's YOLO question add between the two, and the details bar below it ([`details`]: the session's status,
//! title, host, age, managed checkout, directory and command line). Neither row keeps anything that belongs to a
//! session's state: both read the store and the operation store while they draw. The bar's copy fields' brief
//! confirmations are the one thing kept, with the mounted terminals, so they go when the selection does.
//!
//! - **One set of terminals, remade on every change of selection.** Choosing another session drops the set (which ends
//!   every one of its streams, tabs included) and makes a new one with a fresh attachment lease. This is the old app's
//!   keyed remount (its `SessionView` keyed by the session's id, so a switch tore the old view down and mounted a new
//!   one, which attached again), not the GPUI prototype's deck of terminals kept attached across selections, which the
//!   old app never had. A terminal is made for whichever session is selected, listed or not, because a selection the
//!   list cannot show (past a cut listing, say) is still the open session.
//! - **One UI session.** The terminal is made with the current UI session's opener and retires with that session. The
//!   pane keeps it in [`PerSession`], which compares the current UI session with the one it was made under each time
//!   the pane reaches it, drawing included, so a terminal made for an earlier connection is never drawn or focused. The
//!   session that a reset rebuilds gets a new terminal set for the selection that survived.
//! - **Focus.** The shown terminal is home (SPEC.md, "Where focus rests"). When the pane makes the set, home is the
//!   remembered tab if it can be shown, or the agent terminal. It follows each choice in the strip and returns to
//!   the agent terminal when a chosen tab goes.
//!   Each time, the pane tells the focus coordinator, which gives it focus only if nothing else holds it. That covers
//!   the automatic choices: the session opened at startup, and the next session after the open one went away, from
//!   the command line or an agent included. When the user chooses a session ([`chosen`]), its terminal also takes
//!   focus, from whatever holds it, through [`focus::choose_terminal`] with the mark of the click that chose it.
//! - **A Delete of the open session.** From the moment its request is sent until the helm answers, the top row shows
//!   what the row says in place of its actions ([`header`]), a scrim over the terminal says it again (stopping the
//!   agent, stopping its terminal tabs, or deleting), and the terminal's own notices are held back
//!   ([`AttachedTerminal::hold_notices`](crate::terminal::attached::AttachedTerminal::hold_notices)): the Delete kills
//!   the agent and detaches the viewer, so the exit line ("Process ended by signal 15") and the reconnect card
//!   ("Session unavailable — reconnecting") would otherwise read as failures while the Delete is going as asked.
//!   SPEC.md, "Lifecycle operations": "a scrim over its terminal says it again. The terminal's own 'Detached' notice,
//!   which the delete itself causes, is held back meanwhile; if the delete fails, the row and header come back with the
//!   refusal shown, and a held notice appears." Both the scrim and the hold ask the operation store while they draw
//!   ([`delete::in_flight`]), so neither keeps a copy that could outlive the request. A refusal therefore takes the
//!   scrim away and lets the terminal show whatever it has to say by then, while the row shows the refusal. A Delete
//!   that succeeds removes the session, and the selection rules take the pane from there. Ported from the old app's
//!   `.terminal-delete-overlay` (`session_view.rs` and app.css at the reference commit) and its held banner
//!   (`showBanner` in terminal.js), which held every banner the delete caused, not only "Detached".
//!
//!   The scrim draws no hitbox and tracks no focus, so the terminal under it keeps focus, keys and clicks: the old
//!   overlay took no pointer events either, so that a click is never eaten by something on its way out. The Delete
//!   holds the session's other operations meanwhile anyway.
//! - **A Restart of the open session.** While this window's Restart of the session (or its Restart with, which runs as
//!   the same work) is in flight, the terminal's reconnect card is held back, and nothing else is. A Restart stops the
//!   agent, and the supervisor moves the viewer to the relaunched terminal with a detach that reconnects, so the card
//!   would announce an outage the user just asked for; the old app's `terminal-restart.spec.ts` ("a restarted session
//!   reconnects without a detach banner") already ruled out even a brief red banner there. Holding the card for the
//!   Restart, and showing it if the reattach fails, was decided by the migration's orchestrator on 2026-11-15. The
//!   pane's hold is the Delete's ([`restart::in_flight`] in place of [`delete::in_flight`]), asked while drawing, so it
//!   keeps no copy that could outlive the request; but the helm answers a Restart while the stream still waits to
//!   reattach, so the terminal keeps holding the card for a recovery that began under the hold until that recovery
//!   ends or its first attempt fails ([`AttachedTerminal`](crate::terminal::attached::AttachedTerminal)'s
//!   `recovery_held`). A stall's or refusal's band and the exit line are not held: the decision named only the card,
//!   and only the agent's terminal is held, since a Restart detaches no tab.
//! - **A Suspend of the open session's host** from this window holds back the reconnect card too (decided by this area
//!   on 2026-11-16, extending the orchestrator's 2026-11-15 Restart hold): it ends the stream at once, but the helm
//!   pushes the window nothing until its command answers, so the row reads fresh and the card would show for the whole
//!   Suspend. Not held: a shown tab's card (the set holds only the agent's), and another session's row's Suspend.
//!
//! The pane acts from observers (the selection, the user's choices, the connection), never from drawing: focus moves
//! made while a frame is drawn are not reliably drawn themselves, and observers also run for a window that is not drawn
//! at all. Only the scrim's spinner handle is taken and dropped while drawing, as the session list's is, since it
//! matters only to frames. A terminal made in a hidden window opens its stream only once it is drawn and measured, as
//! any terminal does.
//!
//! With no session open, the pane shows what the old app showed then (its `.main-empty`), with the old rule for it:
//! three honest states, not one claim. It says "No sessions — create one" only once the helm has shown that there are
//! none ([`crate::session_list::empty_fleet`]); before the session list has been read it says "Loading sessions…"; and
//! when there are sessions it is blank, because the session list is about to open one. On the same proof, and only
//! then, the session list's New button is home (SPEC.md, "Where focus rests": "focus rests on New when there are no
//! sessions"): the pane holds New's focus handle for that, and names it home whenever that proof holds and it has
//! nothing mounted.
//!
//! - **The interrupted card.** A session whose agent a host reboot or a Farhelm update ended has no terminal left, so
//!   the pane mounts none for it and draws the interrupted card ([`card`]) where the terminals would be, between the
//!   top row and the details bar, with the card as home. That is the one thing the pane learns from the store rather
//!   than from the selection and the connection: when the open session goes into its card the pane drops its
//!   terminals (their streams with them), and when it comes out (restarted, from here or anywhere) it mounts new ones
//!   under a new lease, as a fresh view would ([`card::Follow`] tells it when).
//! - **The unavailable card.** A session whose host the helm cannot reach shows the unavailable card ([`unavailable`])
//!   over the area the terminals take, with its last-known status and age in a band above that area. Stale beats
//!   interrupted, so such a session never shows the interrupted card. The terminals the pane already shows stay, inert
//!   beneath the card with their notices held back, and come back to life when the host answers; a session opened while
//!   its host is away gets none until then. While the card shows it is home, and an inert terminal holds no focus, so
//!   focus one of the session's terminals held goes home to the card through the focus coordinator. For a suspended or
//!   stopped host the card offers Resume, and a session's own suspended environment reads "Session suspended" in its
//!   provider's words (`unavailable.rs`, "A suspended or stopped host offers Resume"). When the host answers, retained
//!   terminals skip what is left of their reconnect wait (up to 30 seconds), as the old app's did when the session's
//!   `unavailable` flag cleared (`terminal.js` `sync`).
//! - **The retired card.** A retired containerized session supports Delete only and has no terminal to attach to, so
//!   the pane mounts none for it, ever, and draws the retired card ([`retired`]) where the terminals would be, with the
//!   card as home, while its host is reachable; the unavailable card stands there alone while it is not. A set made
//!   before the session's row arrived is dropped once the row says it is retired.
//! - **The trust hint band** ([`trust_band`]), between the top row's lines and the terminal, taking height from it,
//!   while the session's agent (Claude, Codex or Muse) asks whether to trust a managed checkout. Its state is an entity
//!   the pane keeps for the window's life: it reads trust, which the helm does not push, and remembers hidden
//!   occurrences across session switches.
//!
//! NOTE: The scrim covers whichever terminal is shown, as the old one covered every pane; a Delete holds back the
//! notices of all of them.

pub(crate) mod card;
pub(crate) mod details;
pub(crate) mod header;
mod hook_band;
#[cfg(test)]
pub(crate) use hook_band::golden as hook_band_golden;
#[cfg(test)]
pub(crate) use trust_band::golden as trust_band_golden;
mod retired;
pub(crate) mod trust_band;
pub(crate) mod unavailable;

#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod carried_tests;
#[cfg(test)]
mod details_tests;
#[cfg(test)]
mod environment_tests;
#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod retired_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod trust_band_tests;
#[cfg(test)]
mod unavailable_tests;

use farhelm_helm_client::Collections;
use farhelm_proto::ui_channel::{Collection, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, Global,
    InteractiveElement as _, IntoElement, ParentElement, Render, Role, ScrollHandle, Stateful,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div,
};

use crate::components::{Spinner, status_card, status_heading};
use crate::desktop::title_band::{self, Row};
use crate::focus::{self, Mark};
use crate::local_state::{self, LocalState};
use crate::local_time;
use crate::session::{self, PerSession, UiSession};
use crate::session_list::{age, delete, environment, restart};
use std::rc::Rc;

use crate::terminal::CellMetrics;
use crate::terminal::attached::Hold;
use crate::terminal::set::{SharedHold, TerminalSet};
use crate::{session_list, theme, ticker};

/// The user's latest choice of a session, kept until the pane has acted on it.
///
/// The session list records a choice when the user clicks a row, but the terminal that is to take focus does not exist
/// yet then: the pane makes it once it sees the selection change, after the click's handler has returned. So the click
/// leaves its session and mark here, and the pane picks them up.
///
/// NOTE: This is app-wide while focus is per window, so it assumes one window. With two windows each pane would act on
/// a click made in the other, whose mark has not moved on in that window, and pull focus off a control there. The app
/// opens one window today, and the selection itself ([`LocalState`]) is app-wide too. Supporting more would mean
/// recording the clicked window with the choice; the row's click handler has it.
#[derive(Default)]
struct Chosen {
    /// Counts the choices made, so the pane acts on each exactly once. A choice of the session already open changes
    /// nothing else the pane could notice.
    serial: u64,
    /// The latest choice: the session's key and the mark taken when the user made it.
    latest: Option<(String, Mark)>,
}

impl Global for Chosen {}

/// The user chose the session filed under `key`, with `mark` taken at the click that chose it: its terminal takes focus
/// once the pane has it, unless the user has moved on since `mark` (SPEC.md: "Choosing a session, including the one
/// already selected, focuses its terminal"). The session list calls this beside selecting the session; automatic
/// selections do not, so their terminal only becomes home.
pub(crate) fn chosen(key: &str, mark: Mark, cx: &mut App) {
    let chosen = cx.default_global::<Chosen>();
    chosen.serial += 1;
    chosen.latest = Some((key.to_owned(), mark));
}

/// The main pane. The main view holds one for the window's life.
///
/// Like the session list, it must be made after the connection ([`session::start`]): it observes the connection it
/// finds when it is made.
pub struct SessionPane {
    /// The selected session's terminal under the current UI session, `None` with no session open (or while the window is
    /// inert). Reached only through [`PerSession::current`], so a reset drops it.
    mounted: PerSession<Option<Mounted>>,
    /// The serial of the last choice acted on ([`Chosen`]).
    handled: u64,
    /// The pane's spinner, held exactly while the scrim, the Stop button's "Stopping…", a long Restart's or Replace's
    /// line or the unavailable card shows in an active window, and worked out at the top of each frame as the session
    /// list works out its own: everything that decides it (the operation store, the shared clocks, the selection, the
    /// window's activation) redraws the window when it changes.
    spinner: Option<Spinner>,
    /// Where the top row's buttons were last drawn, and the question it opened ([`header::Header`]).
    header: header::Header,
    /// The pulse's ticks for the details bar's running mark, held exactly while the bar shows one in an active window,
    /// and worked out at the top of each frame like `spinner`.
    pulse: Option<ticker::Pulsing>,
    /// The ticker's 30-second clock, held exactly while the details bar or the stale band shows an age, so the age
    /// moves on with the rows' ages and no timer of the pane's own (SPEC_impl.md, "Animation time: the ticker"). Worked
    /// out at the top of each frame like `spinner`.
    clock: Option<ticker::Clock>,
    /// Tells the pane when the open session's recovery card changes or its host comes into or out of reach,
    /// and when the fleet becomes empty or stops being so ([`card::Follow`]).
    _card: Entity<card::Follow>,
    /// The session list's New button's focus handle ([`crate::session_list::SessionList::new_focus`]), home while no
    /// session is open and there are no sessions (SPEC.md, "Where focus rests"). It is safe as home while New is not
    /// drawn (disabled, or hidden by the Hosts panel): the focus coordinator counts an undrawn home as none
    /// (`focus::drawn_home`).
    ///
    /// NOTE: A first launch over an empty fleet ends on the new session's terminal only because the helm pushes a
    /// create's new session before its reply: that push opens the session while New is still disabled, so New is never
    /// a drawn home in between. A create that replies "accepted" and pushes its outcome later (the coming sandbox and
    /// container creates) must open its session from that push. Otherwise the reply re-enables New while the fleet
    /// still reads empty, focus rests on New, and the session's later automatic open does not take it from there.
    new: FocusHandle,
    /// The trust hint band's state ([`trust_band`]), drawn above the open session's terminal while it applies.
    trust_band: Entity<trust_band::TrustBand>,
    hook_band: Entity<hook_band::HookBand>,
    /// Act on a change of the selection, of the user's choice, of the connection and of the open session's card, and
    /// redraw when the trust hint band's state changes.
    _observing: Vec<Subscription>,
}

/// What the pane keeps for the open session: the session's key, the terminals it shows (the session's agent terminal
/// and its tabs) unless the session shows its interrupted card or opened on an unreachable host, the cards' focus
/// handle, the unavailable card's scroll state, and the details bar's copy confirmations. All of it is made and dropped
/// with the selection (the old view's keyed remount reset it the same way); the terminals also go when the session goes
/// into or out of its interrupted card. The Replace question the interrupted card asked is kept by the card
/// ([`card::keep_question`]), since the row menu reads it too, and `sync` drops it when the selection changes and
/// whenever the card does not show, the unavailable card in its place included ([`card::drop_question`]).
struct Mounted {
    key: String,
    /// `None` while the session shows its interrupted card, while a session opened on an unreachable host waits for it,
    /// or when the terminal font could not be measured.
    set: Option<Entity<TerminalSet>>,
    /// Keeps home on the set's shown terminal as the user chooses tabs and tabs come and go.
    _home: Option<Subscription>,
    copies: details::Copies,
    /// The focus handle of whichever card shows, the interrupted card, the retired card or the unavailable one, a tab
    /// stop: home while one shows. One handle for all three, since a session never shows two, and a card that turns
    /// into another (its host dropping away) keeps focus on the card that replaces it.
    card_focus: FocusHandle,
    /// The unavailable card's scroll state, so its region stays where the user scrolled it from frame to frame.
    scroll: ScrollHandle,
    /// The row the interrupted card was last drawn from, kept only to go on drawing the card while the store holds no
    /// row for the session at all: between the helm removing a confirmed Replace's source and its reply, the selection
    /// stays on the source (`session_list::selection`), and the card stays up with the Replace's wait line until the
    /// reply opens the replacement, as the old view, which kept its own copy of the session, did. Never drawn while the
    /// store has a row, so it cannot show anything the store contradicts. Kept with the key it was read under and drawn
    /// only for that key. A `Mounted` belongs to one session anyway (`sync` makes a new one whenever the selection's key
    /// changes, and `render` draws the key of the `Mounted` it holds), so the key only states that in the value, and
    /// `render` asserts it in debug builds.
    card_row: Option<(String, SessionRow)>,
    /// The row's stale reading at the last `sync`, kept per open session so a session switch never reads as recovery.
    stale: bool,
}

impl Mounted {
    /// What the pane keeps for the session filed under `key` under `session`, with its terminals when `terminal` says
    /// so ([`terminals`]). `None` when it wants terminals and they cannot be made: the pane then stays blank, and the
    /// next change it observes tries again.
    fn new(
        session: &Entity<UiSession>,
        key: String,
        terminal: bool,
        cx: &mut Context<SessionPane>,
    ) -> Option<Self> {
        let set = if terminal {
            Some(terminals(session, &key, cx)?)
        } else {
            None
        };
        Some(Mounted {
            key,
            set,
            _home: None,
            copies: details::Copies::new(cx),
            card_focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            card_row: None,
            stale: false, // The `sync` that mounts this sets it at once.
        })
    }

    /// Where focus rests for this session: the unavailable card while `unavailable` says it shows, over the terminals
    /// or not; else the set's shown terminal, or the interrupted or retired card while `card` says one shows. `None`
    /// with none of them (terminals that could not be made), rather than a handle nothing draws.
    fn home(&self, card: bool, unavailable: bool, cx: &App) -> Option<FocusHandle> {
        if unavailable {
            return Some(self.card_focus.clone());
        }
        match &self.set {
            Some(set) => set.read(cx).home(cx),
            None => card.then(|| self.card_focus.clone()),
        }
    }

    /// [`Mounted::home`] as the store says now.
    fn home_now(&self, cx: &App) -> Option<FocusHandle> {
        let card = card::shows(&self.key, cx) || retired::shows(&self.key, cx);
        self.home(card, unavailable::shows(&self.key, cx), cx)
    }
}

/// The terminals of the session filed under `key` ([`TerminalSet`]: the agent terminal and its tabs, under one lease of
/// their own), made for `session`, at the text size every open terminal is drawn at now
/// ([`crate::terminal::text_size::current`]). `None` when the text system cannot measure the terminal font, which is
/// logged.
fn terminals(
    session: &Entity<UiSession>,
    key: &str,
    cx: &mut Context<SessionPane>,
) -> Option<Entity<TerminalSet>> {
    // At the size every open terminal is drawn at now, so a new one does not start at the default and jump.
    let Some(metrics) = CellMetrics::measure(crate::terminal::text_size::current(cx), cx) else {
        tracing::warn!("could not measure the terminal font; the session's terminal is not shown");
        return None;
    };
    let held = key.to_owned();
    // Asked while the terminals draw, from the current UI session's operation store, which notifies when a request
    // starts or settles, and its store, which notifies with every push. A terminal outlives no UI session (it retires
    // with its own), so the stores it reads are always the ones its Delete or Restart went through and its row arrives
    // in. Everything is held while a Delete runs (see the module docs) and while the unavailable card shows over the
    // terminals, which says why they are not live, so their own reconnect card would only say it again; the reconnect
    // card alone while this window's Restart or Suspend is in flight. A Delete and a Restart never overlap: one per
    // session at a time.
    let hold: SharedHold = Rc::new(move |cx| {
        if delete::in_flight(&held, cx).is_some() || unavailable::shows(&held, cx) {
            Hold::Everything
        } else if restart::in_flight(&held, cx) || environment::suspend_in_flight(&held, cx) {
            Hold::ReconnectCard
        } else {
            Hold::Nothing
        }
    });
    Some(cx.new(|cx| TerminalSet::new(session, key.to_owned(), metrics, hold, cx)))
}

impl SessionPane {
    /// The pane in `window`, which follows the selection, the user's choices and the connection from here on, and
    /// mounts the selected session's terminal once the window's first frame exists. `new` is the session list's New
    /// button's focus handle, home when there are no sessions.
    pub fn new(new: FocusHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut observing = vec![
            cx.observe_global_in::<LocalState>(window, Self::sync),
            cx.observe_global_in::<Chosen>(window, Self::sync),
        ];
        // The pane follows the connection, not the store: what it mounts depends on the UI session and the selection,
        // and on the store only for the open session's recovery card and reason, whether its row's host is
        // reached (a stale row shows the unavailable card), whether the session is retired (the retired card, and never
        // a terminal) and whether the fleet is empty (New as home), which the card's follower reports when any of them
        // changes. So it does not use `session::observe_current` itself, which would add a sync on every pushed
        // change, and whose callback gets no window, which every focus move of `sync` needs.
        if let Some(connection) = session::connection(cx) {
            observing.push(cx.observe_in(&connection, window, |pane, _, window, cx| {
                pane.sync(window, cx)
            }));
        }
        let follow = cx.new(card::Follow::new);
        observing.push(cx.observe_in(&follow, window, |pane, _, window, cx| {
            pane.sync(window, cx);
            // A card can stay open while the reason for the missing terminal changes.
            cx.notify();
        }));
        let trust_band = cx.new(trust_band::TrustBand::new);
        observing.push(cx.observe(&trust_band, |_, _, cx| cx.notify()));
        let hook_band = cx.new(hook_band::HookBand::new);
        observing.push(cx.observe(&hook_band, |_, _, cx| cx.notify()));
        // After the window's first frame exists rather than during its construction, as the focus coordinator's own
        // first recovery is: the terminal becomes home, and home takes focus.
        cx.defer_in(window, Self::sync);
        SessionPane {
            mounted: PerSession::default(),
            handled: cx.try_global::<Chosen>().map_or(0, |chosen| chosen.serial),
            spinner: None,
            header: header::Header::default(),
            pulse: None,
            clock: None,
            _card: follow,
            new,
            trust_band,
            hook_band,
            _observing: observing,
        }
    }

    /// Bring the pane up to date: terminals for the selected session under the current UI session (none when nothing is
    /// selected, while the window is inert, while the session shows its interrupted card, when it opened on a host the
    /// helm cannot reach, or ever for a retired session), inert exactly while the unavailable card shows over them, the
    /// shown terminal or the card made home every time (New when nothing is mounted and there are no sessions), and the
    /// user's latest choice acted on.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session =
            session::connection(cx).and_then(|connection| connection.read(cx).session().cloned());
        let wanted = session.as_ref().and_then(|_| local_state::selected(cx));
        let card = wanted.as_deref().is_some_and(|key| card::shows(key, cx));
        let reachable = wanted.as_deref().is_none_or(|key| card::reachable(key, cx));
        let unavailable = wanted
            .as_deref()
            .is_some_and(|key| unavailable::shows(key, cx));
        // A retired session never gets terminals, stale or not; its card shows only while its host is reachable.
        let retired = wanted.as_deref().is_some_and(|key| retired::is(key, cx));
        let retired_card = wanted.as_deref().is_some_and(|key| retired::shows(key, cx));
        let mounted = self.mounted.current(cx);
        if mounted.as_ref().map(|mounted| &mounted.key) != wanted.as_ref() {
            // The old terminals go with this assignment, and their streams with them. A session opened on a host the
            // helm cannot reach gets none, whatever its last status says (stale beats interrupted): there is nothing
            // live to attach to, and the branch below mounts them once the host answers. The old card's question goes
            // too: the session now open may show a card of its own, which the drop below leaves alone, and the
            // question would otherwise go on refusing the actions of the session it was about.
            card::drop_question(cx);
            *mounted = session.zip(wanted).and_then(|(session, key)| {
                Mounted::new(&session, key, !card && !unavailable && !retired, cx)
            });
            cx.notify();
        } else if let (Some(mounted), Some(session)) = (mounted.as_mut(), session.as_ref())
            && ((mounted.set.is_some() && (card || retired))
                || (mounted.set.is_none() && !card && !retired && reachable))
        {
            // The open session went into its card (its terminals go, streams and all) or came out of it (new ones,
            // under a new lease for the replacement terminal). Out of it means on a host the helm reaches:
            // a card whose host drops away mounts nothing, since the terminal is gone there too. The card's question
            // goes either way. A retired session's row arriving after its terminals were made drops them the same way,
            // and nothing ever mounts new ones for it.
            mounted.set = if card || retired {
                None
            } else {
                terminals(session, &mounted.key, cx)
            };
            mounted._home = None;
            cx.notify();
        }
        // The card's question goes whenever the card does not show, however it went: the session recovered, its host
        // dropped away (no card and no terminal, so nothing would draw the question while it went on refusing the
        // session's actions), or another session is open. A card that comes back starts with no question, as the old
        // card's did (its `use_clear_when_hidden`).
        if !card {
            card::drop_question(cx);
        }
        // Home follows the shown terminal: a tab the user chooses (the set has already focused it) or the agent
        // terminal when the chosen tab goes, which takes focus if the tab held it. Only while the set is still the one
        // mounted under the current UI session: a set a reset left behind must not pull home back to its retired
        // terminals.
        if let Some(mounted) = mounted.as_mut()
            && mounted._home.is_none()
            && let Some(set) = &mounted.set
        {
            mounted._home = Some(cx.observe_in(set, window, |pane, set, window, cx| {
                // Through the session's whole rule, not the set's alone: the set also notifies when it goes inert under
                // the unavailable card, which must stay home then.
                let home = pane
                    .mounted
                    .current(cx)
                    .as_ref()
                    .filter(|mounted| mounted.set.as_ref() == Some(&set))
                    .map(|mounted| mounted.home_now(cx));
                if let Some(home) = home {
                    focus::set_home(home.as_ref(), window, cx);
                }
            }));
        }
        // The unavailable card over the terminals: the set inert exactly while it shows. Inert terminals hold no focus,
        // so focus one of them held goes home through the focus coordinator once a frame no longer tracks it, and home
        // is the card (below), so it settles there. Focus anywhere else stays where it is.
        if let Some(mounted) = mounted.as_ref()
            && let Some(set) = &mounted.set
        {
            set.update(cx, |set, cx| set.set_inert(unavailable, cx));
        }
        // The host answering again ends every reconnect wait (the module docs, "The unavailable card"). An attempt
        // already dialing is not cut short, as in the old app: the helm refuses it, and the stream waits its next rung.
        if let Some(mounted) = mounted.as_mut() {
            if mounted.stale
                && !unavailable
                && let Some(set) = &mounted.set
            {
                let terminals: Vec<_> = set.read(cx).terminals().cloned().collect();
                for terminal in terminals {
                    terminal.update(cx, |terminal, cx| terminal.reconnect_now(cx));
                }
            }
            mounted.stale = unavailable;
        }
        // Every time, not only when the pane mounted something else: a reset drops the mounted terminals when `mounted`
        // is next reached (above) while the selection stays, so the key compares equal, and home would otherwise keep
        // naming a terminal that is gone.
        //
        // A mounted session's home always wins (its terminal, its interrupted card or its unavailable card). With
        // nothing mounted, New is home when the store shows there are no sessions, and nothing is otherwise: focus then
        // rests on the root, where keys still reach Farhelm's bindings.
        let home = match mounted.as_ref() {
            Some(mounted) => mounted.home(card || retired_card, unavailable, cx),
            // `card::empty_fleet`, beside `card::Follow`, which notifies when it changes.
            None => card::empty_fleet(cx).then(|| self.new.clone()),
        };
        focus::set_home(home.as_ref(), window, cx);
        self.act_on_choice(window, cx);
    }

    /// The session shortcuts can reach the main view before the terminal sees their keys. Ask the shown terminal
    /// directly whether the focused input method still has marked text.
    pub(crate) fn terminal_is_composing(&self, window: &Window, cx: &App) -> bool {
        self.mounted
            .read(cx)
            .and_then(Option::as_ref)
            .and_then(|mounted| mounted.set.as_ref())
            .and_then(|set| set.read(cx).shown())
            .is_some_and(|terminal| terminal.read(cx).is_composing(window))
    }

    /// The open session's terminals under the current UI session, for tests.
    #[cfg(test)]
    pub(crate) fn terminal_set(&mut self, cx: &App) -> Option<Entity<TerminalSet>> {
        self.mounted
            .current(cx)
            .as_ref()
            .and_then(|mounted| mounted.set.clone())
    }

    /// The key of the session whose terminal the pane shows, under the current UI session.
    fn open_key(&mut self, cx: &App) -> Option<String> {
        self.mounted
            .current(cx)
            .as_ref()
            .map(|mounted| mounted.key.clone())
    }

    /// Focus the terminal of the session the user last chose, once per choice, if it is the one mounted. A choice of
    /// another session (one the selection did not take) is dropped.
    fn act_on_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chosen) = cx.try_global::<Chosen>() else {
            return;
        };
        if chosen.serial == self.handled {
            return;
        }
        self.handled = chosen.serial;
        let Some((key, mark)) = chosen.latest.clone() else {
            return;
        };
        let home = self
            .mounted
            .current(cx)
            .as_ref()
            .filter(|mounted| mounted.key == key)
            .and_then(|mounted| mounted.home_now(cx));
        if let Some(handle) = home {
            focus::choose_terminal(&handle, &mark, window, cx);
        }
    }
}

impl Render for SessionPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Counted in test builds, so a test can tell what redraws the pane (the details bar's clock, say).
        crate::render_count::record("session-pane", cx);
        let shown = self.mounted.current(cx).as_ref().map(|mounted| {
            (
                mounted.key.clone(),
                mounted.set.clone(),
                mounted.copies.clone(),
                mounted.card_focus.clone(),
                mounted.scroll.clone(),
            )
        });
        let Some((key, terminals, copies, card_focus, scroll)) = shown else {
            self.header.clear_action();
            self.spinner = None;
            self.pulse = None;
            self.clock = None;
            self.trust_band
                .update(cx, |band, cx| band.tick_while(false, cx));
            return empty(cx);
        };
        // Read from the store, not from whether terminals are mounted, so a frame drawn before `sync` has caught up
        // with a push already shows what the session is now.
        let stored = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&key)
                .cloned()
        });
        // The unavailable card's notice, worded from the store's row and registry now (`unavailable::Notice`).
        let notice = stored
            .as_ref()
            .filter(|row| unavailable::shows_row(row))
            .and_then(|row| {
                let store = session::current_store(cx)?;
                Some(unavailable::Notice::of(
                    row,
                    store.read(cx).collections(),
                    local_time::zone(cx),
                ))
            });
        // The stale band, from the store's row too, with the viewer's clock.
        let now = age::seconds(ticker::wall_now(cx));
        let zone = local_time::zone(cx);
        let band = stored
            .as_ref()
            .and_then(|row| unavailable::Band::of(row, now, zone));
        let deleting = delete::in_flight(&key, cx);
        // The unavailable card's heading turns the spinner too, as the old card's did.
        let spins = (deleting.is_some() || Self::header_spins(&key, cx) || notice.is_some())
            && ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        let header = self.header_row(&key, step, window, cx);
        // The store's row whenever it has one (remembered while it shows the card); the card's last row only while it
        // has none (see `Mounted::card_row`).
        let held = self
            .mounted
            .current(cx)
            .as_mut()
            .and_then(|mounted| match &stored {
                Some(row) => {
                    mounted.card_row =
                        card::shows_row(&key, row, cx).then(|| (key.clone(), row.clone()));
                    None
                }
                None => {
                    let (held, row) = mounted.card_row.clone()?;
                    debug_assert_eq!(
                        held, key,
                        "a held card row is drawn only for its own session"
                    );
                    (held == key).then_some(row)
                }
            });
        let row = stored.or(held);
        let shows_card = row
            .as_ref()
            .is_some_and(|row| card::shows_row(&key, row, cx));
        let lines = self.header_lines(&key, step, shows_card, cx);
        let bar = self.details_bar(
            &key,
            &copies,
            band.as_ref().is_some_and(|band| band.age.is_some()),
            now,
            window,
            cx,
        );
        let band = band.map(|band| band.draw(theme::palette(cx)));
        // Whether the unavailable card has no terminal under it, read before the terminals are drawn.
        let alone = terminals.is_none();
        // The trust hint band, only while the agent terminal is the one drawn and live: no tab chosen, and no card
        // over the terminals or in their place (`trust_band.rs`). A retired session or an interrupted one has none.
        let agent_shown = notice.is_none()
            && !shows_card
            && !row.as_ref().is_some_and(retired::shows_row)
            && terminals.as_ref().is_some_and(|set| {
                let set = set.read(cx);
                set.shown() == Some(set.agent())
            });
        let trust = row
            .as_ref()
            .and_then(|row| trust_band::draw(&self.trust_band, &key, row, agent_shown, cx));
        let hooks = row
            .as_ref()
            .filter(|_| agent_shown)
            .and_then(|row| hook_band::draw(&self.hook_band, &key, row, cx));
        // The band reads trust on the ticker's clock exactly while it shows.
        let trust_shown = trust.is_some();
        self.trust_band
            .update(cx, |band, cx| band.tick_while(trust_shown, cx));
        let area: Option<AnyElement> = match row.as_ref() {
            Some(row) if shows_card => Some(card::draw(
                card::Drawn {
                    key: &key,
                    row,
                    action: self.header.action(),
                    focus: &card_focus,
                    question: card::question(&key, cx),
                    launcher_wait: session_list::menu::launcher_wait(cx),
                    step,
                },
                cx.weak_entity(),
                window,
                cx,
            )),
            // A retired session keeps its own card even if its terminal is missing.
            Some(row) if retired::shows_row(row) => Some(retired::draw(&card_focus, window, cx)),
            _ => terminals.map(IntoElement::into_any_element),
        };
        // Over the inert terminals, or alone where a session opened unreachable has none.
        let over = notice.map(|notice| {
            unavailable::draw(
                unavailable::Drawn {
                    key: &key,
                    notice,
                    focus: &card_focus,
                    scroll: &scroll,
                    step,
                    alone,
                },
                window,
                cx,
            )
        });
        // The top row, then the lines a long Restart or Replace, a refusal and a YOLO question add, then the trust hint
        // band, then a stale session's band, then the terminal (or the interrupted card in its place, or the
        // unavailable card over it), which takes the rest of the height (SPEC.md: "The terminal loses no height in the
        // steady state"; the lines and the bands show only while there is something to say), then the details bar.
        // The bar comes last in drawing order as well as on screen, so its controls follow the top row and the terminal
        // in Tab's order (SPEC.md: "Keyboard order follows the top row, terminal, then bottom bar").
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .children(lines)
            .children(trust)
            .children(hooks)
            .children(band)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .children(area)
                    .children(over)
                    .children(deleting.map(|label| delete_scrim(label, step, cx)))
                    // The approval region starts its cards below this box's top, clear of the tabs and the lines
                    // above the terminal, as the old cards started below `.terminal-panes`.
                    .child(crate::approvals::region::track_terminal_area(window, cx)),
            )
            .children(bar)
            .into_any_element()
    }
}

impl SessionPane {
    /// The details bar for the open session filed under `key`, read from the store by key whether or not the list
    /// shows it, or `None` when the store holds no row for it (see [`details`]'s module docs) or the window has no UI
    /// session, with the viewer's clock at `now`. Its copy fields confirm as `copies` records. Takes or drops the pulse
    /// and clock handles the bar needs for this frame, as the session list does for its rows; the clock also while the
    /// stale band shows an age (`band_ages`), since the band takes the age the bar leaves out then.
    fn details_bar(
        &mut self,
        key: &str,
        copies: &details::Copies,
        band_ages: bool,
        now: Option<i64>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let zone = local_time::zone(cx);
        let shown = session::current_store(cx).and_then(|store| {
            let collections = store.read(cx).collections();
            collections
                .get::<SessionRow>(key)
                .map(|row| details::Details::of(row, collections, now, zone))
        });
        let pulses =
            shown.as_ref().is_some_and(details::Details::pulses) && ticker::is_active(window, cx);
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let ages = band_ages || shown.as_ref().is_some_and(|shown| shown.age.is_some());
        if ages != self.clock.is_some() {
            self.clock = ages.then(|| ticker::clock(cx));
        }
        let pulse = self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx));
        shown.map(|shown| details::bar(shown, copies, pulse, cx))
    }
}

/// The scrim repeats the row's Delete progress over the open terminal. The words and spinner sit on the floating
/// accent card ([`status_card`]). Visible to the crate for the screenshot goldens.
pub(crate) fn delete_scrim(label: &'static str, step: u8, cx: &App) -> Stateful<Div> {
    // The component layer's status card, which the terminal's reconnect card shares, as the old stylesheet's two
    // cards shared one look.
    let card = status_card(cx).child(status_heading(step, label, cx));
    // No listener, no focus tracking and no occlusion, so GPUI gives it no hitbox: the terminal below keeps every
    // press, and with it focus (see the module docs).
    div()
        .id("terminal-delete-scrim")
        .debug_selector(|| "terminal-delete-scrim".into())
        .role(Role::Status)
        .aria_label(label)
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::color(theme::palette(cx).scrim))
        .child(card)
}

/// The pane with no session open. In the macOS window a strip of the window's title band lies over its top, standing in
/// for the session's top row (the old `.main-empty-titlebar`), so the band still spans the whole window (SPEC.md,
/// "Desktop window chrome": "With no session open, the main pane's half of the band is empty background too").
fn empty(cx: &App) -> AnyElement {
    let store = session::current_store(cx);
    let text = placeholder(store.as_ref().map(|store| store.read(cx).collections()));
    div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::color(theme::palette(cx).fg_2))
        .child(text)
        .children(
            title_band::strip("main-empty-titlebar", Row::EmptyPane)
                .map(|strip| div().absolute().top_0().left_0().right_0().child(strip)),
        )
        .into_any_element()
}

/// What the pane says over `collections` with no session open (`None` while the window is inert, when the window shows
/// its connection state instead and the pane is not drawn).
pub(crate) fn placeholder(collections: Option<&Collections>) -> &'static str {
    match collections {
        None => "",
        Some(collections) if collections.meta(Collection::Sessions).is_none() => {
            "Loading sessions…"
        }
        Some(collections) if session_list::empty_fleet(collections) => "No sessions — create one",
        Some(_) => "",
    }
}

//! The main pane beside the session list: the selected session's top row and live terminal, or what the window says
//! with no session open.
//!
//! SPEC.md, "Session view": "Opening a session shows the agent's real TUI, live". The pane owns the selected session's
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
//!   session that a reset rebuilds gets a terminal of its own for the selection that survived.
//! - **Focus.** The shown terminal is home (SPEC.md, "Where focus rests"): the agent terminal when the pane makes the
//!   set, then whichever terminal the user chooses in the strip, and the agent terminal again when a chosen tab goes.
//!   Each time, the pane tells the focus coordinator, which gives it focus only if nothing else holds it. That covers
//!   the automatic choices: the session opened at startup, and the next session after the open one went away, from
//!   another window or an agent included. When the user chooses a session ([`chosen`]), its terminal also takes focus,
//!   from whatever holds it, through [`focus::choose_terminal`] with the mark of the click that chose it.
//! - **A Delete of the open session.** From the moment its request is sent until the helm answers, the top row shows
//!   what the row says in place of its actions ([`header`]), a scrim over the terminal says it again (stopping the
//!   agent, stopping its terminal tabs, or deleting), and the terminal's own notices are held back
//!   ([`AttachedTerminal::hold_notices`](crate::terminal::attached::AttachedTerminal::hold_notices)): the Delete kills
//!   the agent and detaches the viewer, so the exit line ("Process ended by signal 15") and the status line ("Session
//!   unavailable — reconnecting") would otherwise read as failures while the Delete is going as asked. SPEC.md,
//!   "Lifecycle operations": "a scrim over its terminal says it again. The terminal's own 'Detached' notice, which the
//!   delete itself causes, is held back meanwhile; if the delete fails, the row and header come back with the refusal
//!   shown, and a held notice appears." Both the scrim and the hold ask the operation store while they draw
//!   ([`delete::in_flight`]), so neither keeps a copy that could outlive the request. A refusal therefore takes the
//!   scrim away and lets the terminal show whatever it has to say by then, while the row shows the refusal. A Delete
//!   that succeeds removes the session, and the selection rules take the pane from there. Ported from the old app's
//!   `.terminal-delete-overlay` (`session_view.rs` and app.css at the reference commit) and its held banner
//!   (`showBanner` in terminal.js), which held every banner the delete caused, not only "Detached".
//!
//!   The scrim draws no hitbox and tracks no focus, so the terminal under it keeps focus, keys and clicks: the old
//!   overlay took no pointer events either, so that a click is never eaten by something on its way out. The Delete
//!   holds the session's other operations meanwhile anyway.
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
//! when there are sessions it is blank, because the session list is about to open one.
//!
//! - **The interrupted card.** A session whose agent a host reboot or a Farhelm update ended has no terminal left, so
//!   the pane mounts none for it and draws the interrupted card ([`card`]) where the terminals would be, between the
//!   top row and the details bar, with the card as home. That is the one thing the pane learns from the store rather
//!   than from the selection and the connection: when the open session goes into its card the pane drops its
//!   terminals (their streams with them), and when it comes out (restarted, from here or anywhere) it mounts new ones
//!   under a new lease, as a fresh view would ([`card::Follow`] tells it when).
//!
//! NOTE: The scrim covers whichever terminal is shown, as the old one covered every pane; a Delete holds back the
//! notices of all of them.

pub(crate) mod card;
pub(crate) mod details;
pub(crate) mod header;

#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod carried_tests;
#[cfg(test)]
mod details_tests;
#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod tests;

use farhelm_helm_client::Collections;
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::{Collection, SessionRow};
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, FontWeight, Global,
    InteractiveElement as _, IntoElement, ParentElement, Render, Role, Stateful,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};

use crate::components::{Spinner, spinner_icon};
use crate::desktop::title_band::{self, Row};
use crate::focus::{self, Mark};
use crate::local_state::{self, LocalState};
use crate::prompt::Inline;
use crate::session::{self, PerSession, UiSession};
use crate::session_list::{age, delete, replace};
use std::rc::Rc;

use crate::terminal::CellMetrics;
use crate::terminal::set::{SharedHold, TerminalSet};
use crate::theme::metrics;
use crate::{layers, session_list, theme, ticker};

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
    /// The pane's spinner, held exactly while the scrim, the Stop button's "Stopping…" or a long Restart's or Replace's
    /// line shows in an active window, and worked out at the top of each frame as the session list works out its own:
    /// everything that decides it (the operation store, the shared clocks, the selection, the window's activation)
    /// redraws the window when it changes.
    spinner: Option<Spinner>,
    /// Where the top row's buttons were last drawn, and the question it opened ([`header::Header`]).
    header: header::Header,
    /// The pulse's ticks for the details bar's running mark, held exactly while the bar shows one in an active window,
    /// and worked out at the top of each frame like `spinner`.
    pulse: Option<ticker::Pulsing>,
    /// The ticker's 30-second clock, held exactly while the details bar shows an age, so the age moves on with the
    /// rows' ages and no timer of the pane's own (SPEC_impl.md, "Animation time: the ticker"). Worked out at the top of
    /// each frame like `spinner`.
    clock: Option<ticker::Clock>,
    /// Tells the pane when the open session goes into or out of its interrupted card ([`card::Follow`]).
    _card: Entity<card::Follow>,
    /// Act on a change of the selection, of the user's choice, of the connection and of the open session's card.
    _observing: Vec<Subscription>,
}

/// What the pane keeps for the open session: the session's key, the terminals it shows (the session's agent terminal
/// and its tabs) unless the session shows its interrupted card, the card's focus handle and the Replace question the
/// card asked, and the details bar's copy confirmations. All of it is made and dropped with the selection (the old
/// view's keyed remount reset it the same way); the terminals and the card's question also go when the session goes
/// into or out of its card.
struct Mounted {
    key: String,
    /// `None` while the session shows its interrupted card (or when the terminal font could not be measured).
    set: Option<Entity<TerminalSet>>,
    /// Keeps home on the set's shown terminal as the user chooses tabs and tabs come and go.
    _home: Option<Subscription>,
    copies: details::Copies,
    /// The interrupted card's focus handle, a tab stop: home while the card shows.
    card_focus: FocusHandle,
    /// The Replace question the interrupted card asked, which the card draws while it is open.
    question: Option<Inline<replace::Frozen>>,
    /// The row the interrupted card was last drawn from, kept only to go on drawing the card while the store holds no
    /// row for the session at all: between the helm removing a confirmed Replace's source and its reply, the selection
    /// stays on the source (`session_list::selection`), and the card stays up with the Replace's wait line until the
    /// reply opens the replacement, as the old view, which kept its own copy of the session, did. Never drawn while the
    /// store has a row, so it cannot show anything the store contradicts. Kept with the key it was read under and drawn
    /// only for that key. A `Mounted` belongs to one session anyway (`sync` makes a new one whenever the selection's key
    /// changes, and `render` draws the key of the `Mounted` it holds), so the key only states that in the value, and
    /// `render` asserts it in debug builds.
    card_row: Option<(String, SessionRow)>,
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
            question: None,
            card_row: None,
        })
    }

    /// Where focus rests for this session: the set's shown terminal, or the card while `card` says it shows. `None` with
    /// neither (terminals that could not be made), rather than a handle nothing draws.
    fn home(&self, card: bool, cx: &App) -> Option<FocusHandle> {
        match &self.set {
            Some(set) => set.read(cx).home(cx),
            None => card.then(|| self.card_focus.clone()),
        }
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
    let deleting = key.to_owned();
    // Asked while the terminals draw, from the current UI session's operation store, which notifies when a request
    // starts or settles. A terminal outlives no UI session (it retires with its own), so the store it reads is always
    // the one its Delete went through.
    let hold: SharedHold = Rc::new(move |cx| delete::in_flight(&deleting, cx).is_some());
    Some(cx.new(|cx| TerminalSet::new(session, key.to_owned(), metrics, hold, cx)))
}

impl SessionPane {
    /// The pane in `window`, which follows the selection, the user's choices and the connection from here on, and
    /// mounts the selected session's terminal once the window's first frame exists.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut observing = vec![
            cx.observe_global_in::<LocalState>(window, Self::sync),
            cx.observe_global_in::<Chosen>(window, Self::sync),
        ];
        // The pane follows the connection, not the store: what it mounts depends on the UI session and the selection,
        // and on the store only for whether the open session shows its interrupted card, which the card's follower
        // reports when it changes. So it does not use `session::observe_current` itself, which would add a sync on
        // every pushed change, and whose callback gets no window, which every focus move of `sync` needs.
        if let Some(connection) = session::connection(cx) {
            observing.push(cx.observe_in(&connection, window, |pane, _, window, cx| {
                pane.sync(window, cx)
            }));
        }
        let follow = cx.new(card::Follow::new);
        observing.push(cx.observe_in(&follow, window, |pane, _, window, cx| pane.sync(window, cx)));
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
            _observing: observing,
        }
    }

    /// Bring the pane up to date: terminals for the selected session under the current UI session (none when nothing is
    /// selected, while the window is inert, or while the session shows its interrupted card), the shown terminal or the
    /// card made home every time, and the user's latest choice acted on.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session =
            session::connection(cx).and_then(|connection| connection.read(cx).session().cloned());
        let wanted = session.as_ref().and_then(|_| local_state::selected(cx));
        let card = wanted.as_deref().is_some_and(|key| card::shows(key, cx));
        let reachable = wanted.as_deref().is_none_or(|key| card::reachable(key, cx));
        let mounted = self.mounted.current(cx);
        if mounted.as_ref().map(|mounted| &mounted.key) != wanted.as_ref() {
            // The old terminals go with this assignment, and their streams with them.
            *mounted = session
                .zip(wanted)
                .and_then(|(session, key)| Mounted::new(&session, key, !card, cx));
            cx.notify();
        } else if let (Some(mounted), Some(session)) = (mounted.as_mut(), session.as_ref())
            && ((mounted.set.is_some() && card) || (mounted.set.is_none() && !card && reachable))
        {
            // The open session went into its card (its terminals go, streams and all) or came out of it (new ones,
            // under a new lease, since the reboot took the old terminal). Out of it means on a host the helm reaches:
            // a card whose host drops away mounts nothing, since the terminal is gone there too. The card's question
            // goes either way.
            mounted.set = if card {
                None
            } else {
                terminals(session, &mounted.key, cx)
            };
            mounted._home = None;
            mounted.question = None;
            cx.notify();
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
                let current = pane
                    .mounted
                    .current(cx)
                    .as_ref()
                    .is_some_and(|mounted| mounted.set.as_ref() == Some(&set));
                if current {
                    let home = set.read(cx).home(cx);
                    focus::set_home(home.as_ref(), window, cx);
                }
            }));
        }
        // Every time, not only when the pane mounted something else: a reset drops the mounted terminals when `mounted`
        // is next reached (above) while the selection stays, so the key compares equal, and home would otherwise keep
        // naming a terminal that is gone.
        let home = mounted.as_ref().and_then(|mounted| mounted.home(card, cx));
        focus::set_home(home.as_ref(), window, cx);
        self.act_on_choice(window, cx);
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
        let card = card::shows(&key, cx);
        let home = self
            .mounted
            .current(cx)
            .as_ref()
            .filter(|mounted| mounted.key == key)
            .and_then(|mounted| mounted.home(card, cx));
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
                mounted.question.clone(),
            )
        });
        let Some((key, terminals, copies, card_focus, question)) = shown else {
            self.spinner = None;
            self.pulse = None;
            self.clock = None;
            return empty(cx);
        };
        let deleting = delete::in_flight(&key, cx);
        let spins =
            (deleting.is_some() || Self::header_spins(&key, cx)) && ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        let header = self.header_row(&key, step, window, cx);
        // Read from the store, not from whether terminals are mounted, so a frame drawn before `sync` has caught up
        // with a push already shows what the session is now.
        let stored = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&key)
                .cloned()
        });
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
        let bar = self.details_bar(&key, &copies, window, cx);
        let area: Option<AnyElement> = match row.as_ref() {
            Some(row) if shows_card => Some(card::draw(
                card::Drawn {
                    key: &key,
                    row,
                    focus: &card_focus,
                    question,
                    step,
                },
                cx.weak_entity(),
                window,
                cx,
            )),
            _ => terminals.map(IntoElement::into_any_element),
        };
        // The top row, then the lines a long Restart or Replace, a refusal and a YOLO question add, then the terminal
        // (or the interrupted card in its place), which takes the rest of the height (SPEC.md: "The terminal loses no
        // height in the steady state"; the lines show only while there is something to say), then the details bar. The
        // bar comes last in drawing order as well as on screen, so its controls follow the top row and the terminal in
        // Tab's order (SPEC.md: "Keyboard order follows the top row, terminal, then bottom bar").
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(header)
            .children(lines)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .children(area)
                    .children(deleting.map(|guard| delete_scrim(guard, step, cx)))
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
    /// session. Its copy fields confirm as `copies` records. Takes or drops the pulse and clock handles the bar needs
    /// for this frame, as the session list does for its rows.
    fn details_bar(
        &mut self,
        key: &str,
        copies: &details::Copies,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let now = age::seconds(ticker::wall_now(cx));
        let shown = session::current_store(cx).and_then(|store| {
            let collections = store.read(cx).collections();
            collections
                .get::<SessionRow>(key)
                .map(|row| details::Details::of(row, collections, now))
        });
        let pulses =
            shown.as_ref().is_some_and(details::Details::pulses) && ticker::is_active(window, cx);
        if pulses != self.pulse.is_some() {
            self.pulse = pulses.then(|| ticker::pulse(window, cx));
        }
        let ages = shown.as_ref().is_some_and(|shown| shown.age.is_some());
        if ages != self.clock.is_some() {
            self.clock = ages.then(|| ticker::clock(cx));
        }
        let pulse = self.pulse.as_ref().map_or(0, |pulse| pulse.step(cx));
        shown.map(|shown| details::bar(shown, copies, pulse, cx))
    }
}

/// The scrim over the open session's terminal while its Delete is in flight with `guard`, its spinner at `step`: the
/// old `.terminal-delete-overlay`, a black veil over the whole terminal with the row's words beside a 16-pixel spinner,
/// centered on a floating card with an accent edge, in the accent. Visible to the crate for the screenshot goldens.
pub(crate) fn delete_scrim(guard: DeleteGuard, step: u8, cx: &App) -> Stateful<Div> {
    let palette = theme::palette(cx);
    let ink = theme::color(palette.accent);
    let label = delete::progress_label(guard);
    let card = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .py(px(14.))
        .px(px(20.))
        .bg(theme::color(palette.bg_2))
        .border_1()
        .border_color(ink)
        .rounded(metrics::RADIUS_MD)
        // `--shadow-menu`, as the old card had.
        .shadow(layers::menu_shadow(palette))
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(spinner_icon(step, px(16.), ink))
        .child(label);
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
        .bg(theme::color(palette.scrim))
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

//! The main pane beside the session list: the selected session's live terminal, or what the window says with no session
//! open.
//!
//! SPEC.md, "Session view": "Opening a session shows the agent's real TUI, live". The pane owns one
//! [`AttachedTerminal`], for the selected session's agent terminal, and nothing else that belongs to a session.
//!
//! - **One terminal, remade on every change of selection.** Choosing another session drops the terminal (which ends
//!   its stream) and makes a new one with a fresh attachment lease. This is the old app's keyed remount (its
//!   `SessionView` keyed by the session's id, so a switch tore the old view down and mounted a new one, which attached
//!   again), not the GPUI prototype's deck of terminals kept attached across selections, which the old app never had.
//!   A terminal is made for whichever session is selected, listed or not, because a selection the list cannot show (past
//!   a cut listing, say) is still the open session.
//! - **One UI session.** The terminal is made with the current UI session's opener and retires with that session. The
//!   pane keeps it in [`PerSession`], which compares the current UI session with the one it was made under each time
//!   the pane reaches it, drawing included, so a terminal made for an earlier connection is never drawn or focused. The
//!   session that a reset rebuilds gets a terminal of its own for the selection that survived.
//! - **Focus.** The terminal is home (SPEC.md, "Where focus rests"): every time the pane makes one it tells the focus
//!   coordinator, which gives it focus only if nothing else holds it. That covers the automatic choices: the session
//!   opened at startup, and the next session after the open one went away, from another window or an agent included.
//!   When the user chooses a session ([`chosen`]), its terminal also takes focus, from whatever holds it, through
//!   [`focus::choose_terminal`] with the mark of the click that chose it.
//!
//! The pane acts from observers (the selection, the user's choices, the connection), never from drawing: focus moves
//! made while a frame is drawn are not reliably drawn themselves, and observers also run for a window that is not drawn
//! at all. A terminal made in a hidden window opens its stream only once it is drawn and measured, as any terminal does.
//!
//! With no session open, the pane shows what the old app showed then (its `.main-empty`), with the old rule for it:
//! three honest states, not one claim. It says "No sessions — create one" only once the helm has shown that there are
//! none ([`crate::session_list::empty_fleet`]); before the session list has been read it says "Loading sessions…"; and
//! when there are sessions it is blank, because the session list is about to open one.
//!
//! NOTE: Not ported yet: the session header, terminal tabs, the text size control (the terminal is drawn at the old
//! default size), the card a session without terminal contents shows instead, and Delete's scrim over the terminal.

#[cfg(test)]
mod tests;

use farhelm_helm_client::Collections;
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::ui_channel::Collection;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Global, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div,
};

use crate::focus::{self, Mark};
use crate::local_state::{self, LocalState};
use crate::session::{self, PerSession, UiSession};
use crate::terminal::attached::{AttachedTerminal, TerminalTarget};
use crate::terminal::{CellMetrics, TextSize};
use crate::{session_list, theme};

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
    /// Act on a change of the selection, of the user's choice and of the connection.
    _observing: Vec<Subscription>,
}

/// The terminal the pane shows, and the session it is for.
struct Mounted {
    key: String,
    terminal: Entity<AttachedTerminal>,
}

impl Mounted {
    /// The agent terminal of the session filed under `key`, made for `session` with a lease of its own, at the old
    /// app's default text size. `None` when the text system cannot measure the terminal font, which is logged: the pane
    /// then stays blank, and the next change it observes tries again, most likely with the same result.
    fn new(
        session: &Entity<UiSession>,
        key: String,
        cx: &mut Context<SessionPane>,
    ) -> Option<Self> {
        let Some(metrics) = CellMetrics::measure(TextSize::DEFAULT, cx) else {
            tracing::warn!(
                "could not measure the terminal font; the session's terminal is not shown"
            );
            return None;
        };
        let open = session.read(cx).terminal_opener();
        let target = TerminalTarget {
            session: key.clone(),
            tab: None,
            lease: AttachmentLease::new(),
        };
        let terminal = cx.new(|cx| AttachedTerminal::new(session, target, metrics, open, cx));
        Some(Mounted { key, terminal })
    }
}

impl SessionPane {
    /// The pane in `window`, which follows the selection, the user's choices and the connection from here on, and
    /// mounts the selected session's terminal once the window's first frame exists.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut observing = vec![
            cx.observe_global_in::<LocalState>(window, Self::sync),
            cx.observe_global_in::<Chosen>(window, Self::sync),
        ];
        if let Some(connection) = session::connection(cx) {
            observing.push(cx.observe_in(&connection, window, |pane, _, window, cx| {
                pane.sync(window, cx)
            }));
        }
        // After the window's first frame exists rather than during its construction, as the focus coordinator's own
        // first recovery is: the terminal becomes home, and home takes focus.
        cx.defer_in(window, Self::sync);
        SessionPane {
            mounted: PerSession::default(),
            handled: cx.try_global::<Chosen>().map_or(0, |chosen| chosen.serial),
            _observing: observing,
        }
    }

    /// Bring the pane up to date: a terminal for the selected session under the current UI session (none when nothing is
    /// selected or the window is inert), made home whenever it changes, and the user's latest choice acted on.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session =
            session::connection(cx).and_then(|connection| connection.read(cx).session().cloned());
        let wanted = session.as_ref().and_then(|_| local_state::selected(cx));
        let mounted = self.mounted.current(cx);
        if mounted.as_ref().map(|mounted| &mounted.key) != wanted.as_ref() {
            // The old terminal goes with this assignment, and its stream with it.
            *mounted = session
                .zip(wanted)
                .and_then(|(session, key)| Mounted::new(&session, key, cx));
            let home = mounted
                .as_ref()
                .map(|mounted| mounted.terminal.read(cx).focus_handle().clone());
            focus::set_home(home.as_ref(), window, cx);
            cx.notify();
        }
        self.act_on_choice(window, cx);
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
        let terminal = self
            .mounted
            .current(cx)
            .as_ref()
            .filter(|mounted| mounted.key == key)
            .map(|mounted| mounted.terminal.clone());
        if let Some(terminal) = terminal {
            let handle = terminal.read(cx).focus_handle().clone();
            focus::choose_terminal(&handle, &mark, window, cx);
        }
    }
}

impl Render for SessionPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(mounted) = self.mounted.current(cx) {
            return div()
                .size_full()
                .child(mounted.terminal.clone())
                .into_any_element();
        }
        empty(cx)
    }
}

/// The pane with no session open.
fn empty(cx: &App) -> AnyElement {
    let store = session::current_store(cx);
    let text = placeholder(store.as_ref().map(|store| store.read(cx).collections()));
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme::color(theme::palette(cx).fg_2))
        .child(text)
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

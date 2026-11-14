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
//! - **A Delete of the open session.** From the moment its request is sent until the helm answers, a scrim over the
//!   terminal says what the row says (stopping the agent, stopping its terminal tabs, or deleting), and the terminal's
//!   own notices are held back ([`AttachedTerminal::hold_notices`]): the Delete kills the agent and detaches the
//!   viewer, so the exit line ("Process ended by signal 15") and the status line ("Session unavailable —
//!   reconnecting") would otherwise read as failures while the Delete is going as asked. SPEC.md, "Lifecycle
//!   operations": "a scrim over its terminal says it again. The terminal's own 'Detached' notice, which the delete
//!   itself causes, is held back meanwhile; if the delete fails, the row and header come back with the refusal shown,
//!   and a held notice appears." Both the scrim and the hold ask the operation store while they draw
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
//! NOTE: Not ported yet: the session header, terminal tabs, the text size control (the terminal is drawn at the old
//! default size), and the card a session without terminal contents shows instead. With no header, the header half of
//! SPEC.md's Delete sentence ("the open session's header shows the same in place of its actions", and comes back with
//! the refusal) does not apply yet; the header brings it. With no tabs, the scrim covers the agent terminal only, where
//! the old one covered every pane.

#[cfg(test)]
mod tests;

use farhelm_helm_client::Collections;
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::DeleteGuard;
use farhelm_proto::ui_channel::Collection;
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, Global,
    InteractiveElement as _, IntoElement, ParentElement, Render, Role, Stateful,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};

use crate::focus::{self, Mark};
use crate::local_state::{self, LocalState};
use crate::session::{self, PerSession, UiSession};
use crate::session_list::delete;
use crate::terminal::attached::{AttachedTerminal, TerminalTarget};
use crate::terminal::{CellMetrics, TextSize};
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
    /// The scrim's spinner, held exactly while the scrim shows in an active window, and worked out at the top of each
    /// frame as the session list works out its own: everything that decides it (the operation store, the selection,
    /// the window's activation) redraws the window when it changes.
    spinner: Option<delete::Spinner>,
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
        let deleting = key.clone();
        let terminal = cx.new(|cx| {
            let mut terminal = AttachedTerminal::new(session, target, metrics, open, cx);
            // Asked while the terminal draws, from the current UI session's operation store, which notifies when a
            // request starts or settles. A terminal outlives no UI session (it retires with its own), so the store it
            // reads is always the one its Delete went through.
            terminal.hold_notices(Box::new(move |cx| {
                delete::in_flight(&deleting, cx).is_some()
            }));
            terminal
        });
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
            spinner: None,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self
            .mounted
            .current(cx)
            .as_ref()
            .map(|mounted| (mounted.key.clone(), mounted.terminal.clone()));
        let Some((key, terminal)) = shown else {
            self.spinner = None;
            return empty(cx);
        };
        let deleting = delete::in_flight(&key, cx);
        let spins = deleting.is_some() && ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| delete::Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        div()
            .relative()
            .size_full()
            .child(terminal)
            .children(deleting.map(|guard| delete_scrim(guard, step, cx)))
            .into_any_element()
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
        .child(delete::spinner_icon(step, px(16.), ink))
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

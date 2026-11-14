//! One attached session terminal: the owner between a helm client terminal stream and the renderer.
//!
//! NOTE: This is cut to what the first milestone's slice needs (the migration's orchestrator trimmed it on 2026-11-13):
//! attach a terminal, show its output, say when it is not live, take focus and send the program its keys. Paste, focus
//! reports (mode 1004), an input method handler, scrollback, selection, links and the reconnect display with its phase
//! and Retry come later in the terminal area's work. The session pane ([`crate::session_pane`]) mounts one for the
//! selected session.
//!
//! [`AttachedTerminal`] owns one stream ([`TerminalHandle`] and [`TerminalEvents`]) and the [`TerminalView`]s that show
//! it. Here's the TLDR of what it adds on top of the stream and the renderer, each a rule the old app or the GPUI
//! prototype learned the hard way:
//!
//! - Fit before attach. The stream is opened only once the pane has been measured, at the size it will be drawn at,
//!   because the attach's size is what the replay is rendered at (the prototype lost 9 of 12,000 lines to an eager
//!   80x24 attach followed by a resize). Later size changes go out as resizes, only when the size changes.
//! - Staged replay. Each attachment replays the whole screen and history from nothing, so its output goes into a fresh
//!   view kept off screen until the helm says the replay is complete; only then does it replace what is shown. A
//!   reconnect, a takeover or a refusal therefore never shows half a replay, and the last complete screen stays up
//!   while the stream is not live.
//! - Bounded delivery. Each GUI turn takes at most [`BATCH`] events from the stream, applies consecutive output as one
//!   batch (so a flood redraws once per turn, not once per message), and then yields to the executor, so a flood cannot
//!   hold the main thread and frames keep their turn.
//! - One status line. Whenever the stream is not live and not catching up, one line in the old app's red banner says
//!   why, with a Take control button where the stream waits for the user.
//! - A protocol fault ends the stream. A message the grid model refuses means the peer broke the format; the owner
//!   closes the stream and says so on the status line. It does not restart the attachment by itself (that, too, waits
//!   for the terminal area).
//! - It retires with its UI session. A reset or a displacement retires the [`UiSession`] the owner was made for
//!   (SPEC_impl.md, "One failure path: the reset"); the owner then closes its stream, cancels its delivery and drops its
//!   views, so no reconnect worker or old screen outlives the session.
//!
//! The owner does not own the grid itself: each [`TerminalView`] owns its grid model, as the renderer's module explains,
//! and this module decides which view is shown and which one receives output.
//!
//! ## Input
//!
//! The grid's area (the pane below its status line) is one focusable element in [`actions::TERMINAL_CONTEXT`], tracked
//! through the focus coordinator as an element a press focuses ([`OnPress::Focus`]); the session view makes it home and
//! hands it focus through [`focus::choose_terminal`] and its siblings, with [`AttachedTerminal::focus_handle`]. Keys
//! reach its listener only after the keymap has had them, so a key some binding takes (an app shortcut that has its
//! feature) never gets here. For every key that does, the listener asks the keymap whose it is
//! ([`actions::terminal_key_owner`]) rather than deciding again itself:
//!
//! - An app shortcut whose feature has no binding yet is dropped, press and repeats alike. It is Farhelm's, so it
//!   must not reach the program in the meantime either.
//! - The terminal's own commands do nothing yet. Each needs something this milestone does not have: Shift+PageUp and
//!   Shift+PageDown scroll the history, Cmd+A selects and Cmd+C copies the selection, and Cmd+V pastes. They are
//!   dropped rather than sent, so a program never learns to depend on getting them.
//! - Every other key is the program's. `farhelm-input-mapping`'s `map_key` decides what it means (the supervisor
//!   encodes the bytes). A key it sends as a key goes out as one; a key it leaves to text input goes out as the text
//!   the platform says it typed, because there is no input method handler yet to insert it (GPUI inserts text only
//!   through one). Held keys repeat to the program, as in any terminal: the keymap's repeat guard
//!   (`actions::ONCE_PER_PRESS`) stops a held app shortcut's repeats before they get here, and `map_key` holds back
//!   nothing else.
//!
//! NOTE: Two kinds of typing do not work yet, and sending the platform's text cannot fix either; both need the input
//! method handler. A Mac dead key (Option-E, US-International's `'`, German `^`) arrives with the spacing accent as its
//! text, so the terminal sends `´` and then a plain `e` where the old app typed `é`. On Linux, text an input method
//! (XIM, IBus) commits goes to the handler GPUI asks for, and with none it is dropped. Linux's own dead keys do work:
//! the composed key arrives with the composed character as its text.
//!
//! Input goes out only while the terminal is live ([`AttachedTerminal::is_live`]): never while it catches up on a
//! replay, reconnects, waits detached or refused, after a protocol fault, or once retired. Keys typed then are
//! dropped, not kept for later, as the helm client drops input for a stream that is not attached. Each group of input
//! is stamped with the attachment it was typed for, so the helm client never writes it to a later one.
//!
//! The cursor blinks on the ticker ([`ticker::blink`]) only while the terminal has focus in an active window and is
//! live, and is drawn outlined otherwise. Blinking only while focused is the old app's rule. Drawing the outline in an
//! inactive window is new: the old app kept a focused terminal's filled cursor there and froze its blink, possibly in
//! the hidden half. A cursor the program asked to stay steady never blinks, and under reduced motion none does.

#[cfg(test)]
mod tests;

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use farhelm_grid_model::{ExitState, GridError, GridOutput};
use farhelm_helm_client::terminal::{
    Attachment, AttachmentLease, StreamState, TerminalAttach, TerminalEvent, TerminalEvents,
    TerminalHandle,
};
use farhelm_input_mapping::pointer::Geometry;
use farhelm_input_mapping::{
    Composition, Decision, KeyDown, NativeIdentity, NativeKey, Platform, Route, committed_text,
    map_key,
};
use farhelm_proto::DetachCode;
use farhelm_proto::grid::GridInput;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClipboardItem, Context, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, NativeKeyIdentity, ParentElement, Pixels,
    Render, SharedString, Styled, Subscription, Task, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};

use super::{CellMetrics, CursorPhase, TerminalView, style};
use crate::actions::{self, KeyOwner};
use crate::assets::TERMINAL_FONT;
use crate::components::{Banner, Button, HoverHelp, Role};
use crate::focus::{self, OnPress};
use crate::host_text::{HostSentence, HostText};
use crate::render_count;
use crate::session::UiSession;
use crate::{theme, ticker};

/// The most stream events one GUI turn takes before it yields. The prototype's bound (eight messages, at most 2 MiB);
/// the byte half is gone because the helm client hands over decoded messages, but each one came from a frame of at
/// most 1 MiB, so a turn still handles a bounded amount of output.
pub(crate) const BATCH: usize = 8;

/// Padding between the pane's edge and the grid: the old `.terminal`'s 4 pixels plus the `.grid-terminal`'s 4 inside
/// it, at the reference commit.
const PANE_PADDING: f32 = 8.;

/// The column the old fit kept free for the scrollback bar (`SCROLLBAR_WIDTH` in grid-terminal.js). Kept although
/// this module draws no scrollbar yet, so that the bar's arrival does not change every terminal's width.
const SCROLLBAR_WIDTH: f32 = 14.;

/// The largest grid in either direction the format allows (`MAX_DIMENSION` in grid-terminal.js).
const MAX_DIMENSION: f32 = 1000.;

/// The old app's catch-up placeholder (`CONNECTING_TEXT` in terminal.js at the reference commit): plain and
/// unalarming, because every attach passes through it.
pub(crate) const CONNECTING_TEXT: &str = "Connecting — catching up on this terminal's history…";

/// The placeholder's text size and padding: `.terminal-connecting`'s `font-size: 14px` and `padding: 2em`.
const PLACEHOLDER_TEXT: f32 = 14.;
const PLACEHOLDER_PADDING: f32 = 28.;

/// Take control's hover help, the old button's tooltip word for word.
const TAKE_CONTROL_HELP: &str =
    "Take control: attach this view, detaching any other client attached to this session";

/// Which terminal an owner attaches: the session's agent terminal or one of its tabs, under the session view's lease.
#[derive(Clone, Debug)]
pub struct TerminalTarget {
    /// The session's served id, as the sessions collection keys it.
    pub session: String,
    /// `None` for the agent terminal, otherwise exactly that tab.
    pub tab: Option<String>,
    /// The session view's lease, shared by its agent terminal and tabs (see [`AttachmentLease`]).
    pub lease: AttachmentLease,
}

/// Opens the stream once the owner knows its size. The app passes its UI session's
/// [`UiSession::terminal_opener`], which opens nothing once the session is retired; tests pass one that hands out a
/// fake terminal's handle and feed. Called at most once
/// per owner: the stream reconnects by itself, and Take control goes through the same stream.
pub type OpenStream = Box<dyn FnOnce(TerminalAttach) -> (TerminalHandle, TerminalEvents)>;

/// What the status line says, and whether it offers Take control. `reason` is the helm's or supervisor's own
/// explanation of a detach, which is host-supplied text and is drawn escaped after `words`.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusLine {
    pub words: SharedString,
    pub reason: Option<HostText>,
    pub take_control: bool,
}

/// The owner of one attached terminal. See the module docs.
pub struct AttachedTerminal {
    target: TerminalTarget,
    metrics: CellMetrics,
    /// Taken when the stream is opened, and dropped on retirement so a late fit cannot open one.
    opener: Option<OpenStream>,
    handle: Option<TerminalHandle>,
    /// The delivery loop, which owns the stream's event feed. Dropping it cancels delivery and ends the stream.
    pump: Option<Task<()>>,
    /// The stream state last delivered.
    stream: StreamState,
    /// The view on screen: the last attachment whose replay completed.
    committed: Option<Entity<TerminalView>>,
    /// The current attachment's view while its replay is still arriving. Never drawn.
    staging: Option<Entity<TerminalView>>,
    /// Whether `committed` belongs to the live attachment, so output goes to it rather than being dropped.
    live: bool,
    /// The status line after a protocol fault. Once set the stream is closed and nothing else is delivered.
    fault: Option<SharedString>,
    /// The grid size last fitted, which the stream was opened at or last resized to.
    fit: Option<(u16, u16)>,
    /// The display's scale at the last fit, which the exit line's placement needs to know the grid's cell.
    scale: Option<f32>,
    /// The height the grid actually has, the pane less the status line and padding, which is where the exit line must
    /// fit. Measured apart from the fit because the fit deliberately ignores the status line.
    room: Option<f32>,
    retired: bool,
    /// The pane's focus: a tab stop, which a press on the pane focuses. Lives as long as the owner, so the session
    /// view can name it as home across redraws.
    focus: FocusHandle,
    /// The attachment the stream last announced, which input is stamped with. Output, and so liveness, is always the
    /// latest announced attachment's, so this is the attachment the screen shows whenever the terminal is live.
    attachment: Option<Attachment>,
    /// The input mapping's dead-key state across key presses. Forgotten whenever the terminal does not take input, so
    /// a dead key pressed before focus moved away cannot change a key pressed after it came back.
    composition: Composition,
    /// The cursor blink's ticks, held while the cursor blinks (see the module docs, "Input").
    blink: Option<ticker::Blinking>,
    _session: [Subscription; 2],
    /// How many delivery turns ran, and how many times output was applied to a view, so tests can check the per-turn
    /// bound and that consecutive output is applied as one batch.
    #[cfg(test)]
    turns: usize,
    #[cfg(test)]
    applies: usize,
}

impl AttachedTerminal {
    /// An owner for `target`, made for `session`, that opens its stream through `open` once it has been drawn and
    /// measured. An owner made for a session that is already retired starts retired and never opens a stream.
    pub fn new(
        session: &Entity<UiSession>,
        target: TerminalTarget,
        metrics: CellMetrics,
        open: OpenStream,
        cx: &mut Context<Self>,
    ) -> Self {
        let retired = session.read(cx).is_retired();
        // A retiring session notifies its observers; one that is dropped outright never does, hence both.
        let subscriptions = [
            cx.observe(session, |owner, session, cx| {
                if session.read(cx).is_retired() {
                    owner.retire(cx);
                }
            }),
            cx.observe_release(session, |owner, _, cx| owner.retire(cx)),
        ];
        #[cfg(any(feature = "agent", test))]
        probe::enroll(cx.weak_entity(), cx);
        AttachedTerminal {
            target,
            metrics,
            opener: (!retired).then_some(open),
            handle: None,
            pump: None,
            stream: StreamState::Attaching,
            committed: None,
            staging: None,
            live: false,
            fault: None,
            fit: None,
            scale: None,
            room: None,
            retired,
            focus: cx.focus_handle().tab_stop(true),
            attachment: None,
            composition: Composition::default(),
            blink: None,
            _session: subscriptions,
            #[cfg(test)]
            turns: 0,
            #[cfg(test)]
            applies: 0,
        }
    }

    /// Whether the terminal shows its live attachment's complete screen and carries input. Input is sent only while
    /// this holds.
    pub fn is_live(&self) -> bool {
        self.live && self.fault.is_none() && !self.retired
    }

    /// The pane's focus handle, for the focus coordinator: the session view passes it to [`focus::choose_terminal`]
    /// when the user chooses this terminal's session, and to [`focus::set_home`] when it becomes or stops being home.
    /// Never focus it any other way.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    /// The shown screen's text, as the renderer reads it from its paint cache; `None` before any replay completed.
    pub fn terminal_text(&self, cx: &App) -> Option<String> {
        self.committed
            .as_ref()
            .and_then(|view| view.read(cx).terminal_text())
    }

    /// What the status line says now, if anything. `None` while live, while catching up, and once retired.
    pub fn status_line(&self) -> Option<StatusLine> {
        let line = |words: String, reason, take_control| {
            Some(StatusLine {
                words: words.into(),
                reason,
                take_control,
            })
        };
        if self.retired {
            return None;
        }
        if let Some(fault) = &self.fault {
            return line(fault.to_string(), None, false);
        }
        match &self.stream {
            StreamState::Attaching | StreamState::Attached(_) => None,
            // The heading of the old app's outage card. Its explanation, phase and Reconnect now are the terminal
            // area's later work.
            StreamState::Reconnecting => {
                line("Session unavailable — reconnecting".into(), None, false)
            }
            // The old app offered Take control only after a takeover. The helm client parks a stalled viewer and a
            // refused attach until Take control as well, and nothing else in this milestone can ask it to try again
            // (Retry comes with the reconnect display), so they offer it too, on purpose. A closed tab is gone, so there
            // is nothing to take.
            StreamState::Detached { code, reason } => line(
                "Detached: ".into(),
                Some(HostText::from_host(reason.as_str())),
                *code != Some(DetachCode::TabClosed),
            ),
            // The old app's words for a socket it could not open. The helm's status goes to the log, not the screen.
            StreamState::Refused { .. } => line("Connection error".into(), None, true),
            StreamState::Closed => line("Connection closed".into(), None, false),
        }
    }

    /// The user's Take control: attach again through the displacing route. Only the states that wait for it offer it.
    pub fn take_control(&mut self, cx: &mut Context<Self>) {
        let waiting = matches!(
            self.stream,
            StreamState::Detached { .. } | StreamState::Refused { .. }
        );
        if let (Some(handle), true, None) = (&self.handle, waiting, &self.fault) {
            handle.take_control();
            cx.notify();
        }
    }

    /// The pane was drawn at `pane` on a display of `scale`: open the stream at the size that fits, or resize it when
    /// the size changed. Runs deferred after the frame that measured it, never during paint.
    fn fitted(&mut self, pane: Bounds<Pixels>, scale: f32, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        if self.scale != Some(scale) {
            self.scale = Some(scale);
            // Only the screen's placement reads it; anything else a new scale changes arrives as output.
            cx.notify();
        }
        if self.fault.is_some() {
            return;
        }
        let width = f32::from(pane.size.width) - 2. * PANE_PADDING;
        let height = f32::from(pane.size.height) - 2. * PANE_PADDING;
        let Some(size) = fit(self.metrics, width, height, scale) else {
            return;
        };
        if self.fit == Some(size) {
            return;
        }
        self.fit = Some(size);
        match &self.handle {
            None => self.open(size, cx),
            // The stream delivers a resize whatever state it is in: at once on an open socket, as the first message
            // of a socket still connecting, and in the query of a later attach otherwise.
            Some(handle) => handle.resize(size.0, size.1),
        }
    }

    /// The grid's area was drawn `height` pixels tall (inside its padding, below the status line).
    fn grid_measured(&mut self, height: f32, cx: &mut Context<Self>) {
        if self.room != Some(height) {
            self.room = Some(height);
            if self.committed.is_some() {
                cx.notify();
            }
        }
    }

    /// Open the stream at `size` and start delivering its events.
    fn open(&mut self, (cols, rows): (u16, u16), cx: &mut Context<Self>) {
        let Some(open) = self.opener.take() else {
            return;
        };
        let (handle, mut events) = open(TerminalAttach {
            session: self.target.session.clone(),
            tab: self.target.tab.clone(),
            lease: self.target.lease.clone(),
            cols,
            rows,
        });
        self.handle = Some(handle);
        self.pump = Some(cx.spawn(async move |owner, cx| {
            while let Some(first) = events.recv().await {
                let mut batch = vec![first];
                while batch.len() < BATCH {
                    match events.try_recv() {
                        Some(event) => batch.push(event),
                        None => break,
                    }
                }
                let more = owner
                    .update(cx, |owner, cx| owner.deliver(batch, cx))
                    .unwrap_or(false);
                if !more {
                    break;
                }
                // A full feed is ready again at once, so without this one task would apply a whole flood in a single
                // turn of the main thread. Yielding puts it behind everything else queued there, frames included.
                YieldNow(false).await;
            }
        }));
    }

    /// Apply one turn's events in order. Returns whether delivery should go on.
    fn deliver(&mut self, batch: Vec<TerminalEvent>, cx: &mut Context<Self>) -> bool {
        #[cfg(test)]
        {
            self.turns += 1;
        }
        let mut events = batch.into_iter().peekable();
        while let Some(event) = events.next() {
            if self.retired || self.fault.is_some() {
                break;
            }
            match event {
                TerminalEvent::Output(first) => {
                    let mut run = vec![first];
                    while matches!(events.peek(), Some(TerminalEvent::Output(_))) {
                        if let Some(TerminalEvent::Output(next)) = events.next() {
                            run.push(next);
                        }
                    }
                    self.output(run, cx);
                }
                TerminalEvent::ReplayComplete => self.replay_complete(cx),
                TerminalEvent::State(state) => self.stream_changed(state, cx),
            }
        }
        !self.retired && self.fault.is_none()
    }

    /// Apply consecutive output to the view that should get it: the staged replay, or the live screen.
    fn output(&mut self, run: Vec<GridOutput>, cx: &mut Context<Self>) {
        let (view, live) = match (&self.staging, &self.committed) {
            (Some(staging), _) => (staging.clone(), false),
            (None, Some(committed)) if self.live => (committed.clone(), true),
            // The stream announces every attachment before its output, so this is output for no attachment.
            _ => return,
        };
        #[cfg(test)]
        {
            self.applies += 1;
        }
        let state = view.read(cx).state;
        let (effects, applied) = view.update(cx, |view, cx| view.apply(run, cx));
        // The view redraws itself for what it draws, but this owner draws from the terminal's state too (the pane's
        // ground and the exit line), so a live state change must redraw it: a program that exits while watched sends
        // nothing but a new state.
        if live && view.read(cx).state != state {
            cx.notify();
        }
        // A program's own copy (OSC 52), as the old app applied it. The supervisor sends clipboard writes only once a
        // replay is complete, so a replay never repeats an old one.
        if let Some(text) = effects.clipboard_text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
        if let Err(error) = applied {
            self.fail(error, cx);
        }
    }

    /// The helm says the current attachment's replay is complete: show it.
    fn replay_complete(&mut self, cx: &mut Context<Self>) {
        // One marker per attachment; a second one has nothing staged and changes nothing.
        let Some(staging) = self.staging.take() else {
            return;
        };
        match staging.update(cx, |view, cx| view.mark_replay_complete(cx)) {
            Ok(()) => {
                self.committed = Some(staging);
                self.live = true;
                cx.notify();
            }
            Err(error) => self.fail(error, cx),
        }
    }

    /// The stream moved to `state`.
    fn stream_changed(&mut self, state: StreamState, cx: &mut Context<Self>) {
        // Whatever the new state, a replay that had not completed will not: drop it and keep the last complete screen.
        // An attachment going live starts a new one, because its replay rebuilds the grid from nothing.
        self.staging = None;
        self.live = false;
        if let StreamState::Attached(attachment) = state {
            self.attachment = Some(attachment);
            let metrics = self.metrics;
            self.staging = Some(cx.new(|_| TerminalView::new(metrics)));
        }
        self.stream = state;
        cx.notify();
    }

    /// End the stream over a message the grid model refused: the peer broke the format, so this attachment cannot go
    /// on (the format's rule). The error's wording is the model's or decoder's own, never peer text.
    fn fail(&mut self, error: GridError, cx: &mut Context<Self>) {
        tracing::warn!(%error, "a terminal stream broke the grid format; closing it");
        self.fault = Some(format!("Terminal stream error: {error}").into());
        self.staging = None;
        self.live = false;
        if let Some(handle) = &self.handle {
            handle.close();
        }
        cx.notify();
    }

    /// A key reached the pane's listener. Only the program's keys are sent, and only while live; every key that gets
    /// here is consumed, whoever it belongs to (see the module docs, "Input").
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Only the pane that holds focus types into its program. Nothing inside the pane takes focus today, so this is
        // a guard for when something does (a link card, say): that element's keys are its own, not the program's.
        if !self.focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        let group = match actions::terminal_key_owner(event) {
            KeyOwner::App | KeyOwner::Terminal => return,
            KeyOwner::Program => {
                program_input(event, actions::THIS_PLATFORM, &mut self.composition)
            }
        };
        self.send(group);
    }

    /// Send `group` to the program, stamped with the live attachment, as one request. Dropped unless the terminal is
    /// live, and so is an empty group.
    fn send(&mut self, group: Vec<GridInput>) {
        if !self.is_live() || group.is_empty() {
            return;
        }
        if let (Some(handle), Some(attachment)) = (&self.handle, self.attachment) {
            handle.input(attachment, group);
        }
    }

    /// The cursor as it should be drawn now, and the blink's ticks held or dropped to match. Called from `render`:
    /// everything it reads redraws this owner when it changes, so working it out there cannot miss a change. A focus
    /// move and a window's activation redraw the whole window, and the blink's ticks notify this owner. Output notifies
    /// the shown view, not this owner (unless the terminal's state changed), but GPUI redraws a notified view's
    /// ancestors with it, so a program that turns the blink on reaches this too.
    fn cursor_phase(&mut self, window: &Window, cx: &mut Context<Self>) -> CursorPhase {
        let focused =
            self.is_live() && self.focus.is_focused(window) && ticker::is_active(window, cx);
        if !focused {
            self.composition = Composition::default();
        }
        let blinks = focused
            && self.committed.as_ref().is_some_and(|view| {
                view.read(cx)
                    .cursor
                    .is_some_and(|cursor| cursor.visible && cursor.blink)
            });
        // A new handle starts the blink on its shown half, so a terminal that takes focus shows its cursor at once.
        if blinks != self.blink.is_some() {
            self.blink = blinks.then(|| ticker::blink(window, cx));
        }
        CursorPhase {
            focused,
            on: self.blink.as_ref().is_none_or(|blink| blink.is_on(cx)),
        }
    }

    /// The UI session this owner was made for is gone: close the stream, cancel delivery and drop the screens.
    fn retire(&mut self, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.retired = true;
        self.opener = None;
        if let Some(handle) = self.handle.take() {
            handle.close();
        }
        self.pump = None;
        self.committed = None;
        self.staging = None;
        self.live = false;
        self.blink = None;
        cx.notify();
    }

    /// Where the shown screen goes in the room it has: how far it is lifted, and its cell height and row count. `None`
    /// until the room and the display's scale have been measured, when the screen is drawn unlifted.
    ///
    /// NOTE: The room is measured after the frame that draws it, so when the status line appears or goes away, the
    /// lift follows one frame later: for one frame the newest rows are clipped under a new band, or the screen keeps a
    /// lift it no longer needs. Accepted for the first milestone.
    fn placement(&self, shown: &TerminalView) -> Option<Placement> {
        let (scale, room) = (self.scale?, self.room?);
        let rows = u16::try_from(shown.rows.len()).unwrap_or(u16::MAX);
        let cell_height = shown.metrics.cell_size(shown.cols, rows, scale).height;
        let exit = shown.state.and_then(|state| state.exit).is_some();
        let band = self.status_line().is_some();
        Some(Placement {
            lift: lift(rows, cell_height, room, exit, band),
            cell_height,
            rows,
        })
    }
}

/// Where the shown screen is drawn: lifted by `lift` pixels, `rows` rows of `cell_height`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    lift: f32,
    cell_height: f32,
    rows: u16,
}

/// The grid size that fits a `width` by `height` area (logical pixels, the pane less its padding) on a display of
/// `scale`, by the old app's fit: the cell of an 80x24 reference grid (a fixed reference keeps one fit from depending
/// on the previous answer), the scrollbar's column left out, never below 2x1 and never past 1000x1000. `None` for an
/// area with no room at all, as for a pane that has not been laid out, which proposes nothing.
///
/// Ported from the old app's `fit` and `proposeSize` (grid-terminal.js at the reference commit), which came from
/// @xterm/addon-fit 0.11.0's `proposeDimensions` (MIT; notices in `farhelm-input-mapping`'s LICENSE-PORTED, where the
/// cell rounding lives).
fn fit(metrics: CellMetrics, width: f32, height: f32, scale: f32) -> Option<(u16, u16)> {
    if !(width > 0. && height > 0.) {
        return None;
    }
    let cell = Geometry::for_character(
        f64::from(metrics.char_width),
        f64::from(metrics.char_height),
        80,
        24,
        f64::from(scale),
    )?;
    // The old fit read the container's CSS size with `parseInt`, so it saw whole pixels.
    let cols = ((f64::from(width.floor()) - f64::from(SCROLLBAR_WIDTH)) / cell.cell_width).floor();
    let rows = (f64::from(height.floor()) / cell.cell_height).floor();
    Some((
        cols.clamp(2., f64::from(MAX_DIMENSION)) as u16,
        rows.clamp(1., f64::from(MAX_DIMENSION)) as u16,
    ))
}

/// The exit line's words, the old app's `exitText`.
pub(crate) fn exit_text(exit: ExitState) -> String {
    match exit {
        ExitState::Code(code) => format!("Process exited with status {code}"),
        ExitState::Signal(signal) => format!("Process ended by signal {signal}"),
    }
}

/// What the program gets for a key that is its own, typed on `platform`: the key itself, the text it typed, or nothing.
///
/// `map_key` (`farhelm-input-mapping`, ported from the old app's key handling) decides. A key it sends is one
/// `Key` message. A key it leaves to text input (a Shift letter, a space, an Option character on a Mac) becomes the
/// text the platform says the key typed, in `Text` messages: with no input method handler, nothing else would insert
/// it. A dead key, a key it cancels and anything it does not handle send nothing. Its local decisions (paging, select
/// all) are the keymap's to make, which already gave those keys to the terminal, so they send nothing here either.
///
/// NOTE: The `Text` branch and the listener's `stop_propagation` for these keys must change together when the
/// terminal gets an input method handler. GPUI hands text to a handler only for a key that keeps propagating, so with
/// the listener as it is a handler would never see these keys (and input methods and Mac dead keys would stay broken);
/// letting them propagate while keeping this branch would send each one twice. The module docs, "Input", name what
/// this branch cannot do.
fn program_input(
    event: &KeyDownEvent,
    platform: Platform,
    composition: &mut Composition,
) -> Vec<GridInput> {
    let keystroke = &event.keystroke;
    let modifiers = keystroke.modifiers;
    let native = event.native_key.as_ref().map(|native| NativeKey {
        identity: match &native.identity {
            NativeKeyIdentity::Xkb(name) => NativeIdentity::Xkb(name.as_ref()),
            NativeKeyIdentity::Mac(code) => NativeIdentity::Mac(*code),
        },
        shift: native.shift,
    });
    let key = KeyDown {
        native,
        modifiers: farhelm_input_mapping::Modifiers {
            control: modifiers.control,
            alt: modifiers.alt,
            shift: modifiers.shift,
            platform: modifiers.platform,
            function: modifiers.function,
        },
        key: &keystroke.key,
        key_char: keystroke.key_char.as_deref(),
        is_held: event.is_held,
        prefer_character_input: event.prefer_character_input,
    };
    let route = Route {
        terminal_focused: true,
        ..Route::default()
    };
    match map_key(key, platform, composition, route) {
        Decision::Send(input) => vec![input],
        Decision::Text { cancel: false } => match keystroke.key_char.as_deref() {
            // This is the committed insert, so it ends a dead-key sequence, as an input method handler's commit would
            // (the old app cleared it on every text it sent). Left set, the mapping would hold back the next key that
            // sends a sequence: Enter after a composed `é` would send nothing.
            Some(text) => {
                composition.dead_pending = false;
                committed_text(text)
            }
            None => Vec::new(),
        },
        Decision::Text { cancel: true }
        | Decision::Composition
        | Decision::AppShortcut
        | Decision::Suppress
        | Decision::PageUp
        | Decision::PageDown
        | Decision::SelectAll
        | Decision::Unhandled
        | Decision::Unsupported => Vec::new(),
    }
}

/// How far the screen moves up to fit `room`: whole rows, at most all of them, so no row is left half visible and the
/// newest rows (a program's latest output, a shell's prompt) are the ones that show.
///
/// With an `exit` line the screen needs a row more, and any shortfall lifts it, so the line never covers a row: the old
/// app's `layoutExit`. Without one the screen is lifted only while the status line shows (`band`), which takes room
/// from the top of the pane while the fit (deliberately) ignores it. Otherwise nothing is lifted, whatever the
/// shortfall: the fit sizes rows by an 80x24 reference cell while the grid is drawn with its own, so at fractional
/// scales (1.25, 1.75, 2.5) a fitted grid can be a pixel or two taller than its room, and lifting a whole row of a live
/// terminal away for that would hide its top row for nothing. The bottom few pixels are clipped instead.
fn lift(rows: u16, cell_height: f32, room: f32, exit: bool, band: bool) -> f32 {
    let rows_height = f32::from(rows) * cell_height;
    let (needed, tolerance) = if exit {
        (rows_height + cell_height, 0.)
    } else if band {
        (rows_height, 0.)
    } else {
        return 0.;
    };
    let shortfall = needed - room;
    if shortfall > tolerance && cell_height > 0. {
        rows_height.min((shortfall / cell_height - 1e-6).ceil() * cell_height)
    } else {
        0.
    }
}

impl AttachedTerminal {
    /// The status line: the old terminal's red `.banner` (the shared [`Banner`]), with `.banner-reclaim`'s Take
    /// control. It sits above the grid in the pane's flow, as the old banner did, but the fit measures the whole pane,
    /// so showing or hiding it never resizes the program; the screen is lifted by whole rows instead (see [`lift`]).
    fn banner(&self, line: StatusLine, cx: &mut Context<Self>) -> Banner {
        let mut sentence = HostSentence::new("terminal-status").text(line.words);
        if let Some(reason) = &line.reason {
            sentence = sentence.host(reason);
        }
        let owner = cx.weak_entity();
        Banner::new(sentence).when(line.take_control, |banner| {
            banner.action(
                Button::new(
                    "terminal-take-control",
                    "Take control",
                    HoverHelp::tooltip(TAKE_CONTROL_HELP),
                    Role::Button,
                )
                .on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |owner, cx| owner.take_control(cx));
                }),
            )
        })
    }

    /// The shown screen, lifted to fit its room, with its exit line below its last row.
    fn screen(&self, view: &Entity<TerminalView>, cx: &App) -> Vec<AnyElement> {
        let shown = view.read(cx);
        let Some(placement) = self.placement(shown) else {
            return vec![view.clone().into_any_element()];
        };
        let screen = div()
            .mt(px(-placement.lift))
            .child(view.clone())
            .into_any_element();
        let Some(exit) = shown.state.and_then(|state| state.exit) else {
            return vec![screen];
        };
        let cell_height = placement.cell_height;
        let (fg, bg) = style::defaults(shown.state);
        // The terminal's defaults reversed, so the line reads as the app's own rather than part of the program's
        // output, as in the old app.
        let line = div()
            .debug_selector(|| "terminal-exit".into())
            .absolute()
            .top(px(f32::from(placement.rows) * cell_height - placement.lift))
            .left_0()
            .right_0()
            .h(px(cell_height))
            .px(px(4.))
            .truncate()
            .font_family(TERMINAL_FONT)
            .text_size(shown.metrics.font_size())
            .line_height(px(cell_height))
            .text_color(bg.hsla(1.))
            .bg(fg.hsla(1.))
            .child(exit_text(exit))
            .into_any_element();
        vec![screen, line]
    }
}

impl Render for AttachedTerminal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("attached-terminal", cx);
        // Handed to the shown view before it draws in this same frame, as a parent passes a child its props: the view's
        // own notify is not acted on while the frame is drawn, and is not needed, since the view draws next. That holds
        // only while the view stays uncached (drawn as a plain view in `screen`): a cached view would check whether it
        // is dirty against what was known before this frame, keep its old paint and show the old cursor.
        let phase = self.cursor_phase(window, cx);
        if let Some(view) = &self.committed {
            view.update(cx, |view, cx| view.set_cursor_phase(phase, cx));
        }
        let state = self.committed.as_ref().and_then(|view| view.read(cx).state);
        let (_, ground) = style::defaults(state);
        let status = self.status_line();
        let placeholder =
            (status.is_none() && self.committed.is_none() && !self.retired).then(|| {
                div()
                    .debug_selector(|| "terminal-connecting".into())
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .p(px(PLACEHOLDER_PADDING))
                    .text_size(px(PLACEHOLDER_TEXT))
                    .text_color(theme::color(theme::palette(cx).fg_dim))
                    .child(CONNECTING_TEXT)
            });
        let owner = cx.weak_entity();
        // Measures the room the screen has, below the status line and inside the padding, handed over after the frame
        // like the pane's fit. It is there before any screen is, so a first screen is placed in its first frame.
        let room = {
            let owner = owner.clone();
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    let height = f32::from(bounds.size.height);
                    window.defer(cx, move |_, cx| {
                        let _ = owner.update(cx, |owner, cx| owner.grid_measured(height, cx));
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };
        let screen = self
            .committed
            .as_ref()
            .map(|view| self.screen(view, cx))
            .unwrap_or_default();
        // The grid's area is what takes focus and keys, not the whole pane: the status line above it holds a Take
        // control button, whose own Tab and Escape the terminal's key context would otherwise give to the program
        // (`actions::TERMINAL_KEYS` binds them to nothing there), leaving a focused button with no way off it.
        let body = div()
            .id("terminal-grid")
            .key_context(actions::TERMINAL_CONTEXT)
            .on_key_down(cx.listener(Self::key))
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .p(px(PANE_PADDING))
            .child(
                div()
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .child(room)
                    .children(screen),
            );
        let body = focus::track(body, &self.focus, OnPress::Focus);
        // Measures the whole pane, banner included (see `banner`). The measurement is handed over after the frame:
        // a notify during paint would be lost.
        let measure = canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                let scale = window.scale_factor();
                window.defer(cx, move |_, cx| {
                    let _ = owner.update(cx, |owner, cx| owner.fitted(bounds, scale, cx));
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        div()
            .id("attached-terminal")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(ground.hsla(1.))
            .children(status.map(|line| self.banner(line, cx)))
            .child(body)
            .children(placeholder)
            .child(measure)
    }
}

/// A future that is pending once, waking itself, so the task awaiting it goes to the back of its executor's queue.
struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// The test bridge's `terminal_text` probe, which `scripts/gui-agent.py terminal-text` reads: the screen of the
/// terminal that holds focus in its window, which is the one a script's typing goes to. With focus elsewhere (a
/// control the script clicked, say) it reads nothing, rather than guessing which terminal was meant. The registry and
/// its reader are compiled into test builds too, so ordinary test runs cover them; only the bridge registration is
/// agent-only.
#[cfg(any(feature = "agent", test))]
mod probe {
    use gpui::{App, Global, WeakEntity};

    use super::AttachedTerminal;

    /// Every owner made so far, oldest first; dropped owners are pruned as new ones enroll.
    #[derive(Default)]
    struct Owners(Vec<WeakEntity<AttachedTerminal>>);

    impl Global for Owners {}

    /// Record a new owner, and make sure the bridge knows the probe.
    pub(super) fn enroll(owner: WeakEntity<AttachedTerminal>, cx: &mut App) {
        let owners = cx.default_global::<Owners>();
        owners.0.retain(|owner| owner.upgrade().is_some());
        owners.0.push(owner);
        #[cfg(feature = "agent")]
        gpui_agent::probe(cx, "terminal_text", |cx| {
            terminal_text(cx).map_or(serde_json::Value::Null, serde_json::Value::String)
        });
    }

    /// The screen text of the owner whose pane holds focus in its window; `None` when none does, or it shows no screen
    /// yet. Asks each window rather than trusting a flag the owner keeps, so an owner that stopped being drawn while
    /// it held focus is not read: focus moved on without it. Each window can have a focused terminal, so the active
    /// window's comes first, as that is the one a script types into; among inactive ones the oldest owner wins.
    pub(super) fn terminal_text(cx: &mut App) -> Option<String> {
        let owners: Vec<_> = cx
            .try_global::<Owners>()?
            .0
            .iter()
            .filter_map(|owner| owner.upgrade())
            .collect();
        let windows = cx.windows();
        // Per owner: whether its pane holds focus in some window, and whether that window is active.
        let focus_of = |owner: &gpui::Entity<AttachedTerminal>, cx: &mut App| {
            let focus = owner.read(cx).focus.clone();
            windows.iter().find_map(|window| {
                window
                    // The ticker's notion of active, which the headless bridge runs pin on.
                    .update(cx, |_, window, cx| {
                        focus
                            .is_focused(window)
                            .then(|| super::ticker::is_active(window, cx))
                    })
                    .ok()
                    .flatten()
            })
        };
        let mut fallback = None;
        for owner in owners {
            match focus_of(&owner, cx) {
                Some(true) => return owner.read(cx).terminal_text(cx),
                Some(false) if fallback.is_none() => fallback = Some(owner),
                _ => {}
            }
        }
        fallback?.read(cx).terminal_text(cx)
    }
}

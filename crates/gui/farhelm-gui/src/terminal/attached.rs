//! One attached session terminal: the owner between a helm client terminal stream and the renderer.
//!
//! NOTE: This is only the output side, cut to what the first milestone's slice needs (the migration's orchestrator
//! trimmed it on 2026-11-13): attach a terminal, show its output, say when it is not live. Input, focus, scrollback,
//! selection, links and the reconnect display with its phase and Retry come later in the terminal area's work. Nothing
//! mounts this in the app's window yet; the session view does that.
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

#[cfg(test)]
mod tests;

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

use farhelm_grid_model::{ExitState, GridError, GridOutput};
use farhelm_helm_client::terminal::{
    AttachmentLease, StreamState, TerminalAttach, TerminalEvent, TerminalEvents, TerminalHandle,
};
use farhelm_input_mapping::pointer::Geometry;
use farhelm_proto::DetachCode;
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClipboardItem, Context, Entity, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, SharedString, Styled, Subscription, Task, Window,
    canvas, div, prelude::FluentBuilder as _, px,
};

use super::{CellMetrics, TerminalView, style};
use crate::assets::TERMINAL_FONT;
use crate::components::{Banner, Button, HoverHelp, Role};
use crate::host_text::{HostSentence, HostText};
use crate::render_count;
use crate::session::UiSession;
use crate::theme;

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
            _session: subscriptions,
            #[cfg(test)]
            turns: 0,
            #[cfg(test)]
            applies: 0,
        }
    }

    /// Whether the terminal shows its live attachment's complete screen and carries input. Input (the terminal area's
    /// next step) is sent only while this holds.
    pub fn is_live(&self) -> bool {
        self.live && self.fault.is_none() && !self.retired
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
            StreamState::Attaching | StreamState::Attached => None,
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
        if state == StreamState::Attached {
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_count::record("attached-terminal", cx);
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
        let body = div()
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

/// The test bridge's `terminal_text` probe, which `scripts/gui-agent.py terminal-text` reads.
///
/// NOTE: The probe is meant to read the focused terminal, but terminals take focus only with the input side. Until
/// then it reads the newest owner that shows a screen. The registry and its reader are compiled into test builds too,
/// so ordinary test runs cover them; only the bridge registration is agent-only.
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

    /// The newest live owner's screen text; `None` when no owner shows one.
    pub(super) fn terminal_text(cx: &App) -> Option<String> {
        let owners = cx.try_global::<Owners>()?;
        owners
            .0
            .iter()
            .rev()
            .filter_map(|owner| owner.upgrade())
            .find_map(|owner| owner.read(cx).terminal_text(cx))
    }
}

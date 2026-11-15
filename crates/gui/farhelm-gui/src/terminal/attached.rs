//! One attached session terminal: the owner between a helm client terminal stream and the renderer.
//!
//! NOTE: This is cut to what the first milestone's slice needs (the migration's orchestrator trimmed it on 2026-11-13):
//! attach a terminal, show its output, say when it is not live, take focus and send the program its keys. The terminal
//! area added paste, committed text from the platform's text input, focus reports, scrollback, the local selection,
//! mouse reporting, web links with a program's hyperlink's destination display, and the Mac's tunnel offer for a remote
//! host's loopback links ([`super::tunnels`]) since. File links and the reconnect display with its phase and Retry come
//! later in that work. The session pane ([`crate::session_pane`]) mounts one for
//! the selected session.
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
//!   why, with a Take control button where the stream waits for the user. Its owner can hold it back, with the exit
//!   line, while something else explains what is happening ([`AttachedTerminal::hold_notices`]).
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
//! - The terminal's own commands have their actions, whose bindings take their keys before this listener sees them:
//!   paste ([`actions::Paste`], [`AttachedTerminal::paste`]), the page keys, Shift+PageUp and Shift+PageDown
//!   ([`actions::ScrollTerminalPageUp`] and its sibling), and on a Mac select all and copy, Cmd+A and Cmd+C
//!   ([`actions::SelectAllInTerminal`], [`actions::CopyTerminalSelection`]). Should one of their keys reach the
//!   listener anyway it is dropped rather than sent, so a program never learns to depend on getting it.
//! - Every other key is the program's. `farhelm-input-mapping`'s `map_key` decides what it means (the supervisor
//!   encodes the bytes). A key it sends as a key goes out as one, and the listener stops it there. A key it leaves to
//!   text input (a Shift letter, a space, an Option character, a dead key, a key typed while an input method composes)
//!   keeps propagating, so GPUI hands it to the platform's text input, which commits whatever text it makes through the
//!   terminal's input handler (the `EntityInputHandler` impl below): the character, a Mac dead key's `é` once the
//!   letter follows, an input method's string. That text goes to the program once, as typed text. This is the old
//!   app's split too, where the browser's hidden input element supplied the text. A paging decision (Shift+PageUp or
//!   Shift+PageDown with Ctrl, Alt or Cmd also held, which the keymap does not bind) pages the history, as the bindings
//!   do, and sends nothing. Held keys repeat to the program, as in any terminal: the keymap's repeat guard
//!   (`actions::ONCE_PER_PRESS`) stops a held app shortcut's repeats before they get here, and `map_key` holds back
//!   nothing else.
//!
//! NOTE: On Linux a key's text reaches text input only when at most Shift is held (GPUI's Linux rule, on Wayland and
//! X11 alike), so a key the mapping leaves to text input with Alt or Ctrl+Alt held types nothing there. The old app
//! never had a Linux window to compare with; the gap is recorded, not built around.
//!
//! An input method's composition in progress (marked text) is not shown at the cursor, and its candidate window sits
//! wherever the platform puts it for a field it cannot locate, as in the old app: input method work stays at what
//! GPUI gives (decided by the maintainer on 2026-11-12). Shift+Enter is a key the mapping sends, so it still reaches
//! the program as one write. The keymap's once-per-press guard cannot see a composition (actions.rs says why); a
//! listed shortcut typed mid-composition still runs, which is recorded, not built around.
//!
//! When the program asks for focus reports (mode 1004), it is told when the terminal gains or loses focus, and the
//! current state when it starts asking or a new attachment goes live ([`AttachedTerminal::report_focus`]).
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

use std::cell::Cell;
use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

use farhelm_grid_model::{
    ExitState, GridError, GridOutput, LinkKind, Position, Selection, SelectionMode, TerminalLink,
    tunnel_port, web_target,
};
use farhelm_helm_client::terminal::{
    Attachment, AttachmentLease, StreamState, TerminalAttach, TerminalEvent, TerminalEvents,
    TerminalHandle,
};
use farhelm_input_mapping::pointer::{
    Geometry, MouseDeduper, WheelAccumulator, WheelUnit, mode_reports, wheel_goes_to_program,
    wheel_pixels,
};
use farhelm_input_mapping::{
    Composition, Decision, KeyDown, NativeIdentity, NativeKey, Platform, Route, committed_text,
    map_key, pasted_text,
};
use farhelm_proto::DetachCode;
use farhelm_proto::grid::{GridInput, MouseEventKind};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClipboardEntry, ClipboardItem, Context,
    DispatchPhase, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, HoverListenerMode,
    InteractiveElement, IntoElement, KeyDownEvent, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, NativeKeyIdentity, ParentElement, Pixels, Point, Render,
    ScrollDelta, ScrollEvent, SharedString, StatefulInteractiveElement, Styled, Subscription, Task,
    UTF16Selection, WeakEntity, Window, canvas, div, prelude::FluentBuilder as _, px,
};

use super::tunnels::{self, Tunnels};
use super::{CellMetrics, CursorPhase, TerminalView, style};
use crate::actions::{self, KeyOwner};
use crate::assets::TERMINAL_FONT;
use crate::components::{
    Banner, Button, HoverHelp, Role, SCROLLBAR_WIDTH, Scrollbar, ScrollbarEvent,
};
use crate::focus::{self, Mark, OnPress};
use crate::host_text::{HostSentence, HostText};
use crate::layers::{self, Anchor, LayerId, Popover};
use crate::render_count;
use crate::session::{self, UiSession};
use crate::{theme, ticker};

/// The most stream events one GUI turn takes before it yields. The prototype's bound (eight messages, at most 2 MiB);
/// the byte half is gone because the helm client hands over decoded messages, but each one came from a frame of at
/// most 1 MiB, so a turn still handles a bounded amount of output.
pub(crate) const BATCH: usize = 8;

/// Padding between the pane's edge and the grid: the old `.terminal`'s 4 pixels plus the `.grid-terminal`'s 4 inside
/// it, at the reference commit.
const PANE_PADDING: f32 = 8.;

/// How long the scrollbar stays shown after a scroll once the pointer is off the terminal (`SCROLLBAR_FADE_MS` in
/// grid-terminal.js, which faded it out over that time instead; see the scrollbar component on why there is no fade).
const SCROLLBAR_HOLD: Duration = Duration::from_millis(800);

/// The scrollbar's hover help. The old bar had none; the component layer gives every control one.
const SCROLLBAR_HELP: &str = "Drag to scroll back through the terminal's history";

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

/// Says, each time the terminal draws, whether its own notices are held back now (see
/// [`AttachedTerminal::hold_notices`]). Asked rather than told, so the answer comes from wherever the caller keeps it
/// and this owner keeps no copy that could go stale.
pub type NoticeHold = Box<dyn Fn(&App) -> bool>;

/// The owner of one attached terminal. See the module docs.
pub struct AttachedTerminal {
    target: TerminalTarget,
    /// Holds the status line and the exit line back while it says so; `None` holds nothing.
    hold: Option<NoticeHold>,
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
    /// Where the grid's area (inside the padding, below the status line) sits in the window, measured with `room`:
    /// where a wheel event's position is turned into a cell.
    grid_origin: Option<Point<Pixels>>,
    /// The part of a line the wheel has gathered toward the program's next wheel report, per terminal.
    program_wheel: WheelAccumulator,
    /// Whether the pointer is over the grid's area, which shows the scrollbar (when there is history to scroll).
    pointer_over: bool,
    /// Keeps the scrollbar shown for [`SCROLLBAR_HOLD`] after the last scroll; it clears itself when it fires.
    scrollbar_hold: Option<ticker::Deadline>,
    /// The local selection being dragged, from its press to its release.
    select: Option<SelectGesture>,
    /// A primary press on a link, until its release decides whether it was a click.
    link_press: Option<LinkPress>,
    /// Where the pointer last rested over the grid (window coordinates), by which a destination display is anchored.
    hover_point: Option<Point<Pixels>>,
    /// The destination display of the program hyperlink the pointer rests on, and the anchor it is drawn by.
    disclosure: Option<Disclosure>,
    /// The anchor the current display is drawn by, built as the owner draws.
    disclosure_anchor: Option<Anchor>,
    /// How many displays this terminal has opened, which numbers the next.
    disclosure_count: u64,
    /// Whether a click starting with the press now being handled may open the link it is on, decided in the press's
    /// capture phase ([`AttachedTerminal::click_admitted`]) and moved into its [`LinkPress`] as the press is handled.
    press_admitted: bool,
    /// The explanation of a refused link click, shown at the top of the terminal until the next press there.
    refusal: Option<SharedString>,
    /// Set by a refused click and cleared once the pointer moves to another cell: while set, no destination display
    /// shows, so the refusal's advice (enlarge the window and hover again) means hovering again, as in the old app,
    /// and the display that was not fully visible does not keep sitting over the explanation.
    refusal_raised: bool,
    /// The session's tunnels: what the destination display offers for a remote host's loopback link, and what a
    /// Cmd-click opens through ([`super::tunnels`]). One per terminal until terminal tabs share one per session.
    tunnels: Entity<Tunnels>,
    /// The platform the tunnel offer is decided for: this one, except in tests, which decide it for the Mac on any
    /// machine ([`AttachedTerminal::set_platform`]).
    platform: Platform,
    /// The buttons held in a press the program's mouse reporting took, as a bit per button (1 left, 2 middle, 4
    /// right); 0 while none is. Moves and releases go to the program until the last of them comes up.
    reporting: u8,
    /// The last mouse report sent, so motion within one cell is not reported again (the format leaves that to the
    /// client). Forgotten with each new attachment.
    mouse_dedupe: MouseDeduper,
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
    /// The length, in UTF-16 units, of the text an input method is composing (its marked text), which is never drawn
    /// and never sent; `None` while nothing is being composed. Kept beside `composition.marked`, which the input
    /// mapping reads, through [`AttachedTerminal::set_marked`].
    marked: Option<usize>,
    /// The focus state last reported to the program (mode 1004), for the attachment that is live now; `None` when
    /// nothing has been reported to it, or it is not asking for reports. See [`AttachedTerminal::report_focus`].
    reported_focus: Option<bool>,
    /// Whether the pane holds focus in a window the platform says is active, as last seen while drawing or when the
    /// window's activation changed ([`AttachedTerminal::window_focus`]). Kept so that a report due without a frame (a
    /// program that starts asking, a reconnect going live, while the window is hidden and draws nothing) can still be
    /// sent at once. A focus move asks for a frame, so in an active window this can lag one by a frame (a program that
    /// starts asking in that gap hears the old state, then the correction); in an inactive window it is false wherever
    /// focus sits.
    window_focus: bool,
    /// The cursor blink's ticks, held while the cursor blinks (see the module docs, "Input").
    blink: Option<ticker::Blinking>,
    /// The latest paste still waiting for the clipboard, which holds the one before it (see
    /// [`AttachedTerminal::paste`]). Dropped on retirement, which cancels every waiting paste.
    paste: Option<Task<()>>,
    /// The input handler GPUI is given each frame ([`TerminalInputHandler`]), made once so its identity is stable.
    text_input: Entity<TerminalInputHandler>,
    /// Reports focus to the program when the window's activation changes, at the moment it changes (see
    /// [`AttachedTerminal::report_focus`]). Set up on the first render, which is where the owner first has its window.
    activation: Option<Subscription>,
    _session: [Subscription; 2],
    /// Redraws the terminal when its tunnels change, since it draws their failure notice.
    _tunnels: Subscription,
    /// Reads the session's tunnels when the pushed state says they are due ([`AttachedTerminal::keep_tunnels_current`]):
    /// as the terminal is made, and whenever the store changes (the session's host becoming known as remote, the shared
    /// tunnel consent changing).
    _tunnel_reads: session::ObservingCurrent,
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
        let tunnels = cx.new(|cx| Tunnels::new(target.session.clone(), cx));
        let tunnel_watch = cx.observe(&tunnels, |_, _, cx| cx.notify());
        let tunnel_reads =
            session::observe_current(cx, |owner: &mut Self, cx| owner.keep_tunnels_current(cx));
        AttachedTerminal {
            target,
            hold: None,
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
            grid_origin: None,
            program_wheel: WheelAccumulator::default(),
            pointer_over: false,
            scrollbar_hold: None,
            select: None,
            link_press: None,
            hover_point: None,
            disclosure: None,
            disclosure_anchor: None,
            disclosure_count: 0,
            press_admitted: false,
            refusal: None,
            refusal_raised: false,
            tunnels,
            platform: actions::THIS_PLATFORM,
            reporting: 0,
            mouse_dedupe: MouseDeduper::default(),
            retired,
            focus: cx.focus_handle().tab_stop(true),
            text_input: {
                let owner = cx.weak_entity();
                cx.new(|_| TerminalInputHandler { owner })
            },
            activation: None,
            attachment: None,
            composition: Composition::default(),
            marked: None,
            reported_focus: None,
            window_focus: false,
            blink: None,
            paste: None,
            _session: subscriptions,
            _tunnels: tunnel_watch,
            _tunnel_reads: tunnel_reads,
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

    /// Hold back this terminal's own notices, the status line and the exit line, whenever `hold` says so, and show them
    /// again, as the stream and program then are, once it stops saying so. Nothing is dropped: the notices are drawn
    /// from the terminal's current state, so a notice that arose while held appears on release if it still applies.
    ///
    /// The session pane holds them while a Delete of its session is in flight (SPEC.md, "Lifecycle operations": the
    /// terminal's own "Detached" notice, which the delete itself causes, is held back meanwhile), because the Delete
    /// kills the agent and detaches the viewer, and those notices would otherwise read as failures. Only drawing
    /// changes: a stream that waits for Take control still waits, input still goes out only while live, and
    /// [`AttachedTerminal::status_line`] keeps reporting the stream's own line.
    ///
    /// The hold does not tell the Delete's notices from others: one already showing when the hold begins (an ended
    /// agent's exit line under "Stopping tabs…", a takeover's band with Take control) goes too, and the screen drops
    /// the lift it had for it. That is accepted because the pane's scrim covers the terminal for exactly as long, and
    /// the notice comes back on release if it still applies.
    ///
    /// `hold` is asked while drawing, so whatever it reads must notify when its answer changes.
    pub fn hold_notices(&mut self, hold: NoticeHold) {
        self.hold = Some(hold);
    }

    /// Whether the notices are held back now.
    fn notices_held(&self, cx: &App) -> bool {
        self.hold.as_ref().is_some_and(|hold| hold(cx))
    }

    /// How far the shown screen is lifted now (see [`lift`]), for the session pane's tests, which check that held
    /// notices take no room; `None` before a screen is shown and measured.
    #[cfg(test)]
    pub(crate) fn shown_lift(&self, cx: &App) -> Option<f32> {
        let view = self.committed.as_ref()?;
        self.placement(view.read(cx), cx)
            .map(|placement| placement.lift)
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
    fn grid_measured(
        &mut self,
        origin: Point<Pixels>,
        height: f32,
        pointer: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.grid_origin = Some(origin);
        if self.room != Some(height) {
            self.room = Some(height);
            if self.committed.is_some() {
                cx.notify();
            }
        }
        // The screen can move under a still pointer without scrolling (lifted for the exit line or the status band, or
        // the pane moved), which leaves the hovered cell naming another row: look again where the pointer is. The view
        // skips the lookup when the cell is unchanged, so this costs nothing frame to frame.
        if self.pointer_over
            && self.select.is_none()
            && self.reporting == 0
            && self.link_press.is_none()
        {
            self.hover_link_at(Some(pointer), cx);
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
            // Not left to the redraw: a hidden window draws no frame, and a program that starts asking for focus
            // reports there must hear that it is unfocused now, not when the window next draws.
            self.report_focus(self.window_focus, cx);
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
                // A drag on the replaced view ends with it: its anchor names that view's lines.
                self.select = None;
                self.live = true;
                cx.notify();
                // As for a state change in `output`: a reconnect that goes live in a hidden window draws nothing.
                self.report_focus(self.window_focus, cx);
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
            // A new attachment's program has heard nothing from this one, so it gets the current focus state again
            // once it is live and asking, as the old app's freshly built grid did on every attach.
            self.reported_focus = None;
            // Nor has it had a mouse report: the next one goes out whatever the last one was.
            self.mouse_dedupe.reset();
            self.reporting = 0;
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
    /// here is consumed, whoever it belongs to, except one left to the platform's text input, which commits its text
    /// through the input handler (see the module docs, "Input").
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Only the pane that holds focus types into its program. Nothing inside the pane takes focus today, so this is
        // a guard for when something does (a link card, say): that element's keys are its own, not the program's.
        if !self.focus.is_focused(window) {
            return;
        }
        let route = match actions::terminal_key_owner(event) {
            KeyOwner::App | KeyOwner::Terminal => KeyRoute::Nothing,
            KeyOwner::Program => {
                program_input(event, actions::THIS_PLATFORM, &mut self.composition)
            }
        };
        match route {
            KeyRoute::Send(input) => {
                cx.stop_propagation();
                if let Some(attachment) = self.attachment {
                    self.send_user(attachment, vec![input], cx);
                }
            }
            KeyRoute::TextInput => {}
            KeyRoute::Page(direction) => {
                cx.stop_propagation();
                self.scroll_page(direction, cx);
            }
            KeyRoute::Nothing => cx.stop_propagation(),
        }
    }

    /// Send `group`, which the user made while `attachment` was live, to the program as one request. Dropped unless
    /// the terminal is live, and so is an empty group. The stream itself writes a group only to the attachment it is
    /// stamped for ([`TerminalHandle::input`]), so input made before a reconnect and sent after it (a paste whose
    /// clipboard read finished late) never lands in the later attachment.
    fn send(&mut self, attachment: Attachment, group: Vec<GridInput>) {
        if !self.is_live() || group.is_empty() {
            return;
        }
        if let Some(handle) = &self.handle {
            handle.input(attachment, group);
        }
    }

    /// Send input the user made (a key, typed or committed text, a paste, a wheel report) and bring the view back to
    /// the bottom, where it lands, as the old app did (grid-terminal.js's `sendUserInput` at the reference commit).
    /// Focus reports are the terminal's own and go through [`AttachedTerminal::send`], leaving the view where it is.
    fn send_user(&mut self, attachment: Attachment, group: Vec<GridInput>, cx: &mut Context<Self>) {
        if !self.is_live() || group.is_empty() {
            return;
        }
        // Input clears the selection and brings the view to the bottom, where it lands, as in the old app.
        if let Some(view) = &self.committed {
            view.update(cx, |view, cx| {
                view.follow(cx);
                view.set_selection(None, cx);
            });
        }
        self.send(attachment, group);
    }

    /// The shown screen's cell size on this display, `None` before it is measured.
    fn cell(&self, cx: &App) -> Option<gpui::Size<f32>> {
        let view = self.committed.as_ref()?.read(cx);
        let rows = u16::try_from(view.rows.len()).unwrap_or(u16::MAX);
        Some(view.metrics.cell_size(view.cols, rows, self.scale?))
    }

    /// The screen cell under `position` (window coordinates), clamped into the screen, or `None` before the grid is
    /// measured. The screen may be lifted for the exit or status line ([`lift`]), which moves its cells up with it.
    /// The lookup is the old app's mouse cell (`Geometry::cell`), where a point exactly on a boundary belongs to the
    /// cell before it.
    fn cell_at(&self, position: Point<Pixels>, cx: &App) -> Option<(u16, u16)> {
        let origin = self.grid_origin?;
        let view = self.committed.as_ref()?.read(cx);
        let placement = self.placement(view, cx)?;
        let cell = self.cell(cx)?;
        let geometry = Geometry {
            cell_width: f64::from(cell.width),
            cell_height: f64::from(placement.cell_height),
            columns: view.cols,
            rows: placement.rows,
        };
        geometry.cell(
            f64::from(f32::from(position.x - origin.x)),
            f64::from(f32::from(position.y - origin.y) + placement.lift),
        )
    }

    /// The wheel over the terminal, by the old app's policy (grid-terminal.js's wheel listener at the reference
    /// commit; `wheel_goes_to_program`): with mouse reporting on, or on the alternate screen, it is the program's, as
    /// one wheel message per event that moves at least a line (a mouse report, or arrow keys the supervisor makes on
    /// the alternate screen without reporting); otherwise it scrolls the terminal's own history. A wheel the program
    /// owns goes out only while the terminal is live; scrolling history works on any shown screen.
    fn wheel(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.committed.clone() else {
            return;
        };
        let Some(cell) = self.cell(cx) else {
            return;
        };
        cx.stop_propagation();
        let (unit, delta) = match event.delta {
            ScrollDelta::Pixels(delta) => (WheelUnit::Pixels, f64::from(f32::from(delta.y))),
            ScrollDelta::Lines(delta) => (WheelUnit::Lines, f64::from(delta.y)),
        };
        let modifiers = event.modifiers;
        let state = view.read(cx).state;
        let mouse = state.map_or(farhelm_proto::grid::MouseMode::Off, |state| state.mouse);
        let alternate = state.is_some_and(|state| state.alt_screen);
        if wheel_goes_to_program(mouse, alternate) {
            let (true, Some(attachment)) = (self.is_live(), self.attachment) else {
                return;
            };
            let mods = modifier_bits(modifiers);
            let rows = u16::try_from(view.read(cx).rows.len()).unwrap_or(u16::MAX);
            // The accumulator takes the browser's sign (positive toward newer lines), the opposite of GPUI's.
            let lines =
                self.program_wheel
                    .consume(-delta, unit, f64::from(cell.height), rows, mods);
            let Some((x, y)) = self.cell_at(event.position, cx) else {
                return;
            };
            if lines == 0.0 {
                return;
            }
            let button = if delta > 0.0 { 0 } else { 1 };
            // A wheel report resets the deduper's comparison like any report (the next move in this cell is sent);
            // on the alternate screen without reporting it is an arrow key, which the deduper never sees.
            if mouse != farhelm_proto::grid::MouseMode::Off {
                self.mouse_dedupe
                    .admit(MouseEventKind::Wheel, button, x, y, mods);
            }
            let wheel = GridInput::Mouse {
                kind: MouseEventKind::Wheel,
                button,
                x,
                y,
                mods,
            };
            self.send_user(attachment, vec![wheel], cx);
            return;
        }
        let pixels = wheel_pixels(
            delta,
            unit,
            modifiers.alt,
            modifiers.shift,
            actions::THIS_PLATFORM == Platform::Mac,
        );
        if pixels != 0. {
            view.update(cx, |view, cx| {
                view.scroll_pixels(pixels as f32, cell.height, cx)
            });
            self.hold_scrollbar(cx);
        }
    }

    /// Show the scrollbar for [`SCROLLBAR_HOLD`] from now, wherever the pointer is: after a scroll through the history
    /// (by the wheel, the page keys or the scrollbar), and when the pointer leaves the grid's area (the old app's
    /// `showScrollbarActivity`). A later call restarts the hold.
    fn hold_scrollbar(&mut self, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        self.scrollbar_hold = Some(ticker::after(SCROLLBAR_HOLD, cx, move |cx| {
            let _ = owner.update(cx, |owner, cx| {
                owner.scrollbar_hold = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Whether the scrollbar is drawn (when there is history to scroll): while the pointer is over the grid's area,
    /// and for [`SCROLLBAR_HOLD`] after a scroll or the pointer leaving, as in the old app.
    fn scrollbar_shown(&self) -> bool {
        self.pointer_over || self.scrollbar_hold.is_some()
    }

    /// A press or drag on the scrollbar moves the view there; letting go starts the hold, as a scroll does.
    fn scrollbar(&mut self, event: ScrollbarEvent, cx: &mut Context<Self>) {
        if let ScrollbarEvent::Scroll(index) = event
            && let Some(view) = &self.committed
        {
            view.update(cx, |view, cx| view.scroll_to_index(index, cx));
        }
        self.hold_scrollbar(cx);
    }

    /// The scrollbar over the shown screen's history, in the column the fit leaves free at the grid's right edge. It is
    /// drawn while the pointer is over the grid's area or a scroll was recent, and only when there is history to
    /// scroll, which the component itself decides.
    fn scrollbar_element(&self, view: &Entity<TerminalView>, cx: &Context<Self>) -> AnyElement {
        let shown = view.read(cx);
        let (fg, _) = style::defaults(shown.state);
        let owner = cx.weak_entity();
        Scrollbar::new(
            "terminal-scrollbar",
            "Terminal history",
            HoverHelp::tooltip(SCROLLBAR_HELP),
            Role::ScrollBar,
            shown.scroll_position(),
            fg.hsla(1.),
        )
        .shown(self.scrollbar_shown())
        .on_scroll(move |event, _, cx| {
            let _ = owner.update(cx, |owner, cx| owner.scrollbar(event, cx));
        })
        .into_any_element()
    }

    /// Shift+PageUp (`direction` -1) or Shift+PageDown (1): scroll the shown screen a page less a line, the old app's
    /// step, so the line at one edge stays in view as a landmark. Reaching the live screen follows it again. On the
    /// alternate screen there is no history, so nothing moves, and the key still does not reach the program.
    fn scroll_page(&mut self, direction: i64, cx: &mut Context<Self>) {
        let Some(view) = self.committed.clone() else {
            return;
        };
        let page = view.read(cx).rows.len().saturating_sub(1);
        let lines = direction * i64::try_from(page).unwrap_or(i64::MAX);
        view.update(cx, |view, cx| view.scroll_lines(lines, cx));
        self.hold_scrollbar(cx);
    }

    /// The user's Paste ([`actions::Paste`], from a key or a menu): send the clipboard's text to the program as one
    /// paste. Nothing happens unless the terminal is live.
    ///
    /// The paste is the clipboard's plain text, and nothing at all when the read reports a file or an image (see
    /// [`clipboard_text`]); those are for the attachment work. The vendored macOS read reports a copied file, so a
    /// Finder copy pastes nothing. The vendored Wayland read never reports a file list, only an image or text, so a
    /// file copied on Linux still pastes whatever text its file manager offers (usually its path) until the attachment
    /// work. The paste goes out as one group of the input mapping's paste parts, so a paste of any size (the old app's
    /// test pasted over a megabyte) arrives whole and in order or not at all. Bracketed paste, its markers, line endings
    /// and the sanitizing that strips an embedded end marker so the paste cannot end early are all the supervisor's
    /// (`grid_input.rs`), so nothing here rewrites the text.
    ///
    /// The clipboard is read asynchronously, because SPEC.md has nothing in Farhelm stall on a slow or hung clipboard
    /// and Wayland's synchronous read blocks the main thread on another program's pipe. Only Wayland's read is really
    /// asynchronous in the vendored GPUI: the macOS read answers at once from the main thread, so a hung pasteboard
    /// there still stalls the window, which is the vendored clipboard layer's to fix. So the paste is stamped with
    /// the attachment that was live when the user asked, and goes nowhere if a different one is live by the time the
    /// text arrives. Each paste waits for the one before it, so two quick pastes reach the program in the order asked
    /// even if their reads finish the other way round.
    fn paste(&mut self, _: &actions::Paste, _: &mut Window, cx: &mut Context<Self>) {
        let (true, Some(attachment)) = (self.is_live(), self.attachment) else {
            return;
        };
        let read = cx.read_from_clipboard_async();
        let earlier = self.paste.take();
        self.paste = Some(cx.spawn(async move |owner, cx| {
            if let Some(earlier) = earlier {
                earlier.await;
            }
            // A failed read is silent, as every clipboard failure is (SPEC.md, "Terminal experience").
            let Ok(Some(item)) = read.await else {
                return;
            };
            let Some(text) = clipboard_text(&item).filter(|text| !text.is_empty()) else {
                return;
            };
            let _ = owner.update(cx, |owner, cx| {
                owner.send_user(attachment, pasted_text(&text), cx)
            });
        }));
    }

    /// The cursor as it should be drawn now, and the blink's ticks held or dropped to match. Called from `render`:
    /// everything it reads redraws this owner when it changes, so working it out there cannot miss a change. A focus
    /// move and a window's activation redraw the whole window, and the blink's ticks notify this owner. Output notifies
    /// the shown view, not this owner (unless the terminal's state changed), but GPUI redraws a notified view's
    /// ancestors with it, so a program that turns the blink on reaches this too.
    fn cursor_phase(&mut self, window: &Window, cx: &mut Context<Self>) -> CursorPhase {
        let focused =
            self.is_live() && self.focus.is_focused(window) && ticker::is_active(window, cx);
        self.window_focus = self.focus_in_active_window(window);
        self.report_focus(self.window_focus, cx);
        if !focused {
            self.composition = Composition::default();
            self.marked = None;
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

    /// Whether the pane holds focus in a window the platform says is active: the focus state focus reports carry
    /// (whether the program is live and asking is [`AttachedTerminal::report_focus`]'s question).
    ///
    /// It asks GPUI about the window, not the ticker (`ticker::is_active`), which the cursor uses: agent builds pin the
    /// ticker's notion of an active window on, because headless Mutter reports every window inactive, and a focus
    /// report taken from that pin would never tell a program its window went to the background, in the bridge runs
    /// that check exactly that on a Mac. The flip side: on headless Linux, where Mutter reports every window inactive,
    /// a program asking for reports always hears that it is unfocused, while the cursor (on the ticker's pin) shows
    /// the terminal focused. Only bridge runs there are affected, and none of them asks for reports today.
    fn focus_in_active_window(&self, window: &Window) -> bool {
        self.focus.is_focused(window) && window.is_window_active()
    }

    /// Record what an input method is composing: `Some(length)` while it composes, `None` once it commits or gives up.
    /// The input mapping holds a composing key back from the program (`Composition::marked`), so both are set here.
    fn set_marked(&mut self, marked: Option<usize>) {
        self.marked = marked;
        self.composition.marked = marked.is_some();
    }

    /// Tell the program that its terminal gained or lost focus, when it asked to hear about it (mode 1004), as the old
    /// app did: once whenever the state changes, and once with the current state when the program starts asking or a
    /// new attachment goes live. `focused` is [`AttachedTerminal::focus_in_active_window`], so switching to another app
    /// reports focus out, as a browser's blur did in the old app.
    ///
    /// Called while drawing (a focus move redraws the whole window), from the window's activation observer, and where
    /// the program starts asking or a new attachment goes live, with the focus last seen. Not only while drawing,
    /// because a redraw is only a request for a frame: on macOS a window that is hidden (Cmd-H, minimized, fully
    /// covered) draws none, so a report made only while drawing would miss the switch away and, the state being
    /// unchanged by the time it draws again, the switch back too.
    fn report_focus(&mut self, focused: bool, cx: &App) {
        let asking = self.is_live()
            && self
                .committed
                .as_ref()
                .is_some_and(|view| view.read(cx).state.is_some_and(|state| state.focus));
        let report = asking.then_some(focused);
        if report == self.reported_focus {
            return;
        }
        self.reported_focus = report;
        if let (Some(focused), Some(attachment)) = (report, self.attachment) {
            self.send(attachment, vec![GridInput::Focus { focused }]);
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
        self.select = None;
        self.staging = None;
        self.live = false;
        self.blink = None;
        self.paste = None;
        cx.notify();
    }

    /// Where the shown screen goes in the room it has: how far it is lifted, and its cell height and row count. `None`
    /// until the room and the display's scale have been measured, when the screen is drawn unlifted.
    ///
    /// NOTE: The room is measured after the frame that draws it, so when the status line appears or goes away, the
    /// lift follows one frame later: for one frame the newest rows are clipped under a new band, or the screen keeps a
    /// lift it no longer needs. Accepted for the first milestone.
    fn placement(&self, shown: &TerminalView, cx: &App) -> Option<Placement> {
        let (scale, room) = (self.scale?, self.room?);
        let rows = u16::try_from(shown.rows.len()).unwrap_or(u16::MAX);
        let cell_height = shown.metrics.cell_size(shown.cols, rows, scale).height;
        // Held notices take no room, so the screen is not lifted for a line nobody sees.
        let held = self.notices_held(cx);
        let exit = !held && shown.state.and_then(|state| state.exit).is_some();
        let band = !held && self.status_line().is_some();
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

/// The text a paste sends from clipboard `item`: its plain-text entries, joined, or `None` when it holds a file or an
/// image at all.
///
/// SPEC.md's "Attachments" ranks a paste's flavors files first, then images, then plain text, so a clipboard holding a
/// file is a file paste even when it carries text too: a Finder copy on macOS adds the file's path as text, and GPUI's
/// own `ClipboardItem::text` falls back to the path. Turning file and image pastes into uploads is the attachment
/// work's; until then they paste nothing, rather than a path or label the user did not mean to type.
fn clipboard_text(item: &ClipboardItem) -> Option<String> {
    let mut text = String::new();
    for entry in item.entries() {
        match entry {
            ClipboardEntry::String(string) => text.push_str(string.text()),
            ClipboardEntry::Image(_) | ClipboardEntry::ExternalPaths(_) => return None,
        }
    }
    Some(text)
}

/// The exit line's words, the old app's `exitText`.
pub(crate) fn exit_text(exit: ExitState) -> String {
    match exit {
        ExitState::Code(code) => format!("Process exited with status {code}"),
        ExitState::Signal(signal) => format!("Process ended by signal {signal}"),
    }
}

/// What becomes of a key that is the program's (see [`program_input`]).
#[derive(Clone, Debug, PartialEq)]
enum KeyRoute {
    /// Send these messages: the key itself, as the input mapping encodes it.
    Send(GridInput),
    /// Leave the key to the platform's text input: the listener lets it propagate, and whatever text it commits (the
    /// character it types, a dead-key composition, an input method's string) arrives once, through the terminal's input
    /// handler ([`AttachedTerminal::replace_text_in_range`]).
    TextInput,
    /// Page through the history, -1 up and 1 down: Shift+PageUp or Shift+PageDown with more modifiers held, which the
    /// old app paged on too. The plain chords never get here: their bindings take them first.
    Page(i64),
    /// Nothing at all: the key is swallowed.
    Nothing,
}

/// What becomes of a key that is the program's, typed on `platform`: sent as a key, left to text input, or nothing.
///
/// `map_key` (`farhelm-input-mapping`, ported from the old app's key handling) decides. A key it sends is one `Key`
/// message. A key it leaves to text input (a Shift letter, a space, an Option character on a Mac, a dead key, a key
/// typed while an input method composes) goes to the platform's text input, which commits text through the input
/// handler, as the old app's hidden input element got its text from the browser. A key it cancels and anything it does
/// not handle send nothing. Its paging decision covers Shift+PageUp and Shift+PageDown with any further modifiers, as
/// the old app's key handling did; the keymap binds only the plain chords, so the others page from here. Select all is
/// the keymap's to make, which already gave its key to the terminal, so it sends nothing here.
///
/// NOTE: A key routed to text input must keep propagating (GPUI hands a key's text to the input handler only then),
/// and a key sent here must not (or its text would arrive a second time through the handler). The listener's
/// `stop_propagation` follows the route for exactly that reason.
fn program_input(
    event: &KeyDownEvent,
    platform: Platform,
    composition: &mut Composition,
) -> KeyRoute {
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
        Decision::Send(input) => KeyRoute::Send(input),
        Decision::Text { cancel: false } | Decision::Composition => KeyRoute::TextInput,
        Decision::PageUp => KeyRoute::Page(-1),
        Decision::PageDown => KeyRoute::Page(1),
        Decision::Text { cancel: true }
        | Decision::AppShortcut
        | Decision::Suppress
        | Decision::SelectAll
        | Decision::Unhandled
        | Decision::Unsupported => KeyRoute::Nothing,
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
        let Some(placement) = self.placement(shown, cx) else {
            return vec![view.clone().into_any_element()];
        };
        let screen = div()
            .mt(px(-placement.lift))
            .child(view.clone())
            .into_any_element();
        let exit = shown.state.and_then(|state| state.exit);
        let Some(exit) = exit.filter(|_| !self.notices_held(cx)) else {
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
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |owner, window, cx| {
                owner.window_focus = owner.focus_in_active_window(window);
                owner.report_focus(owner.window_focus, cx);
            }));
        }
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
        // The placeholder goes by the stream's own line, held or not: while a notice is held back, the pane shows
        // neither it nor "Connecting", which would claim a catch-up that is not happening.
        let status = self.status_line();
        let banner = status.clone().filter(|_| !self.notices_held(cx));
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
        // like the pane's fit. It is there before any screen is, so a first screen is placed in its first frame. It
        // also hands the platform the terminal's input handler for this frame (see "Input" in the module docs), which
        // GPUI takes only during paint and uses only while the pane holds focus.
        let room = {
            let owner = owner.clone();
            let focus = self.focus.clone();
            let text_input = self.text_input.clone();
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    window.handle_input(&focus, ElementInputHandler::new(bounds, text_input), cx);
                    // A selection drag follows the pointer anywhere in the window, and ends at the release wherever
                    // it happens, as the old app's document-level listeners did. Registered in every frame, not only
                    // in frames drawn while a drag is held: a press, its moves and its release can all arrive before
                    // the next frame (a fast drag, or the test bridge's), and a listener that waited for a frame to
                    // appear would miss them. Outside a drag they return at once.
                    // A press the program's mouse reporting took is followed the same way.
                    let moved = owner.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            let _ = moved.update(cx, |owner, cx| {
                                owner.link_press_moved(event.position);
                                owner.report_moved(event, cx);
                                owner.drag_moved(event, cx);
                            });
                        }
                    });
                    let released = owner.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        let _ = released.update(cx, |owner, cx| {
                            owner.report_released(event, cx);
                            if event.button == MouseButton::Left {
                                owner.end_selection(Some(event), cx);
                                owner.link_released(event, window, cx);
                                // Hover lookups pause during a press; the pointer may rest on other text now. Only
                                // while it is over the grid: a release over a dialog covering the terminal must not
                                // underline a link through its scrim.
                                if owner.pointer_over {
                                    owner.hover_link_at(Some(event.position), cx);
                                }
                            }
                        });
                    });
                    let height = f32::from(bounds.size.height);
                    let origin = bounds.origin;
                    let pointer = window.mouse_position();
                    window.defer(cx, move |_, cx| {
                        let _ = owner.update(cx, |owner, cx| {
                            owner.grid_measured(origin, height, pointer, cx)
                        });
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
        // In the body's padding box, as the old bar sat in its element's: over the right padding and the column the
        // fit leaves free (`SCROLLBAR_WIDTH`), the full height of the grid's area.
        let scrollbar = self.committed.as_ref().map(|view| {
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(SCROLLBAR_WIDTH))
                .child(self.scrollbar_element(view, cx))
        });
        let disclosure_anchor = self.place_disclosure(window, cx);
        let refusal = self.refusal_line(cx);
        let tunnel_notice = tunnels::notice_line(&self.tunnels, window, cx);
        // The refused click's explanation and the tunnel notice, stacked at the top of the grid's area so neither
        // hides the other. Drawn outside the grid's element: a press on the notice's Dismiss must not also be a press
        // on the terminal (a selection, a mouse report, a link under it), and its button must not sit inside the
        // terminal's key context, where Tab and Escape are the program's.
        let overlay = (refusal.is_some() || tunnel_notice.is_some()).then(|| {
            div()
                .absolute()
                .top(px(PANE_PADDING + 4.))
                .left(px(PANE_PADDING + 4.))
                .right(px(PANE_PADDING + 4.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(refusal)
                .children(tunnel_notice)
        });
        let hovering_link = self
            .committed
            .as_ref()
            .is_some_and(|view| view.read(cx).hovered_link().is_some());
        // The grid's area is what takes focus and keys, not the whole pane: the status line above it holds a Take
        // control button, whose own Tab and Escape the terminal's key context would otherwise give to the program
        // (`actions::TERMINAL_KEYS` binds them to nothing there), leaving a focused button with no way off it.
        let body = div()
            .id("terminal-grid")
            .key_context(actions::TERMINAL_CONTEXT)
            .on_key_down(cx.listener(Self::key))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy_selection))
            // Before anything handles the press (the layer manager closes popovers on a press outside them, the
            // destination display among them), decide whether a click on a link starting here may open it.
            .capture_any_mouse_down(cx.listener(|owner, event: &MouseDownEvent, window, cx| {
                owner.press_admitted = event.button == MouseButton::Left
                    && owner
                        .pressed_link(event.position, cx)
                        .is_some_and(|link| owner.click_admitted(&link, window, cx));
            }))
            .on_any_mouse_down(cx.listener(Self::press))
            // A link under the pointer shows the pointing hand, as the old app's hovered links did.
            .when(hovering_link, |body| body.cursor_pointer())
            .on_mouse_move(
                cx.listener(|owner, event: &MouseMoveEvent, _, cx| owner.hover_moved(event, cx)),
            )
            .on_action(
                cx.listener(|owner, _: &actions::ScrollTerminalPageUp, _, cx| {
                    owner.scroll_page(-1, cx)
                }),
            )
            .on_action(
                cx.listener(|owner, _: &actions::ScrollTerminalPageDown, _, cx| {
                    owner.scroll_page(1, cx)
                }),
            )
            .on_scroll(cx.listener(Self::wheel))
            // Typing does not count as the pointer leaving: the old app kept the bar shown while the pointer stayed
            // over the terminal, keys or not.
            .hover_listener_mode(HoverListenerMode::InputModalityIndependent)
            .on_hover(cx.listener(|owner, hovered: &bool, _, cx| {
                if !*hovered {
                    owner.hover_link_at(None, cx);
                }
                if owner.pointer_over != *hovered {
                    owner.pointer_over = *hovered;
                    // Leaving shows the bar for the hold, as the old app's `mouseleave` did.
                    if *hovered {
                        cx.notify();
                    } else {
                        owner.hold_scrollbar(cx);
                    }
                }
            }))
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
                    .children(screen)
                    .children(disclosure_anchor),
            )
            .children(scrollbar);
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
            .children(banner.map(|line| self.banner(line, cx)))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(body)
                    .children(overlay),
            )
            .children(placeholder)
            .child(measure)
    }
}

// ---- Links -----------------------------------------------------------------------------------------------------------

/// How far, in pixels, a press on a link may wander before its release and still count as a click (the old app's
/// `CLICK_JITTER_PX`); SPEC.md: small pointer jitter still counts as a click, a drag never opens.
const CLICK_JITTER: f32 = 4.;

/// Who a click on a web link belongs to, from the Terminal settings' link choice (the shared preference
/// `terminal_link_click`; unset means intercept).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkClick {
    /// Farhelm opens the link; under mouse reporting the press is withheld from the program, with its drag and
    /// release, so a program that opens links itself cannot open a second tab from the same click.
    Intercept,
    /// Farhelm opens the link and the program gets the click too.
    Both,
    /// The program alone gets the click; Farhelm opens nothing.
    Program,
}

impl LinkClick {
    /// The current choice, from the helm's preferences; intercept when they have not arrived or say nothing else.
    fn current(cx: &App) -> Self {
        let preferences = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<Preferences>(PREFERENCES_KEY)
                .and_then(|preferences| preferences.terminal_link_click.clone())
        });
        match preferences.as_deref() {
            Some("both") => LinkClick::Both,
            Some("program") => LinkClick::Program,
            _ => LinkClick::Intercept,
        }
    }
}

/// A primary press that landed on a link, followed to its release: the link it was on, and where it was pressed, so a
/// release on the same link within [`CLICK_JITTER`] of the press is a click.
struct LinkPress {
    link: TerminalLink,
    at: Point<Pixels>,
    /// Whether a click may open the link, decided before anything the press did could change it (closing popovers,
    /// the destination display among them); see [`AttachedTerminal::click_admitted`].
    admitted: bool,
    /// Whether the link was offered a tunnel at the press, so a Cmd-click tunnels only an address whose display the
    /// admission checked (a tunnel read answering between press and release changes nothing).
    hinted: bool,
}

/// The web address a click on `link` opens: a printed address by the strict rule for text, a program's hyperlink by
/// its parsed scheme (the grid model's `web_target`). `None` for anything else, which a click never opens.
fn link_web_target(link: &TerminalLink) -> Option<String> {
    let plain = link.kind == LinkKind::Text;
    web_target(&link.uri, plain).map(|url| url.to_string())
}

impl AttachedTerminal {
    /// The view cell under `position` (window coordinates) for links, as (column, row) of what is shown: `None` off
    /// the shown screen (the old app's `linkCell`). Unlike a selection's position, nothing clamps to the edge.
    fn link_cell(&self, position: Point<Pixels>, cx: &App) -> Option<(u16, u16)> {
        let origin = self.grid_origin?;
        let view = self.committed.as_ref()?.read(cx);
        let placement = self.placement(view, cx)?;
        let cell = self.cell(cx)?;
        let x = f32::from(position.x - origin.x);
        let y = f32::from(position.y - origin.y) + placement.lift;
        let width = f32::from(view.cols) * cell.width;
        let height = f32::from(placement.rows) * placement.cell_height;
        if !(x >= 0. && x < width && y >= placement.lift && y < height) {
            return None;
        }
        Some((
            (x / cell.width).floor() as u16,
            (y / placement.cell_height).floor() as u16,
        ))
    }

    /// The line position under `position` for links: [`AttachedTerminal::link_cell`]'s cell, by the line its row
    /// shows; `None` on the notice line, which holds no link.
    fn link_position(&self, position: Point<Pixels>, cx: &App) -> Option<Position> {
        let (col, row) = self.link_cell(position, cx)?;
        let view = self.committed.as_ref()?.read(cx);
        if view.row_is_notice(row) {
            return None;
        }
        Some(Position {
            line: view.line_at_row(row)?,
            col,
        })
    }

    /// The pointer rests on `position` with no press under way: tell the view, which finds the link there. Reaching
    /// another cell counts as hovering again, after a refused click or a display put away with no press (Escape), and
    /// redraws so the display shows again.
    fn hover_link_at(&mut self, position: Option<Point<Pixels>>, cx: &mut Context<Self>) {
        let was = self
            .hover_point
            .and_then(|position| self.link_cell(position, cx));
        self.hover_point = position;
        let cell = position.and_then(|position| self.link_cell(position, cx));
        if cell != was {
            // Reaching another cell counts as hovering again, after a refused click or a display put away with no
            // press (Escape). Nothing else redraws for a move within one link, so ask for the frame that shows it.
            let dismissed = self
                .disclosure
                .as_ref()
                .is_some_and(|shown| shown.dismissed_at.is_some());
            if std::mem::take(&mut self.refusal_raised) || dismissed {
                cx.notify();
            }
        }
        if let Some(view) = self.committed.clone() {
            let before = view.read(cx).hovered_link().is_some();
            view.update(cx, |view, cx| view.set_hover(cell, cx));
            // The pointing hand is drawn by this owner, over the grid.
            if view.read(cx).hovered_link().is_some() != before {
                cx.notify();
            }
        }
    }

    /// The hovered link a primary press at `position` landed on: the link the pointer is over, not one found anew at
    /// the press (the old app's rule).
    fn pressed_link(&self, position: Point<Pixels>, cx: &App) -> Option<TerminalLink> {
        let cell = self.link_position(position, cx)?;
        let view = self.committed.as_ref()?.read(cx);
        view.hovered_link()
            .filter(|link| link.covers(cell))
            .cloned()
    }

    /// Whether a primary press on `link` under mouse reporting is withheld from the program: in intercept mode, for a
    /// link Farhelm would open, the press and its drag and release are Farhelm's alone (the old app's
    /// `interceptLinkPress`): a printed web address or a program's hyperlink to one.
    fn withholds_press(link: &TerminalLink, cx: &App) -> bool {
        LinkClick::current(cx) == LinkClick::Intercept && link_web_target(link).is_some()
    }

    /// The pointer moved during a press on a link: once it is farther than [`CLICK_JITTER`] from the press, the press
    /// is a drag and opens nothing at its release.
    fn link_press_moved(&mut self, position: Point<Pixels>) {
        let Some(press) = &self.link_press else {
            return;
        };
        let moved = position - press.at;
        if f32::from(moved.x).hypot(f32::from(moved.y)) > CLICK_JITTER {
            self.link_press = None;
        }
    }

    /// The primary button came up: a press on a link that is released on the same link, still in the text, within
    /// the jitter, with nothing selected, is a click, and opens the link's web address in the browser with no prompt
    /// (SPEC.md) unless the link choice leaves clicks to the program. A link that has a destination display (a
    /// program's hyperlink, or a printed address with a tunnel hint) opens only while that display is fully visible;
    /// otherwise the click is refused, and says why at the top of the terminal. On a Mac, Cmd alone on a link that was
    /// offered a tunnel at the press opens it through one ([`super::tunnels`]) instead of as printed.
    fn link_released(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(press) = self.link_press.take() else {
            return;
        };
        let moved = event.position - press.at;
        if f32::from(moved.x).hypot(f32::from(moved.y)) > CLICK_JITTER {
            return;
        }
        let Some(view) = self.committed.clone() else {
            return;
        };
        let still = self
            .link_position(event.position, cx)
            .and_then(|cell| view.read(cx).link_at(cell));
        if still.as_ref() != Some(&press.link)
            || view.read(cx).selected_text().is_some()
            || LinkClick::current(cx) == LinkClick::Program
        {
            return;
        }
        // A link that is not a web address (a program's `file://` hyperlink, say) opens nothing, and says nothing.
        let Some(target) = link_web_target(&press.link) else {
            return;
        };
        if !press.admitted {
            self.refusal = Some(DISCLOSURE_REFUSAL.into());
            self.refusal_raised = true;
            cx.notify();
            return;
        }
        // Cmd and nothing else, on a link the display offered a tunnel for: through the tunnel, with the link as
        // printed (the helm rewrites it to the tunnel's local address). Anything else opens the link as it is, on this
        // Mac, which the hint says too.
        let modifiers = event.modifiers;
        let tunnel_gesture = self.platform == Platform::Mac
            && modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && !modifiers.function;
        if tunnel_gesture && press.hinted {
            let mark = Mark::take(cx);
            let uri = press.link.uri.clone();
            self.tunnels
                .update(cx, |tunnels, cx| tunnels.open(uri, &mark, window, cx));
            return;
        }
        cx.open_url(&target);
    }
}

/// What a click on a link with a destination display says when that display was not fully visible (the old app's
/// refusal, `admitDisclosure` in terminal-files.js; the form for a display another surface covers is not built yet).
const DISCLOSURE_REFUSAL: &str =
    "cannot open: full link details are not visible; enlarge the window and hover again";

/// Where the destination display's anchor sits against the pointer: its left edge 12 pixels right of it, spanning from
/// 4 pixels above to 12 below. The layer manager puts a popover 4 pixels past its anchor, so the display's top lands 16
/// pixels below the pointer, or its bottom 8 pixels above it when there is no room below: the old `showLinkTarget`'s
/// offsets.
const DISCLOSURE_OFFSET_X: f32 = 12.;
const DISCLOSURE_ABOVE: f32 = 4.;
const DISCLOSURE_BELOW: f32 = 12.;

/// The destination display of the link the pointer rests on: a program's hyperlink, or a printed address offered a
/// tunnel.
struct Disclosure {
    /// The link it is for: a program's hyperlink, or a printed address offered a tunnel.
    link: TerminalLink,
    /// The tunnel hint it ends with, for a remote host's loopback link on a Mac.
    hint: Option<HostSentence>,
    /// Where the pointer was when it opened, which it stays anchored by while the pointer rests on the link.
    at: Point<Pixels>,
    /// Which display this is, counting every one this terminal opened: its anchor's key, so each display has an anchor
    /// of its own and a new one never reuses an old one's.
    generation: u64,
    /// Its layer once opened. A display whose layer has closed (a press elsewhere, Escape, a reset) shows nothing.
    layer: Option<LayerId>,
    /// Where the display was last drawn; a click checks it was fully visible.
    drawn: super::disclosure::Drawn,
    /// Whether a press in the terminal came since it opened. A press closes it (the layer manager closes a popover on
    /// any press outside it) and it comes back once the press ends, so a second click works; closed with no press (by
    /// Escape, chiefly), it stays closed while the pointer rests, rather than reopening at once and losing the Escape.
    pressed: bool,
    /// The cell the pointer rested on when it was found closed with no press since it opened ([`Disclosure::pressed`]):
    /// it stays closed until the pointer reaches another cell, which counts as hovering again.
    dismissed_at: Option<(u16, u16)>,
}

impl AttachedTerminal {
    /// The hovered link when it gets a destination display, with the tunnel hint the display ends with: a program's
    /// hyperlink to a web address always, since its text need not resemble where it goes; a printed web address only
    /// when the session's tunnels offer it a hint (a remote host's loopback address on a Mac), since otherwise what it
    /// shows is what opens.
    fn disclosed_link(&self, cx: &App) -> Option<(TerminalLink, Option<HostSentence>)> {
        let view = self.committed.as_ref()?.read(cx);
        let link = view.hovered_link()?;
        link_web_target(link)?;
        let hint = self.tunnel_hint(link, cx);
        (matches!(link.kind, LinkKind::Osc { .. }) || hint.is_some()).then(|| (link.clone(), hint))
    }

    /// Ask for the session's tunnels the first time the pointer rests on a loopback link on a Mac, which is when the
    /// offer first depends on them. The read goes out after this frame: the owner is drawing.
    fn request_tunnels(&mut self, cx: &mut Context<Self>) {
        let loopback = self
            .committed
            .as_ref()
            .and_then(|view| view.read(cx).hovered_link())
            .is_some_and(|link| tunnel_port(&link.uri).is_some());
        if self.platform != Platform::Mac {
            return;
        }
        let tunnels = self.tunnels.clone();
        if loopback && tunnels.read(cx).unrequested() {
            cx.defer(move |cx| tunnels.update(cx, |tunnels, cx| tunnels.request(cx)));
        } else if !loopback && tunnels.read(cx).retry_wanted() {
            // A failed read is tried again the next time a loopback link is hovered, not while this one still is,
            // which would send it again every frame.
            cx.defer(move |cx| tunnels.update(cx, |tunnels, _| tunnels.rearm()));
        }
    }

    /// Read the session's tunnels when they are due ([`Tunnels::stale`]): as a remote session's terminal opens, so its
    /// pill lists them at once, and whenever the shared consent changed since the last read. Called by the store
    /// observer, never while drawing; the read goes out after the update that called it.
    fn keep_tunnels_current(&mut self, cx: &mut Context<Self>) {
        let remote = tunnels::session_is_remote(&self.target.session, cx);
        if self.tunnels.read(cx).stale(remote, cx) {
            let tunnels = self.tunnels.clone();
            cx.defer(move |cx| tunnels.update(cx, |tunnels, cx| tunnels.refresh(cx)));
        }
    }

    /// The pill for this session's open tunnels, for the session's top row to place in its left region; `None` while
    /// there are none ([`tunnels::pill`]).
    pub(crate) fn tunnel_pill(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        tunnels::pill(&self.tunnels, window, cx)
    }

    /// The tunnel hint `link` gets in its destination display, if the session's tunnels offer one for it.
    fn tunnel_hint(&self, link: &TerminalLink, cx: &App) -> Option<HostSentence> {
        let program_owns_clicks = LinkClick::current(cx) == LinkClick::Program;
        self.tunnels
            .read(cx)
            .hint(&link.uri, self.platform, program_owns_clicks)
    }

    /// Decide the tunnel offer for `platform` instead of this machine's, so tests on any machine cover the Mac's.
    #[cfg(test)]
    pub(crate) fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    /// The session's tunnels, for tests that answer their reads and check their notice.
    #[cfg(test)]
    pub(crate) fn tunnels(&self) -> &Entity<Tunnels> {
        &self.tunnels
    }

    /// Keep the destination display in step with the hovered link, as the owner draws, and return the anchor to draw.
    ///
    /// A hyperlink under the pointer gets a display, opened after this frame once its anchor has been drawn. Another
    /// link (the next row of a wrapped one counts as another, since each row is its own link to the grid model), no
    /// link, or a display that a press closed, once the press has ended, gets a fresh one or none. One closed with no
    /// press (Escape) stays closed until the hover changes. No display shows after a refused click until the pointer
    /// moves to another cell, and none opens while a dialog or menu holds the keyboard.
    fn place_disclosure(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pressing = self.link_press.is_some() || self.select.is_some() || self.reporting != 0;
        self.request_tunnels(cx);
        let wanted = self.disclosed_link(cx).filter(|_| !self.refusal_raised);
        // The hint counts too: a display opened before the session's tunnels were read opens again with it.
        let cell = self
            .committed
            .as_ref()
            .and_then(|view| view.read(cx).hover_cell());
        // A display closed with no press since it opened stays dismissed for the cell the pointer rested on then.
        if let Some(shown) = &mut self.disclosure
            && !shown.pressed
            && shown.dismissed_at.is_none()
            && shown
                .layer
                .is_some_and(|layer| !layers::is_open(layer, window, cx))
        {
            shown.dismissed_at = cell;
        }
        let current = self.disclosure.as_ref().is_some_and(|shown| {
            wanted.as_ref() == Some(&(shown.link.clone(), shown.hint.clone()))
                && (pressing
                    || shown.dismissed_at.is_some_and(|at| Some(at) == cell)
                    || shown
                        .layer
                        .is_none_or(|layer| layers::is_open(layer, window, cx)))
        });
        if !current {
            let old = self.disclosure.take().and_then(|shown| shown.layer);
            if let (Some((link, hint)), Some(at)) = (wanted, self.hover_point) {
                self.disclosure_count += 1;
                let generation = self.disclosure_count;
                self.disclosure = Some(Disclosure {
                    link,
                    hint,
                    at,
                    generation,
                    layer: None,
                    drawn: super::disclosure::Drawn::default(),
                    pressed: false,
                    dismissed_at: None,
                });
            }
            let opening = self.disclosure.as_ref().map(|shown| shown.generation);
            let mark = Mark::take(cx);
            let owner = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                // The layer manager also closes the old display once its anchor is no longer drawn; closing it here
                // takes it away in the same frame the new one opens.
                if let Some(old) = old {
                    layers::close(old, window, cx);
                }
                if let Some(generation) = opening {
                    let _ = owner.update(cx, |owner, cx| {
                        owner.open_disclosure(generation, &mark, window, cx)
                    });
                }
            });
        }
        self.disclosure_anchor_element(window, cx)
    }

    /// The zero-width element the current display is anchored to, drawn by the pointer where the display opened.
    fn disclosure_anchor_element(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let shown = self.disclosure.as_ref()?;
        let origin = self.grid_origin?;
        let anchor = Anchor::for_element(
            gpui::ElementId::Name(format!("terminal-link-disclosure-{}", shown.generation).into()),
            window,
            cx,
        );
        self.disclosure_anchor = Some(anchor.clone());
        Some(
            div()
                .absolute()
                .left(shown.at.x - origin.x + px(DISCLOSURE_OFFSET_X))
                .top(shown.at.y - origin.y - px(DISCLOSURE_ABOVE))
                .w(px(0.))
                .h(px(DISCLOSURE_ABOVE + DISCLOSURE_BELOW))
                .child(anchor.track())
                .into_any_element(),
        )
    }

    /// Open display `generation`, if it is still the one wanted: a popover that takes no focus, on the anchor drawn in
    /// the frame that asked. A display that is still wanted but cannot open (a dialog or menu holds the keyboard, or
    /// its anchor or view is gone) is dropped rather than left pending, so the next frame asks again: left pending, it
    /// would count as current for as long as the same link stays hovered, showing nothing and refusing every click on
    /// it.
    fn open_disclosure(
        &mut self,
        generation: u64,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pending = self
            .disclosure
            .as_ref()
            .is_some_and(|shown| shown.generation == generation && shown.layer.is_none());
        if !pending {
            return;
        }
        match self.open_layer(mark, window, cx) {
            Some(layer) => {
                if let Some(shown) = &mut self.disclosure {
                    shown.layer = Some(layer);
                }
            }
            None => self.disclosure = None,
        }
    }

    /// The layer of the pending display, opened now; `None` when it may not open. No display opens while a dialog or
    /// menu holds the keyboard: it would float above it, for a hover that started before it or that a menu (which has
    /// no veil) lets through. The display follows the hover once the dialog or menu closes.
    fn open_layer(
        &self,
        mark: &Mark,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<LayerId> {
        if layers::focus_layer_open(window, cx) {
            return None;
        }
        let shown = self.disclosure.as_ref()?;
        let view = self.committed.clone()?;
        let anchor = self.disclosure_anchor.clone()?;
        let text = view.read(cx).link_disclosure(&shown.link)?;
        let drawn = shown.drawn.clone();
        let hint = shown.hint.clone();
        let content = cx.new(|_| super::disclosure::DisclosureView::new(&text, hint, drawn));
        let popover = Popover {
            view: content.into(),
            container: cx.focus_handle(),
            start: None,
            anchor,
            // A display that never takes focus returns none, so this is never used.
            returns_to: focus::ReturnTo::Opener,
            traps_tab: true,
        };
        layers::open_popover(popover, mark, window, cx)
    }

    /// Whether a click on `link` that starts now may open it, decided at the press, before anything the press does
    /// (closing popovers, among them this display) can change it: a printed web address with no tunnel hint always
    /// may; a program's hyperlink, or a printed address whose display carries a tunnel hint, only while its destination
    /// display, for this very link, is open and was fully visible (inside the window) when last drawn (SPEC.md: the
    /// hover display counts as disclosing the target only while fully visible, and this applies to Cmd-click tunnel
    /// opens too).
    fn click_admitted(&self, link: &TerminalLink, window: &Window, cx: &App) -> bool {
        let disclosed = match link.kind {
            LinkKind::Text => self.tunnel_hint(link, cx).is_some(),
            LinkKind::Osc { .. } => true,
        };
        !disclosed
            || self.disclosure.as_ref().is_some_and(|shown| {
                shown.link == *link
                    && shown
                        .layer
                        .is_some_and(|layer| layers::is_open(layer, window, cx))
                    && super::disclosure::fully_visible(&shown.drawn)
            })
    }

    /// The refused click's explanation, drawn at the top of the grid's area until the next press there (the old app's
    /// `.terminal-file-message`: a status line clear of the newest output at the bottom, at most three lines tall). The
    /// display it is about is closed while it shows, so nothing covers it.
    fn refusal_line(&self, cx: &App) -> Option<AnyElement> {
        let text = self.refusal.clone()?;
        let palette = theme::palette(cx);
        Some(
            div()
                .id("terminal-link-refusal")
                .debug_selector(|| "terminal-link-refusal".into())
                .role(Role::Status)
                .aria_label(text.clone())
                .px(px(6.))
                .py(px(4.))
                .max_h(px(54.))
                .overflow_hidden()
                .bg(theme::color(palette.bg_2))
                .text_color(theme::color(palette.fg_1))
                .text_size(theme::metrics::TEXT_SM)
                .child(text)
                .into_any_element(),
        )
    }
}

// ---- Mouse reporting -------------------------------------------------------------------------------------------------

/// The format's number for `button` in a press, release or move report: 0 left, 1 middle, 2 right; `None` for the
/// buttons the format has no number for (the old app's `pressButton`).
fn report_button(button: MouseButton) -> Option<i8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

/// The input mapping's modifier bits for GPUI's `modifiers`, which a mouse report carries.
fn modifier_bits(modifiers: Modifiers) -> u8 {
    farhelm_input_mapping::Modifiers {
        control: modifiers.control,
        alt: modifiers.alt,
        shift: modifiers.shift,
        platform: modifiers.platform,
        function: modifiers.function,
    }
    .bits()
}

impl AttachedTerminal {
    /// Send the program a mouse report of `kind` for `button` at the cell under `position`, if its mouse reporting
    /// mode asks for that kind (`mode_reports`: presses and releases in every mode, moves with a button held from drag
    /// mode up, moves without one only in motion mode), the terminal is live, and it is not a move in the same cell as
    /// the last report (the deduper). A report is input from the user, so like a key it clears the selection and
    /// brings the view to the live screen (the old app's `mouseMessage`).
    fn mouse_report(
        &mut self,
        kind: MouseEventKind,
        button: i8,
        position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = &self.committed else {
            return;
        };
        let mode = view
            .read(cx)
            .state
            .map_or(farhelm_proto::grid::MouseMode::Off, |state| state.mouse);
        if !mode_reports(mode, kind, button) {
            return;
        }
        let (true, Some(attachment)) = (self.is_live(), self.attachment) else {
            return;
        };
        let Some((x, y)) = self.cell_at(position, cx) else {
            return;
        };
        let mods = modifier_bits(modifiers);
        if !self.mouse_dedupe.admit(kind, button, x, y, mods) {
            return;
        }
        let report = GridInput::Mouse {
            kind,
            button,
            x,
            y,
            mods,
        };
        self.send_user(attachment, vec![report], cx);
    }

    /// A press while the program reports the mouse and the forcing modifier is not held: the program's. Its button is
    /// held from now until its release, and moves meanwhile go to the program as a drag.
    fn report_press(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(button) = report_button(event.button) else {
            return;
        };
        self.reporting |= 1 << button;
        self.mouse_report(
            MouseEventKind::Down,
            button,
            event.position,
            event.modifiers,
            cx,
        );
    }

    /// The pointer moved anywhere in the window while the program holds a press: report the move with the lowest of the
    /// buttons the program holds (the old app's `moveButton`: the left first, then the middle, then the right). The
    /// buttons come from this terminal's own record, not the event's: GPUI's Wayland backend reports only the button
    /// pressed last, and a side button (Back, Forward) pressed during a drag shows up there too.
    ///
    /// A move with no button down is a release the window did not deliver (one a native menu or another listener
    /// took, say): the program hears the release of each button it still thinks is held, so it is not left mid-drag.
    /// On Wayland, letting go of one button of a held pair also reads as no button down, so that ends the whole press
    /// early; the backend tracks one button.
    fn report_moved(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.reporting == 0 {
            return;
        }
        if event.pressed_button.is_none() {
            let held = std::mem::take(&mut self.reporting);
            for button in (0..3).filter(|button| held & (1 << button) != 0) {
                self.mouse_report(
                    MouseEventKind::Up,
                    button,
                    event.position,
                    event.modifiers,
                    cx,
                );
            }
            return;
        }
        let button = (0..3)
            .find(|button| self.reporting & (1 << button) != 0)
            .unwrap_or(0);
        self.mouse_report(
            MouseEventKind::Move,
            button,
            event.position,
            event.modifiers,
            cx,
        );
    }

    /// A button came up anywhere in the window: if the program held it, report the release; the press is over when its
    /// last button is up.
    fn report_released(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        let Some(button) = report_button(event.button) else {
            return;
        };
        if self.reporting & (1 << button) == 0 {
            return;
        }
        self.reporting &= !(1 << button);
        self.mouse_report(
            MouseEventKind::Up,
            button,
            event.position,
            event.modifiers,
            cx,
        );
    }

    /// The pointer moved over the grid with no press under way: the view learns which link it rests on, and in motion
    /// mode the program hears the move, with no button (`-1`; the old app's `mousemove` without buttons).
    fn hover_moved(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if event.pressed_button.is_some() || self.reporting != 0 || self.select.is_some() {
            return;
        }
        self.hover_link_at(Some(event.position), cx);
        self.mouse_report(
            MouseEventKind::Move,
            -1,
            event.position,
            event.modifiers,
            cx,
        );
    }
}

// ---- Selection -------------------------------------------------------------------------------------------------------

/// A drag held past the top or bottom of the screen scrolls a step this often while it stays there: 20 steps a second,
/// the old app's `DRAG_SCROLL_INTERVAL_MS`.
const DRAG_SCROLL_INTERVAL: Duration = Duration::from_millis(50);

/// How far past the screen's edge the pointer must be for the fastest drag scroll, in pixels, and how many lines that
/// step moves (`DRAG_SCROLL_MAX_DISTANCE` and `DRAG_SCROLL_MAX_LINES`, from xterm.js 6.0.0's
/// `DRAG_SCROLL_MAX_THRESHOLD` and `DRAG_SCROLL_MAX_SPEED`; MIT, notices in `farhelm-input-mapping`'s LICENSE-PORTED).
const DRAG_SCROLL_MAX_DISTANCE: f32 = 50.;
const DRAG_SCROLL_MAX_LINES: f32 = 15.;

/// What a selection gesture's moving end selects, set by the press that started it, with what stays put.
#[derive(Clone, Copy, Debug)]
enum SelectMode {
    /// From the press's cell boundary (`anchor`) to the pointer's: a plain or Shift drag.
    Char { anchor: Position },
    /// A rectangle from the press's corner (`anchor`) to the pointer's (Alt-drag off a Mac).
    Column { anchor: Position },
    /// Whole words, always keeping the double-clicked one (`pressed`).
    Word { pressed: Selection },
    /// Whole soft-wrapped lines, always keeping the triple-clicked one (`pressed`).
    Line { pressed: Selection },
}

/// A local selection being dragged, from its press to its release. The view holds the selection itself; this holds
/// what the drag needs to move its far end. One exists only for a press that was allowed to select (mouse reporting
/// off, or the forcing modifier held), so its release always copies what was selected.
struct SelectGesture {
    mode: SelectMode,
    /// Where the pointer last was (window coordinates): where a drag-scroll step extends the selection to.
    pointer: Point<Pixels>,
    /// Lines a drag-scroll step moves, negative toward older lines; 0 while the pointer is over the screen.
    scroll_lines: i64,
    /// The next drag-scroll step, while one is due.
    scroll_tick: Option<ticker::Deadline>,
    /// Whether the press is still recent enough for an Alt-click to move the cursor: true until
    /// [`ALT_CLICK_WINDOW`] after the press, when `_quick_window` clears it.
    quick: Rc<Cell<bool>>,
    _quick_window: ticker::Deadline,
}

/// An Alt-click moves the program's cursor only when its release comes within this long of its press (the old app's
/// `altClickMovesCursor`, which compared the events' timestamps against 500 ms): a slower Alt-press is a selection.
const ALT_CLICK_WINDOW: Duration = Duration::from_millis(500);

/// Whether a press holds this platform's modifier that forces a local selection over mouse reporting: Option on a
/// Mac, Shift elsewhere (SPEC.md's selection paragraph; the old app's `pressForcesSelection`).
fn forces_selection(modifiers: Modifiers) -> bool {
    match actions::THIS_PLATFORM {
        Platform::Mac => modifiers.alt,
        Platform::Linux => modifiers.shift,
    }
}

/// Lines a drag-scroll step moves for a pointer `distance` pixels past the screen's edge (negative above it): at least
/// one, and up to [`DRAG_SCROLL_MAX_LINES`] at [`DRAG_SCROLL_MAX_DISTANCE`] or farther, the old app's
/// `updateDragScroll`.
fn drag_scroll_lines(distance: f32) -> i64 {
    if distance == 0. || !distance.is_finite() {
        return 0;
    }
    let speed = (distance.abs().min(DRAG_SCROLL_MAX_DISTANCE) / DRAG_SCROLL_MAX_DISTANCE
        * DRAG_SCROLL_MAX_LINES)
        .round()
        .max(1.);
    speed as i64 * distance.signum() as i64
}

impl AttachedTerminal {
    /// The line position under `position` (window coordinates) for a selection: the nearest cell boundary when
    /// `boundary` (how a character or column selection's ends move), else the cell itself. Rows above or below the
    /// screen clamp to its first or last row, so a drag past an edge selects to it; the notice line stands for the
    /// line below it. `None` before the grid is measured.
    fn selection_position(
        &self,
        position: Point<Pixels>,
        boundary: bool,
        cx: &App,
    ) -> Option<Position> {
        let origin = self.grid_origin?;
        let view = self.committed.as_ref()?.read(cx);
        let placement = self.placement(view, cx)?;
        let cell = self.cell(cx)?;
        let geometry = Geometry {
            cell_width: f64::from(cell.width),
            cell_height: f64::from(placement.cell_height),
            columns: view.cols,
            rows: placement.rows,
        };
        let (col, row) = geometry.selection_cell(
            f64::from(f32::from(position.x - origin.x)),
            f64::from(f32::from(position.y - origin.y) + placement.lift),
            boundary,
        )?;
        let line = view.line_at_row(row)?;
        Some(Position { line, col })
    }

    /// A press of any button on the grid. With mouse reporting off, or with the forcing modifier held, a primary press
    /// starts a local selection: a click an empty one at the nearest boundary (which clears what was selected), a
    /// double click the word, a triple click the soft-wrapped line, Alt off a Mac a rectangle, and Shift with a
    /// selection already there extends it (the old app's `startSelection`, from xterm.js 6.0.0's `SelectionService`;
    /// MIT, notices in `farhelm-input-mapping`'s LICENSE-PORTED). Under mouse reporting a press without the forcing
    /// modifier, of any button, is the program's ([`AttachedTerminal::report_press`]) and selects nothing.
    fn press(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // A refused click's explanation stays until the next press in the terminal, as the old app's did.
        if self.refusal.take().is_some() {
            cx.notify();
        }
        if let Some(shown) = &mut self.disclosure {
            shown.pressed = true;
        }
        let Some(view) = self.committed.clone() else {
            return;
        };
        let tracking = view
            .read(cx)
            .state
            .is_some_and(|state| state.mouse != farhelm_proto::grid::MouseMode::Off);
        let forced = forces_selection(event.modifiers);
        let link = if event.button == MouseButton::Left {
            self.pressed_link(event.position, cx)
        } else {
            None
        };
        let admitted = std::mem::take(&mut self.press_admitted);
        self.link_press = link.clone().map(|link| LinkPress {
            hinted: self.tunnel_hint(&link, cx).is_some(),
            link,
            at: event.position,
            admitted,
        });
        if tracking && !forced {
            // A press on a web link Farhelm opens is withheld from the program, in intercept mode, with its drag and
            // release: it is never reported, so its release is not either. It clears the selection, as the old app's
            // did.
            if link.is_some_and(|link| Self::withholds_press(&link, cx)) {
                view.update(cx, |view, cx| view.set_selection(None, cx));
                return;
            }
            self.report_press(event, cx);
            return;
        }
        if event.button != MouseButton::Left {
            return;
        }
        let mac = actions::THIS_PLATFORM == Platform::Mac;
        let current = view.read(cx).selection();
        let (mode, selection) = match event.click_count {
            2 | 3 => {
                let Some(cell) = self.selection_position(event.position, false, cx) else {
                    return;
                };
                let range = if event.click_count == 2 {
                    view.read(cx).word_range(cell)
                } else {
                    view.read(cx).line_range(cell)
                };
                let Some(range) = range else {
                    return;
                };
                let mode = if event.click_count == 2 {
                    SelectMode::Word { pressed: range }
                } else {
                    SelectMode::Line { pressed: range }
                };
                (mode, range)
            }
            _ => {
                let Some(at) = self.selection_position(event.position, true, cx) else {
                    return;
                };
                if event.modifiers.alt && !mac {
                    let selection = Selection::new(at, at, SelectionMode::Column);
                    (SelectMode::Column { anchor: at }, selection)
                } else if let Some(current) = current.filter(|current| {
                    event.modifiers.shift
                        && !current.is_empty()
                        && current.mode == SelectionMode::Linear
                }) {
                    let selection = Selection::new(current.start, at, SelectionMode::Linear);
                    (
                        SelectMode::Char {
                            anchor: current.start,
                        },
                        selection,
                    )
                } else {
                    let selection = Selection::new(at, at, SelectionMode::Linear);
                    (SelectMode::Char { anchor: at }, selection)
                }
            }
        };
        view.update(cx, |view, cx| view.set_selection(Some(selection), cx));
        let quick = Rc::new(Cell::new(true));
        let expired = quick.clone();
        let quick_window = ticker::after(ALT_CLICK_WINDOW, cx, move |_| expired.set(false));
        self.select = Some(SelectGesture {
            mode,
            pointer: event.position,
            scroll_lines: 0,
            scroll_tick: None,
            quick,
            _quick_window: quick_window,
        });
    }

    /// Move a selection drag's far end to `pointer`, by its mode (the old app's `extendTo`).
    fn extend_selection(&mut self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(gesture) = &self.select else {
            return;
        };
        let mode = gesture.mode;
        let boundary = matches!(mode, SelectMode::Char { .. } | SelectMode::Column { .. });
        let Some(at) = self.selection_position(pointer, boundary, cx) else {
            return;
        };
        let Some(view) = self.committed.clone() else {
            return;
        };
        // The view cleared the selection under the drag: a reset (a resize, a history reset, the alternate screen
        // switching) made its line numbers name other text, history trimming dropped all of it, or input went to the
        // program. The drag ends with it, and copies nothing; extending from the old anchor would select text the
        // user never pointed at.
        if view.read(cx).selection().is_none() {
            self.select = None;
            return;
        }
        let selection = match mode {
            SelectMode::Char { anchor } => Selection::new(anchor, at, SelectionMode::Linear),
            SelectMode::Column { anchor } => Selection::new(anchor, at, SelectionMode::Column),
            SelectMode::Word { pressed } | SelectMode::Line { pressed } => {
                let found = match mode {
                    SelectMode::Word { .. } => view.read(cx).word_range(at),
                    _ => view.read(cx).line_range(at),
                };
                let found = found.unwrap_or(Selection::new(at, at, SelectionMode::Linear));
                Selection::new(
                    found.start.min(pressed.start),
                    found.end.max(pressed.end),
                    SelectionMode::Linear,
                )
            }
        };
        view.update(cx, |view, cx| view.set_selection(Some(selection), cx));
    }

    /// The pointer moved while a selection is dragged (anywhere in the window): extend the selection, and scroll while
    /// the pointer is held past the screen's top or bottom. A move with the primary button up is a release the window
    /// never saw (let go outside it), which ends the drag.
    fn drag_moved(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.select.is_none() {
            return;
        }
        if event.pressed_button != Some(MouseButton::Left) {
            self.end_selection(None, cx);
            return;
        }
        if let Some(gesture) = &mut self.select {
            gesture.pointer = event.position;
        }
        self.extend_selection(event.position, cx);
        let lines = self
            .drag_scroll_distance(event.position, cx)
            .map_or(0, drag_scroll_lines);
        let Some(gesture) = &mut self.select else {
            return;
        };
        gesture.scroll_lines = lines;
        if lines == 0 {
            gesture.scroll_tick = None;
        } else if gesture.scroll_tick.is_none() {
            self.schedule_drag_scroll(cx);
        }
    }

    /// How far `pointer` is past the shown screen's top (negative) or bottom (positive), 0 inside it.
    fn drag_scroll_distance(&self, pointer: Point<Pixels>, cx: &App) -> Option<f32> {
        let origin = self.grid_origin?;
        let view = self.committed.as_ref()?.read(cx);
        let placement = self.placement(view, cx)?;
        let top = f32::from(origin.y);
        let bottom = top + f32::from(placement.rows) * placement.cell_height - placement.lift;
        let y = f32::from(pointer.y);
        Some(if y < top {
            y - top
        } else if y > bottom {
            y - bottom
        } else {
            0.
        })
    }

    /// Arm the next drag-scroll step: one [`DRAG_SCROLL_INTERVAL`] from now it scrolls by the gesture's current step,
    /// extends the selection to the pointer's last place, and arms the next while the drag is still held past an edge.
    fn schedule_drag_scroll(&mut self, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        let tick = ticker::after(DRAG_SCROLL_INTERVAL, cx, move |cx| {
            let _ = owner.update(cx, |owner, cx| {
                let Some(gesture) = &mut owner.select else {
                    return;
                };
                gesture.scroll_tick = None;
                let (lines, pointer) = (gesture.scroll_lines, gesture.pointer);
                if lines == 0 {
                    return;
                }
                // No link is hovered while a drag scrolls: the frozen press cell would otherwise underline each
                // link passing under it. The release looks again.
                if let Some(view) = &owner.committed {
                    view.update(cx, |view, cx| {
                        view.set_hover(None, cx);
                        view.scroll_lines(lines, cx);
                    });
                }
                owner.hold_scrollbar(cx);
                owner.extend_selection(pointer, cx);
                owner.schedule_drag_scroll(cx);
            });
        });
        if let Some(gesture) = &mut self.select {
            gesture.scroll_tick = Some(tick);
        }
    }

    /// The primary button came up while a selection is dragged: the gesture ends, and a non-empty selection goes to the
    /// clipboard, every time, even when it is the text already there (SPEC.md: every completed selection re-copies;
    /// the old app's `copySelectionOnMouseUp`). A plain click left an empty selection, so it copies nothing and leaves
    /// the clipboard alone. A quick Alt-click then moves the program's cursor ([`AttachedTerminal::alt_click`]);
    /// `release` is the button's release, `None` for one the window never saw, which moves nothing.
    fn end_selection(&mut self, release: Option<&MouseUpEvent>, cx: &mut Context<Self>) {
        let Some(gesture) = self.select.take() else {
            return;
        };
        let text = self
            .committed
            .as_ref()
            .and_then(|view| view.read(cx).selected_text());
        if let Some(text) = &text {
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        }
        if let Some(release) = release
            && release.modifiers.alt
            && gesture.quick.get()
        {
            self.alt_click(release.position, text.as_deref(), cx);
        }
    }

    /// A quick Alt-click (Option on a Mac) that selected at most one character, with the view on the live screen,
    /// moves the program's cursor to the clicked cell by typing arrow keys, as the old app did
    /// (`altClickMovesCursor`, with the input mapping's port of xterm.js's MoveToCell). The keys go to the program as
    /// one group of ordinary key messages, so the supervisor encodes them in the program's cursor-key mode; like any
    /// input they clear the selection.
    fn alt_click(
        &mut self,
        position: Point<Pixels>,
        selected: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        // The old app's `text.length > 1`, which counts UTF-16 units: one character beyond the Basic Multilingual
        // Plane (an emoji) already counts as more than one there, and here too.
        if selected.is_some_and(|text| text.encode_utf16().count() > 1) {
            return;
        }
        let (Some(view), Some(attachment)) = (self.committed.clone(), self.attachment) else {
            return;
        };
        if !view.read(cx).is_following() {
            return;
        }
        let Some((x, y)) = self.cell_at(position, cx) else {
            return;
        };
        let keys = view.read(cx).cursor_moves_to(x, y, actions::THIS_PLATFORM);
        self.send_user(attachment, keys, cx);
    }

    /// Select everything the terminal shows, history included ([`actions::SelectAllInTerminal`], Cmd+A on a Mac).
    /// Selecting is not copying, as in the old app: Cmd+C copies it.
    fn select_all(
        &mut self,
        _: &actions::SelectAllInTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A drag in progress ends: its anchor would otherwise rebuild a selection over this one at the next move.
        self.select = None;
        if let Some(view) = &self.committed {
            view.update(cx, |view, cx| view.select_all(cx));
        }
    }

    /// Copy the local selection to the clipboard ([`actions::CopyTerminalSelection`], Cmd+C on a Mac). With nothing
    /// selected it does nothing, so the clipboard keeps what another program put there.
    fn copy_selection(
        &mut self,
        _: &actions::CopyTerminalSelection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self
            .committed
            .as_ref()
            .and_then(|view| view.read(cx).selected_text());
        if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }
}

// ---- Text input ------------------------------------------------------------------------------------------------------

impl AttachedTerminal {
    /// Text the platform committed: the character a key typed, a finished dead-key sequence, an input method's string.
    /// It ends any composition and dead-key sequence (left pending, the input mapping would hold back the next key
    /// that sends a sequence, so Enter after a composed `é` would send nothing), and goes to the program as typed text
    /// while the terminal is live.
    fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.set_marked(None);
        self.composition.dead_pending = false;
        if let Some(attachment) = self.attachment {
            self.send_user(attachment, committed_text(text), cx);
        }
    }

    /// An input method is composing `text` (empty when it gave up): recorded, neither drawn nor sent.
    fn mark_text(&mut self, text: &str) {
        self.set_marked((!text.is_empty()).then(|| text.encode_utf16().count()));
    }

    /// A paste handed to the focused text input by the platform: the same as the app's own Paste
    /// ([`AttachedTerminal::paste`]), never typed text.
    ///
    /// NOTE: No platform GPUI ships for Farhelm calls this today (GPUI names only the web's DOM paste as a caller); it
    /// is here so that the input handler's default, which would type a paste as text, can never apply to a terminal.
    /// It does not wait behind the app's own pastes still reading the clipboard.
    fn paste_item(&mut self, item: &ClipboardItem, cx: &mut Context<Self>) {
        let Some(text) = clipboard_text(item).filter(|text| !text.is_empty()) else {
            return;
        };
        if let Some(attachment) = self.attachment {
            self.send_user(attachment, pasted_text(&text), cx);
        }
    }
}

/// The terminal's input handler: where the platform's text input commits text (see "Input" in the module docs).
///
/// A terminal has no document for an input method to edit, so this answers as an empty field with the caret at the
/// end of whatever is being composed: no text to read back, no position on screen for the candidate window (the old
/// app left it where the browser put it, too), and committed text goes straight to the program as typed text. Marked
/// text (an input method's composition, a Mac dead key's accent) is recorded but neither drawn nor sent: showing it
/// at the cursor was left out of the old app as not worth its complexity, and the migration keeps input method work
/// to what GPUI gives (decided by the maintainer on 2026-11-12).
///
/// NOTE: It is an entity of its own that points at its terminal weakly, rather than the terminal itself. GPUI keeps
/// the frame's input handler (and so a strong handle to whatever implements it) past the frame that drew it, so a
/// terminal that was its own handler outlived its retirement by a frame, and with it its focus handle: focus stayed
/// on a terminal nobody drew, the focus coordinator's recovery never ran, and the window was left with nothing
/// focused (`session_pane`'s reset test caught it). Once the terminal is gone, every call here does nothing.
struct TerminalInputHandler {
    owner: WeakEntity<AttachedTerminal>,
}

impl TerminalInputHandler {
    /// Run `f` on the terminal, if it still exists.
    fn on_owner<R>(
        &self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut AttachedTerminal, &mut Context<AttachedTerminal>) -> R,
    ) -> Option<R> {
        self.owner.update(cx, f).ok()
    }
}

impl EntityInputHandler for TerminalInputHandler {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let caret = self.owner.upgrade()?.read(cx).marked.unwrap_or(0);
        Some(UTF16Selection {
            range: caret..caret,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let marked = self.owner.upgrade()?.read(cx).marked;
        marked.map(|length| 0..length)
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.on_owner(cx, |owner, _| owner.set_marked(None));
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_owner(cx, |owner, cx| owner.commit_text(text, cx));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        new_text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_owner(cx, |owner, _| owner.mark_text(new_text));
    }

    fn paste(&mut self, item: ClipboardItem, _: &mut Window, cx: &mut Context<Self>) {
        self.on_owner(cx, |owner, cx| owner.paste_item(&item, cx));
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
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

    /// Whether any live owner's view says the pointer rests on a link: a test's premise that a terminal it cannot
    /// reach directly (inside the main view) is seeing the hover it then checks the consequences of.
    #[cfg(test)]
    pub(crate) fn any_hovered_link(cx: &App) -> bool {
        cx.try_global::<Owners>().is_some_and(|owners| {
            owners
                .0
                .iter()
                .filter_map(|owner| owner.upgrade())
                .any(|owner| {
                    owner
                        .read(cx)
                        .committed
                        .as_ref()
                        .is_some_and(|view| view.read(cx).hovered_link().is_some())
                })
        })
    }
}

/// Whether any terminal says the pointer rests on a link ([`probe::any_hovered_link`]), for tests outside this module.
#[cfg(test)]
pub(crate) fn any_hovered_link(cx: &App) -> bool {
    probe::any_hovered_link(cx)
}

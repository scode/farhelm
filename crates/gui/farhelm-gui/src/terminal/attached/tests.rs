//! The attached terminal's contract with its stream, against the helm client's fake terminal: when it attaches and at
//! what size, which view a replay goes to, how much one turn delivers, what the pane says while the stream is not live,
//! and that a reset takes the whole thing down.
//!
//! The fake plays the helm (`farhelm_helm_client::fake::terminal`); GPUI's deterministic executor cannot wait on a real
//! socket. The owner is drawn in a pane of a fixed size inside a bootstrapped window, so its first fit is a real
//! measurement and its Take control is a real button.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{
    FakeAttach, FakePhase, FakeTerminal, FakeTerminalRequest,
};
use farhelm_proto::DetachCode;
use gpui::{
    AppContext as _, Context, Entity, Modifiers, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, px,
};
use gpui_component::Root;

use super::*;
use crate::session::{self, Connection};
use crate::{focus, layers, test_support};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The pane every test draws the owner in, in logical pixels.
const PANE: (f32, f32) = (816., 496.);

/// Fixed metrics, so the fit is arithmetic the test can check: a 10 by 22 pixel character.
fn metrics() -> CellMetrics {
    CellMetrics::new(10., 22., 16.).unwrap()
}

/// The window's root: a pane of the size in `size` holding the owner, with the focus and layer foundations attached
/// as in the app, so the Take control button behaves as it does there.
struct Pane {
    owner: Entity<AttachedTerminal>,
    size: Rc<Cell<(f32, f32)>>,
    _layers: layers::Attachment,
    focus: focus::Attachment,
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (width, height) = self.size.get();
        focus::root(div(), &self.focus)
            .size_full()
            .child(div().w(px(width)).h(px(height)).child(self.owner.clone()))
            .children(layers::overlay(window, cx))
    }
}

/// One owner in its window, the helm behind its UI session, and the fake stream it opened (once it has).
struct Harness<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    connection: Entity<Connection>,
    owner: Entity<AttachedTerminal>,
    size: Rc<Cell<(f32, f32)>>,
    opened: Rc<RefCell<Option<(FakeTerminal, FakeAttach)>>>,
}

/// A bootstrapped app connected to the fake helm, with an owner made for its UI session and drawn once. The owner's
/// opener records the attach it was asked for and hands out a fake stream for it.
fn harness(cx: &mut TestAppContext) -> Harness<'_> {
    test_support::install(cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    helm.connect(Vec::new());
    cx.run_until_parked();
    let session = cx.update(|cx| connection.read(cx).session().cloned().unwrap());
    let opened = Rc::new(RefCell::new(None));
    let size = Rc::new(Cell::new(PANE));
    let (slot, pane_size) = (opened.clone(), size.clone());
    let mut owner = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let open: OpenStream = Box::new(move |attach| {
            let (fake, handle, events) = FakeTerminal::open(&attach);
            let asked = FakeAttach {
                take_control: true,
                cols: attach.cols,
                rows: attach.rows,
            };
            *slot.borrow_mut() = Some((fake, asked));
            (handle, events)
        });
        let target = TerminalTarget {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
        };
        let made = cx.new(|cx| AttachedTerminal::new(&session, target, metrics(), open, cx));
        owner = Some(made.clone());
        let pane = cx.new(|cx| Pane {
            owner: made,
            size: pane_size,
            _layers: layers::attach(window, cx),
            focus: focus::attach(window, cx),
        });
        Root::new(pane, window, cx)
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    Harness {
        cx,
        helm,
        connection,
        owner: owner.unwrap(),
        size,
        opened,
    }
}

impl Harness<'_> {
    /// The fake stream; panics if the owner has not opened one.
    fn fake(&self) -> std::cell::RefMut<'_, FakeTerminal> {
        std::cell::RefMut::map(self.opened.borrow_mut(), |opened| {
            &mut opened.as_mut().expect("the owner opened no stream").0
        })
    }

    /// Let the app drain what the fake sent and redraw, then run the harness's per-step checks.
    fn settle(&mut self) {
        self.cx.run_until_parked();
        self.cx.update(test_support::check_invariants);
    }

    fn read<R>(&mut self, read: impl FnOnce(&AttachedTerminal, &App) -> R) -> R {
        let owner = self.owner.clone();
        self.cx.update(|_, cx| read(owner.read(cx), cx))
    }

    fn text(&mut self) -> Option<String> {
        self.read(|owner, cx| owner.terminal_text(cx))
    }

    fn status(&mut self) -> Option<StatusLine> {
        self.read(|owner, _| owner.status_line())
    }

    fn renders(&mut self, view: &str) -> usize {
        self.cx.update(|_, cx| render_count::count(view, cx))
    }

    fn reset_renders(&mut self) {
        self.cx.update(|_, cx| render_count::reset(cx));
    }

    /// The fake's attach opens and replays a complete screen showing `text` at the attach's own size.
    fn attach_showing(&mut self, text: &str) {
        let asked = self.fake().attach();
        for message in screen(asked.cols, asked.rows, text, serde_json::Value::Null) {
            self.fake().output(message);
        }
        self.fake().replay_complete();
        self.settle();
    }
}

/// A complete first screen: hello at `cols` by `rows`, the state with `exit`, `text` on the first row, one frame.
fn screen(cols: u16, rows: u16, text: &str, exit: serde_json::Value) -> Vec<GridOutput> {
    [
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":0}),
        serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":exit}),
        serde_json::json!({"t":"rows","rows":[[0,{"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]}]]}),
        frame(1, cols, rows),
    ]
    .into_iter()
    .map(|json| serde_json::from_value(json).unwrap())
    .collect()
}

/// A frame committing revision `rev` of a `cols` by `rows` screen.
fn frame(rev: u64, cols: u16, rows: u16) -> serde_json::Value {
    serde_json::json!({"t":"frame","rev":rev,"cols":cols,"rows":rows,
        "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}})
}

/// Live output after a first screen: `text` on the first row, committed as revision `rev`.
fn update(rev: u64, cols: u16, rows: u16, text: &str) -> [GridOutput; 2] {
    let rows_message = serde_json::json!({"t":"rows","rows":[[0,{"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]}]]});
    [
        serde_json::from_value(rows_message).unwrap(),
        serde_json::from_value(frame(rev, cols, rows)).unwrap(),
    ]
}

/// The size the old app's fit gives the test pane: 816 by 496 less 8 pixels of padding a side is 800 by 480; less the
/// 14-pixel scrollbar column that is 786 pixels of 10-pixel cells and 480 pixels of 22-pixel rows.
const FITTED: (u16, u16) = (78, 21);

// ---- Fit -----------------------------------------------------------------------------------------------------------

/// The fit is the old app's arithmetic, which the program's size follows: an 80x24 reference cell, the scrollbar's
/// column left out, whole pixels, a 2x1 floor and a 1000x1000 ceiling, and nothing at all for an unmeasured pane. A
/// drift here resizes every program by a column or a row.
#[test]
fn the_fit_is_the_old_apps() {
    assert_eq!(fit(metrics(), 800., 480., 1.), Some(FITTED));
    // Fractional room counts in whole pixels, as `parseInt` read it.
    assert_eq!(fit(metrics(), 800.9, 480.9, 1.), Some(FITTED));
    assert_eq!(fit(metrics(), 20., 10., 1.), Some((2, 1)));
    assert_eq!(fit(metrics(), 100_000., 100_000., 1.), Some((1000, 1000)));
    assert_eq!(fit(metrics(), 0., 480., 1.), None);
    assert_eq!(fit(metrics(), 800., -3., 1.), None);
    // On a 2x display the row rounds up to whole device pixels (21.6 becomes 22) while the advance stays 10.3: 786
    // pixels hold 76 columns.
    let odd = CellMetrics::new(10.3, 21.6, 16.).unwrap();
    assert_eq!(fit(odd, 800., 480., 2.), Some((76, 21)));
}

/// Fit before attach: the stream is opened only after the pane was measured, and at the size it will be drawn at, so
/// the replay is rendered for the real size. An eager default-size attach followed by a resize lost replayed lines in
/// the prototype. Until a screen arrives the pane shows the old app's catch-up placeholder and no status line.
#[gpui::test]
fn the_stream_opens_at_the_fitted_size(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let asked = h.opened.borrow().as_ref().map(|(_, asked)| *asked);
    assert_eq!(
        asked,
        Some(FakeAttach {
            take_control: true,
            cols: FITTED.0,
            rows: FITTED.1
        })
    );
    // The fake agrees: its displacing attach carries the size the stream was opened with.
    assert_eq!(
        h.fake().attach(),
        FakeAttach {
            take_control: true,
            cols: FITTED.0,
            rows: FITTED.1
        }
    );
    assert!(h.cx.debug_bounds("terminal-connecting").is_some());
    assert_eq!(h.status(), None);
    assert_eq!(h.text(), None);
    assert_eq!(h.fake().received(), []);
}

/// A changed size goes out as one resize; a redraw at the same size sends nothing. A resize made while an attach is
/// connecting reaches no attachment in the helm client, so the owner sends the size again once the next attachment
/// is live, and only then.
#[gpui::test]
fn a_new_size_is_sent_once_and_again_after_attaching(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.size.set((PANE.0 - 40., PANE.1));
    h.cx.update(|window, _| window.refresh());
    h.settle();
    h.cx.update(|window, _| window.refresh());
    h.settle();
    let smaller = (FITTED.0 - 4, FITTED.1);
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: smaller.0,
            rows: smaller.1
        }]
    );
    // The fake's attach was built when the stream started attaching, so it still asks for the first size.
    let asked = h.fake().attach();
    assert_eq!((asked.cols, asked.rows), FITTED);
    for message in screen(asked.cols, asked.rows, "x", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    h.settle();
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: smaller.0,
            rows: smaller.1
        }]
    );
    // Live: a later resize goes out once and is not repeated.
    h.size.set(PANE);
    h.cx.update(|window, _| window.refresh());
    h.settle();
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: FITTED.0,
            rows: FITTED.1
        }]
    );
}

// ---- Delivery ------------------------------------------------------------------------------------------------------

/// Staged replay: a new attachment's replay goes to a view that is not drawn, so the last complete screen stays up
/// through a reconnect and nothing redraws while the replay arrives; the replay replaces it in one redraw when the
/// helm says it is complete. A replay cut off by a detach is dropped and the old screen stays.
#[gpui::test]
fn a_replay_is_shown_only_once_complete(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("first");
    assert_eq!(h.text().as_deref().map(first_line), Some("first"));
    assert!(h.read(|owner, _| owner.is_live()));

    h.fake().drop_link();
    h.settle();
    assert_eq!(
        h.status().map(|line| line.words),
        Some("Session unavailable — reconnecting".into())
    );
    let asked = h.fake().attach();
    h.fake()
        .output(screen(asked.cols, asked.rows, "second", serde_json::Value::Null)[0].clone());
    h.settle();
    // The attachment is announced: no status line, the old screen still up, not live.
    assert_eq!(h.status(), None);
    assert!(!h.read(|owner, _| owner.is_live()));

    h.reset_renders();
    for message in screen(asked.cols, asked.rows, "second", serde_json::Value::Null)
        .into_iter()
        .skip(1)
    {
        h.fake().output(message);
    }
    h.settle();
    assert_eq!(h.text().as_deref().map(first_line), Some("first"));
    assert_eq!(
        (h.renders("attached-terminal"), h.renders("terminal")),
        (0, 0)
    );

    h.fake().replay_complete();
    h.settle();
    assert_eq!(h.text().as_deref().map(first_line), Some("second"));
    assert_eq!(h.renders("attached-terminal"), 1);
    assert!(h.read(|owner, _| owner.is_live()));

    // A takeover in the middle of the next replay keeps the second screen.
    h.fake().drop_link();
    let asked = h.fake().attach();
    for message in screen(asked.cols, asked.rows, "third", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake()
        .detach(Some(DetachCode::TakenOver), "another window took it");
    h.settle();
    assert_eq!(h.text().as_deref().map(first_line), Some("second"));
}

/// A flood is delivered at most `BATCH` events per turn of the main thread, and the delivery then yields, so other
/// work queued on the main thread (input, a frame) runs between batches instead of after the whole flood. Consecutive
/// output in a turn is applied as one batch, so the terminal redraws once per turn, never once per message. Twenty
/// messages are three turns and three applications. The proof of the yield is a task queued behind the delivery: it
/// runs after the first batch, not after the last.
#[gpui::test]
fn a_flood_is_delivered_in_bounded_turns_that_yield(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("start");
    let (turns_before, applies_before) = h.read(|owner, _| (owner.turns, owner.applies));
    h.reset_renders();
    for rev in 2..12 {
        for message in update(rev, FITTED.0, FITTED.1, &format!("line {rev}")) {
            h.fake().output(message);
        }
    }
    // The first message woke the delivery loop, so it is queued first; this task is queued behind it.
    let seen = Rc::new(Cell::new(None));
    let (owner, witness) = (h.owner.clone(), seen.clone());
    let _behind = h.cx.update(|_, cx| {
        cx.spawn(async move |cx| witness.set(Some(cx.update(|cx| owner.read(cx).turns))))
    });
    h.settle();
    assert_eq!(
        seen.get(),
        Some(turns_before + 1),
        "turns when the queued task ran"
    );
    let (turns, applies) = h.read(|owner, _| (owner.turns, owner.applies));
    assert_eq!((turns - turns_before, applies - applies_before), (3, 3));
    let redraws = h.renders("terminal");
    assert!((1..=3).contains(&redraws), "{redraws} redraws for 3 turns");
    assert_eq!(h.text().as_deref().map(first_line), Some("line 11"));
}

/// A program's own copy (OSC 52) reaches the clipboard, as in the old app.
#[gpui::test]
fn a_programs_clipboard_write_reaches_the_clipboard(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("x");
    assert_eq!(h.cx.read_from_clipboard(), None);
    let write: GridOutput = serde_json::from_value(
        serde_json::json!({"t":"clipboard","target":"clipboard","text":"copied","last":true}),
    )
    .unwrap();
    h.fake().output(write);
    h.settle();
    assert_eq!(
        h.cx.read_from_clipboard().and_then(|item| item.text()),
        Some("copied".into())
    );
}

// ---- What the pane says --------------------------------------------------------------------------------------------

/// A detach that waits for the user shows "Detached:" with the helm's reason, escaped as host text, and a Take
/// control button that attaches again through the displacing route. A refusal shows the old app's "Connection error"
/// and offers the same, since the stream waits for Take control there too: without the button the terminal would stay
/// dead until the session is reopened. A closed tab offers nothing, since there is nothing left to take.
#[gpui::test]
fn detached_and_refused_offer_take_control(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake()
        .detach(Some(DetachCode::TakenOver), "taken over\u{202e}");
    h.settle();
    let line = h.status().unwrap();
    assert_eq!(line.words, "Detached: ");
    assert_eq!(
        line.reason.as_ref().map(HostText::raw),
        Some("taken over\u{202e}")
    );
    assert!(line.take_control);
    // The screen stays up under the banner.
    assert_eq!(h.text().as_deref().map(first_line), Some("screen"));

    let at = h.cx.debug_bounds("terminal-take-control").unwrap().center();
    h.cx.simulate_click(at, Modifiers::none());
    h.settle();
    // The fake acts on the request when asked, and then tells the owner it is attaching again.
    assert_eq!(h.fake().phase(), FakePhase::Attaching);
    assert!(
        h.fake()
            .received()
            .contains(&FakeTerminalRequest::TakeControl)
    );
    h.settle();
    assert_eq!(h.status(), None);
    assert!(h.fake().attach().take_control);

    h.fake().drop_link();
    h.fake().refuse(401);
    h.settle();
    let line = h.status().unwrap();
    assert_eq!(line.words, "Connection error");
    assert!(line.take_control);
    assert!(h.cx.debug_bounds("terminal-take-control").is_some());

    let at = h.cx.debug_bounds("terminal-take-control").unwrap().center();
    h.cx.simulate_click(at, Modifiers::none());
    h.settle();
    h.fake().phase();
    h.settle();
    h.attach_showing("tab");
    h.fake()
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    h.settle();
    assert!(!h.status().unwrap().take_control);
    assert!(h.cx.debug_bounds("terminal-take-control").is_none());
}

/// A message the grid model refuses ends the stream: the owner closes it, says "Terminal stream error:" with the
/// model's own words and offers nothing, and delivers nothing after it. The screen from before the fault stays.
/// Restarting the attachment by itself is deliberately not done yet.
#[gpui::test]
fn a_protocol_fault_closes_the_stream_with_an_error_line(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("before");
    let second_hello: GridOutput = serde_json::from_value(
        serde_json::json!({"t":"hello","cols":FITTED.0,"rows":FITTED.1,"history_cap":0}),
    )
    .unwrap();
    h.fake().output(second_hello);
    h.settle();
    let line = h.status().unwrap();
    assert_eq!(
        line.words,
        "Terminal stream error: grid message breaks a bound: a second hello on one attachment"
    );
    assert!(!line.take_control);
    assert_eq!(h.fake().phase(), FakePhase::Closed);
    assert!(!h.read(|owner, _| owner.is_live()));
    assert_eq!(h.text().as_deref().map(first_line), Some("before"));
    assert!(h.cx.debug_bounds("terminal-take-control").is_none());
}

/// The exit line's words are the old app's, for a status and for a signal. A replay that already holds the exit
/// shows the line with the screen, in the one redraw that shows the replay, rather than a frame later.
#[gpui::test]
fn a_replayed_exit_shows_its_line_with_the_screen(cx: &mut TestAppContext) {
    assert_eq!(
        exit_text(ExitState::Code(3)),
        "Process exited with status 3"
    );
    assert_eq!(exit_text(ExitState::Signal(9)), "Process ended by signal 9");
    let mut h = harness(cx);
    let asked = h.fake().attach();
    let mut replay = screen(
        asked.cols,
        asked.rows,
        "bye",
        serde_json::json!({"code": 3}),
    )
    .into_iter();
    h.fake().output(replay.next().unwrap());
    h.settle();
    h.reset_renders();
    for message in replay {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    h.settle();
    assert!(h.cx.debug_bounds("terminal-exit").is_some());
    assert_eq!(h.renders("attached-terminal"), 1);
}

/// A program that exits while the terminal is live sends nothing but a new state; the owner draws the exit line from
/// that state, so the state alone must redraw it. This is the ordinary end of an agent session, watched.
#[gpui::test]
fn a_live_exit_shows_its_line(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("running");
    assert!(h.cx.debug_bounds("terminal-exit").is_none());
    let exited: GridOutput = serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
        "alt_screen":false,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":{"signal":15}}))
    .unwrap();
    h.fake().output(exited);
    h.settle();
    assert!(h.cx.debug_bounds("terminal-exit").is_some());
}

/// The screen is lifted by whole rows to fit its room, so the newest rows stay in view: under the status line, which
/// takes room the fit leaves to the grid, and above the exit line, which needs a row of its own (the old app's
/// placement). Neither showing, nothing is lifted.
#[test]
fn the_screen_lifts_by_whole_rows_to_fit_its_room() {
    // With an exit line: 21 rows of 22 pixels in 480 leave 18 pixels, short of a line by 4, so one row goes up.
    assert_eq!(lift(21, 22., 480., true, false), 22.);
    assert_eq!(lift(20, 22., 480., true, false), 0.);
    assert_eq!(lift(20, 22., 462., true, false), 0.);
    // A grid far taller than its room lifts by no more than all of its rows.
    assert_eq!(lift(3, 22., 10., true, false), 66.);
    // Under the status line: a 29-pixel band over a fitted grid lifts it two rows, and any shortfall lifts.
    assert_eq!(lift(21, 22., 462. - 29., false, true), 44.);
    assert_eq!(lift(21, 22., 461.5, false, true), 22.);
    assert_eq!(lift(21, 22., 462., false, true), 0.);
}

/// A live terminal is never lifted, even when its fitted grid is taller than its room. The fit sizes rows by an 80x24
/// reference cell and the grid is drawn with its own, so at a fractional scale the grid overshoots its room by a pixel
/// or more: at scale 1.25 a 17-pixel character in 1020 pixels fits 58 rows of 1021/58 pixels, 1.00006 pixels too tall
/// in `f32`. A tolerance of a pixel, which this replaced, lifted that terminal's top row out of view for nothing.
#[test]
fn a_fitted_live_grid_is_never_lifted_at_a_fractional_scale() {
    let metrics = CellMetrics::new(8.4, 17., 14.).unwrap();
    let (cols, rows) = fit(metrics, 800., 1020., 1.25).unwrap();
    let cell_height = metrics.cell_size(cols, rows, 1.25).height;
    let overshoot = f32::from(rows) * cell_height - 1020.;
    assert!(
        overshoot > 1.,
        "the premise: {rows} rows of {cell_height} overshoot by {overshoot}"
    );
    assert_eq!(lift(rows, cell_height, 1020., false, false), 0.);
}

/// While the status line shows, the kept screen's last row stays in view: the screen is lifted by whole rows into the
/// room under the band, rather than its newest rows being cut off at the bottom. Live, nothing is lifted.
#[gpui::test]
fn the_status_line_keeps_the_newest_rows_in_view(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let placed = |h: &mut Harness| {
        h.read(|owner, cx| {
            let shown = owner.committed.as_ref().unwrap().read(cx);
            (owner.placement(shown).unwrap(), owner.room.unwrap())
        })
    };
    assert_eq!(placed(&mut h).0.lift, 0.);
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    h.settle();
    assert!(h.status().is_some(), "the status line shows");
    let (placement, room) = placed(&mut h);
    assert!(placement.lift > 0., "{placement:?} in {room}");
    let bottom = f32::from(placement.rows) * placement.cell_height - placement.lift;
    assert!(
        bottom <= room,
        "the last row ends at {bottom}, below the room's {room}"
    );
}

// ---- Lifetime ------------------------------------------------------------------------------------------------------

/// A reset retires the UI session, and the terminal owner goes with it: its stream is closed (the helm client's
/// reconnect worker ends with it), its screens are dropped, and it shows nothing from the old connection. SPEC_impl.md,
/// "One failure path: the reset": nothing started for an old connection may outlive it.
#[gpui::test]
fn a_reset_retires_the_owner(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("old connection");
    h.helm.break_channel();
    h.settle();
    assert!(h.cx.update(|_, cx| h.connection.read(cx).session().is_none()));
    assert_eq!(h.fake().phase(), FakePhase::Closed);
    assert_eq!(h.text(), None);
    assert_eq!(h.status(), None);
    assert!(!h.read(|owner, _| owner.is_live()));
    assert!(h.cx.debug_bounds("terminal-connecting").is_none());
}

/// The bridge's `terminal_text` probe reads the newest owner that shows a screen, so an end-to-end run can read what
/// the terminal displays; an owner still catching up is skipped.
#[gpui::test]
fn the_probe_reads_the_newest_shown_screen(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    assert_eq!(h.cx.update(|_, cx| probe::terminal_text(cx)), None);
    h.attach_showing("probed");
    assert_eq!(
        h.cx.update(|_, cx| probe::terminal_text(cx))
            .as_deref()
            .map(first_line),
        Some("probed")
    );
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

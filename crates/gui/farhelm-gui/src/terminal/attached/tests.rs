//! The attached terminal's contract with its stream, against the helm client's fake terminal: when it attaches and at
//! what size, which view a replay goes to, how much one turn delivers, what the pane says while the stream is not live,
//! that a reset takes the whole thing down, and the input side: focus, which keys reach the program and when, and the
//! cursor blink.
//!
//! The fake plays the helm (`farhelm_helm_client::fake::terminal`); GPUI's deterministic executor cannot wait on a real
//! socket. The owner is drawn in a pane of a fixed size inside a window from the shared harness (`test_support`), so its
//! first fit is a real measurement, its Take control is a real button, and the focus and layer invariants are checked
//! after every step.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{
    FakeAttach, FakePhase, FakeTerminal, FakeTerminalRequest,
};
use farhelm_input_mapping::Modifiers as KeyModifiers;
use farhelm_proto::DetachCode;
use gpui::{
    AppContext as _, Context, Entity, KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement,
    Render, Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};

use super::*;
use crate::actions::{CONTEXT, DeleteSession, TERMINAL_KEYS, THIS_PLATFORM, owner_on};
use crate::session::{self, Connection};
use crate::test_support::{self, advance, click, keys, redraw, settle};
use crate::{focus, root};

// ---- Helpers -------------------------------------------------------------------------------------------------------

/// The pane every test draws the owner in, in logical pixels.
const PANE: (f32, f32) = (816., 496.);

/// Fixed metrics, so the fit is arithmetic the test can check: a 10 by 22 pixel character.
fn metrics() -> CellMetrics {
    CellMetrics::new(10., 22., 16.).unwrap()
}

/// The size of the second owner's pane, in tests that draw two: small enough that both fit the test window.
const SECOND_PANE: (f32, f32) = (400., 200.);

/// The window's root: a pane of the size in `size` holding the owner (and, in tests that need one, a second, smaller
/// pane below it), wired through [`root`] as the app's root views are, so the Take control button and focus behave as
/// they do there. It handles the session Delete shortcut's action on the root element, as the session view will,
/// counting its runs. Its own root view rather than [`test_support::open`]'s scene because the test changes its size
/// and reads its counts.
struct Pane {
    owner: Entity<AttachedTerminal>,
    second: Option<Entity<AttachedTerminal>>,
    size: Rc<Cell<(f32, f32)>>,
    deletes: Rc<Cell<usize>>,
    root: root::Attachment,
}

impl Pane {
    /// The pane for `owner` (and `second`) at [`PANE`]'s size, attached to the window it is built in.
    fn new(
        owner: Entity<AttachedTerminal>,
        second: Option<Entity<AttachedTerminal>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Pane {
        Pane {
            owner,
            second,
            size: Rc::new(Cell::new(PANE)),
            deletes: Rc::new(Cell::new(0)),
            root: root::attach(window, cx),
        }
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (width, height) = self.size.get();
        let deletes = self.deletes.clone();
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(move |_: &DeleteSession, _, _| deletes.set(deletes.get() + 1))
            .child(
                div()
                    .flex_none()
                    .w(px(width))
                    .h(px(height))
                    .child(self.owner.clone()),
            )
            .children(self.second.clone().map(|second| {
                div()
                    .flex_none()
                    .w(px(SECOND_PANE.0))
                    .h(px(SECOND_PANE.1))
                    .child(second)
            }));
        root::wrap(content, &self.root, window, cx)
    }
}

/// Where an owner's opener leaves the fake stream it opened and the attach it was asked for.
type Opened = Rc<RefCell<Option<(FakeTerminal, FakeAttach)>>>;

/// One owner in its window (and maybe a second), the helm behind its UI session, and the fake stream each opened
/// (once it has).
struct Harness<'a> {
    cx: &'a mut VisualTestContext,
    helm: FakeHelm,
    connection: Entity<Connection>,
    owner: Entity<AttachedTerminal>,
    size: Rc<Cell<(f32, f32)>>,
    opened: Opened,
    second: Option<(Entity<AttachedTerminal>, Opened)>,
    /// How many times the session Delete shortcut's action ran.
    deletes: Rc<Cell<usize>>,
}

/// A bootstrapped app connected to the fake helm, with an owner made for its UI session and drawn in a window the shared
/// harness opened ([`test_support::open_view`], which has run the per-step checks on its first frame). The owner's
/// opener records the attach it was asked for and hands out a fake stream for it. Every step a test takes after this
/// ends in one of the harness's step helpers ([`settle`] for a fake's delivery or a raw key event), so the checks run
/// after each.
fn harness(cx: &mut TestAppContext) -> Harness<'_> {
    harness_with(cx, false)
}

/// [`harness`], with a second owner, made after the first, drawn below it when `two` is set.
fn harness_with(cx: &mut TestAppContext, two: bool) -> Harness<'_> {
    test_support::install(cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    helm.connect(Vec::new());
    cx.run_until_parked();
    let session = cx.update(|cx| connection.read(cx).session().cloned().unwrap());
    let opened: Opened = Rc::new(RefCell::new(None));
    let second_opened: Opened = Rc::new(RefCell::new(None));
    let slots = [opened.clone(), second_opened.clone()];
    let (pane, cx) = test_support::open_view(cx, |window, cx| {
        let [first, second] = slots;
        let first = cx.new(|cx| owner(&session, first, cx));
        let second = two.then(|| cx.new(|cx| owner(&session, second, cx)));
        Pane::new(first, second, window, cx)
    });
    let (owner, second, size, deletes) = cx.update(|_, cx| {
        let pane = pane.read(cx);
        (
            pane.owner.clone(),
            pane.second.clone(),
            pane.size.clone(),
            pane.deletes.clone(),
        )
    });
    Harness {
        cx,
        helm,
        connection,
        owner,
        size,
        opened,
        second: second.map(|owner| (owner, second_opened)),
        deletes,
    }
}

/// An owner of session `s1`'s agent terminal for `session`, at [`metrics`], whose opener reports into `slot`.
fn owner(
    session: &Entity<UiSession>,
    slot: Opened,
    cx: &mut Context<AttachedTerminal>,
) -> AttachedTerminal {
    let target = TerminalTarget {
        session: "s1".into(),
        tab: None,
        lease: AttachmentLease::new(),
    };
    AttachedTerminal::new(session, target, metrics(), opener(slot), cx)
}

/// An opener that records the attach it was asked for in `slot` and hands out a fake stream for it.
fn opener(slot: Opened) -> OpenStream {
    Box::new(move |attach| {
        let (fake, handle, events) = FakeTerminal::open(&attach);
        // What the opener was given; no resize can have overtaken it yet.
        let asked = FakeAttach {
            take_control: true,
            cols: attach.cols,
            rows: attach.rows,
            resize_on_open: None,
        };
        *slot.borrow_mut() = Some((fake, asked));
        (handle, events)
    })
}

impl Harness<'_> {
    /// The fake stream; panics if the owner has not opened one.
    fn fake(&self) -> std::cell::RefMut<'_, FakeTerminal> {
        std::cell::RefMut::map(self.opened.borrow_mut(), |opened| {
            &mut opened.as_mut().expect("the owner opened no stream").0
        })
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
        settle(self.cx);
    }

    /// [`Harness::attach_showing`], with the screen's cursor blinking.
    fn attach_blinking(&mut self) {
        let asked = self.fake().attach();
        let mut replay = screen(asked.cols, asked.rows, "blink", serde_json::Value::Null);
        if let Some(last) = replay.last_mut() {
            *last = serde_json::from_value(serde_json::json!({"t":"frame","rev":1,
                "cols":asked.cols,"rows":asked.rows,
                "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":true}}))
            .unwrap();
        }
        for message in replay {
            self.fake().output(message);
        }
        self.fake().replay_complete();
        settle(self.cx);
    }

    /// The user chooses this terminal's session: the focus coordinator makes `owner` home and focuses it.
    fn choose(&mut self, owner: &Entity<AttachedTerminal>) {
        let handle = self
            .cx
            .update(|_, cx| owner.read(cx).focus_handle().clone());
        self.cx.update(|window, cx| {
            let chosen = focus::Mark::take(cx);
            focus::choose_terminal(&handle, &chosen, window, cx);
        });
        settle(self.cx);
        assert!(
            self.focused(owner),
            "the premise: the chosen terminal holds focus"
        );
    }

    /// Whether `owner`'s pane holds focus now.
    fn focused(&mut self, owner: &Entity<AttachedTerminal>) -> bool {
        let handle = self
            .cx
            .update(|_, cx| owner.read(cx).focus_handle().clone());
        self.cx.update(|window, _| handle.is_focused(window))
    }

    /// A key-down for `chord`: a fresh press, or with `held` an auto-repeat of the key already held down, which is how
    /// the platform reports one. Filled in as GPUI's own simulated typing fills it (the text a plain key types).
    fn key_down(&mut self, chord: &'static str, held: bool) {
        self.cx.simulate_event(KeyDownEvent {
            native_key: None,
            keystroke: keystroke(chord),
            is_held: held,
            prefer_character_input: false,
        });
        settle(self.cx);
    }

    /// Hold `chord` down for a press and two auto-repeats, then release it.
    fn hold(&mut self, chord: &'static str) {
        self.key_down(chord, false);
        self.key_down(chord, true);
        self.key_down(chord, true);
        self.cx.simulate_event(KeyUpEvent {
            native_key: None,
            keystroke: keystroke(chord),
        });
        settle(self.cx);
    }

    /// The input the owner sent since the last call, with whether the stream carried each to the attachment it was
    /// stamped for. Other requests (resizes) are left out.
    fn sent(&mut self) -> Vec<(GridInput, bool)> {
        self.fake()
            .received()
            .into_iter()
            .filter_map(|request| match request {
                FakeTerminalRequest::Input { input, delivered } => Some((input, delivered)),
                _ => None,
            })
            .collect()
    }

    /// The cursor the shown screen draws.
    fn phase(&mut self) -> CursorPhase {
        self.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).phase)
    }

    /// Let `ticks` animation ticks pass, one at a time.
    fn ticks(&mut self, ticks: u32) {
        for _ in 0..ticks {
            advance(self.cx, ticker::TICK);
        }
    }
}

/// `chord` as GPUI's simulated typing would send it: parsed, with the text a plain key types filled in.
fn keystroke(chord: &'static str) -> Keystroke {
    match Keystroke::parse(chord) {
        Ok(keystroke) => keystroke.with_simulated_ime(),
        Err(error) => panic!("{chord} does not parse: {error}"),
    }
}

/// The `Key` message a plain key sends on this platform: `key` and its DOM key code, no modifiers.
fn key(key: &str, kc: u32) -> GridInput {
    GridInput::Key {
        key: key.into(),
        code: String::new(),
        kc,
        mods: 0,
        mac: THIS_PLATFORM == Platform::Mac,
    }
}

fn text(data: &str) -> GridInput {
    GridInput::Text { data: data.into() }
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
            rows: FITTED.1,
            resize_on_open: None,
        })
    );
    // The fake agrees: its displacing attach carries the size the stream was opened with.
    assert_eq!(
        h.fake().attach(),
        FakeAttach {
            take_control: true,
            cols: FITTED.0,
            rows: FITTED.1,
            resize_on_open: None,
        }
    );
    assert!(h.cx.debug_bounds("terminal-connecting").is_some());
    assert_eq!(h.status(), None);
    assert_eq!(h.text(), None);
    assert_eq!(h.fake().received(), []);
}

/// A changed size goes out as one resize, and a redraw at the same size sends nothing: each resize can reflow the
/// program's screen and its history, so a repeated one is not free. That holds while the first attach is still
/// connecting too, where the helm client sends the new size as soon as the socket opens; the owner must not resend it
/// once the attachment is live, which it once did to work around a client that dropped that resize.
#[gpui::test]
fn a_new_size_is_sent_once(cx: &mut TestAppContext) {
    let h = harness(cx);
    h.size.set((PANE.0 - 40., PANE.1));
    redraw(h.cx);
    redraw(h.cx);
    let smaller = (FITTED.0 - 4, FITTED.1);
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: smaller.0,
            rows: smaller.1
        }]
    );
    // The fake's attach was built when the stream started attaching, so its query still asks for the first size, and
    // the new one follows as the socket's first message.
    let asked = h.fake().attach();
    assert_eq!((asked.cols, asked.rows), FITTED);
    assert_eq!(asked.resize_on_open, Some(smaller));
    for message in screen(asked.cols, asked.rows, "x", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert_eq!(
        h.fake().received(),
        [],
        "the owner does not resend the size once the attachment is live"
    );
    // Live: a later resize goes out once and is not repeated.
    h.size.set(PANE);
    redraw(h.cx);
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
    settle(h.cx);
    assert_eq!(
        h.status().map(|line| line.words),
        Some("Session unavailable — reconnecting".into())
    );
    let asked = h.fake().attach();
    h.fake()
        .output(screen(asked.cols, asked.rows, "second", serde_json::Value::Null)[0].clone());
    settle(h.cx);
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
    settle(h.cx);
    assert_eq!(h.text().as_deref().map(first_line), Some("first"));
    assert_eq!(
        (h.renders("attached-terminal"), h.renders("terminal")),
        (0, 0)
    );

    h.fake().replay_complete();
    settle(h.cx);
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
    settle(h.cx);
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
    settle(h.cx);
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
    settle(h.cx);
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
    settle(h.cx);
    let line = h.status().unwrap();
    assert_eq!(line.words, "Detached: ");
    assert_eq!(
        line.reason.as_ref().map(HostText::raw),
        Some("taken over\u{202e}")
    );
    assert!(line.take_control);
    // The screen stays up under the banner.
    assert_eq!(h.text().as_deref().map(first_line), Some("screen"));

    click(h.cx, "terminal-take-control");
    // The fake acts on the request when asked, and then tells the owner it is attaching again.
    assert_eq!(h.fake().phase(), FakePhase::Attaching);
    assert!(
        h.fake()
            .received()
            .contains(&FakeTerminalRequest::TakeControl)
    );
    settle(h.cx);
    assert_eq!(h.status(), None);
    assert!(h.fake().attach().take_control);

    h.fake().drop_link();
    h.fake().refuse(401);
    settle(h.cx);
    let line = h.status().unwrap();
    assert_eq!(line.words, "Connection error");
    assert!(line.take_control);
    assert!(h.cx.debug_bounds("terminal-take-control").is_some());

    click(h.cx, "terminal-take-control");
    h.fake().phase();
    settle(h.cx);
    h.attach_showing("tab");
    h.fake()
        .detach(Some(DetachCode::TabClosed), "the tab was closed");
    settle(h.cx);
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
    settle(h.cx);
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
    settle(h.cx);
    h.reset_renders();
    for message in replay {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
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
    settle(h.cx);
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
            (owner.placement(shown, cx).unwrap(), owner.room.unwrap())
        })
    };
    assert_eq!(placed(&mut h).0.lift, 0.);
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    settle(h.cx);
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
    settle(h.cx);
    assert!(h.cx.update(|_, cx| h.connection.read(cx).session().is_none()));
    assert_eq!(h.fake().phase(), FakePhase::Closed);
    assert_eq!(h.text(), None);
    assert_eq!(h.status(), None);
    assert!(!h.read(|owner, _| owner.is_live()));
    assert!(h.cx.debug_bounds("terminal-connecting").is_none());
}

/// The bridge's `terminal_text` probe reads the terminal that holds focus, which is where an end-to-end run's typing
/// goes, so the run reads the screen it typed into. Reading the newest terminal instead, as before terminals took
/// focus, reads the wrong one once a session view shows more than one. With no terminal focused it reads nothing.
#[gpui::test]
fn the_probe_reads_the_focused_terminal(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    let probe = |h: &mut Harness| {
        h.cx.cx
            .update(probe::terminal_text)
            .map(|text| first_line(&text).to_owned())
    };
    h.attach_showing("older");
    let (second, second_opened) = h.second.clone().unwrap();
    let asked = second_opened.borrow_mut().as_mut().unwrap().0.attach();
    for message in screen(asked.cols, asked.rows, "newer", serde_json::Value::Null) {
        second_opened
            .borrow_mut()
            .as_mut()
            .unwrap()
            .0
            .output(message);
    }
    second_opened
        .borrow_mut()
        .as_mut()
        .unwrap()
        .0
        .replay_complete();
    settle(h.cx);
    assert_eq!(probe(&mut h), None, "no terminal holds focus");

    let first = h.owner.clone();
    h.choose(&first);
    assert_eq!(probe(&mut h).as_deref(), Some("older"));
    h.choose(&second);
    assert_eq!(probe(&mut h).as_deref(), Some("newer"));
}

/// Each window keeps its own focus, so two windows can each have a focused terminal. The probe then reads the active
/// window's, the one a script types into, and among windows that are all inactive (a headless run before activation,
/// say) the oldest terminal's, so the answer never depends on which window happened to be opened last.
#[gpui::test]
fn the_probe_prefers_the_active_windows_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("first");
    let first = h.owner.clone();
    h.choose(&first);

    // A second window with a terminal of its own, made after the first, for the same UI session.
    let connection = h.connection.clone();
    let session =
        h.cx.update(|_, cx| connection.read(cx).session().cloned().unwrap());
    let opened: Opened = Rc::new(RefCell::new(None));
    let slot = opened.clone();
    let (pane, mut other) = test_support::another_window(h.cx, |window, cx| {
        let owner = cx.new(|cx| owner(&session, slot, cx));
        Pane::new(owner, None, window, cx)
    });
    let second = other.update(|_, cx| pane.read(cx).owner.clone());
    {
        let mut opened = opened.borrow_mut();
        let fake = &mut opened
            .as_mut()
            .expect("the second terminal opened no stream")
            .0;
        let asked = fake.attach();
        for message in screen(asked.cols, asked.rows, "second", serde_json::Value::Null) {
            fake.output(message);
        }
        fake.replay_complete();
    }
    settle(&mut other);
    other.update(|window, cx| {
        let handle = second.read(cx).focus_handle().clone();
        focus::choose_terminal(&handle, &focus::Mark::take(cx), window, cx);
    });
    settle(&mut other);
    let probe = |h: &mut Harness| {
        h.cx.cx
            .update(probe::terminal_text)
            .map(|text| first_line(&text).to_owned())
    };
    assert!(
        h.focused(&first)
            && other.update(|window, cx| second.read(cx).focus_handle().is_focused(window)),
        "the premise: each window's terminal holds its focus"
    );
    assert!(other.update(|window, _| window.is_window_active()));
    assert_eq!(
        probe(&mut h).as_deref(),
        Some("second"),
        "the second window is active"
    );

    h.cx.update(|window, _| window.activate_window());
    settle(h.cx);
    assert_eq!(
        probe(&mut h).as_deref(),
        Some("first"),
        "the first window is active"
    );

    h.cx.deactivate_window();
    assert!(!other.update(|window, _| window.is_window_active()));
    assert_eq!(
        probe(&mut h).as_deref(),
        Some("first"),
        "no window is active: the oldest terminal"
    );
}

// ---- Input ---------------------------------------------------------------------------------------------------------

/// SPEC.md, "Where focus rests": choosing a session focuses its terminal, taking focus from whatever holds it, and a
/// press on a terminal focuses it on every platform (a click there is for typing into it). The terminal takes focus
/// only through the focus coordinator, so this is what the session view's choice and the user's click do.
#[gpui::test]
fn clicking_or_choosing_a_terminal_focuses_it(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    let (first, second) = (h.owner.clone(), h.second.clone().unwrap().0);
    assert!(
        !h.focused(&first) && !h.focused(&second),
        "the premise: focus rests on the root"
    );
    // The first pane sits at the window's top left; both panes carry the same debug selector.
    h.cx.simulate_click(point(px(PANE.0 / 2.), px(PANE.1 / 2.)), Modifiers::none());
    settle(h.cx);
    assert!(
        h.focused(&first),
        "a click focuses the terminal it lands on"
    );
    h.choose(&second);
    assert!(!h.focused(&first));
}

/// Only the grid's area is the terminal's for keys, not the status line above it: Tab moves through the status line's
/// Take control button and on into the terminal, where it then stays for the program. With the whole pane in the
/// terminal's key context, the context that hands Tab and Escape to the program, a focused Take control button kept
/// Tab and Escape from doing anything, and the keyboard could not leave it.
#[gpui::test]
fn tab_moves_through_the_status_line_into_the_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    settle(h.cx);
    let owner = h.owner.clone();
    assert!(h.cx.debug_bounds("terminal-take-control").is_some());
    let before = h.cx.update(|window, cx| window.focused(cx));
    keys(h.cx, "tab");
    // The window's tab stops are the button and the terminal (the root, where focus rested, is not one), so focus
    // that moved and is not on the terminal is on the button.
    let after = h.cx.update(|window, cx| window.focused(cx));
    assert!(
        after.is_some() && after != before && !h.focused(&owner),
        "the premise: Tab reached the button first"
    );
    keys(h.cx, "tab");
    assert!(
        h.focused(&owner),
        "Tab moved on from the button into the terminal"
    );
    keys(h.cx, "tab");
    assert!(h.focused(&owner), "Tab in the terminal is the program's");
}

/// The program's keys go out encoded by `farhelm-input-mapping`, stamped with the live attachment so the stream
/// carries them to it: a plain letter and Enter as keys, and a Shift letter and a space as the text they type, which
/// with no input method handler nothing else would send. After a reconnect, typing goes to the new attachment.
#[gpui::test]
fn program_keys_are_sent_to_the_live_attachment(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    keys(h.cx, "a enter shift-a space");
    assert_eq!(
        h.sent(),
        [
            (key("a", 65), true),
            (key("Enter", 13), true),
            (text("A"), true),
            (text(" "), true),
        ]
    );

    h.fake().drop_link();
    h.attach_showing("again");
    keys(h.cx, "b");
    assert_eq!(h.sent(), [(key("b", 66), true)]);
}

/// Nothing is sent while the terminal is not live: while it attaches, while its replay arrives (the screen it would
/// type at is not shown yet), while it reconnects, and while it waits detached or refused. Keys typed then are
/// dropped, not sent once it is live again, as the helm client does with input for a stream that is not attached.
#[gpui::test]
fn nothing_is_sent_unless_the_terminal_is_live(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    keys(h.cx, "a");
    assert_eq!(h.sent(), [], "attaching");

    let asked = h.fake().attach();
    let mut replay = screen(asked.cols, asked.rows, "x", serde_json::Value::Null).into_iter();
    h.fake().output(replay.next().unwrap());
    settle(h.cx);
    keys(h.cx, "b");
    assert_eq!(h.sent(), [], "replaying");
    for message in replay {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert!(h.focused(&owner) && h.read(|owner, _| owner.is_live()));
    keys(h.cx, "c");
    assert_eq!(h.sent(), [(key("c", 67), true)], "live");

    h.fake().drop_link();
    settle(h.cx);
    keys(h.cx, "d");
    assert_eq!(h.sent(), [], "reconnecting");

    h.attach_showing("y");
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    settle(h.cx);
    keys(h.cx, "e");
    assert_eq!(h.sent(), [], "detached");

    h.owner
        .clone()
        .update(h.cx, |owner, cx| owner.take_control(cx));
    settle(h.cx);
    h.fake().refuse(401);
    settle(h.cx);
    keys(h.cx, "f");
    assert!(h.focused(&owner), "the premise: still focused");
    assert_eq!(h.sent(), [], "refused");
}

/// Keys that are not the program's never reach it: Farhelm's shortcuts that work from a terminal (bound or not yet
/// bound) and the terminal's own commands, which do nothing until scrollback, selection and paste exist. Each is
/// pressed in a live, focused terminal, which sends a program key typed after them.
#[gpui::test]
fn app_and_terminal_keys_never_reach_the_program(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    let kept: Vec<&str> = TERMINAL_KEYS
        .iter()
        .filter(|row| row.owner != KeyOwner::Program)
        .filter_map(|row| row.chord(THIS_PLATFORM))
        .collect();
    assert!(!kept.is_empty());
    for chord in kept {
        keys(h.cx, chord);
        assert_eq!(h.sent(), [], "{chord} reached the program");
    }
    keys(h.cx, "x");
    assert_eq!(h.sent(), [(key("x", 88), true)]);
}

/// A held ordinary key repeats to the program, as in any terminal: held arrows and letters must keep coming.
#[gpui::test]
fn a_held_ordinary_key_repeats_to_the_program(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.hold("a");
    assert_eq!(h.sent(), vec![(key("a", 65), true); 3]);
}

/// A held app shortcut never reaches the program: a bound one that acts once per press runs once (the keymap's repeat
/// guard drops its repeats before the terminal sees them), and one whose feature has no binding yet is dropped by the
/// terminal, press and repeats alike.
#[gpui::test]
fn a_held_app_shortcut_never_reaches_the_program(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (delete, switcher) = match THIS_PLATFORM {
        Platform::Mac => ("cmd-w", "cmd-k"),
        Platform::Linux => ("ctrl-shift-backspace", "ctrl-shift-k"),
    };
    h.cx.update(|_, cx| cx.bind_keys([KeyBinding::new(delete, DeleteSession, Some(CONTEXT))]));
    h.hold(delete);
    assert_eq!(h.deletes.get(), 1, "the bound shortcut runs once");
    h.hold(switcher);
    assert_eq!(h.sent(), [], "neither reached the program");
    assert!(h.focused(&owner), "the premise: the terminal kept focus");
}

// ---- Paste ---------------------------------------------------------------------------------------------------------

/// This platform's paste chords, as the keymap binds them.
fn paste_chords() -> Vec<&'static str> {
    crate::actions::TERMINAL_KEYS
        .iter()
        .filter_map(|key| Some((key.chord(THIS_PLATFORM)?, key.bind?)))
        .filter(|(chord, bind)| bind(chord).action().partial_eq(&crate::actions::Paste))
        .map(|(chord, _)| chord)
        .collect()
}

fn paste(data: &str, last: bool) -> GridInput {
    GridInput::Paste {
        data: data.into(),
        last,
    }
}

/// A paste sends the clipboard's text to the program as a paste, not as typing, so the supervisor can bracket it for a
/// program that asked: from each of this platform's chords (Cmd+V on a Mac; Ctrl+Shift+V and Shift+Insert on Linux)
/// and from the action itself, which is how a menu item reaches the focused terminal. The text goes out exactly as
/// copied, an embedded end-of-paste marker included: stripping that marker so it cannot end the paste is the
/// supervisor's sanitizing (`grid_input.rs`), and a second copy here would drift from it.
#[gpui::test]
fn a_paste_sends_the_clipboards_text_as_a_paste(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    let copied = "one\ntwo \u{1b}[201~ three";
    h.cx.write_to_clipboard(ClipboardItem::new_string(copied.into()));
    let chords = paste_chords();
    assert!(
        !chords.is_empty(),
        "the premise: this platform has a paste chord"
    );
    for chord in chords {
        keys(h.cx, chord);
        assert_eq!(h.sent(), [(paste(copied, true), true)], "{chord}");
    }
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [(paste(copied, true), true)], "the action");
}

/// A paste of more than a megabyte reaches the program whole and in order, as the old app's large-paste test asked
/// (`terminal-flood.spec.ts` at the reference commit). A single input message over 64 KiB would end the attachment, so
/// the paste goes out as the input mapping's parts, one group, with only the last part closing the paste. The text is
/// a 23-letter cycle, which no part boundary lines up with, so a lost, repeated or reordered part changes the joined
/// text.
#[gpui::test]
fn a_large_paste_arrives_whole_and_in_order(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    let size = 1024 * 1024 + 1;
    let copied: String = "abcdefghijklmnopqrstuvw"
        .chars()
        .cycle()
        .take(size)
        .collect();
    h.cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()));
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    let sent = h.sent();
    assert!(sent.len() > 1, "a paste this large goes in parts");
    let mut joined = String::new();
    for (index, (input, delivered)) in sent.iter().enumerate() {
        assert!(delivered, "part {index} reached the attachment");
        match input {
            GridInput::Paste { data, last } => {
                assert_eq!(
                    *last,
                    index == sent.len() - 1,
                    "only the last part closes the paste"
                );
                joined.push_str(data);
            }
            other => panic!("part {index} is not a paste: {other:?}"),
        }
    }
    assert!(joined == copied, "the parts join to the copied text");
}

/// A paste goes to the program only while the terminal is live, like typing: not while it attaches or replays, and not
/// while it reconnects, so a paste never lands in a screen the user cannot see.
#[gpui::test]
fn a_paste_goes_nowhere_unless_the_terminal_is_live(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.cx.write_to_clipboard(ClipboardItem::new_string("copied".into()));
    assert!(h.focused(&owner), "the premise: focused");
    assert!(
        !h.read(|owner, _| owner.is_live()),
        "the premise: attaching"
    );
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [], "attaching");

    h.attach_showing("screen");
    assert!(h.focused(&owner), "the premise: still focused");
    h.fake().drop_link();
    settle(h.cx);
    assert!(
        !h.read(|owner, _| owner.is_live()),
        "the premise: reconnecting"
    );
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [], "reconnecting");

    h.attach_showing("again");
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [(paste("copied", true), true)], "live again");
}

/// Input made for one attachment never reaches a later one: a paste whose clipboard read finishes after a reconnect
/// is dropped rather than typed into the new attachment, which the user may not have seen yet. The stream's own stamp
/// check does the dropping; this pins that the terminal sends with the stamp it was given, not the one live now.
///
/// NOTE: The test calls the stamped send directly, because the test platform's clipboard answers at once and cannot
/// hold a read across a reconnect. So two things about a slow read are not exercised: that `paste` stamps the group
/// with the attachment live when the user asked, and that a second paste waits for the first. Both are a few lines
/// in `AttachedTerminal::paste`, read by review.
#[gpui::test]
fn input_made_for_an_earlier_attachment_is_dropped(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("first");
    let first = h.read(|owner, _| owner.attachment).unwrap();
    h.fake().drop_link();
    h.attach_showing("second");
    let second = h.read(|owner, _| owner.attachment).unwrap();
    assert_ne!(
        first, second,
        "the premise: the reconnect made a new attachment"
    );
    let owner = h.owner.clone();
    owner.update(h.cx, |owner, _| {
        owner.send(first, vec![paste("late", true)]);
        owner.send(second, vec![paste("now", true)]);
    });
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [(paste("late", true), false), (paste("now", true), true)],
        "the stream refuses the earlier attachment's input and carries the live one's"
    );
}

/// Only plain text is pasted, and a clipboard read that reports a file or an image pastes nothing at all, even with
/// text beside it: SPEC.md's "Attachments" ranks a paste's flavors files first, then images, then text, and a Finder
/// copy on macOS carries the file's path as text too, which would otherwise be typed into the program. Turning those
/// pastes into uploads is the attachment work's. (The vendored Wayland read never reports a file, so on Linux a copied
/// file's text still pastes; that is the read's limit, not this rule's.)
#[gpui::test]
fn only_the_clipboards_text_is_pasted(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    let file = ClipboardEntry::ExternalPaths(gpui::ExternalPaths(
        vec![std::path::PathBuf::from("/tmp/copied.png")].into(),
    ));
    h.cx.write_to_clipboard(ClipboardItem {
        entries: vec![file.clone()],
    });
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [], "a file alone pastes nothing");

    h.cx.write_to_clipboard(ClipboardItem {
        entries: vec![
            file,
            ClipboardEntry::String(gpui::ClipboardString::new("text".into())),
        ],
    });
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [], "nor a file with text beside it");

    h.cx.write_to_clipboard(ClipboardItem::new_string("text".into()));
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [(paste("text", true), true)],
        "the premise: text alone pastes"
    );
}

// ---- Text input and focus reports ----------------------------------------------------------------------------------

/// Run `f` on the terminal's input handler as GPUI runs it, with the window: how a test plays the platform's text
/// input (an input method's commit or composition, a Mac dead key's accent), which the test platform has no keystroke
/// for. It goes through the handler entity the terminal hands GPUI, so these tests cover that too.
fn text_input<R>(
    h: &mut Harness<'_>,
    f: impl FnOnce(&mut TerminalInputHandler, &mut Window, &mut Context<TerminalInputHandler>) -> R,
) -> R {
    let owner = h.owner.clone();
    let result = h.cx.update(|window, cx| {
        let handler = owner.read(cx).text_input.clone();
        handler.update(cx, |handler, cx| f(handler, window, cx))
    });
    settle(h.cx);
    result
}

/// A live state message: the program asks for focus reports (mode 1004) or stops asking.
fn focus_mode(on: bool) -> GridOutput {
    serde_json::from_value(
        serde_json::json!({"t":"state","mouse":"off","focus":on,"alt_screen":false,
        "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
    )
    .unwrap()
}

fn focus_input(focused: bool) -> GridInput {
    GridInput::Focus { focused }
}

/// Text an input method commits reaches the program once, as typed text. Before the terminal had an input handler,
/// GPUI had nowhere to put such text on Linux (IBus, XIM), so it was dropped; SPEC.md's IME decision keeps this to
/// what GPUI gives, which is exactly the committed string. Nothing goes out while the terminal is not live.
#[gpui::test]
fn text_an_input_method_commits_reaches_the_program(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    text_input(&mut h, |handler, window, cx| {
        handler.replace_text_in_range(None, "日本語", window, cx)
    });
    assert_eq!(h.sent(), [(text("日本語"), true)]);

    h.fake().drop_link();
    settle(h.cx);
    assert!(
        !h.read(|owner, _| owner.is_live()),
        "the premise: reconnecting"
    );
    text_input(&mut h, |handler, window, cx| {
        handler.replace_text_in_range(None, "後", window, cx)
    });
    assert_eq!(h.sent(), [], "nothing while not live");
}

/// While an input method composes, nothing reaches the program: the composition is reported as marked text and not
/// drawn, and only its commit goes out, as text. After it, Enter is the program's key again. This is also the Mac
/// dead key's path: the accent is marked text until the letter commits the composed character, where the terminal
/// used to send the spacing accent and then a plain letter. (On macOS, keys typed during a composition go to the input
/// method before any key listener; elsewhere they reach the listener, which leaves them to text input, as the key
/// routing's table test pins for a composing Enter.)
#[gpui::test]
fn a_composition_reaches_the_program_only_when_it_commits(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    text_input(&mut h, |handler, window, cx| {
        handler.replace_and_mark_text_in_range(None, "´", None, window, cx)
    });
    let marked = text_input(&mut h, |handler, window, cx| {
        handler.marked_text_range(window, cx)
    });
    assert_eq!(marked, Some(0..1), "the composition is marked");
    assert_eq!(h.sent(), [], "a composition sends nothing");
    text_input(&mut h, |handler, window, cx| {
        handler.replace_text_in_range(None, "é", window, cx)
    });
    let marked = text_input(&mut h, |handler, window, cx| {
        handler.marked_text_range(window, cx)
    });
    assert_eq!(marked, None, "the commit ends the composition");
    keys(h.cx, "enter");
    assert_eq!(h.sent(), [(text("é"), true), (key("Enter", 13), true)]);
}

/// A Linux dead key is composed by the platform: the dead key itself types nothing, and the composed key arrives with
/// the composed character as its text, which goes to the program through the input handler. That ends the dead-key
/// sequence: left pending, the mapping held back the next key that sends a sequence, so Enter after a composed `é`
/// sent nothing and only a second Enter went through.
#[gpui::test]
fn a_dead_key_composes_with_the_next_key(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    // Sent as a raw event: GPUI's simulated keystrokes (`keys`) give every key its own name as typed text, and a dead
    // key types nothing on a real platform. The composed key after it does type its character, as `keys` sends it.
    h.cx.simulate_event(KeyDownEvent {
        native_key: None,
        keystroke: Keystroke {
            modifiers: Modifiers::none(),
            key: "dead_acute".into(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });
    settle(h.cx);
    assert_eq!(h.sent(), [], "the dead key types nothing");
    keys(h.cx, "é enter");
    assert_eq!(h.sent(), [(text("é"), true), (key("Enter", 13), true)]);
}

/// A program that asked for focus reports (mode 1004) is told when its terminal gains and loses focus, including when
/// the window does (switching to another app, as a browser's blur did in the old app), and only on a change. A program
/// that did not ask hears nothing.
///
/// NOTE: The test platform draws every changed window as each step settles, so the window's activation reaches the
/// report through a redraw here. What matters on a Mac, where a hidden window draws nothing, is not what this
/// exercises: the terminal's activation observer (a check on the test Mac, posting real app switches and minimizing
/// the window, covers it), and the reports sent without a frame when a program starts asking or a reconnect goes live
/// (read by review only).
#[gpui::test]
fn a_program_asking_for_focus_reports_hears_focus_change(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    assert_eq!(h.sent(), [], "not asking yet");
    h.fake().output(focus_mode(true));
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [(focus_input(true), true)],
        "starting to ask hears the current state"
    );
    h.fake().output(focus_mode(true));
    settle(h.cx);
    assert_eq!(h.sent(), [], "asking again repeats nothing");

    h.cx.deactivate_window();
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [(focus_input(false), true)],
        "the window went inactive"
    );
    h.cx.update(|window, _| window.activate_window());
    settle(h.cx);
    assert_eq!(h.sent(), [(focus_input(true), true)], "and active again");

    let (second, _) = h.second.clone().unwrap();
    h.choose(&second);
    assert_eq!(
        h.sent(),
        [(focus_input(false), true)],
        "focus moved to another terminal"
    );
    h.choose(&owner);
    assert_eq!(h.sent(), [(focus_input(true), true)]);

    h.fake().output(focus_mode(false));
    settle(h.cx);
    h.choose(&second);
    assert_eq!(h.sent(), [], "a program that stopped asking hears nothing");
}

/// A program that starts asking for focus reports hears the current state at once, without waiting for a frame: on a
/// Mac a hidden window draws none, and a program that turned reports on while the user was in another app would
/// otherwise hear nothing until the window came back, and then only that it is focused. Specified: with the window
/// inactive, the report is recorded within the very update that delivered the program's request, before any draw (the
/// test platform draws only once an update's effects are flushed).
#[gpui::test]
fn a_program_that_starts_asking_hears_at_once_without_a_frame(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.cx.deactivate_window();
    settle(h.cx);
    assert_eq!(h.sent(), [], "the premise: not asking yet");
    let reported = h.cx.update(|_, cx| {
        owner.update(cx, |owner, cx| {
            owner.deliver(vec![TerminalEvent::Output(focus_mode(true))], cx);
            owner.reported_focus
        })
    });
    assert_eq!(reported, Some(false), "reported before any frame");
    settle(h.cx);
    assert_eq!(h.sent(), [(focus_input(false), true)], "and sent once");
}

/// Each attachment's program hears the current focus state once it is live and asking, as the old app's grid, built
/// afresh for every attach, reported it: the program behind a reconnect cannot know what the last attachment said.
/// The realistic case is a program that turned reports on long ago, so its replay already says it is asking, and the
/// reconnect, replay and going live all land before the terminal draws again: the state the terminal last reported
/// (focused) is the state it would report now, and only forgetting it per attachment sends the report.
#[gpui::test]
fn a_new_attachment_hears_the_focus_state_again(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.fake().output(focus_mode(true));
    settle(h.cx);
    assert_eq!(h.sent(), [(focus_input(true), true)]);

    // No settle until the new attachment is live: nothing draws in between. The premise is checked below: the whole
    // reconnect arrives in one delivery turn, so no frame could have cleared the last report on the way.
    let turns = h.read(|owner, _| owner.turns);
    h.fake().drop_link();
    let asked = h.fake().attach();
    for message in screen(asked.cols, asked.rows, "again", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().output(focus_mode(true));
    h.fake().replay_complete();
    settle(h.cx);
    assert!(
        h.read(|owner, _| owner.is_live()) && h.focused(&owner),
        "the premise: live again, still focused"
    );
    assert_eq!(
        h.read(|owner, _| owner.turns),
        turns + 1,
        "the premise: one delivery turn"
    );
    assert_eq!(
        h.sent(),
        [(focus_input(true), true)],
        "the new attachment hears it once"
    );
}

/// The cursor blinks on the ticker, half a second shown and half hidden (the old cursor's blink), only while the
/// terminal has focus in an active window: unfocused, or with the window inactive, it is drawn outlined and nothing
/// ticks, so an idle window does not redraw twice a second for a cursor nobody is typing at.
#[gpui::test]
fn the_cursor_blinks_only_while_focused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_blinking();
    let owner = h.owner.clone();
    h.reset_renders();
    h.ticks(10);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: false,
            on: true
        }
    );
    assert_eq!(
        h.renders("attached-terminal"),
        0,
        "unfocused, nothing ticks"
    );

    h.choose(&owner);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: true,
            on: true
        }
    );
    h.ticks(ticker::BLINK_HALF_TICKS as u32);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: true,
            on: false
        }
    );
    h.ticks(ticker::BLINK_HALF_TICKS as u32);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: true,
            on: true
        }
    );

    h.cx.deactivate_window();
    settle(h.cx);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: false,
            on: true
        }
    );
    h.reset_renders();
    h.ticks(10);
    assert_eq!(h.renders("attached-terminal"), 0, "inactive, nothing ticks");
}

/// Under reduced motion the cursor holds still, shown, in a focused terminal: nothing on screen moves on its own.
#[gpui::test]
fn reduced_motion_keeps_the_cursor_steady(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.cx.update(|_, cx| ticker::set_reduced_motion(true, cx));
    h.attach_blinking();
    let owner = h.owner.clone();
    h.choose(&owner);
    h.reset_renders();
    h.ticks(10);
    assert_eq!(
        h.phase(),
        CursorPhase {
            focused: true,
            on: true
        }
    );
    assert_eq!(h.renders("attached-terminal"), 0);
}

/// The keymap (`actions::TERMINAL_KEYS`, through [`owner_on`]) and the input mapping decide terminal keys in two
/// places, so they are compared on both platforms: every chord the keymap gives the program must be one the terminal
/// sends, and none it keeps for the app or the terminal itself may be, whether pressed or held (the input mapping is
/// told an app shortcut is one, as its route asks). Then the other way round: every chord the input mapping treats
/// as the terminal's own command (paging, select all) must be the terminal's in the keymap.
///
/// Some program keys have no legacy encoding, so the terminal sends nothing for them, and both sides are right:
/// GPUI Kit's inspector toggle (Cmd+Option+I, Ctrl+Shift+I) is the program's only so that Kit's debug-only binding
/// cannot take it from a focused terminal, and the input mapping, like the old app, has no sequence for either chord
/// and neither types text. They are listed, so that a program key that starts or stops sending fails here and whoever
/// changes it updates the list.
#[test]
fn the_keymap_and_the_input_mapping_agree_on_terminal_keys() {
    let no_legacy_encoding: [(Platform, &str); 2] = [
        (Platform::Mac, "cmd-alt-i"),
        (Platform::Linux, "ctrl-shift-i"),
    ];
    let event = |chord: &'static str, held: bool| KeyDownEvent {
        native_key: None,
        keystroke: keystroke(chord),
        is_held: held,
        prefer_character_input: false,
    };
    let mut silent = Vec::new();
    for platform in [Platform::Mac, Platform::Linux] {
        for row in TERMINAL_KEYS {
            let Some(chord) = row.chord(platform) else {
                continue;
            };
            assert_eq!(
                owner_on(platform, &event(chord, false)),
                row.owner,
                "{chord}"
            );
            let pressed = event(chord, false);
            let delivers = delivers(
                &program_input(&pressed, platform, &mut Composition::default()),
                &pressed,
            );
            match row.owner {
                KeyOwner::Program if !delivers => silent.push((platform, chord)),
                KeyOwner::Program => {}
                KeyOwner::Terminal => assert!(!delivers, "{platform:?} {chord}"),
                KeyOwner::App => {
                    for held in [false, true] {
                        let keystroke = keystroke(chord);
                        let decision = map_key(
                            KeyDown {
                                native: None,
                                modifiers: KeyModifiers {
                                    control: keystroke.modifiers.control,
                                    alt: keystroke.modifiers.alt,
                                    shift: keystroke.modifiers.shift,
                                    platform: keystroke.modifiers.platform,
                                    function: keystroke.modifiers.function,
                                },
                                key: &keystroke.key,
                                key_char: keystroke.key_char.as_deref(),
                                is_held: held,
                                prefer_character_input: false,
                            },
                            platform,
                            &mut Composition::default(),
                            Route {
                                terminal_focused: true,
                                modal_open: false,
                                app_shortcut: true,
                            },
                        );
                        assert!(
                            !matches!(decision, Decision::Send(_)),
                            "{platform:?} {chord} held={held}: {decision:?}"
                        );
                    }
                }
            }
        }
        for chord in [
            "shift-pageup",
            "shift-pagedown",
            "cmd-a",
            "ctrl-a",
            "pageup",
        ] {
            let decision = map_key(
                KeyDown {
                    native: None,
                    modifiers: {
                        let modifiers = keystroke(chord).modifiers;
                        KeyModifiers {
                            control: modifiers.control,
                            alt: modifiers.alt,
                            shift: modifiers.shift,
                            platform: modifiers.platform,
                            function: modifiers.function,
                        }
                    },
                    key: &keystroke(chord).key,
                    key_char: None,
                    is_held: false,
                    prefer_character_input: false,
                },
                platform,
                &mut Composition::default(),
                Route {
                    terminal_focused: true,
                    ..Route::default()
                },
            );
            if matches!(
                decision,
                Decision::PageUp | Decision::PageDown | Decision::SelectAll
            ) {
                assert_eq!(
                    owner_on(platform, &event(chord, false)),
                    KeyOwner::Terminal,
                    "{platform:?} {chord}: {decision:?}"
                );
            }
        }
    }
    assert_eq!(
        silent, no_legacy_encoding,
        "program chords the terminal sends nothing for"
    );
}

/// Whether a key on `route` reaches the program at all: a key sent as one, or a key left to text input that types
/// something (the platform commits its text through the input handler). A key left to text input that types nothing,
/// such as Ctrl+Shift+I, reaches nothing.
fn delivers(route: &KeyRoute, event: &KeyDownEvent) -> bool {
    match route {
        KeyRoute::Send(_) => true,
        KeyRoute::TextInput => event.keystroke.key_char.is_some(),
        KeyRoute::Nothing => false,
    }
}

/// The key rules as a table, so that each rule is pinned apart from the window: which keys go out as keys, which are
/// left to the platform's text input (whose committed text the input handler sends), and which reach nothing.
#[test]
fn program_input_routes_keys_to_the_program_or_text_input() {
    let event = |chord: &'static str| KeyDownEvent {
        native_key: None,
        keystroke: keystroke(chord),
        is_held: false,
        prefer_character_input: false,
    };
    let on = |chord: &'static str| {
        program_input(&event(chord), Platform::Linux, &mut Composition::default())
    };
    let mods = |mods: u8, kc: u32, key: &str| {
        KeyRoute::Send(GridInput::Key {
            key: key.into(),
            code: String::new(),
            kc,
            mods,
            mac: false,
        })
    };
    assert_eq!(on("a"), mods(0, 65, "a"));
    assert_eq!(on("ctrl-c"), mods(4, 67, "c"));
    assert_eq!(on("escape"), mods(0, 27, "Escape"));
    assert_eq!(
        on("shift-enter"),
        mods(1, 13, "Enter"),
        "one key, one write"
    );
    assert_eq!(on("shift-a"), KeyRoute::TextInput);
    assert_eq!(on("space"), KeyRoute::TextInput);
    assert_eq!(on("fn-a"), KeyRoute::Nothing, "a function-modified key");
    // A key with text the platform typed, unknown to the mapping's US inference, is text input's.
    let mut accented = event("é");
    accented.keystroke.key_char = Some("é".into());
    assert_eq!(
        program_input(&accented, Platform::Linux, &mut Composition::default()),
        KeyRoute::TextInput
    );
    // A Mac Option character that is not a dead key is text, as in the old app: Option-S types ß on a US layout.
    let mut option = event("alt-s");
    option.keystroke.key_char = Some("ß".into());
    assert_eq!(
        program_input(&option, Platform::Mac, &mut Composition::default()),
        KeyRoute::TextInput
    );
    // While an input method composes, even Enter is its, not the program's: it commits the composition.
    let mut composing = Composition {
        marked: true,
        dead_pending: false,
    };
    assert_eq!(
        program_input(&event("enter"), Platform::Linux, &mut composing),
        KeyRoute::TextInput
    );
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

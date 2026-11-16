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
    AppContext as _, Context, Entity, KeyBinding, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, PlatformInput, Render, ScrollDelta, ScrollEvent,
    Styled, TestAppContext, VisualTestContext, Window, div, point, px,
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

/// The height of the row the test pane draws the tunnel pill in, as the session's top row is at least 30 pixels high.
const PILL_ROW: f32 = 30.;

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
        // The open-tunnels pill, as the session's top row places it, in a row of its own above the terminal while the
        // session has tunnels open (never otherwise, so no other test's layout moves).
        let pill = self
            .owner
            .update(cx, |owner, cx| owner.tunnel_pill(window, cx))
            .map(|pill| {
                div()
                    .flex_none()
                    .h(px(PILL_ROW))
                    .flex()
                    .items_center()
                    .child(pill)
            });
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .on_action(move |_: &DeleteSession, _, _| deletes.set(deletes.get() + 1))
            .children(pill)
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
    harness_connected(cx, two, Vec::new())
}

/// [`harness`], with the session `s1` listed on a remote host: the pinned session fixture, whose host is the pinned
/// host list's SSH host 2. Its terminal reads its tunnels as it opens.
fn remote_harness(cx: &mut TestAppContext) -> Harness<'_> {
    let hosts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let entries: Vec<_> = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| farhelm_proto::ui_channel::KeyedEntry {
            key: farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()),
            value: host.clone(),
        })
        .collect();
    let keys: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let hosts = farhelm_proto::ui_channel::CollectionState {
        collection: farhelm_proto::ui_channel::Collection::Hosts,
        meta: test_support::meta(&keys),
        entries,
    };
    harness_connected(cx, false, vec![test_support::sessions(&["s1"]), hosts])
}

/// The harness, connected with an empty preferences row and `more` collections.
fn harness_connected(
    cx: &mut TestAppContext,
    two: bool,
    more: Vec<farhelm_proto::ui_channel::CollectionState>,
) -> Harness<'_> {
    test_support::install(cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    // An empty preferences row, as a fresh helm pushes it: a test changes a preference by pushing to it (the client
    // ignores changes to a collection it has no snapshot of).
    // `more` may bring its own preferences row; otherwise the row is empty.
    let mut snapshot = Vec::new();
    if !more
        .iter()
        .any(|state| state.collection == farhelm_proto::ui_channel::Collection::Preferences)
    {
        snapshot.push(farhelm_proto::ui_channel::CollectionState {
            collection: farhelm_proto::ui_channel::Collection::Preferences,
            meta: test_support::meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
            entries: vec![farhelm_proto::ui_channel::KeyedEntry {
                key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
                value: serde_json::json!({}),
            }],
        });
    }
    snapshot.extend(more);
    helm.connect(snapshot);
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

/// The pane owns the program's size (SPEC.md, "Terminal experience": "the PTY resizes to that client"), so a first
/// screen that arrives at another size than the pane fits (an attach-time resize the supervisor clamped or lost) must
/// be corrected, once, or the program keeps drawing for a grid the pane does not show. Specified: the attach asks for
/// the fitted size; when its replay says hello at two columns fewer, the owner sends one resize to the fitted size, and
/// a later redraw sends nothing more; after a dropped link, the reconnect's first screen at the wrong size gets one
/// resize again, and so does the first screen of a Take control after another window took the session over (the
/// likeliest wrong size: that window resized the program to its own pane).
#[gpui::test]
fn a_first_screen_at_another_size_is_resized_once(cx: &mut TestAppContext) {
    let h = harness(cx);
    let asked = h.fake().attach();
    assert_eq!(
        (asked.cols, asked.rows),
        FITTED,
        "the premise: the attach asks for the fit"
    );
    for message in screen(FITTED.0 - 2, FITTED.1, "narrow", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: FITTED.0,
            rows: FITTED.1
        }]
    );
    redraw(h.cx);
    assert_eq!(h.fake().received(), [], "once");

    // A reconnect is a new attach: its first screen at the wrong size is corrected too.
    h.fake().drop_link();
    settle(h.cx);
    h.fake().attach();
    for message in screen(FITTED.0 - 2, FITTED.1, "narrow", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert_eq!(
        h.fake().received(),
        [FakeTerminalRequest::Resize {
            cols: FITTED.0,
            rows: FITTED.1
        }],
        "and after a reconnect"
    );

    // Take control never passes through a reconnect, and the attach before it already sent its correction.
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    settle(h.cx);
    let owner = h.owner.clone();
    h.cx.update(|_, cx| owner.update(cx, |owner, cx| owner.take_control(cx)));
    settle(h.cx);
    h.fake().attach();
    for message in screen(FITTED.0 + 3, FITTED.1, "wide", serde_json::Value::Null) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert_eq!(
        h.fake().received(),
        [
            FakeTerminalRequest::TakeControl,
            FakeTerminalRequest::Resize {
                cols: FITTED.0,
                rows: FITTED.1
            }
        ],
        "and after Take control"
    );
}

/// What the bundled font measures at 13 points, 0.6 and 1.32 times the size (the terminal module's cell test pins
/// that). Its 7.8-pixel advance is no whole number of pixels, which is where a fit that depended on the size it fitted
/// before could round a column differently; the old grid-fit test used the same advance for that reason.
fn thirteen_points() -> CellMetrics {
    CellMetrics::new(7.8, 17.16, 13.).unwrap()
}

/// The pane, at the origin, that leaves the grid `room` pixels for its columns and `height` for its rows: the room
/// plus the scrollbar's column, and the padding on each side.
fn pane_for(room: f32, height: f32) -> Bounds<Pixels> {
    let width = room + SCROLLBAR_WIDTH + 2. * PANE_PADDING;
    let pane = gpui::size(px(width), px(height + 2. * PANE_PADDING));
    Bounds::new(point(px(0.), px(0.)), pane)
}

/// A new owner of `s1`'s agent terminal at `metrics`, for the harness's UI session, whose opener reports into the
/// returned slot. It is drawn nowhere: a test fits it by calling `fitted` as the pane does after each frame, which
/// lets it name a display scale the test window does not have.
fn undrawn_owner(h: &mut Harness, metrics: CellMetrics) -> (Entity<AttachedTerminal>, Opened) {
    let connection = h.connection.clone();
    let slot: Opened = Rc::default();
    let opened = slot.clone();
    let owner = h.cx.update(|_, cx| {
        let session = connection.read(cx).session().cloned().unwrap();
        let target = TerminalTarget {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
        };
        cx.new(|cx| AttachedTerminal::new(&session, target, metrics, opener(slot), cx))
    });
    (owner, opened)
}

/// A new owner of `s1`'s agent terminal at `metrics`, in a window of its own, in a pane of `size` from its very first
/// frame: what a session switch makes, a fresh terminal in the pane the last one was drawn in. Returns its opener's
/// slot, the pane's size and the window's context.
fn owner_window(
    h: &mut Harness,
    metrics: CellMetrics,
    size: (f32, f32),
) -> (Opened, Rc<Cell<(f32, f32)>>, VisualTestContext) {
    let connection = h.connection.clone();
    let slot: Opened = Rc::default();
    let opened = slot.clone();
    let (pane, mut window) = test_support::another_window(h.cx, |window, cx| {
        let session = connection.read(cx).session().cloned().unwrap();
        let target = TerminalTarget {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
        };
        let owner = cx.new(|cx| AttachedTerminal::new(&session, target, metrics, opener(slot), cx));
        let pane = Pane::new(owner, None, window, cx);
        pane.size.set(size);
        pane
    });
    let size = window.update(|_, cx| pane.read(cx).size.clone());
    (opened, size, window)
}

/// The resizes `opened`'s fake stream received since the last look, as sizes.
fn resizes(opened: &Opened) -> Vec<(u16, u16)> {
    let mut opened = opened.borrow_mut();
    let fake = &mut opened.as_mut().expect("the owner opened a stream").0;
    let received = fake.received().into_iter();
    received
        .filter_map(|request| match request {
            FakeTerminalRequest::Resize { cols, rows } => Some((cols, rows)),
            _ => None,
        })
        .collect()
}

/// The size `opened`'s stream was opened at.
fn opened_at(opened: &Opened) -> (u16, u16) {
    let asked = opened
        .borrow()
        .as_ref()
        .expect("the owner opened a stream")
        .1;
    (asked.cols, asked.rows)
}

/// The same pane must give the program the same size however the pane got there: the old app once fitted from its
/// current size, so with a fractional advance a pane that grew by two pixels kept 84 columns where a fresh mount of it
/// fitted 83, and the program's size depended on the window's history. GPUI's fit reads only the pane, the cell and
/// the scale, so this is a fence against reintroducing that.
///
/// Specified: at 13 points (a 7.8-pixel advance) and scale 1.5, an owner fitted to rooms of 800x500, 653x400 and
/// 655x402 in turn opens at 102x28 and sends one resize, to exactly the size a fresh owner opens at for 655x402, which
/// is 83 columns.
#[gpui::test]
fn a_resize_history_ends_at_the_size_a_fresh_owner_fits(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let history = [
        pane_for(800., 500.),
        pane_for(653., 400.),
        pane_for(655., 402.),
    ];
    let (resized, resized_opened) = undrawn_owner(&mut h, thirteen_points());
    for pane in history {
        h.cx.update(|_, cx| resized.update(cx, |owner, cx| owner.fitted(pane, 1.5, cx)));
    }
    let (fresh, fresh_opened) = undrawn_owner(&mut h, thirteen_points());
    h.cx.update(|_, cx| fresh.update(cx, |owner, cx| owner.fitted(history[2], 1.5, cx)));
    settle(h.cx);
    let fresh_size = opened_at(&fresh_opened);
    assert_eq!(fresh_size.0, 83, "the premise: 655 pixels hold 83 columns");
    assert_eq!(
        opened_at(&resized_opened),
        (102, 28),
        "the premise: the history starts elsewhere"
    );
    assert_eq!(resizes(&resized_opened), [fresh_size]);
}

/// The fit is the old app's documented formula at every text size and display scale, with the bundled font's real
/// measurements: a fit that drifted at one size or scale would give the program another size than the old app did
/// only for the users at that zoom or on that display. The formula is restated here from the old grid-fit test rather
/// than taken from the code, so a change to either shows up.
///
/// Specified: for every text size from 9 to 28 points at scales 1, 1.5 and 2, a room of 987.6 by 595.3 pixels (no
/// round multiple of any cell, and fractions the fit must drop) fits `floor((987 - 14) / w)` columns and
/// `floor(595 / h)` rows, clamped to 2x1 and 1000x1000, where the measured character's advance `a` and height `c` give
/// `w = round(a * s * 80 / s) / 80` and `h = round(ceil(c * s) * 24 / s) / 24` at scale `s`.
#[test]
fn the_fit_is_the_documented_formula_at_every_text_size_and_scale() {
    let mut cx = test_support::headless().unwrap();
    let (room, height) = (987.6_f32, 595.3_f32);
    for points in super::super::TextSize::MIN..=super::super::TextSize::MAX {
        let m = cx
            .update(|cx| CellMetrics::measure(super::super::TextSize::new(points), cx))
            .unwrap();
        let (a, c) = (f64::from(m.char_width), f64::from(m.char_height));
        for s in [1., 1.5, 2.] {
            let w = (a * s * 80. / s).round() / 80.;
            let h = ((c * s).ceil() * 24. / s).round() / 24.;
            let cols = ((987. - 14.) / w).floor().clamp(2., 1000.) as u16;
            let rows = (595. / h).floor().clamp(1., 1000.) as u16;
            assert_eq!(
                fit(m, room, height, s as f32),
                Some((cols, rows)),
                "{points} points at scale {s}"
            );
        }
    }
}

/// Switching sessions makes a new terminal in the pane the last one was drawn in, and it must ask for the size the
/// program already has: an attach at another size, or a resize right after one, makes the program redraw, which the
/// old app's switch test watched for as a quiet program's full-width text and rules moving on a switch back. Run at
/// 13 points like the old test; a fit that depended on the size fitted before is guarded by
/// `a_resize_history_ends_at_the_size_a_fresh_owner_fits`, not here.
///
/// Specified: a live owner taken through two pane sizes ends at another size than it opened at; after its window
/// closes, a new owner in a pane of the last size opens its stream at exactly the last size the first one sent, and
/// through its replay and a redraw sends no resize at all.
#[gpui::test]
fn a_fresh_owner_on_the_same_pane_asks_for_the_last_size_and_resizes_nothing(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    let (first, first_size, mut first_window) = owner_window(&mut h, thirteen_points(), PANE);
    let live = |opened: &Opened, window: &mut VisualTestContext| {
        let asked = opened.borrow_mut().as_mut().unwrap().0.attach();
        for message in screen(asked.cols, asked.rows, "x", serde_json::Value::Null) {
            opened.borrow_mut().as_mut().unwrap().0.output(message);
        }
        opened.borrow_mut().as_mut().unwrap().0.replay_complete();
        settle(window);
    };
    live(&first, &mut first_window);
    let history = [(PANE.0 - 117., PANE.1 - 63.), (PANE.0 - 41., PANE.1 - 22.)];
    for pane in history {
        first_size.set(pane);
        redraw(&mut first_window);
    }
    let last = *resizes(&first)
        .last()
        .expect("the premise: the history resized the program");
    assert_ne!(
        last,
        opened_at(&first),
        "the premise: the history ends at another size than it began at"
    );
    first_window.update(|window, _| window.remove_window());
    first_window.run_until_parked();

    let (fresh, _, mut fresh_window) = owner_window(&mut h, thirteen_points(), history[1]);
    assert_eq!(opened_at(&fresh), last);
    live(&fresh, &mut fresh_window);
    redraw(&mut fresh_window);
    assert_eq!(resizes(&fresh), []);
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
    assert!(
        h.read(|owner, _| owner.reconnecting()),
        "the premise: reconnecting"
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

/// The first part of a replay as the supervisor sends it, ahead of its history: hello at `cols` by `rows` with a
/// history cap of 1000, the state, `text` on the first row, and the frame.
fn first_screen(cols: u16, rows: u16, text: &str) -> Vec<GridOutput> {
    let mut replay = screen(cols, rows, text, serde_json::Value::Null);
    replay[0] = serde_json::from_value(
        serde_json::json!({"t":"hello","cols":cols,"rows":rows,"history_cap":1000}),
    )
    .unwrap();
    replay
}

/// A history message carrying `lines` lines ("h0" oldest) at `cols`.
fn history(lines: usize, cols: u16) -> GridOutput {
    let rows: Vec<_> = (0..lines)
        .map(|i| {
            let text = format!("h{i}");
            serde_json::json!({"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]})
        })
        .collect();
    serde_json::from_value(serde_json::json!({"t":"history","first":0,"rows":rows,"cols":cols}))
        .unwrap()
}

/// A restart replaces the session's terminal with a new one that starts with the old one's scrollback above a blank
/// screen (SPEC.md, "Lifecycle operations"), and the supervisor detaches the viewer with `other` to move it there. The
/// client must come back by itself: a restart is not another client taking over, so offering Take control would ask the
/// user to fix something nobody broke (old `terminal-restart.spec.ts`, "a restarted session's terminal still shows the
/// previous run's scrollback above the new one", whose client half this is). The old screen must give way to the new
/// run's in one redraw, never to half a replay. Specified: with a live, focused terminal showing "screen" above 40 history lines, a
/// detach with `other` offers no Take control and keeps "screen" up; the stream reattaches by the non-displacing route,
/// and while the new replay (a screen with "new run" on its first row and the 40 lines above it) arrives nothing is
/// redrawn and "screen" stays; on completion the owner redraws once to show "new run" with no line of the old screen,
/// following the bottom; the wheel then reaches the carried-over history, and a typed key is delivered to the new
/// attachment. What the pane says during the backoff is the reconnect card's, which its own tests pin; here it is only
/// checked to be no red band.
#[gpui::test]
fn a_restarts_detach_reattaches_by_itself_and_shows_the_new_run_in_one_redraw(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    assert_eq!(
        h.text().as_deref().map(first_line),
        Some("screen"),
        "the premise"
    );

    h.fake()
        .detach(Some(DetachCode::Other), "session restarted");
    settle(h.cx);
    assert_eq!(h.fake().phase(), FakePhase::Reconnecting);
    assert_eq!(h.status(), None, "no band, so no Take control");
    assert!(h.cx.debug_bounds("terminal-take-control").is_none());
    assert_eq!(h.text().as_deref().map(first_line), Some("screen"));

    let asked = h.fake().attach();
    assert!(!asked.take_control, "it reattaches without displacing");
    let mut replay = first_screen(asked.cols, asked.rows, "new run");
    replay.push(history(40, asked.cols));
    let mut replay = replay.into_iter();
    // The first message announces the attachment, which redraws the owner (the reconnect card stays up through the
    // catch-up); the rest is the replay proper.
    h.fake().output(replay.next().unwrap());
    settle(h.cx);
    h.reset_renders();
    for message in replay {
        h.fake().output(message);
    }
    settle(h.cx);
    assert_eq!(h.text().as_deref().map(first_line), Some("screen"));
    assert_eq!(
        (h.renders("attached-terminal"), h.renders("terminal")),
        (0, 0)
    );

    h.fake().replay_complete();
    settle(h.cx);
    assert_eq!(h.renders("attached-terminal"), 1, "one redraw");
    let text = h.text().unwrap();
    assert_eq!(first_line(&text), "new run");
    assert!(!text.contains("screen"), "the old screen is gone: {text:?}");
    assert!(h.following());

    h.wheel(1.);
    assert!(
        h.text().unwrap().lines().any(|line| line.starts_with('h')),
        "the carried-over history is reachable"
    );
    keys(h.cx, "a");
    assert_eq!(h.sent(), [(key("a", 65), true)]);
}

/// SPEC.md, "Terminal experience": attaching from a second client "visibly detaches the first, which keeps a non-live
/// snapshot". When that happens during the first attach's replay there is no earlier screen to keep, and the
/// supervisor sends no end of replay to a viewer it has detached, so a terminal that only ever showed complete replays
/// would sit blank under its Detached line. The old app showed what had arrived (old `grid-attach.spec.ts`, "a takeover
/// during a grid terminal's catch-up reveals what arrived, under the banner"). A replay that a reconnect cuts off is
/// still dropped, since the reconnect replays again; so is one with no complete screen yet, which has nothing to draw.
/// Specified: a takeover after only the hello and state shows no screen and says "Detached: " with Take control; after
/// Take control, a complete first screen ("$ echo" on its first row) cut off by a dropped link shows no screen either;
/// after the reconnect delivers that screen and part of its history, a takeover shows "$ echo" on the first row under
/// "Detached: " with Take control, not live.
#[gpui::test]
fn a_takeover_during_the_first_replay_keeps_what_arrived(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let asked = h.fake().attach();
    for message in first_screen(asked.cols, asked.rows, "$ echo")
        .into_iter()
        .take(2)
    {
        h.fake().output(message);
    }
    h.fake()
        .detach(Some(DetachCode::TakenOver), "another client attached");
    settle(h.cx);
    assert_eq!(h.text(), None, "no screen had arrived");
    assert!(h.status().is_some_and(|line| line.take_control));

    click(h.cx, "terminal-take-control");
    assert_eq!(
        h.fake().phase(),
        FakePhase::Attaching,
        "the premise: Take control attaches again"
    );
    let asked = h.fake().attach();
    assert!(asked.take_control, "the premise: as a takeover");
    for message in first_screen(asked.cols, asked.rows, "$ echo") {
        h.fake().output(message);
    }
    h.fake().drop_link();
    settle(h.cx);
    assert_eq!(h.text(), None, "a reconnect's cut replay is dropped");

    let asked = h.fake().attach();
    for message in first_screen(asked.cols, asked.rows, "$ echo") {
        h.fake().output(message);
    }
    h.fake().output(history(10, asked.cols));
    h.fake()
        .detach(Some(DetachCode::TakenOver), "another client attached");
    settle(h.cx);
    assert_eq!(h.text().as_deref().map(first_line), Some("$ echo"));
    let line = h.status().expect("the Detached line");
    assert_eq!(line.words, "Detached: ");
    assert!(line.take_control);
    assert!(!h.read(|owner, _| owner.is_live()));
    assert!(h.cx.debug_bounds("terminal-connecting").is_none());
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
/// dead until the session is reopened. A tab closed after it was attached says nothing at all: the session view drops
/// it moments later, and SPEC.md has a reaped tab disappear silently.
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
    assert_eq!(h.status(), None, "a closed tab goes silently");
    assert!(h.cx.debug_bounds("terminal-take-control").is_none());
}

/// A tab the helm cannot attach (the supervisor never minted it, or it went before the attach) is detached as a closed
/// tab before anything was shown. Unlike a tab that was attached and then reaped, it is not about to be dropped by the
/// session view, so it must say why its pane is blank (the old spec's "a tab the supervisor cannot attach explains
/// itself instead of showing a blank pane"), and offer nothing to take. Specified: a stream detached as a closed tab
/// before its first replay shows "Detached: " with the helm's reason and no Take control.
#[gpui::test]
fn a_tab_closed_before_it_was_ever_attached_explains_itself(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.fake().attach();
    h.fake()
        .detach(Some(DetachCode::TabClosed), "no such terminal tab");
    settle(h.cx);
    let line = h.status().expect("the blank pane says why");
    assert_eq!(line.words, "Detached: ");
    assert_eq!(
        line.reason.as_ref().map(HostText::raw),
        Some("no such terminal tab")
    );
    assert!(!line.take_control);
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
/// that state, so the state alone must redraw it. This is the ordinary end of an agent session, watched. The line
/// belongs to the latest state only: every state is sent whole, so one that carries no exit means none, and the line
/// goes and the screen drops back into the room it took (the old app's `exit: null` step).
///
/// Specified: a live state with an exit draws the exit line and lifts the fitted screen; a later state with
/// `exit: null` removes the line and the lift.
#[gpui::test]
fn a_live_exit_shows_its_line(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("running");
    assert!(h.cx.debug_bounds("terminal-exit").is_none());
    let state = |exit: serde_json::Value| -> GridOutput {
        serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
            "alt_screen":false,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":exit}))
        .unwrap()
    };
    h.fake().output(state(serde_json::json!({"signal": 15})));
    settle(h.cx);
    assert!(h.cx.debug_bounds("terminal-exit").is_some());
    assert!(
        h.read(|owner, cx| owner.shown_lift(cx)).unwrap() > 0.,
        "the premise: the exit line lifts the fitted screen"
    );

    h.fake().output(state(serde_json::Value::Null));
    settle(h.cx);
    assert!(
        h.cx.debug_bounds("terminal-exit").is_none(),
        "a state without an exit removes the line"
    );
    assert_eq!(h.read(|owner, cx| owner.shown_lift(cx)), Some(0.));
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

// ---- The reconnect card --------------------------------------------------------------------------------------------

impl Harness<'_> {
    /// Whether the last frame drew the reconnect card.
    fn card_drawn(&mut self) -> bool {
        self.cx.debug_bounds(RECONNECT_CARD_ID).is_some()
    }

    /// Whether the last frame drew the red status band.
    fn banner_drawn(&mut self) -> bool {
        self.cx.debug_bounds(STATUS_BANNER_ID).is_some()
    }

    /// Every line of text a fresh frame painted.
    fn painted(&mut self) -> Vec<String> {
        crate::session_list::tests::painted(&test_support::frame(self.cx))
    }

    /// How many Reconnect now requests the owner sent since the last call to the fake's `received`.
    fn reconnect_nows(&mut self) -> usize {
        self.fake()
            .received()
            .iter()
            .filter(|request| **request == FakeTerminalRequest::ReconnectNow)
            .count()
    }

    /// The hover help the layer manager shows once the pointer has rested on the element under `selector`.
    fn hover_help(&mut self, selector: &'static str) -> Option<String> {
        let at = test_support::center(self.cx, selector);
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        advance(self.cx, layers::HOVER_DELAY);
        self.cx
            .update(|window, cx| layers::shown_tooltip(window, cx))
            .map(|shown| shown.text.to_string())
    }
}

/// SPEC.md, "Session list": a dropped terminal stream "uses the same card with `Session unavailable — reconnecting` and
/// a neutral `Reconnect now` button; recoverable drops show no red detach banner". A drop the stream recovers from by
/// itself is nothing the user has to fix, and the red band reads as a failure waiting for them. The card must also leave
/// the screen and focus alone: the user may be reading the screen, and their typing belongs in the terminal once it is
/// back. Specified: with a live, focused terminal showing "screen", a dropped link draws the card with the heading, the
/// explanation and Reconnect now, whose hover help is the old tooltip; no red band is drawn and there is no status line;
/// "screen" is still the shown screen, the terminal keeps focus, and a typed key is not sent.
#[gpui::test]
fn a_dropped_stream_shows_the_reconnect_card_over_the_kept_screen(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    assert!(!h.card_drawn(), "the premise: live, no card");

    h.fake().drop_link();
    settle(h.cx);
    assert_eq!(
        h.fake().phase(),
        FakePhase::Reconnecting,
        "the premise: reconnecting"
    );
    assert!(h.card_drawn());
    let painted = h.painted();
    for words in [RECONNECTING_TEXT, RECONNECT_EXPLANATION, "Reconnect now"] {
        assert_eq!(
            painted.iter().filter(|text| *text == words).count(),
            1,
            "{words:?} once in {painted:?}"
        );
    }
    assert!(!h.banner_drawn(), "no red band");
    assert_eq!(h.status(), None);
    assert_eq!(h.text().as_deref().map(first_line), Some("screen"));
    assert!(h.focused(&owner), "the card took no focus");
    keys(h.cx, "x");
    assert_eq!(h.sent(), [], "nothing is sent while reconnecting");
    assert_eq!(
        h.hover_help(RECONNECT_NOW_ID).as_deref(),
        Some("Reconnect now: try to reattach this terminal right away")
    );
}

/// Reconnect now exists so a user who knows their network is back need not sit out a backoff they can see on screen
/// (old `terminal-reconnect.spec.ts`, "manual-reconnect-is-offered-during-the-first-rung"). It must ask the stream once
/// per press: a second request would be harmless to the stream, but a button that sends more than it was pressed hides
/// a bug in whatever drives it. Specified: with the link dropped, a click on Reconnect now sends exactly one
/// ReconnectNow and nothing else; after the reconnect's socket opens and its replay arrives, the stream is no longer
/// coming back from a drop once the replay completes, and the card is gone.
#[gpui::test]
fn reconnect_now_sends_one_request_per_press(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake().drop_link();
    settle(h.cx);
    assert_eq!(h.fake().received(), [], "the premise: nothing sent yet");

    click(h.cx, RECONNECT_NOW_ID);
    assert_eq!(h.fake().received(), [FakeTerminalRequest::ReconnectNow]);

    h.attach_showing("again");
    assert!(!h.card_drawn());
    assert!(!h.read(|owner, _| owner.reconnecting()));
}

/// SPEC.md, "Reaching everything by keyboard", and the component rule that every control is reachable the way buttons
/// are: Reconnect now is a tab stop like any button, while the card around it never takes focus by itself. Tab is the
/// program's from a focused terminal, so the button's place in Tab's order is right after the grid's area, where the
/// controls around the terminal reach it. Specified: with the link dropped and focus resting on nothing, the button is
/// not focused (the card took nothing); Shift+Tab from there, the last stop in the window, focuses it; Enter sends
/// exactly one ReconnectNow.
#[gpui::test]
fn reconnect_now_is_reached_and_pressed_from_the_keyboard(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake().drop_link();
    settle(h.cx);
    assert!(h.card_drawn(), "the premise: the card shows");
    assert_eq!(
        test_support::focused_id(h.cx),
        None,
        "focus rests on nothing: the card took none"
    );
    h.fake().received();

    keys(h.cx, "shift-tab");
    assert_eq!(
        test_support::focused_id(h.cx).as_deref(),
        Some(RECONNECT_NOW_ID)
    );
    keys(h.cx, "enter");
    assert_eq!(h.reconnect_nows(), 1);
}

/// The card sits over a screen that still answers the pointer: the scrim around it lets presses through, so the user
/// can still select and copy the kept screen while the stream comes back, but a press on the card's own words must not
/// start a selection (or, under mouse reporting, a report for the program) in the screen hidden behind them.
/// Specified: with the link dropped and nothing selected, a drag across the card selects nothing; a drag along the top
/// row, on the scrim beside the card, selects.
#[gpui::test]
fn a_drag_on_the_card_reaches_nothing_behind_it_and_the_scrim_lets_presses_through(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.attach_showing("screen line to select");
    h.fake().drop_link();
    settle(h.cx);
    assert!(h.card_drawn(), "the premise: the card shows");
    let on_card = test_support::center(h.cx, RECONNECT_CARD_ID);
    let card = h.cx.debug_bounds(RECONNECT_CARD_ID).unwrap();
    let (from, to) = (h.at_cell(0, 0), h.at_cell(6, 0));
    assert!(
        !card.contains(&from) && !card.contains(&to),
        "the premise: the top row's start lies beside the card"
    );

    h.drag(on_card, on_card + point(px(60.), px(0.)), Modifiers::none());
    assert_eq!(h.selection(), None, "a drag on the card selects nothing");
    h.drag(from, to, Modifiers::none());
    assert!(h.selection().is_some(), "a drag on the scrim selects");
}

/// An inert terminal lies under another status card (the session pane's unreachable-host card), which says what is
/// wrong and owns the keyboard meanwhile. A reconnect card drawn there would put a second, competing message under the
/// veil and a Reconnect now that Tab could still reach. An unreachable host very likely drops the stream as well, so
/// this is the common case where the two meet. Specified: with the terminal inert and its link dropped, the stream is
/// reconnecting (the premise), yet neither the card nor Reconnect now is drawn; once the terminal is no longer inert,
/// the card shows.
#[gpui::test]
fn an_inert_terminal_draws_no_reconnect_card(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.cx.update(|_, cx| owner.update(cx, |owner, cx| owner.set_inert(true, cx)));
    h.fake().drop_link();
    settle(h.cx);
    assert!(
        h.read(|owner, _| owner.reconnecting()),
        "the premise: reconnecting"
    );
    assert!(!h.card_drawn());
    assert!(h.cx.debug_bounds(RECONNECT_NOW_ID).is_none());

    h.cx.update(|_, cx| owner.update(cx, |owner, cx| owner.set_inert(false, cx)));
    settle(h.cx);
    assert!(
        h.card_drawn(),
        "the card shows once the terminal is usable again"
    );
}

/// The card says the stream is coming back, so it must stay until it is back: the kept screen takes no input through
/// the reconnect's catch-up either, and a card that went at the attach would leave a dead screen with nothing saying
/// so. It must then go in the redraw that shows the new screen, so the terminal never sits live under it. Specified:
/// after a dropped link, the reconnect's socket opening and the first part of its replay keep the card up over the old
/// screen; the end of the replay shows the new screen and takes the card away, with the terminal live and nothing
/// held over from the outage.
#[gpui::test]
fn the_card_stays_through_the_reconnects_catch_up_and_goes_when_it_is_live(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.attach_showing("first");
    h.fake().drop_link();
    settle(h.cx);
    assert!(h.card_drawn(), "the premise: the card shows");

    let asked = h.fake().attach();
    let replay = screen(asked.cols, asked.rows, "second", serde_json::Value::Null);
    h.fake().output(replay[0].clone());
    settle(h.cx);
    assert!(
        !h.read(|owner, _| owner.is_live()),
        "the premise: catching up"
    );
    assert!(h.card_drawn(), "still up through the catch-up");
    assert_eq!(h.text().as_deref().map(first_line), Some("first"));

    for message in replay.into_iter().skip(1) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert!(h.read(|owner, _| owner.is_live()));
    assert_eq!(h.text().as_deref().map(first_line), Some("second"));
    assert!(!h.card_drawn());
    assert!(!h.banner_drawn());
}

/// The card is for drops the stream recovers from by itself. A takeover, a refusal and a close stop the stream until
/// the user acts (or for good), and SPEC.md keeps "explicit banners" for such decisions, so each must still show the
/// red band and never the calm card, even when it ends an outage. Specified: a detach as taken over during an outage
/// shows the band with Take control and no card; Take control, then a refusal, shows the band ("Connection error") and
/// no card; closing the stream then shows "Connection closed" in the band and no card.
#[gpui::test]
fn detached_refused_and_closed_streams_keep_the_red_band(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake().drop_link();
    settle(h.cx);
    assert!(h.card_drawn(), "the premise: an outage");
    h.fake().attach();
    h.fake()
        .detach(Some(DetachCode::TakenOver), "another client attached");
    settle(h.cx);
    assert!(h.banner_drawn());
    assert!(h.status().is_some_and(|line| line.take_control));
    assert!(!h.card_drawn());

    click(h.cx, "terminal-take-control");
    h.fake().phase();
    h.fake().refuse(401);
    settle(h.cx);
    assert!(h.banner_drawn());
    assert_eq!(
        h.status().map(|line| line.words),
        Some("Connection error".into())
    );
    assert!(!h.card_drawn());

    h.read(|owner, _| owner.handle.clone().unwrap()).close();
    assert_eq!(h.fake().phase(), FakePhase::Closed, "the premise: closed");
    settle(h.cx);
    assert_eq!(
        h.status().map(|line| line.words),
        Some("Connection closed".into())
    );
    assert!(h.banner_drawn());
    assert!(!h.card_drawn());
}

/// A message the grid model refuses ends the stream for good, and the owner says so on the red band. One that arrives
/// during an outage's catch-up must not leave the reconnect card promising a reattach that will never come. Specified:
/// after a dropped link and the reconnect's first message, a second hello faults the stream: the band shows the
/// error, the card is gone, and the stream is closed.
#[gpui::test]
fn a_fault_during_a_reconnect_ends_the_card(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    h.fake().drop_link();
    let asked = h.fake().attach();
    h.fake()
        .output(screen(asked.cols, asked.rows, "x", serde_json::Value::Null)[0].clone());
    settle(h.cx);
    assert!(h.card_drawn(), "the premise: catching up after an outage");

    let second_hello: GridOutput = serde_json::from_value(
        serde_json::json!({"t":"hello","cols":asked.cols,"rows":asked.rows,"history_cap":0}),
    )
    .unwrap();
    h.fake().output(second_hello);
    settle(h.cx);
    assert!(h.banner_drawn());
    assert!(!h.card_drawn());
    assert_eq!(h.fake().phase(), FakePhase::Closed);
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

/// A program that tells Shift+Enter from Enter (an agent prompt taking one as a newline and the other as submit) must
/// see the Enter typed right after Shift+Enter as plain: a modifier carried from one key to the next would turn every
/// submit after a newline into another newline (the old app's terminal-keys check that the pty saw `1b 0d 0d`). The
/// input mapping keeps no state but a composition, so this holds by construction today; the test is the fence for
/// that.
///
/// Specified: in a live, focused terminal, Shift+Enter then Enter send two Enter key messages, the first with Shift
/// (mods 1) and the second with no modifiers, each in its own write.
#[gpui::test]
fn a_plain_enter_after_shift_enter_is_sent_without_shift(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    keys(h.cx, "shift-enter enter");
    let shifted = GridInput::Key {
        key: "Enter".into(),
        code: String::new(),
        kc: 13,
        mods: 1,
        mac: THIS_PLATFORM == Platform::Mac,
    };
    assert_eq!(h.sent(), [(shifted, true), (key("Enter", 13), true)]);
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

// ---- Scrollback ----------------------------------------------------------------------------------------------------

impl Harness<'_> {
    /// The fake's attach opens and replays a complete screen ("screen" on its first row) with `lines` history lines
    /// above it ("h0" oldest), then goes live.
    fn attach_with_history(&mut self, lines: usize) {
        let asked = self.fake().attach();
        let mut replay = screen(asked.cols, asked.rows, "screen", serde_json::Value::Null);
        replay[0] = serde_json::from_value(
            serde_json::json!({"t":"hello","cols":asked.cols,"rows":asked.rows,"history_cap":1000}),
        )
        .unwrap();
        let rows: Vec<_> = (0..lines)
            .map(|i| {
                let text = format!("h{i}");
                serde_json::json!({"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]})
            })
            .collect();
        replay.push(
            serde_json::from_value(
                serde_json::json!({"t":"history","first":0,"rows":rows,"cols":asked.cols}),
            )
            .unwrap(),
        );
        for message in replay {
            self.fake().output(message);
        }
        self.fake().replay_complete();
        settle(self.cx);
    }

    /// Whether the shown screen follows the bottom.
    fn following(&mut self) -> bool {
        self.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).is_following())
    }

    /// One wheel event over the middle of the grid: `lines` notches, positive toward older lines.
    fn wheel(&mut self, lines: f32) {
        self.cx.simulate_event(ScrollEvent {
            position: point(px(200.), px(200.)),
            delta: ScrollDelta::Lines(point(0., lines)),
            ..Default::default()
        });
        settle(self.cx);
    }
}

/// On the ordinary screen with no mouse reporting the wheel scrolls the terminal's own history and sends the program
/// nothing; the next input the user sends (a key, committed text or a paste) brings the view back to the bottom, where
/// it lands (the old app's wheel policy and `sendUserInput`).
#[gpui::test]
fn the_wheel_scrolls_history_and_input_returns_to_the_bottom(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    assert!(h.following(), "the premise: following");
    h.wheel(1.);
    assert!(!h.following(), "a notch up scrolls back");
    assert_eq!(h.sent(), [], "the program hears nothing of it");
    assert!(
        h.text().unwrap().lines().any(|line| line.starts_with('h')),
        "history shows"
    );
    keys(h.cx, "a");
    assert!(h.following(), "typing returns to the bottom");
    assert_eq!(h.sent(), [(key("a", 65), true)]);

    // Text the platform commits and a paste are input the user sends too.
    h.wheel(1.);
    assert!(!h.following(), "the premise: scrolled back");
    keys(h.cx, "shift-a");
    assert!(h.following(), "committed text returns to the bottom");
    assert_eq!(h.sent(), [(text("A"), true)]);
    h.wheel(1.);
    assert!(!h.following(), "the premise: scrolled back");
    h.cx.write_to_clipboard(ClipboardItem::new_string("pasted".into()));
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert!(h.following(), "a paste returns to the bottom");
    assert_eq!(h.sent(), [(paste("pasted", true), true)]);
}

/// Shift+PageUp and Shift+PageDown page through the history a page less a line at a time, the old app's step, so the
/// line at the edge the user paged from stays in view as a landmark; paging down onto the live screen follows it
/// again, also with more modifiers held (Ctrl+Shift+PageUp), as in the old app. They are the terminal's own keys: the
/// program hears nothing of any of them.
#[gpui::test]
fn the_page_keys_page_through_history_and_send_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(60);
    let owner = h.owner.clone();
    h.choose(&owner);
    let rows = h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).rows.len());
    assert!(
        rows > 1 && 2 * (rows - 1) < 60,
        "the premise: two pages fit in the history ({rows} rows)"
    );
    keys(h.cx, "shift-pageup");
    assert!(!h.following());
    assert_eq!(
        h.text().unwrap().lines().next(),
        Some(format!("h{}", 60 - (rows - 1)).as_str()),
        "a page less a line back"
    );
    keys(h.cx, "shift-pageup");
    assert_eq!(
        h.text().unwrap().lines().next(),
        Some(format!("h{}", 60 - 2 * (rows - 1)).as_str())
    );
    keys(h.cx, "shift-pagedown");
    assert!(!h.following());
    assert_eq!(
        h.text().unwrap().lines().next(),
        Some(format!("h{}", 60 - (rows - 1)).as_str()),
        "a page less a line forward"
    );
    keys(h.cx, "shift-pagedown");
    assert!(h.following(), "paging down onto the screen follows it");
    // With more modifiers held the keys page all the same, as in the old app; the keymap binds only the plain
    // chords, so these reach the terminal's own key handling.
    keys(h.cx, "ctrl-shift-pageup");
    assert_eq!(
        h.text().unwrap().lines().next(),
        Some(format!("h{}", 60 - (rows - 1)).as_str()),
        "Ctrl+Shift+PageUp pages too"
    );
    keys(h.cx, "ctrl-shift-pagedown");
    assert!(h.following());
    assert_eq!(h.sent(), [], "the program hears nothing of the page keys");
}

/// On the alternate screen the wheel is the program's (the supervisor turns it into arrow keys when the program has
/// not asked for mouse reports): one wheel message per event, up for a notch toward older lines, at the cell under the
/// pointer, and the view does not scroll (the alternate screen has no history).
#[gpui::test]
fn the_wheel_goes_to_the_program_on_the_alternate_screen(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    let alternate: GridOutput =
        serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
        "alt_screen":true,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}))
        .unwrap();
    h.fake().output(alternate);
    settle(h.cx);
    h.wheel(1.);
    h.wheel(-1.);
    let sent = h.sent();
    assert_eq!(sent.len(), 2, "one message per event: {sent:?}");
    let under = h
        .read(|owner, cx| owner.cell_at(point(px(200.), px(200.)), cx))
        .unwrap();
    for ((input, delivered), button) in sent.iter().zip([0, 1]) {
        assert!(delivered);
        assert!(
            matches!(input, GridInput::Mouse { kind: MouseEventKind::Wheel, button: b, x, y, .. }
                if *b == button && (*x, *y) == under),
            "{input:?}, pointer over {under:?}"
        );
    }
    assert!(h.following());
}

/// The cell a pointer report names is the one drawn under the pointer, also when the screen is lifted to make room
/// for its exit line: the lift moves every row up, so the top of the grid's area shows a lower row than row 0. A
/// report that ignored the lift would name a row the user did not point at. A point exactly on a boundary belongs to
/// the cell before it, as in the old app.
#[gpui::test]
fn the_pointer_names_the_cell_drawn_under_it_on_a_lifted_screen(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("running");
    let exited: GridOutput = serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
        "alt_screen":false,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":{"signal":15}}))
    .unwrap();
    h.fake().output(exited);
    settle(h.cx);
    let (origin, width, placement) = h.read(|owner, cx| {
        let view = owner.committed.as_ref().unwrap().read(cx);
        (
            owner.grid_origin.unwrap(),
            owner.cell(cx).unwrap().width,
            owner.placement(view, cx).unwrap(),
        )
    });
    let lifted = (placement.lift / placement.cell_height).round() as u16;
    assert!(lifted > 0, "the premise: the exit line lifts the screen");
    let at = |h: &mut Harness, x: f32, y: f32| {
        h.read(|owner, cx| owner.cell_at(origin + point(px(x), px(y)), cx))
    };
    assert_eq!(
        at(&mut h, width * 2.5, placement.cell_height * 0.5),
        Some((2, lifted))
    );
    assert_eq!(
        at(&mut h, width * 3., placement.cell_height),
        Some((2, lifted)),
        "a boundary belongs to the cell before it"
    );
}

// ---- The scrollbar ---------------------------------------------------------------------------------------------------

/// Whether the terminal's scrollbar is shown now (the owner's rule; the component draws it only with history).
fn scrollbar_shown(h: &mut Harness) -> bool {
    h.read(|owner, _| owner.scrollbar_shown())
}

/// The scrollbar shows while the pointer is over the terminal and for 800 milliseconds after the last scroll or after
/// the pointer leaves, as the old app's did; a later scroll restarts that time. Shown for good it would cover the
/// rightmost column's edge all the time; hidden at once it would vanish under a user still reading where it was.
#[gpui::test]
fn the_scrollbar_shows_on_hover_and_for_a_moment_after_a_scroll(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    // The premise: the pointer is nowhere near the terminal, and nothing has scrolled.
    h.cx.simulate_mouse_move(point(px(-10.), px(-10.)), None, Modifiers::none());
    advance(h.cx, Duration::from_secs(1));
    assert!(!scrollbar_shown(&mut h), "the premise: hidden");

    // Scrolled by the keyboard, so the pointer stays away (a wheel event puts it over the terminal).
    keys(h.cx, "shift-pageup");
    assert!(scrollbar_shown(&mut h), "a scroll shows it");
    advance(h.cx, Duration::from_millis(500));
    keys(h.cx, "shift-pageup");
    advance(h.cx, Duration::from_millis(500));
    assert!(scrollbar_shown(&mut h), "a later scroll restarts the time");
    advance(h.cx, Duration::from_millis(400));
    assert!(
        !scrollbar_shown(&mut h),
        "800 ms after the last scroll it hides"
    );

    // Over the grid, where the wheel helper points too.
    h.cx.simulate_mouse_move(point(px(200.), px(200.)), None, Modifiers::none());
    advance(h.cx, Duration::from_secs(2));
    assert!(
        scrollbar_shown(&mut h),
        "shown while the pointer is over the terminal"
    );
    // Typing does not count as the pointer leaving: the old app kept the bar while the pointer stayed.
    keys(h.cx, "a");
    advance(h.cx, Duration::from_secs(1));
    assert!(
        scrollbar_shown(&mut h),
        "typing with the pointer over the terminal keeps it"
    );
    h.cx.simulate_mouse_move(point(px(-10.), px(-10.)), None, Modifiers::none());
    assert!(
        scrollbar_shown(&mut h),
        "still shown just after the pointer leaves"
    );
    advance(h.cx, Duration::from_millis(900));
    assert!(!scrollbar_shown(&mut h));
}

/// A press at the top of the terminal's scrollbar takes the view to the oldest line, a drag to its bottom brings it
/// back to the live screen, and the program hears nothing of either: the bar is the terminal's own, as in the old
/// app. (The press that a program with mouse reporting must not get is pinned once mouse reporting is built.)
#[gpui::test]
fn the_terminals_scrollbar_moves_the_view_and_sends_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    let bar = test_support::bounds(h.cx, "terminal-scrollbar");
    let top = point(bar.center().x, bar.top() + px(2.));
    h.cx.simulate_mouse_down(top, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        h.text().unwrap().starts_with("h0\n"),
        "a press at the top shows the oldest line"
    );
    let bottom = point(bar.center().x, bar.bottom() + px(50.));
    h.cx.simulate_mouse_move(bottom, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(bottom, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        h.following(),
        "dragged past the bottom, the view follows the screen"
    );
    assert_eq!(h.sent(), [], "the program hears nothing of the bar");
}

// ---- Selection -----------------------------------------------------------------------------------------------------

impl Harness<'_> {
    /// The window point a little inside the left edge of screen cell (`col`, `row`), vertically centred: a press there
    /// lands on the boundary before the cell, a drag's end there selects up to that boundary.
    fn at_cell(&mut self, col: u16, row: u16) -> Point<Pixels> {
        self.read(|owner, cx| {
            let view = owner.committed.as_ref().unwrap().read(cx);
            let placement = owner.placement(view, cx).unwrap();
            let width = owner.cell(cx).unwrap().width;
            owner.grid_origin.unwrap()
                + point(
                    px(f32::from(col) * width + 1.),
                    px((f32::from(row) + 0.5) * placement.cell_height - placement.lift),
                )
        })
    }

    /// Drag with the primary button from `from` to `to`, with `modifiers` held, and release there.
    fn drag(&mut self, from: Point<Pixels>, to: Point<Pixels>, modifiers: Modifiers) {
        self.cx
            .simulate_mouse_down(from, MouseButton::Left, modifiers);
        settle(self.cx);
        self.cx
            .simulate_mouse_move(to, MouseButton::Left, modifiers);
        settle(self.cx);
        self.cx.simulate_mouse_up(to, MouseButton::Left, modifiers);
        settle(self.cx);
    }

    /// The clipboard's text, if it holds text.
    fn clipboard(&mut self) -> Option<String> {
        self.cx.read_from_clipboard().and_then(|item| item.text())
    }

    /// Put `text` on the clipboard, as another program would.
    fn set_clipboard(&mut self, text: &str) {
        self.cx
            .write_to_clipboard(ClipboardItem::new_string(text.into()));
    }

    /// The shown screen's local selection, if any.
    fn selection(&mut self) -> Option<Selection> {
        self.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection())
    }
}

/// A state message turning the program's mouse reporting `mode` on (or `"off"`).
fn mouse_mode(mode: &str) -> GridOutput {
    serde_json::from_value(
        serde_json::json!({"t":"state","mouse":mode,"focus":false,"alt_screen":false,
        "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
    )
    .unwrap()
}

/// This platform's modifier that forces a local selection over mouse reporting: Option on a Mac, Shift elsewhere
/// (SPEC.md's selection paragraph).
fn forcing() -> Modifiers {
    match THIS_PLATFORM {
        Platform::Mac => Modifiers::alt(),
        Platform::Linux => Modifiers::shift(),
    }
}

/// With no mouse reporting a plain drag selects and copies, and every completed selection copies again, even the same
/// text, because something else may have replaced the clipboard meanwhile (SPEC.md's selection paragraph). A plain
/// click selects nothing and leaves the clipboard to whoever filled it. Input sent to the program clears the
/// selection. None of it reaches the program.
#[gpui::test]
fn a_drag_selects_and_copies_and_a_click_does_not(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.set_clipboard("someone else's");
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::none());
    assert_eq!(h.clipboard().as_deref(), Some("hello"));

    h.set_clipboard("someone else's");
    h.drag(from, to, Modifiers::none());
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello"),
        "the same selection copies again"
    );

    h.set_clipboard("someone else's");
    let middle = h.at_cell(2, 0);
    h.cx.simulate_click(middle, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "a click copies nothing"
    );
    assert_eq!(h.sent(), [], "the program hears nothing of selecting");

    h.drag(from, to, Modifiers::none());
    assert!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection())
            .is_some()
    );
    keys(h.cx, "a");
    assert_eq!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection()),
        None,
        "typing clears the selection"
    );
}

/// A press, its moves and its release can all arrive before the app draws another frame (a fast drag; the test bridge
/// sends a whole drag at once). The drag must still follow the pointer and copy at the release: listeners that only
/// came into being in a frame drawn after the press missed all of it, and selected nothing.
#[gpui::test]
fn a_drag_delivered_between_frames_still_selects(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.cx.update(|window, cx| {
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: from,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: to,
                modifiers: Modifiers::none(),
                pressed_button: Some(MouseButton::Left),
            }),
            cx,
        );
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position: to,
                modifiers: Modifiers::none(),
                button: MouseButton::Left,
                click_count: 1,
            }),
            cx,
        );
    });
    settle(h.cx);
    assert_eq!(h.clipboard().as_deref(), Some("hello"));
}

/// A drag ends when what it selects stops existing under it: the alternate screen switching mid-drag (a full-screen
/// program starting) clears the selection, and the drag must not rebuild one from its old anchor, which named the
/// primary screen's text, nor copy anything at the release. A reconnect that replaces the view ends the drag too.
#[gpui::test]
fn a_drag_ends_when_its_selection_is_reset_or_its_view_replaced(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.set_clipboard("someone else's");
    let selection = |h: &mut Harness| {
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection())
    };
    let (from, to, further) = (h.at_cell(0, 0), h.at_cell(5, 0), h.at_cell(8, 0));
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        selection(&mut h).is_some(),
        "the premise: the drag selected"
    );
    let alternate: GridOutput =
        serde_json::from_value(serde_json::json!({"t":"state","mouse":"off","focus":false,
        "alt_screen":true,"default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}))
        .unwrap();
    h.fake().output(alternate);
    settle(h.cx);
    assert!(
        selection(&mut h).is_none(),
        "the premise: the switch cleared it"
    );
    h.cx.simulate_mouse_move(further, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(further, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        selection(&mut h),
        None,
        "no selection rebuilt from the old anchor"
    );
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "and nothing copied"
    );

    // A reconnect mid-drag: the old drag does not reach the new view. Everything on the new view is selected first
    // (through the view, not the Select all action, which ends a drag itself), so a drag still alive would visibly
    // replace that selection at its next move.
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        selection(&mut h).is_some(),
        "the premise: a second drag is under way"
    );
    h.fake().drop_link();
    let asked = h.fake().attach();
    for message in screen(
        asked.cols,
        asked.rows,
        "again and again",
        serde_json::Value::Null,
    ) {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    assert!(
        h.read(|owner, _| owner.is_live()),
        "the premise: live on the new attachment"
    );
    assert_eq!(
        selection(&mut h),
        None,
        "the premise: the new view starts with nothing selected"
    );
    let owner = h.owner.clone();
    h.cx.update(|_, cx| {
        let view = owner.read(cx).committed.clone().unwrap();
        view.update(cx, |view, cx| view.select_all(cx));
    });
    let all = selection(&mut h);
    assert!(
        all.is_some(),
        "the premise: everything on the new view is selected"
    );
    h.cx.simulate_mouse_move(further, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(further, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        selection(&mut h),
        all,
        "the old drag did not reach the new view"
    );
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "and copied nothing"
    );
}

/// An arrow key as the cursor move sends it: an ordinary key message the supervisor encodes in the program's
/// cursor-key mode.
fn arrow(name: &str, kc: u32) -> GridInput {
    GridInput::Key {
        key: name.into(),
        code: name.into(),
        kc,
        mods: 0,
        mac: THIS_PLATFORM == Platform::Mac,
    }
}

/// A quick Alt-click (Option on a Mac) moves the program's cursor to the clicked cell by typing arrow keys, as the old
/// app did, so a shell's line editor puts the cursor where the user clicked: to the release's cell, with Alt read at the
/// release, and still when the press selected one character. It must not fire for a slow Alt-press or an Alt-drag that
/// selected more than a character (a view scrolled back is the next test).
#[gpui::test]
fn a_quick_alt_click_moves_the_programs_cursor(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let cursor = h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).cursor);
    assert_eq!(
        cursor.map(|cursor| (cursor.x, cursor.y)),
        Some((0, 0)),
        "the premise: the cursor is at the first cell"
    );
    let right = |n| vec![(arrow("ArrowRight", 39), true); n];
    let at = h.at_cell(3, 0);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::alt());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    assert_eq!(h.sent(), right(3), "three cells right");

    // The cell is the release's, and Alt counts at the release: a press without it, a nudge of one cell (selecting
    // one character, which still counts as a click) and a release with Alt moves to where the button came up.
    let (two, three) = (h.at_cell(2, 0), h.at_cell(3, 0));
    h.cx.simulate_mouse_down(two, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(three, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(three, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("l"),
        "the premise: one character selected"
    );
    assert_eq!(h.sent(), right(3));

    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    advance(h.cx, Duration::from_millis(600));
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    assert_eq!(h.sent(), [], "a slow Alt-press is not a click");

    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::alt());
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello"),
        "the premise: the drag selected text"
    );
    assert_eq!(h.sent(), [], "an Alt-drag that selected text moves nothing");
}

/// Scrolled back, the clicked cell is a history line, not the program's screen, so an Alt-click there moves nothing.
#[gpui::test]
fn an_alt_click_in_the_history_moves_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.wheel(1.);
    assert!(!h.following(), "the premise: scrolled back");
    let at = h.at_cell(2, 0);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::alt());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    assert_eq!(h.sent(), []);
}

/// A double click selects the word and a triple click the line, and the release copies them, as in the old app.
#[gpui::test]
fn a_double_click_copies_the_word_and_a_triple_click_the_line(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let at = h.at_cell(8, 0);
    for (count, expected) in [(2, "world"), (3, "hello world")] {
        h.cx.simulate_event(MouseDownEvent {
            position: at,
            modifiers: Modifiers::none(),
            button: MouseButton::Left,
            click_count: count,
            first_mouse: false,
        });
        h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        settle(h.cx);
        assert_eq!(h.clipboard().as_deref(), Some(expected), "{count} clicks");
    }
}

/// Shift-click extends the selection to where it lands, from the selection's start, and copies the result; a word drag
/// (a double click held and dragged) grows by whole words and keeps the word it started on; a drag whose release the
/// window never saw (the button let go outside it) ends at the next move with the button up, and copies then. These
/// are the old app's gestures; each, lost, would leave the user selecting something other than what they pointed at.
#[gpui::test]
fn shift_click_extends_a_word_drag_grows_and_a_lost_release_ends(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world again");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (start, three) = (h.at_cell(0, 0), h.at_cell(3, 0));
    h.drag(start, three, Modifiers::none());
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hel"),
        "the premise: a selection to extend"
    );
    let eight = h.at_cell(8, 0);
    h.cx.simulate_mouse_down(eight, MouseButton::Left, Modifiers::shift());
    h.cx.simulate_mouse_up(eight, MouseButton::Left, Modifiers::shift());
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello wo"),
        "Shift-click extends from the start"
    );

    let (in_hello, in_world) = (h.at_cell(1, 0), h.at_cell(8, 0));
    h.cx.simulate_event(MouseDownEvent {
        position: in_hello,
        modifiers: Modifiers::none(),
        button: MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    h.cx.simulate_mouse_move(in_world, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(in_world, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello world"),
        "whole words, from the first"
    );

    h.set_clipboard("someone else's");
    h.cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(three, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    let away = h.at_cell(10, 0);
    h.cx.simulate_mouse_move(away, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hel"),
        "a move with the button up ends the drag and copies"
    );
    let end = h.at_cell(15, 0);
    h.cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selected_text()),
        Some("hel".into()),
        "a later move with the button held drags nothing"
    );
}

/// Off a Mac, Alt-drag selects a rectangle: the same columns on every row, each row copied apart (the old app's column
/// selection). On a Mac Option is the forcing modifier instead, so the same drag there selects ordinary text.
#[gpui::test]
fn alt_drag_selects_a_rectangle_off_a_mac(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.wheel(10.);
    let text = h.text().unwrap();
    let rows: Vec<&str> = text.lines().take(3).collect();
    assert!(
        rows.iter()
            .all(|row| row.len() >= 3 && row.starts_with('h')),
        "the premise: three history rows on top: {rows:?}"
    );
    let (from, to) = (h.at_cell(1, 0), h.at_cell(2, 2));
    h.drag(from, to, Modifiers::alt());
    let expected = match THIS_PLATFORM {
        Platform::Linux => rows
            .iter()
            .map(|row| &row[1..2])
            .collect::<Vec<_>>()
            .join("\n"),
        // On a Mac it is an ordinary selection from the press's boundary to the release's.
        Platform::Mac => format!("{}\n{}\n{}", &rows[0][1..], rows[1], &rows[2][..2]),
    };
    assert_eq!(h.clipboard(), Some(expected));
}

/// Under mouse reporting a plain drag is the program's and selects nothing here, and neither does a drag with the
/// other platform's modifier; the forcing modifier (Option on a Mac, Shift elsewhere) wins a local selection back,
/// and it copies (SPEC.md's selection paragraph).
#[gpui::test]
fn under_mouse_reporting_only_the_forcing_modifier_selects(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    h.fake().output(mouse_mode("drag"));
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.set_clipboard("someone else's");
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::none());
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "a plain drag is the program's"
    );
    assert_eq!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection()),
        None,
        "nor does it select"
    );
    let (forcing, other) = match THIS_PLATFORM {
        Platform::Mac => (Modifiers::alt(), Modifiers::shift()),
        Platform::Linux => (Modifiers::shift(), Modifiers::alt()),
    };
    h.drag(from, to, other);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "the other platform's modifier forces nothing here"
    );
    h.drag(from, to, forcing);
    assert_eq!(h.clipboard().as_deref(), Some("hello"));
}

/// A client displaced by a takeover keeps a non-live snapshot (SPEC.md, "Terminal experience"), and a snapshot the
/// user cannot copy from is only a picture: reading back what the agent printed before another client took over is
/// the point of keeping it. Nothing may reach the program from it, though, since the session now belongs to the client
/// that took it. The old app's "a frozen grid sends nothing but still selects and copies", and the selection half of
/// grid-attach's "a takeover keeps it as a usable snapshot".
///
/// Specified: on a screen whose program asked for click reporting, after a detach as taken over (the status line
/// offering Take control), a key and a plain click send nothing, a drag with the forcing modifier selects and copies
/// the text it covers, and the snapshot and its Take control stay.
#[gpui::test]
fn a_taken_over_snapshot_still_selects_and_copies_and_sends_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    // The text sits on row 10, below the rows the status line's band lifts out of view once detached.
    let asked = h.fake().attach();
    let mut replay = screen(asked.cols, asked.rows, "x", serde_json::Value::Null);
    replay[2] = serde_json::from_value(serde_json::json!({"t":"rows",
        "rows":[[10,{"w":false,"r":[[0,11,"hello world",[0,-1,-1]]]}]]}))
    .unwrap();
    replay.push(mouse_mode("click"));
    for message in replay {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.fake().detach(Some(DetachCode::TakenOver), "taken over");
    settle(h.cx);
    assert!(!h.read(|owner, _| owner.is_live()), "the premise: detached");
    assert!(
        h.status().is_some_and(|line| line.take_control),
        "the premise: the snapshot offers Take control"
    );
    assert!(h.focused(&owner), "the premise: the terminal kept focus");

    keys(h.cx, "a");
    let (from, to) = (h.at_cell(0, 10), h.at_cell(5, 10));
    h.cx.simulate_click(from, Modifiers::none());
    settle(h.cx);
    h.set_clipboard("someone else's");
    h.drag(from, to, forcing());
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello"),
        "the forcing modifier selects and copies"
    );
    assert_eq!(h.sent(), [], "neither the key nor the click reached anyone");
    assert!(h.status().is_some_and(|line| line.take_control));
    assert!(h.text().unwrap().contains("hello world"));
}

/// A drag held above the screen scrolls back through the history while it stays there, extending the selection with
/// it, at the old app's speed: 50 pixels or more past the edge moves 15 lines a step, a step every 50 milliseconds
/// (20 a second). Releasing stops it, and what was selected, history included, is copied.
#[gpui::test]
fn a_drag_held_past_the_top_scrolls_and_selects_history(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(60);
    let owner = h.owner.clone();
    h.choose(&owner);
    let top_line = |h: &mut Harness| {
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).line_at_row(0))
            .unwrap()
    };
    let start = top_line(&mut h);
    let from = h.at_cell(3, 0);
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    let above = from - point(px(0.), px(100.));
    h.cx.simulate_mouse_move(above, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        top_line(&mut h),
        start,
        "the premise: nothing has scrolled before the first step"
    );
    advance(h.cx, Duration::from_millis(50));
    assert_eq!(top_line(&mut h), start - 15, "one step of 15 lines");
    advance(h.cx, Duration::from_millis(50));
    assert_eq!(top_line(&mut h), start - 30, "and another while held");
    h.cx.simulate_mouse_move(from, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    advance(h.cx, Duration::from_millis(200));
    assert_eq!(
        top_line(&mut h),
        start - 30,
        "back over the screen, the scrolling stops"
    );
    h.cx.simulate_mouse_move(above, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    advance(h.cx, Duration::from_millis(50));
    assert_eq!(
        top_line(&mut h),
        start - 45,
        "and starts again past the edge"
    );
    h.cx.simulate_mouse_up(above, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    advance(h.cx, Duration::from_millis(200));
    assert_eq!(top_line(&mut h), start - 45, "the release stops it");
    let copied = h.clipboard().unwrap();
    // From the top row's column 3 (the end of a history line `hNN`) down to where the press was.
    assert!(
        copied.contains("\nh31\nh32\n"),
        "history is selected: {copied:?}"
    );
}

/// Select all and Copy, the terminal's own commands (Cmd+A and Cmd+C on a Mac), select everything the terminal can
/// show and copy it; Copy with nothing selected leaves the clipboard alone. Selecting all does not copy by itself.
#[gpui::test]
fn select_all_and_copy_copy_the_whole_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(5);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.set_clipboard("someone else's");
    h.cx.dispatch_action(actions::CopyTerminalSelection);
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "nothing selected, nothing copied"
    );
    h.cx.dispatch_action(actions::SelectAllInTerminal);
    settle(h.cx);
    assert_eq!(
        h.clipboard().as_deref(),
        Some("someone else's"),
        "selecting is not copying"
    );
    h.cx.dispatch_action(actions::CopyTerminalSelection);
    settle(h.cx);
    let copied = h.clipboard().unwrap();
    assert!(
        copied.starts_with("h0\nh1\nh2\nh3\nh4\nscreen"),
        "{copied:?}"
    );
}

// ---- Links ---------------------------------------------------------------------------------------------------------

/// A screen whose first row prints a web address at columns 3 to 23 and another from column 29.
const LINKED: &str = "go https://example.com/x and https://example.com/y";
const LINKED_URL: &str = "https://example.com/x";
const OTHER_URL: &str = "https://example.com/y";

impl Harness<'_> {
    /// Set the Terminal settings' link choice (`intercept`, `both` or `program`) as the helm pushes it.
    fn link_click(&mut self, mode: &str) {
        self.helm
            .push(farhelm_proto::ui_channel::CollectionChanges {
                collection: farhelm_proto::ui_channel::Collection::Preferences,
                meta: test_support::meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
                upsert: vec![farhelm_proto::ui_channel::KeyedEntry {
                    key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
                    value: serde_json::json!({ "terminal_link_click": mode }),
                }],
                remove: Vec::new(),
            });
        settle(self.cx);
    }

    /// The link the view says the pointer rests on, by its address.
    fn hovered(&mut self) -> Option<String> {
        self.read(|owner, cx| {
            owner
                .committed
                .as_ref()
                .unwrap()
                .read(cx)
                .hovered_link()
                .map(|link| link.uri.clone())
        })
    }

    /// A click at `at`, the pointer having come to rest there first, as a real pointer does.
    fn click_at(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        settle(self.cx);
        self.cx
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        self.cx
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        settle(self.cx);
    }
}

/// A web address printed in the terminal is a link: resting the pointer on it hovers it (the view underlines it and
/// the pointing hand shows) and moving off leaves it; a click opens it in the browser with no prompt (SPEC.md's links
/// paragraph). Small jitter between press and release still counts as a click; a drag never opens anything.
#[gpui::test]
fn a_printed_web_address_hovers_and_opens_on_a_click(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing(LINKED);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (on, off) = (h.at_cell(5, 0), h.at_cell(0, 0));
    h.cx.simulate_mouse_move(on, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.hovered().as_deref(), Some(LINKED_URL));
    h.cx.simulate_mouse_move(off, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.hovered(), None, "moving off leaves it");

    h.cx.simulate_mouse_move(on, None, Modifiers::none());
    settle(h.cx);
    let far = on + point(px(30.), px(0.));
    h.drag(on, far, Modifiers::none());
    assert_eq!(h.cx.opened_url(), None, "a drag opens nothing");
    // A drag that wanders off and comes back is still a drag, not a click (the old app dropped the click candidate
    // once the pointer left the jitter).
    h.cx.simulate_mouse_move(on, None, Modifiers::none());
    settle(h.cx);
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(far, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.cx.opened_url(),
        None,
        "a drag back to the press opens nothing"
    );

    let nudged = on + point(px(3.), px(0.));
    h.cx.simulate_mouse_move(on, None, Modifiers::none());
    settle(h.cx);
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(nudged, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(nudged, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some(LINKED_URL),
        "jitter is still a click"
    );
    assert_eq!(h.sent(), [], "the program hears nothing of it");
}

/// Hover lookups pause while a button is held, so the release looks again: a drag from a link onto plain text, let go
/// there with no further move, leaves no link underlined, and a press on plain text released over a link hovers it.
#[gpui::test]
fn the_release_looks_up_the_hover_again(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing(LINKED);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (on, plain) = (h.at_cell(5, 0), h.at_cell(1, 0));
    h.cx.simulate_mouse_move(on, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.hovered().as_deref(),
        Some(LINKED_URL),
        "the premise: the link is hovered"
    );
    h.drag(on, plain, Modifiers::none());
    assert_eq!(h.hovered(), None, "released on plain text");
    h.drag(plain, on, Modifiers::none());
    assert_eq!(
        h.hovered().as_deref(),
        Some(LINKED_URL),
        "released on the link"
    );
}

/// The Terminal settings' link choice decides who a click on a web link belongs to under mouse reporting. Intercept
/// (the default) withholds the press from the program, with its release, and Farhelm opens the link, so a program
/// that opens links itself cannot open a second tab; both sends the click on and opens it too; program leaves it to
/// the program alone, and Farhelm opens nothing, with or without mouse reporting.
#[gpui::test]
fn the_link_choice_decides_who_gets_a_click_on_a_link(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Up};
    let mut h = harness(cx);
    h.attach_showing(LINKED);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(5, 0);

    // Program first, while nothing has been opened (the test platform remembers only the last address opened).
    h.link_click("program");
    h.click_at(on);
    assert_eq!(
        h.cx.opened_url(),
        None,
        "program, no reporting: nothing opens"
    );
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    h.click_at(on);
    assert_eq!(
        h.sent(),
        [report(Down, 0, (5, 0)), report(Up, 0, (5, 0))],
        "program: the click is the program's"
    );
    assert_eq!(h.cx.opened_url(), None, "and Farhelm opens nothing");

    h.link_click("intercept");
    h.click_at(on);
    assert_eq!(
        h.sent(),
        [],
        "intercept: the program hears nothing of the press"
    );
    assert_eq!(h.cx.opened_url().as_deref(), Some(LINKED_URL));

    // The other address, so its opening shows (the test platform keeps the last address opened).
    h.link_click("both");
    let other = h.at_cell(31, 0);
    h.click_at(other);
    assert_eq!(
        h.sent(),
        [report(Down, 0, (31, 0)), report(Up, 0, (31, 0))],
        "both: the program gets it"
    );
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some(OTHER_URL),
        "and Farhelm opens it"
    );
}

// ---- A program's hyperlinks ----------------------------------------------------------------------------------------

impl Harness<'_> {
    /// Attach a screen whose first row is `text`, all of it a program's hyperlink (OSC 8) to `uri`.
    fn attach_hyperlink(&mut self, text: &str, uri: &str) {
        self.attach_rows(
            serde_json::json!([[0, {"w":false,"r":[[0,text.chars().count(),text,[0,-1,-1],0]]}]]),
            serde_json::json!([["1", uri]]),
        );
    }

    /// Rest the pointer on `at` and let the destination display open (it opens after the frame that saw the hover).
    fn rest_on(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        settle(self.cx);
        redraw(self.cx);
    }

    /// Whether the destination display is drawn, and whether in its loud mismatch form.
    fn disclosure_shown(&mut self) -> (bool, bool) {
        (
            self.cx.debug_bounds("terminal-link-disclosure").is_some(),
            self.cx
                .debug_bounds("terminal-link-disclosure-warning")
                .is_some(),
        )
    }
}

/// A program's hyperlink can underline text that differs from where it goes, so resting the pointer on it shows its
/// real destination first, next to the pointer, and a click opens it while that display is shown (SPEC.md's links
/// paragraph). Moving off closes the display.
#[gpui::test]
fn a_program_hyperlink_shows_its_destination_and_opens_on_a_click(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("click here", "https://example.com/page");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (on, off) = (h.at_cell(2, 0), h.at_cell(15, 0));
    h.rest_on(on);
    assert_eq!(h.disclosure_shown(), (true, false), "the ordinary display");
    h.click_at(on);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some("https://example.com/page")
    );
    h.rest_on(off);
    assert_eq!(h.disclosure_shown(), (false, false), "moving off closes it");
}

impl Harness<'_> {
    /// The program rewrites row 0 as `text`, all of it hyperlink `1` to `uri` again, as live output at revision `rev`:
    /// the same link, relabelled.
    fn relabel_hyperlink(&mut self, text: &str, uri: &str, rev: u64) {
        let rows: GridOutput = serde_json::from_value(serde_json::json!({
            "t":"rows",
            "rows":[[0,{"w":false,"r":[[0,text.chars().count(),text,[0,-1,-1],0]]}]],
            "links":[["1",uri]]
        }))
        .unwrap();
        self.fake().output(rows);
        let (cols, rows) = FITTED;
        self.fake()
            .output(serde_json::from_value(frame(rev, cols, rows)).unwrap());
        settle(self.cx);
    }
}

/// The display judges what the user would be agreeing to, so a program that relabels the hyperlink under a resting
/// pointer into a lookalike, keeping its target and its cells, must be judged again: otherwise the ordinary display
/// stays up beside a label that now names somewhere else. Specified: resting on a hyperlink labelled with plain words,
/// the ordinary display shows; once the program rewrites the same cells, same target, as a web address naming another
/// site, the display is the warning, without the pointer moving.
#[gpui::test]
fn a_hyperlink_relabelled_under_the_pointer_is_judged_again(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("open the dashboard now", "https://evil.example/");
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(4, 0);
    h.rest_on(on);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "the premise: the ordinary display"
    );
    h.relabel_hyperlink("https://bank.example/x", "https://evil.example/", 2);
    redraw(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (true, true),
        "the relabel is judged again"
    );
}

/// A press opens a link only if the release lands on the link the user agreed to, and a relabel between the two
/// changes what they agreed to (the old app's rule: a link relabelled between press and release does not open).
/// Specified: pressed on a plainly labelled hyperlink, then relabelled in place as a lookalike before the release, the
/// release opens nothing and the display shows the warning; a click on the link as it now reads then opens it.
#[gpui::test]
fn a_hyperlink_relabelled_between_press_and_release_does_not_open(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("open the dashboard now", "https://evil.example/");
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(4, 0);
    h.rest_on(on);
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    h.relabel_hyperlink("https://bank.example/x", "https://evil.example/", 2);
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.cx.opened_url(), None, "nothing opens");
    redraw(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (true, true),
        "the relabelled link is shown as it now is"
    );
    h.click_at(on);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some("https://evil.example/"),
        "and a click on the link as it now reads opens it"
    );
}

impl Harness<'_> {
    /// Attach a screen whose rows are `rows` (the wire's `[line, {"w": wrapped, "r": runs}]` entries) with the
    /// hyperlinks `links` (`[id, uri]` pairs the runs index), replayed whole.
    fn attach_rows(&mut self, rows: serde_json::Value, links: serde_json::Value) {
        let asked = self.fake().attach();
        let mut replay = screen(asked.cols, asked.rows, "", serde_json::Value::Null);
        replay[2] =
            serde_json::from_value(serde_json::json!({"t":"rows","rows":rows,"links":links}))
                .unwrap();
        for message in replay {
            self.fake().output(message);
        }
        self.fake().replay_complete();
        settle(self.cx);
    }
}

/// A program's hyperlink can spread its label over two rows, and the lookalike defence must judge the whole label,
/// not each row's piece: "https://example.com" on one row and ".evil.test/x" on the next together name another site.
/// The warning informs and does not block. Specified: a hyperlink to `https://example.com/x` labelled across rows 0 and
/// 1 that way shows the warning, naming the joined label `https://example.com.evil.test/x`, when resting on either row,
/// and a click while it is shown opens the target.
#[gpui::test]
fn a_two_row_lookalike_hyperlink_warns_on_both_rows_and_still_opens(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_rows(
        serde_json::json!([
            [0, {"w":false,"r":[[0,19,"https://example.com",[0,-1,-1],0]]}],
            [1, {"w":false,"r":[[0,12,".evil.test/x",[0,-1,-1],0]]}]
        ]),
        serde_json::json!([["tag", "https://example.com/x"]]),
    );
    let owner = h.owner.clone();
    h.choose(&owner);
    let (first, second) = (h.at_cell(4, 0), h.at_cell(4, 1));
    h.rest_on(first);
    assert_eq!(h.disclosure_shown(), (true, true), "the first row warns");
    let painted = crate::session_list::tests::painted(&test_support::frame(h.cx)).join(" ");
    assert!(
        painted.contains("https://example.com.evil.test/x"),
        "judged on the whole label, joined across its rows: {painted}"
    );
    h.rest_on(second);
    assert_eq!(h.disclosure_shown(), (true, true), "and so does the second");
    h.click_at(second);
    assert_eq!(h.cx.opened_url().as_deref(), Some("https://example.com/x"));
}

/// A label is the program's text, so the display shows it as data: a right-to-left override cannot reorder what the
/// user reads, and it shows as its escape (SPEC.md: host text is drawn escaped). A target that is not a web address
/// never reaches the browser and gets no display offering it. Specified: a hyperlink to `https://example.com/page`
/// labelled `https://github.com/` and a U+202E shows the warning (the label names another site) with `<U+202E>`
/// painted; a `javascript:` hyperlink shows no display on rest, and a click opens nothing.
#[gpui::test]
fn hostile_hyperlink_text_and_targets_stay_data(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_rows(
        serde_json::json!([
            [0, {"w":false,"r":[[0,20,"https://github.com/\u{202e}",[0,-1,-1],0]]}],
            [2, {"w":false,"r":[[0,10,"run script",[0,-1,-1],1]]}]
        ]),
        serde_json::json!([
            ["a", "https://example.com/page"],
            ["b", "javascript:alert(1)"]
        ]),
    );
    let owner = h.owner.clone();
    h.choose(&owner);
    let label = h.at_cell(4, 0);
    h.rest_on(label);
    assert_eq!(
        h.disclosure_shown(),
        (true, true),
        "the lookalike label warns"
    );
    let painted = crate::session_list::tests::painted(&test_support::frame(h.cx)).join(" ");
    assert!(painted.contains("<U+202E>"), "shown escaped: {painted}");

    let script = h.at_cell(4, 2);
    h.rest_on(script);
    assert_eq!(
        h.disclosure_shown(),
        (false, false),
        "no display for a script target"
    );
    h.click_at(script);
    assert_eq!(h.cx.opened_url(), None, "and nothing opens");
}

/// A printed address can run past the end of a row, and output that later extends it on the next row changes where a
/// click goes: the click must open the address as it now reads, not as it read when first hovered. Specified: with row 0
/// soft-wrapped and ending in `https://bank.co`, a click opens that (normalized, `https://bank.co/`); once the program
/// prints `m` at the start of row 1, the next click opens `https://bank.com/`.
#[gpui::test]
fn a_printed_address_extended_on_the_next_row_opens_its_new_address(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let (cols, rows) = FITTED;
    let pad = "x ".repeat(((cols as usize) - 15) / 2);
    let pad = format!(
        "{pad}{}",
        if (cols as usize - 15) % 2 == 1 {
            " "
        } else {
            ""
        }
    );
    let row0 = format!("{pad}https://bank.co");
    assert_eq!(
        row0.chars().count(),
        cols as usize,
        "the premise: row 0 is full"
    );
    h.attach_rows(
        serde_json::json!([[0, {"w":true,"r":[[0,cols,row0,[0,-1,-1]]]}]]),
        serde_json::json!([]),
    );
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(cols - 4, 0);
    h.click_at(on);
    assert_eq!(h.cx.opened_url().as_deref(), Some("https://bank.co/"));

    let extend: GridOutput = serde_json::from_value(serde_json::json!({
        "t":"rows","rows":[[1, {"w":false,"r":[[0,1,"m",[0,-1,-1]]]}]]
    }))
    .unwrap();
    h.fake().output(extend);
    h.fake()
        .output(serde_json::from_value(frame(2, cols, rows)).unwrap());
    settle(h.cx);
    h.click_at(on);
    assert_eq!(h.cx.opened_url().as_deref(), Some("https://bank.com/"));
}

/// When a hyperlink's text is itself a web address naming somewhere else, the display becomes the loud warning that
/// shows the text beside the real target: the lookalike defence of SPEC.md's links paragraph.
#[gpui::test]
fn a_lookalike_hyperlink_shows_the_warning(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("https://bank.example/", "https://evil.example/");
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(4, 0);
    h.rest_on(on);
    assert_eq!(h.disclosure_shown(), (true, true));
}

/// The display is the whole safeguard: a click on a program's hyperlink whose display was not fully visible when last
/// drawn is refused, opens nothing, and says why at the top of the terminal until the next press there (the old app's
/// refusal). A program with mouse reporting on hears nothing of a press on the link Farhelm keeps for itself.
#[gpui::test]
fn a_click_whose_destination_is_not_fully_visible_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("click here", "https://example.com/page");
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "the premise: the display is shown"
    );
    // As if the display had run past the window's bottom edge when it was last drawn.
    h.cx.update(|_, cx| {
        let shown = owner.read(cx).disclosure.as_ref().unwrap().drawn.clone();
        let (bounds, window) = shown.get().unwrap();
        shown.set(Some((
            Bounds::new(point(bounds.origin.x, window.height - px(4.)), bounds.size),
            window,
        )));
    });
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.cx.opened_url(), None, "nothing opens");
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_some(),
        "the refusal says why"
    );
    assert_eq!(
        h.disclosure_shown(),
        (false, false),
        "and is not covered by the display"
    );
    assert_eq!(h.sent(), [], "the program heard nothing of the press");
    let off = h.at_cell(15, 0);
    h.cx.simulate_mouse_down(off, MouseButton::Left, Modifiers::alt());
    h.cx.simulate_mouse_up(off, MouseButton::Left, Modifiers::alt());
    settle(h.cx);
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_none(),
        "the next press clears it"
    );
}

/// The display follows the pointer from one hyperlink row to the next (each row of a wrapped hyperlink is its own link
/// to the grid model), and survives a click: the press closes the popover, as any press outside one does, and it comes
/// back while the pointer rests, so a second click opens too. Once, moving between rows closed both displays and every
/// click after was refused.
#[gpui::test]
fn the_display_follows_wrapped_rows_and_returns_after_a_click(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let asked = h.fake().attach();
    let mut replay = screen(asked.cols, asked.rows, "", serde_json::Value::Null);
    replay[2] = serde_json::from_value(serde_json::json!({
        "t":"rows",
        "rows":[
            [0,{"w":true,"r":[[0,10,"first half",[0,-1,-1],0]]}],
            [1,{"w":false,"r":[[0,11,"second half",[0,-1,-1],0]]}]
        ],
        "links":[["1","https://example.com/wrapped"]]
    }))
    .unwrap();
    for message in replay {
        h.fake().output(message);
    }
    h.fake().replay_complete();
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (row0, row1) = (h.at_cell(2, 0), h.at_cell(2, 1));
    h.rest_on(row0);
    assert_eq!(h.disclosure_shown(), (true, false));
    h.rest_on(row1);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "the next row has a display too"
    );
    h.cx.simulate_mouse_down(row1, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(row1, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    redraw(h.cx);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some("https://example.com/wrapped")
    );
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "the display is back while the pointer rests"
    );
    // The test platform keeps only the last address opened, and this is the same one, so the second click shows it
    // opened by not being refused: a refused click is the only outcome here that does not open.
    h.cx.simulate_mouse_down(row1, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(row1, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_none(),
        "a second click opens too"
    );
}

/// The refusal's advice has to work: after a refused click, hovering again brings the display back and a click then
/// opens. Until the pointer moves, the display stays away, so it does not sit over the explanation, which a press
/// elsewhere then clears. Once, a refusal kept every display away until the next press, so hovering again showed
/// nothing and the next click was refused too; and once, a move to another cell of the same link did not count,
/// since nothing redraws for it. Specified: no display while the pointer stays put; a bare move to another cell of the
/// link, with no other cause for a frame, shows it; a click then opens.
#[gpui::test]
fn hovering_again_after_a_refusal_shows_the_display_and_a_click_opens(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("click here", "https://example.com/page");
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    h.cx.update(|_, cx| {
        let shown = owner.read(cx).disclosure.as_ref().unwrap().drawn.clone();
        let (bounds, window) = shown.get().unwrap();
        shown.set(Some((
            Bounds::new(point(bounds.origin.x, window.height - px(4.)), bounds.size),
            window,
        )));
    });
    h.click_at(on);
    assert_eq!(h.cx.opened_url(), None, "the premise: the click is refused");
    redraw(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (false, false),
        "no display while the pointer stays put"
    );
    let next = h.at_cell(3, 0);
    h.cx.simulate_mouse_move(next, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "hovering again, within the link, shows the display"
    );
    h.click_at(next);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some("https://example.com/page"),
        "and a click opens"
    );
}

/// Escape with focus outside the terminal closes the display (the layer manager's Escape rule for a popover); the
/// display must then stay closed while the pointer rests, or it reopens at once and the Escape does nothing visible.
/// Once, it did. Specified: a display put away with no press, by the layer manager's own Escape dismissal
/// (`layers::dismiss_top`, which Escape with focus outside the terminal calls), stays gone through further frames;
/// moving the pointer to another cell of the link shows it again, with no other cause for a frame.
#[gpui::test]
fn a_display_closed_without_a_press_stays_closed_until_the_pointer_moves(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("click here", "https://example.com/page");
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "the premise: the display shows"
    );
    h.cx.update(|window, cx| {
        assert!(
            layers::dismiss_top(window, cx),
            "the premise: Escape put a layer away"
        );
    });
    settle(h.cx);
    redraw(h.cx);
    settle(h.cx);
    redraw(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (false, false),
        "closed, it stays closed"
    );
    // A bare move, no forced frame: within one link, nothing but the terminal itself asks for one.
    let next = h.at_cell(3, 0);
    h.cx.simulate_mouse_move(next, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "hovering again shows it"
    );
}

/// A program's hyperlink that is not a web address (a `file://` link, as `ls --hyperlink` prints) never reaches the
/// browser. Its display is its file check ([`super::super::files`]), which here has nowhere to go and fails, so a
/// click on it admits nothing and must not claim the display was hidden.
#[gpui::test]
fn a_click_on_a_file_hyperlink_says_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_hyperlink("notes.txt", "file:///home/user/notes.txt");
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    assert_eq!(
        h.hovered().as_deref(),
        Some("file:///home/user/notes.txt"),
        "the premise: it is a link"
    );
    h.click_at(on);
    assert_eq!(h.cx.opened_url(), None);
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_none(),
        "no refusal"
    );
}

// ---- Tunnels to a remote host's loopback ---------------------------------------------------------------------------

/// A dev server's address as a remote program prints it.
const LOOPBACK_LINE: &str = "serving http://localhost:5173/app?x=1";
const LOOPBACK_URL: &str = "http://localhost:5173/app?x=1";

/// The hint the display carries for [`LOOPBACK_URL`] on the host the fake helm names.
const LOOPBACK_HINT: &str = "This is port 5173 on build-box, not on this Mac. ⌘-click to open it through a tunnel. A \
                             plain click opens it on this Mac.";

/// Cmd, as the Mac's ⌘ key is held for a click.
fn cmd() -> Modifiers {
    Modifiers {
        platform: true,
        ..Modifiers::none()
    }
}

impl Harness<'_> {
    /// Decide the tunnel offer as the Mac app does, whatever machine runs the test.
    fn on_a_mac(&mut self) {
        let owner = self.owner.clone();
        self.cx.update(|_, cx| {
            owner.update(cx, |owner, _| owner.set_platform(Platform::Mac));
        });
    }

    /// Answer the one command the terminal has sent, which must be its tunnel read, as a helm whose session runs on
    /// the host `build-box`, remote when `available`.
    fn answer_tunnel_read(&mut self, available: bool) {
        let commands = self.helm.commands();
        assert_eq!(
            commands
                .iter()
                .map(|command| (command.method, command.path.as_str()))
                .collect::<Vec<_>>(),
            [(farhelm_helm_client::Method::Get, "/api/sessions/s1/tunnels")],
            "the premise: the terminal read its session's tunnels once"
        );
        self.helm.reply(
            commands[0].id,
            200,
            Some(serde_json::json!({
                "available": available,
                "host_name": "build-box",
                "consent": false,
                "tunnels": [],
            })),
        );
        settle(self.cx);
        redraw(self.cx);
    }

    /// The tunnel hint the open destination display carries, as a screen reader reads it.
    fn hint(&mut self) -> Option<String> {
        self.read(|owner, _| {
            owner
                .disclosure
                .as_ref()
                .and_then(|shown| shown.hint.as_ref())
                .map(|hint| hint.accessible_name().to_string())
        })
    }

    /// Push the shared tunnel consent preference as the helm does.
    fn tunnel_consent(&mut self, given: bool) {
        self.helm
            .push(farhelm_proto::ui_channel::CollectionChanges {
                collection: farhelm_proto::ui_channel::Collection::Preferences,
                meta: test_support::meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
                upsert: vec![farhelm_proto::ui_channel::KeyedEntry {
                    key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
                    value: serde_json::json!({ "tunnel_consent": given }),
                }],
                remove: Vec::new(),
            });
        settle(self.cx);
    }

    /// A press and release at `at` with `modifiers` held, the pointer at rest there already.
    fn click_with(&mut self, at: Point<Pixels>, modifiers: Modifiers) {
        self.cx
            .simulate_mouse_down(at, MouseButton::Left, modifiers);
        self.cx.simulate_mouse_up(at, MouseButton::Left, modifiers);
        settle(self.cx);
    }
}

/// SPEC.md's localhost tunnels: in the Mac app, a loopback address printed in a remote session's terminal shows a
/// display naming the port and host and saying a plain click still opens it on this Mac, which a printed address
/// otherwise never gets. Specified: hovering it reads the session's tunnels once; once the helm says the host is
/// remote, the display shows with the old app's hint, and a plain click opens the address unchanged on this Mac.
#[gpui::test]
fn a_loopback_address_in_a_remote_session_offers_a_tunnel_on_a_mac(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    assert_eq!(
        h.helm.commands(),
        [],
        "nothing is read before a loopback link is hovered"
    );
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    assert_eq!(
        h.disclosure_shown(),
        (true, false),
        "a display, for a printed address"
    );
    assert_eq!(h.hint().as_deref(), Some(LOOPBACK_HINT));
    assert!(
        h.cx.debug_bounds(tunnels::HINT_ID).is_some(),
        "and the hint is drawn"
    );
    h.click_with(on, Modifiers::none());
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some(LOOPBACK_URL),
        "a plain click opens it here"
    );
    assert_eq!(h.helm.commands(), [], "and asks the helm nothing");
}

/// Off a Mac there is no tunnel to offer, so the terminal does not even ask the helm about one. Specified: with the
/// platform decided as Linux, hovering a loopback address sends nothing and shows no display.
#[gpui::test]
fn off_a_mac_a_loopback_address_reads_nothing_and_shows_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let owner = h.owner.clone();
    h.cx.update(|_, cx| owner.update(cx, |owner, _| owner.set_platform(Platform::Linux)));
    h.attach_showing(LOOPBACK_LINE);
    h.choose(&owner);
    let at = h.at_cell(12, 0);
    h.rest_on(at);
    assert_eq!(h.helm.commands(), [], "nothing is read");
    assert_eq!(h.disclosure_shown(), (false, false));
}

/// A local session's loopback address is this Mac's own, so a plain click is already right and there is nothing to
/// offer. Specified: when the helm answers `available: false`, hovering shows no display.
#[gpui::test]
fn a_local_sessions_loopback_address_gets_no_offer(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let at = h.at_cell(12, 0);
    h.rest_on(at);
    h.answer_tunnel_read(false);
    assert_eq!(h.disclosure_shown(), (false, false));
}

/// With the link choice that leaves clicks to the program, Farhelm opens nothing on a click, tunnel or not, so the
/// hint would promise something a click does not do. Specified: under "program" a remote session's loopback address
/// gets no hint and no display.
#[gpui::test]
fn no_tunnel_is_offered_when_the_program_owns_clicks(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.link_click("program");
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let at = h.at_cell(12, 0);
    h.rest_on(at);
    h.answer_tunnel_read(true);
    assert_eq!(h.hint(), None);
    assert_eq!(h.disclosure_shown(), (false, false));
}

/// SPEC.md: "Before the first tunnel opens, the user must accept the security warning", consent being a shared helm
/// preference that is off until accepted; and the helm, not the app, opens the browser at the tunnel's own address.
/// Specified: a Cmd-click without consent asks the warning, its box drawn above the approval cards, and sends nothing;
/// Allow saves `tunnel_consent: true`, and once that is saved the link goes to the helm's open route as printed;
/// nothing opens in the app itself, and the tunnels are read again after the open answers.
#[gpui::test]
fn a_cmd_click_asks_the_warning_then_opens_the_tunnel_through_the_helm(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    h.click_with(on, cmd());
    assert!(
        h.cx.debug_bounds("prompt").is_some(),
        "the warning asks first"
    );
    let raised =
        h.cx.update(|window, cx| crate::layers::top_is_above_approval_region(window, cx));
    assert!(raised, "with its box drawn above the approval cards");
    assert_eq!(
        h.helm.commands(),
        [],
        "and nothing is sent before it is answered"
    );
    click(h.cx, "prompt-confirm");
    let save = h.helm.commands();
    assert_eq!(
        save.iter()
            .map(|c| (c.method, c.path.as_str(), c.body.clone()))
            .collect::<Vec<_>>(),
        [(
            farhelm_helm_client::Method::Put,
            "/api/preferences",
            Some(serde_json::json!({ "tunnel_consent": true }))
        )],
        "Allow saves consent"
    );
    h.helm.reply(save[0].id, 204, None);
    settle(h.cx);
    let open = h.helm.commands();
    assert_eq!(
        open.iter()
            .map(|c| (c.method, c.path.as_str(), c.body.clone()))
            .collect::<Vec<_>>(),
        [(
            farhelm_helm_client::Method::Post,
            "/api/sessions/s1/tunnels/open",
            Some(serde_json::json!({ "url": LOOPBACK_URL }))
        )],
        "then the link goes to the helm as printed"
    );
    h.helm.reply(
        open[0].id,
        200,
        Some(serde_json::json!({
            "tunnel": { "id": "t1", "local_port": 54321, "remote_port": 5173 },
            "url": "http://127.0.0.1:54321/app?x=1",
        })),
    );
    settle(h.cx);
    assert_eq!(
        h.cx.opened_url(),
        None,
        "the helm opened the browser, not the app"
    );
    assert_eq!(
        h.helm
            .commands()
            .iter()
            .map(|c| (c.method, c.path.clone()))
            .collect::<Vec<_>>(),
        [(
            farhelm_helm_client::Method::Get,
            "/api/sessions/s1/tunnels".to_string()
        )],
        "and the tunnels are read again"
    );
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_none(),
        "nothing failed"
    );
}

/// The warning is a real choice: declining it must leave everything as it was, and say nothing, since nothing failed.
/// Specified: Cancel sends nothing and shows no notice.
#[gpui::test]
fn declining_the_warning_opens_nothing_and_says_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    h.click_with(on, cmd());
    click(h.cx, "prompt-cancel");
    assert_eq!(h.helm.commands(), []);
    assert_eq!(h.cx.opened_url(), None);
    assert!(h.cx.debug_bounds(tunnels::NOTICE_ID).is_none());
}

/// Once consent is given (here or in another client: it is a shared preference), Cmd-click opens at once; a refusal is
/// shown in the helm's words in a notice that stays until dismissed. Only Cmd alone means a tunnel: with another
/// modifier the click opens the address as printed, as a plain click does. Specified: with consent pushed, Cmd-click
/// sends the open with no warning; the helm's refusal shows in the notice; Dismiss puts it away; Cmd+Shift opens the
/// address in the app.
#[gpui::test]
fn with_consent_a_cmd_click_opens_at_once_and_a_refusal_shows_until_dismissed(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.tunnel_consent(true);
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    h.click_with(on, cmd());
    assert!(
        h.cx.debug_bounds("prompt").is_none(),
        "no warning once consent is given"
    );
    let open = h.helm.commands();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].path, "/api/sessions/s1/tunnels/open");
    h.helm.reply(
        open[0].id,
        409,
        Some(serde_json::json!("the helm is shutting down")),
    );
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_some(),
        "the refusal shows"
    );
    assert_eq!(
        h.tunnel_notice().as_deref(),
        Some("the helm is shutting down"),
        "in the helm's words"
    );
    let reread = h.helm.commands();
    assert_eq!(reread.len(), 1);
    assert_eq!(
        reread[0].path, "/api/sessions/s1/tunnels",
        "the premise: the re-read"
    );
    h.helm.reply(
        reread[0].id,
        200,
        Some(serde_json::json!({"available": true, "host_name": "build-box", "consent": true, "tunnels": []})),
    );
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_some(),
        "a successful read after it leaves it up"
    );
    // The pointer goes to the button, leaving the link and so its display, which would otherwise cover the notice.
    let dismiss = test_support::center(h.cx, "terminal-tunnel-notice-dismiss");
    h.rest_on(dismiss);
    click(h.cx, "terminal-tunnel-notice-dismiss");
    settle(h.cx);
    redraw(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_none(),
        "Dismiss puts it away"
    );
    let shift = Modifiers {
        shift: true,
        ..cmd()
    };
    let at = h.at_cell(13, 0);
    h.rest_on(at);
    h.click_with(at, shift);
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some(LOOPBACK_URL),
        "Cmd+Shift opens it as printed"
    );
    assert_eq!(h.helm.commands(), []);
}

/// A printed loopback address with a tunnel hint is disclosed like a program's hyperlink, so its display is a
/// safeguard too: the hint is what tells the user a plain click reaches this Mac rather than the remote host, and the
/// old app refused a click on such an address, raw or tunneled, while its display was not fully visible. Specified:
/// with the display faked as having run past the window's edge, a plain click opens nothing and explains why, and a
/// Cmd-click asks no warning and sends nothing.
#[gpui::test]
fn a_hinted_address_whose_display_is_not_fully_visible_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    let off_screen = |h: &mut Harness<'_>| {
        let owner = h.owner.clone();
        h.cx.update(|_, cx| {
            let shown = owner.read(cx).disclosure.as_ref().unwrap().drawn.clone();
            let (bounds, window) = shown.get().unwrap();
            shown.set(Some((
                Bounds::new(point(bounds.origin.x, window.height - px(4.)), bounds.size),
                window,
            )));
        });
    };
    off_screen(&mut h);
    h.click_with(on, Modifiers::none());
    assert_eq!(h.cx.opened_url(), None, "a plain click opens nothing");
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_some(),
        "and says why"
    );
    let back = h.at_cell(13, 0);
    h.rest_on(back);
    off_screen(&mut h);
    h.click_with(back, cmd());
    assert!(
        h.cx.debug_bounds("prompt").is_none(),
        "a Cmd-click asks nothing"
    );
    assert!(
        h.cx.debug_bounds("terminal-link-refusal").is_some(),
        "and is refused, saying why"
    );
    assert_eq!(h.helm.commands(), [], "and sends nothing");
}

impl Harness<'_> {
    /// The steps every Cmd-click test shares: on a Mac, a remote session printing [`LOOPBACK_LINE`], consent given,
    /// the pointer resting on the address with its tunnel hint shown. Returns where the address is.
    fn tunnel_offered(&mut self) -> Point<Pixels> {
        self.on_a_mac();
        self.tunnel_consent(true);
        self.attach_showing(LOOPBACK_LINE);
        let owner = self.owner.clone();
        self.choose(&owner);
        let on = self.at_cell(12, 0);
        self.rest_on(on);
        self.answer_tunnel_read(true);
        assert_eq!(
            self.hint().as_deref(),
            Some(LOOPBACK_HINT),
            "the premise: a tunnel is offered"
        );
        on
    }

    /// The tunnel notice's words, as the helm or the app wrote them, a line per failure; `None` when it is empty.
    fn tunnel_notice(&mut self) -> Option<String> {
        self.read(|owner, cx| {
            let lines: Vec<String> = owner
                .tunnels()
                .read(cx)
                .notice()
                .iter()
                .map(|line| line.raw().to_string())
                .collect();
            (!lines.is_empty()).then(|| lines.join("\n"))
        })
    }
}

/// Only Cmd alone means a tunnel (the old app's rule): Cmd with Control, Option or fn has other meanings or none, and
/// a click that is not the tunnel gesture opens the address as printed, as the hint says a plain click does.
/// Specified: Cmd+Control, Cmd+Option and Cmd+fn send the helm nothing and ask no warning; the address opens as
/// printed.
#[gpui::test]
fn cmd_with_another_modifier_is_not_the_tunnel_gesture(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let on = h.tunnel_offered();
    for (name, modifiers) in [
        (
            "control",
            Modifiers {
                control: true,
                ..cmd()
            },
        ),
        ("option", Modifiers { alt: true, ..cmd() }),
        (
            "fn",
            Modifiers {
                function: true,
                ..cmd()
            },
        ),
    ] {
        h.click_with(on, modifiers);
        assert_eq!(h.helm.commands(), [], "Cmd+{name} sends the helm nothing");
        assert!(h.cx.debug_bounds("prompt").is_none(), "nor asks anything");
        assert!(
            h.cx.debug_bounds("terminal-link-refusal").is_none(),
            "and the click went through, as a plain one does"
        );
    }
    // The test platform keeps only the last address opened, so this shows that the clicks opened the address as
    // printed, not that each did; the helm hearing nothing of any of them is the point.
    assert_eq!(h.cx.opened_url().as_deref(), Some(LOOPBACK_URL));
}

/// The old app ran one tunnel action at a time and said so when a second came; a second open sent while the first
/// is out would race it for the same listener. Specified: a Cmd-click while the session's open is still in flight
/// sends nothing more and shows "Another tunnel action is still waiting".
#[gpui::test]
fn a_second_open_while_one_is_in_flight_says_so(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let on = h.tunnel_offered();
    h.click_with(on, cmd());
    assert_eq!(
        h.helm.commands().len(),
        1,
        "the premise: the first open is out"
    );
    h.click_with(on, cmd());
    assert_eq!(h.helm.commands(), [], "no second open");
    assert_eq!(
        h.tunnel_notice().as_deref(),
        Some("Another tunnel action is still waiting")
    );
}

/// Allow is only half the answer: if the helm refuses to store consent, no tunnel may open, and the user must hear
/// why. Specified: a refused consent save shows the helm's words in the notice and sends no open.
#[gpui::test]
fn a_refused_consent_save_shows_why_and_opens_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    h.click_with(on, cmd());
    click(h.cx, "prompt-confirm");
    let save = h.helm.commands();
    assert_eq!(save.len(), 1);
    assert_eq!(
        save[0].path, "/api/preferences",
        "the premise: Allow's consent save"
    );
    h.helm.reply(
        save[0].id,
        500,
        Some(serde_json::json!("the store is read-only")),
    );
    settle(h.cx);
    assert_eq!(h.helm.commands(), [], "no open");
    assert_eq!(h.tunnel_notice().as_deref(), Some("the store is read-only"));
}

/// A failed read must not cost the offer for as long as the terminal is open, nor be sent again every frame while
/// the pointer rests. Specified: a refused tunnel read shows in the notice; while the pointer stays on the address
/// nothing more is sent; once it has left (here to dismiss the notice, which sits over the address's row) and comes
/// back, the read is sent again, and that answer brings the hint.
#[gpui::test]
fn a_failed_tunnel_read_shows_why_and_is_tried_again_on_the_next_hover(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    let read = h.helm.commands();
    assert_eq!(read.len(), 1);
    h.helm
        .reply(read[0].id, 404, Some(serde_json::json!("no such session")));
    settle(h.cx);
    redraw(h.cx);
    assert_eq!(h.tunnel_notice().as_deref(), Some("no such session"));
    redraw(h.cx);
    redraw(h.cx);
    assert_eq!(
        h.helm.commands(),
        [],
        "not again while the pointer stays on the address"
    );
    let dismiss = test_support::center(h.cx, "terminal-tunnel-notice-dismiss");
    h.rest_on(dismiss);
    click(h.cx, "terminal-tunnel-notice-dismiss");
    settle(h.cx);
    redraw(h.cx);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    assert_eq!(h.hint().as_deref(), Some(LOOPBACK_HINT));
}

/// The notice sits over the terminal's top rows, so a press on it must be the notice's alone: pressing Dismiss must
/// not also select, report a click to a program that asked for mouse reports, or open a link printed under it.
/// Specified: with mouse reporting on, clicking Dismiss puts the notice away and sends the program nothing.
#[gpui::test]
fn dismissing_the_tunnel_notice_does_not_reach_the_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let on = h.tunnel_offered();
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    h.click_with(on, cmd());
    let open = h.helm.commands();
    h.helm.reply(
        open[0].id,
        409,
        Some(serde_json::json!("the helm is shutting down")),
    );
    settle(h.cx);
    let reread = h.helm.commands();
    h.helm.reply(
        reread[0].id,
        200,
        Some(serde_json::json!({"available": true, "host_name": "build-box", "consent": true, "tunnels": []})),
    );
    settle(h.cx);
    redraw(h.cx);
    let _ = h.sent();
    let dismiss = test_support::center(h.cx, "terminal-tunnel-notice-dismiss");
    let grid = h.read(|owner, _| owner.grid_origin.unwrap());
    assert!(
        dismiss.y > grid.y && dismiss.x > grid.x,
        "the premise: Dismiss lies over the grid, where a press would otherwise be reported"
    );
    h.rest_on(dismiss);
    click(h.cx, "terminal-tunnel-notice-dismiss");
    settle(h.cx);
    redraw(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_none(),
        "Dismiss puts it away"
    );
    assert_eq!(h.sent(), [], "and the program heard nothing of the press");
}

/// Allow can meet another preferences save still in flight (an earlier Allow whose answer has not come, or a Settings
/// change); the click must then say so rather than do nothing. Specified: with one consent save unanswered, a second
/// Allow sends nothing more and the notice gives a save's busy line ("Not saved: " and the store's shared reason).
#[gpui::test]
fn a_second_allow_while_consent_is_still_saving_says_so(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.answer_tunnel_read(true);
    h.click_with(on, cmd());
    click(h.cx, "prompt-confirm");
    assert_eq!(
        h.helm.commands().len(),
        1,
        "the premise: the first consent save is out"
    );
    h.rest_on(on);
    h.click_with(on, cmd());
    click(h.cx, "prompt-confirm");
    assert_eq!(h.helm.commands(), [], "no second save");
    assert_eq!(
        h.tunnel_notice().as_deref(),
        Some("Not saved: another operation was in progress. Try again.")
    );
}

/// Whether a click tunnels is decided at the press, with the admission: an address that had no hint (and so no display
/// to check) when pressed must not tunnel because the session's tunnels were read before the release. Specified: a
/// Cmd press on the address before the read answers, released after it does, asks no warning, sends the helm nothing
/// and opens the address as printed.
#[gpui::test]
fn a_tunnel_offer_arriving_mid_click_does_not_turn_it_into_a_tunnel(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.tunnel_consent(true);
    h.attach_showing(LOOPBACK_LINE);
    let owner = h.owner.clone();
    h.choose(&owner);
    let on = h.at_cell(12, 0);
    h.rest_on(on);
    h.cx.simulate_mouse_down(on, MouseButton::Left, cmd());
    settle(h.cx);
    h.answer_tunnel_read(true);
    h.cx.simulate_mouse_up(on, MouseButton::Left, cmd());
    settle(h.cx);
    assert!(h.cx.debug_bounds("prompt").is_none());
    assert_eq!(h.helm.commands(), [], "no tunnel");
    assert_eq!(
        h.cx.opened_url().as_deref(),
        Some(LOOPBACK_URL),
        "opened as printed"
    );
}

/// SPEC.md: the hover "preserves any OSC hyperlink mismatch warning". A program's hyperlink whose text names one site
/// while it goes to the remote host's loopback gets both: the loud warning first, the tunnel hint after it.
#[gpui::test]
fn a_lookalike_loopback_hyperlink_keeps_its_warning_and_adds_the_hint(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on_a_mac();
    h.attach_hyperlink("https://bank.example/", "http://localhost:5173/app?x=1");
    let owner = h.owner.clone();
    h.choose(&owner);
    let at = h.at_cell(4, 0);
    h.rest_on(at);
    h.answer_tunnel_read(true);
    assert_eq!(h.disclosure_shown(), (true, true), "the warning stays");
    assert_eq!(
        h.hint().as_deref(),
        Some(LOOPBACK_HINT),
        "and the hint is added"
    );
}

// ---- The open-tunnels pill and its list ----------------------------------------------------------------------------

impl Harness<'_> {
    /// Answer the one command the terminal has sent, which must be its tunnel read, with `listed` tunnels as (id,
    /// local port, remote port) on the remote host `build-box`.
    fn answer_listing(&mut self, listed: &[(&str, u16, u16)]) {
        let commands = self.helm.commands();
        assert_eq!(
            commands
                .iter()
                .map(|command| (command.method, command.path.as_str()))
                .collect::<Vec<_>>(),
            [(farhelm_helm_client::Method::Get, "/api/sessions/s1/tunnels")],
            "the premise: the terminal read its session's tunnels"
        );
        let tunnels: Vec<_> = listed
            .iter()
            .map(|(id, local, remote)| serde_json::json!({"id": id, "local_port": local, "remote_port": remote}))
            .collect();
        self.helm.reply(
            commands[0].id,
            200,
            Some(serde_json::json!({
                "available": true,
                "host_name": "build-box",
                "consent": true,
                "tunnels": tunnels,
            })),
        );
        settle(self.cx);
        redraw(self.cx);
    }

    /// The pill's accessible name, from a fresh frame's record; `None` when no pill is drawn.
    fn pill_label(&mut self) -> Option<String> {
        test_support::frame(self.cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some(tunnels::PILL_ID))
            .and_then(|node| node.label.map(|label| label.to_string()))
    }
}

/// SPEC.md: the pill lists a session's open tunnels, so its tunnels must be known as the session view opens, not only
/// once a loopback link happens to be hovered; and the listing is not pushed, so the read must not repeat on every
/// frame. Specified: a remote session's terminal sends one tunnel read as it opens, and further frames send nothing.
#[gpui::test]
fn a_remote_sessions_terminal_reads_its_tunnels_once_as_it_opens(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[]);
    redraw(h.cx);
    redraw(h.cx);
    assert_eq!(h.helm.commands(), [], "no second read");
    assert_eq!(h.pill_label(), None, "no pill without tunnels");
}

/// SPEC.md: "A distinctively colored pill beside the terminal tabs shows the local-port → host-port mapping for one
/// listener, or a count for several ... with the full mapping in its tooltip and accessible name." Specified: one
/// tunnel names its mapping; a change of the shared consent reads the listing again (withdrawing it closes every
/// tunnel), and with two tunnels the pill names their count.
#[gpui::test]
fn the_pill_names_one_mapping_or_a_count_and_follows_consent_changes(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[("t1", 54321, 5173)]);
    assert_eq!(
        h.pill_label().as_deref(),
        Some("Open tunnels from this Mac: :54321 → build-box:5173")
    );
    h.tunnel_consent(true);
    h.answer_listing(&[("t1", 54321, 5173), ("t2", 54400, 8080)]);
    assert_eq!(
        h.pill_label().as_deref(),
        Some("2 open tunnels from this Mac")
    );
    h.tunnel_consent(false);
    h.answer_listing(&[]);
    assert_eq!(
        h.pill_label(),
        None,
        "consent withdrawn: the helm closed them, and the pill goes"
    );
}

/// The listing is read again whenever consent changes, and withdrawing consent closes every tunnel, so a change that
/// lands while the first read is still out must be followed by another read: the first answer may predate it. Once,
/// such a change went unread until something else was pushed. Specified: with the first read unanswered, a consent
/// change sends a second read.
#[gpui::test]
fn a_consent_change_during_the_first_read_reads_again(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    let first = h.helm.commands();
    assert_eq!(first.len(), 1, "the premise: the first read is out");
    h.tunnel_consent(true);
    let second = h.helm.commands();
    assert_eq!(
        second.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
        ["/api/sessions/s1/tunnels"],
        "the change sends another read"
    );
}

/// SPEC.md: "Opening the pill shows a list of mappings with a close button for each listener and Close all ...
/// Escape closes the list and returns focus to the pill." Specified: pressing the pill draws the list with focus
/// inside it; Escape closes it and focus is on the pill.
#[gpui::test]
fn the_pill_opens_its_list_and_escape_returns_focus_to_the_pill(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[("t1", 54321, 5173)]);
    // As on a Mac, where a click leaves a button unfocused: the list must return focus to the pill itself, not to
    // whatever held it before.
    h.cx.update(|_, cx| focus::set_click_focuses_controls(false, cx));
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::LIST_ID).is_some(),
        "the list opens"
    );
    assert_eq!(
        test_support::focused_id(h.cx).as_deref(),
        Some(tunnels::CLOSE_ALL_ID),
        "with focus on Close all"
    );
    keys(h.cx, "escape");
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::LIST_ID).is_none(),
        "Escape closes it"
    );
    assert_eq!(
        test_support::focused_id(h.cx).as_deref(),
        Some(tunnels::PILL_ID),
        "and focus is back on the pill"
    );
}

/// SPEC.md: "Tab follows its ordinary controls and leaving the list closes it without reclaiming focus", where a
/// menu would hold Tab. Specified: with the list open and focus on Close all, its last control, Tab leaves the list,
/// which closes, and focus is neither inside it nor sent back to the pill.
#[gpui::test]
fn tab_out_of_the_list_closes_it_without_reclaiming_focus(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[("t1", 54321, 5173)]);
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    assert_eq!(
        test_support::focused_id(h.cx).as_deref(),
        Some(tunnels::CLOSE_ALL_ID),
        "the premise: focus is on Close all"
    );
    keys(h.cx, "tab");
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::LIST_ID).is_none(),
        "leaving the list closes it"
    );
    assert_ne!(
        test_support::focused_id(h.cx).as_deref(),
        Some(tunnels::PILL_ID),
        "without sending focus back to the pill"
    );
}

/// SPEC.md: "Close all attempts the listeners shown at activation in order, continuing after failures and retaining
/// failed listeners with their reasons", in a notice that a refresh does not dismiss. Specified: three tunnels are
/// closed one at a time, in order; the first that closes leaves the list at once; the second's refusal does not stop
/// the third; afterwards the listing is read again, and the notice names the failed tunnel with the helm's words.
#[gpui::test]
fn close_all_closes_in_order_and_keeps_failures_with_their_reasons(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[
        ("t1", 54321, 5173),
        ("t2", 54400, 8080),
        ("t3", 54500, 9000),
    ]);
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    click(h.cx, tunnels::CLOSE_ALL_ID);
    settle(h.cx);
    let mut closed = Vec::new();
    for (id, status, body) in [
        ("t1", 204, None),
        ("t2", 409, Some(serde_json::json!("the listener is busy"))),
        ("t3", 204, None),
    ] {
        let commands = h.helm.commands();
        assert_eq!(commands.len(), 1, "one close at a time");
        assert_eq!(commands[0].method, farhelm_helm_client::Method::Delete);
        closed.push(commands[0].path.clone());
        h.helm.reply(commands[0].id, status, body);
        settle(h.cx);
        if id == "t1" {
            assert!(
                h.cx.debug_bounds("session-tunnel-close-t1").is_none(),
                "a closed tunnel leaves the list at once"
            );
        }
    }
    assert_eq!(
        closed,
        [
            "/api/sessions/s1/tunnels/t1",
            "/api/sessions/s1/tunnels/t2",
            "/api/sessions/s1/tunnels/t3",
        ],
        "in the order shown"
    );
    h.answer_listing(&[("t2", 54400, 8080)]);
    assert_eq!(
        h.tunnel_notice().as_deref(),
        Some(":54400 → build-box:8080: the listener is busy"),
        "the failure, named, survives the read"
    );
}

/// SPEC.md: "refreshing the list does not dismiss a close failure", and the notice must be readable while the list is
/// open, which would otherwise cover it. Specified: after a Close all with one refused tunnel, a failed read of the
/// listing shows its own reason first and keeps the close failure; while the list is open the notice is drawn inside
/// it.
#[gpui::test]
fn a_failed_read_after_close_all_keeps_the_close_failure_inside_the_open_list(
    cx: &mut TestAppContext,
) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[("t1", 54321, 5173)]);
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    click(h.cx, tunnels::CLOSE_ALL_ID);
    settle(h.cx);
    let close = h.helm.commands();
    h.helm.reply(
        close[0].id,
        409,
        Some(serde_json::json!("the listener is busy")),
    );
    settle(h.cx);
    let read = h.helm.commands();
    assert_eq!(
        read[0].path, "/api/sessions/s1/tunnels",
        "the premise: the read after Close all"
    );
    h.helm.reply(
        read[0].id,
        500,
        Some(serde_json::json!("the helm is restarting")),
    );
    settle(h.cx);
    assert_eq!(
        h.tunnel_notice().as_deref(),
        Some("the helm is restarting\n:54321 → build-box:5173: the listener is busy"),
        "the read's failure first, and the close failure kept"
    );
    let (notice, list) = (
        h.cx.debug_bounds(tunnels::NOTICE_ID)
            .expect("the notice is drawn"),
        h.cx.debug_bounds(tunnels::LIST_ID)
            .expect("the premise: the list is open"),
    );
    assert!(
        list.contains(&notice.origin) && list.contains(&notice.bottom_right()),
        "inside the open list"
    );
    let drawn = test_support::frame(h.cx)
        .into_iter()
        .filter(|node| node.element_id.as_deref() == Some(tunnels::NOTICE_ID))
        .count();
    assert_eq!(drawn, 1, "and only there, not also under the list");
    keys(h.cx, "escape");
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::LIST_ID).is_none(),
        "the premise: Escape closed the list"
    );
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_some(),
        "and the notice is back under the top row"
    );
    // The pill closes the list too, and the notice comes back the same way, with no other cause for a frame.
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    assert!(
        h.cx.debug_bounds(tunnels::LIST_ID).is_none(),
        "the premise: the pill closed the list"
    );
    assert!(
        h.cx.debug_bounds(tunnels::NOTICE_ID).is_some(),
        "and the notice is back under the top row"
    );
}

/// Each tunnel's own close button closes just that tunnel. Specified: the second row's close sends that tunnel's
/// delete and nothing else.
#[gpui::test]
fn a_rows_close_button_closes_only_its_tunnel(cx: &mut TestAppContext) {
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    h.answer_listing(&[("t1", 54321, 5173), ("t2", 54400, 8080)]);
    click(h.cx, tunnels::PILL_ID);
    settle(h.cx);
    click(h.cx, "session-tunnel-close-t2");
    settle(h.cx);
    let commands = h.helm.commands();
    assert_eq!(
        commands.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
        ["/api/sessions/s1/tunnels/t2"]
    );
}

/// Settings' tunnel switch turns tunnels on only through the same warning (SPEC.md: off "until they accept the
/// warning"), so the terminal area offers it a way in. Specified: the warning's box is drawn above the approval cards;
/// Allow saves `tunnel_consent: true` and reports the save as stored; Cancel saves nothing and reports nothing.
#[gpui::test]
fn settings_way_into_the_warning_saves_consent_only_on_allow(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello");
    let saved: Rc<RefCell<Vec<Result<(), String>>>> = Rc::default();
    let record = saved.clone();
    h.cx.update(|window, cx| {
        let mark = focus::Mark::take(cx);
        tunnels::ask_consent(&mark, window, cx, move |result, _| {
            record
                .borrow_mut()
                .push(result.map_err(|reason| reason.raw().to_string()));
        });
    });
    settle(h.cx);
    let raised =
        h.cx.update(|window, cx| crate::layers::top_is_above_approval_region(window, cx));
    assert!(
        raised,
        "the warning's box is drawn above the approval cards"
    );
    click(h.cx, "prompt-cancel");
    assert_eq!(h.helm.commands(), [], "Cancel saves nothing");
    assert!(saved.borrow().is_empty(), "and reports nothing");
    let record = saved.clone();
    h.cx.update(|window, cx| {
        let mark = focus::Mark::take(cx);
        tunnels::ask_consent(&mark, window, cx, move |result, _| {
            record
                .borrow_mut()
                .push(result.map_err(|reason| reason.raw().to_string()));
        });
    });
    settle(h.cx);
    click(h.cx, "prompt-confirm");
    let save = h.helm.commands();
    assert_eq!(
        save.iter()
            .map(|c| (c.path.as_str(), c.body.clone()))
            .collect::<Vec<_>>(),
        [(
            "/api/preferences",
            Some(serde_json::json!({ "tunnel_consent": true }))
        )]
    );
    h.helm.reply(save[0].id, 204, None);
    settle(h.cx);
    assert_eq!(*saved.borrow(), [Ok(())], "Allow's save is reported stored");
}

// ---- The dimmed, inert terminal ----------------------------------------------------------------------------------

impl Harness<'_> {
    /// Make the first owner inert, or not, as the session view does under a status card.
    fn inert(&mut self, inert: bool) {
        let owner = self.owner.clone();
        self.cx
            .update(|_, cx| owner.update(cx, |owner, cx| owner.set_inert(inert, cx)));
        settle(self.cx);
    }
}

/// SPEC.md: an unreachable host's session keeps "a retained terminal dimmed and inert beneath" its card. The card is
/// the session view's; the terminal must show its screen dimmed and let nothing reach the program, or a keystroke or a
/// click aimed at the card would land in a program the user cannot see answering. Specified: an inert terminal keeps
/// its screen under the scrim; typing, a press under mouse reporting and a wheel turn over it send nothing; turned
/// back, typing reaches the program again.
#[gpui::test]
fn an_inert_terminal_is_dimmed_and_sends_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    let _ = h.sent();
    h.inert(true);
    assert!(
        h.cx.debug_bounds(INERT_SCRIM_ID).is_some(),
        "the scrim is drawn"
    );
    assert_eq!(
        h.read(|owner, cx| owner.terminal_text(cx))
            .map(|text| first_line(&text).to_owned())
            .as_deref(),
        Some("hello"),
        "over the screen it keeps"
    );
    keys(h.cx, "a");
    let at = h.at_cell(2, 0);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    h.cx.simulate_event(gpui::ScrollEvent {
        position: at,
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-200.))),
        ..Default::default()
    });
    settle(h.cx);
    assert_eq!(h.sent(), [], "no key, press or wheel reaches the program");
    // With no mouse reporting a drag would select and copy; the veil takes it before the grid sees it.
    h.fake().output(mouse_mode("off"));
    settle(h.cx);
    let (from, to) = (h.at_cell(0, 0), h.at_cell(4, 0));
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    h.cx.dispatch_action(actions::SelectAllInTerminal);
    settle(h.cx);
    assert_eq!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selected_text()),
        None,
        "nothing is selected, by the pointer or by Select All"
    );
    h.inert(false);
    assert!(
        h.cx.debug_bounds(INERT_SCRIM_ID).is_none(),
        "the scrim goes"
    );
    h.choose(&owner);
    keys(h.cx, "a");
    assert!(!h.sent().is_empty(), "and typing reaches the program again");
}

/// The mouse report deduper must hear only reports that are sent: a drag move dropped while the terminal was inert,
/// counted as sent, would suppress the same move once the terminal is usable again, and the program's pointer would
/// lag a cell behind until it left that cell. Specified: with drag reporting on and a button held, a move to another
/// cell while inert sends nothing, and the same move once the terminal is usable again sends that cell's report.
#[gpui::test]
fn a_move_dropped_while_inert_is_sent_once_usable_again(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    h.fake().output(mouse_mode("drag"));
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (press, step, far) = (h.at_cell(2, 0), h.at_cell(3, 0), h.at_cell(6, 0));
    h.cx.simulate_mouse_down(press, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(step, Some(MouseButton::Left), Modifiers::none());
    settle(h.cx);
    assert!(
        !h.sent().is_empty(),
        "the premise: the program hears the drag"
    );

    h.inert(true);
    h.cx.simulate_mouse_move(far, Some(MouseButton::Left), Modifiers::none());
    settle(h.cx);
    assert_eq!(h.sent(), [], "nothing is sent while inert");

    h.inert(false);
    h.cx.simulate_mouse_move(far, Some(MouseButton::Left), Modifiers::none());
    settle(h.cx);
    let sent = h.sent();
    assert!(
        sent.iter().any(|(input, _)| matches!(
            input,
            GridInput::Mouse {
                kind: MouseEventKind::Move,
                x: 6,
                y: 0,
                ..
            }
        )),
        "the move reaches the program: {sent:?}"
    );
}

/// What was under way when the terminal went inert must not finish behind the veil: a selection drag would copy at its
/// release and a press on a link would open it. Specified: a drag begun, then inert, then moved and released, copies
/// nothing; a press on a web link, then inert, then released, opens nothing.
#[gpui::test]
fn going_inert_drops_a_drag_and_a_link_press_under_way(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("go https://example.com/x and more words");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.cx.write_to_clipboard(ClipboardItem::new_string("before".into()));
    let (from, mid, to) = (h.at_cell(20, 0), h.at_cell(23, 0), h.at_cell(26, 0));
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(mid, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selected_text())
            .is_some(),
        "the premise: the drag has selected"
    );
    h.inert(true);
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.cx.read_from_clipboard().and_then(|item| item.text()),
        Some("before".to_string()),
        "the drag copied nothing"
    );
    h.inert(false);
    let link = h.at_cell(5, 0);
    h.rest_on(link);
    assert_eq!(
        h.hovered().as_deref(),
        Some("https://example.com/x"),
        "the premise: the press lands on a hovered web link"
    );
    h.cx.simulate_mouse_down(link, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    h.inert(true);
    h.cx.simulate_mouse_up(link, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.cx.opened_url(), None, "the link press opened nothing");
}

/// A paste is the user's input like a key: one still reading the clipboard when the terminal goes inert must not reach
/// the program, even if the terminal is no longer inert when the read answers (the inert check at sending would let it
/// through then). Specified: with the paste asked for, made inert and back in the same update, so the paste is still in
/// flight, nothing is sent; the same paste with no inert step in between is sent, which is the premise.
#[gpui::test]
fn a_paste_in_flight_when_the_terminal_goes_inert_is_dropped(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("screen");
    let owner = h.owner.clone();
    h.choose(&owner);
    h.cx.write_to_clipboard(ClipboardItem::new_string("pasted".into()));
    h.cx.update(|window, cx| {
        owner.update(cx, |owner, cx| owner.paste(&actions::Paste, window, cx));
    });
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [(paste("pasted", true), true)],
        "the premise: the same paste, with nothing in between, is sent"
    );
    h.cx.update(|window, cx| {
        owner.update(cx, |owner, cx| {
            owner.paste(&actions::Paste, window, cx);
            owner.set_inert(true, cx);
            owner.set_inert(false, cx);
        });
    });
    settle(h.cx);
    assert_eq!(h.sent(), [], "the paste in flight was dropped");
}

/// The card standing over an inert terminal is where the keyboard belongs (SPEC.md, "Keyboard focus": a card's
/// controls are reached by Tab), so the terminal under it must leave the Tab order and take it back when it returns.
/// Tab inside a focused terminal is the program's, so this walks in from the window's root. Specified: from the root,
/// with the first of two terminals inert, Tab lands on the second, passing the first; with neither inert, Tab lands on
/// the first.
#[gpui::test]
fn an_inert_terminal_is_no_tab_stop(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    let (first, second) = (h.owner.clone(), h.second.clone().unwrap().0);
    assert!(
        !h.focused(&first) && !h.focused(&second),
        "the premise: focus rests on the root"
    );
    h.inert(true);
    keys(h.cx, "tab");
    assert!(
        h.focused(&second) && !h.focused(&first),
        "Tab passes the inert terminal"
    );
}

/// [`an_inert_terminal_is_no_tab_stop`]'s other half, from a fresh window: with neither of two terminals inert, Tab
/// from the root lands on the first, so the first is a Tab stop again once it is not inert.
#[gpui::test]
fn a_terminal_that_is_not_inert_is_a_tab_stop(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    let first = h.owner.clone();
    h.inert(true);
    h.inert(false);
    keys(h.cx, "tab");
    assert!(h.focused(&first), "Tab lands on the first terminal");
}

impl Harness<'_> {
    /// Push the shared **Color terminal frames by host** preference.
    fn color_frames(&mut self, on: bool) {
        self.helm
            .push(farhelm_proto::ui_channel::CollectionChanges {
                collection: farhelm_proto::ui_channel::Collection::Preferences,
                meta: test_support::meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
                upsert: vec![farhelm_proto::ui_channel::KeyedEntry {
                    key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
                    value: serde_json::json!({ "color_terminal_frames": on }),
                }],
                remove: Vec::new(),
            });
        settle(self.cx);
    }

    /// Push host 2 (the remote harness's session's host) with identity color `color`, a wire word.
    fn host_color(&mut self, color: &str) {
        let hosts: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../farhelm-helm/http-contract/host-list.json"
        ))
        .unwrap();
        let mut host = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|host| host["id"] == 2)
            .unwrap()
            .clone();
        host["color"] = serde_json::json!(color);
        let keys: Vec<String> = hosts["hosts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|host| farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()))
            .collect();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.helm
            .push(farhelm_proto::ui_channel::CollectionChanges {
                collection: farhelm_proto::ui_channel::Collection::Hosts,
                meta: test_support::meta(&keys),
                upsert: vec![farhelm_proto::ui_channel::KeyedEntry {
                    key: farhelm_proto::ui_channel::HostView::key(2),
                    value: host,
                }],
                remove: Vec::new(),
            });
        settle(self.cx);
    }

    /// The frame's color as the terminal last took it from the pushed state.
    fn frame(&mut self) -> Option<farhelm_proto::host_appearance::HostColor> {
        self.read(|owner, _| owner.frame())
    }
}

/// SPEC.md: "Settings can color the thin frame around every session's terminal with that session's host identity
/// color. This is off by default ... a host with the default color uses the ordinary foreground color ... A host color
/// change updates open session frames through the existing hosts feed, and a switch change applies immediately."
/// Specified: with the preference unset no frame is drawn; turned on, the frame is drawn in the host's color (the
/// default color, for this host); the host's color changing changes the frame's at once; turned off, the frame goes.
#[gpui::test]
fn the_frame_takes_the_hosts_color_while_the_preference_is_on(cx: &mut TestAppContext) {
    use farhelm_proto::host_appearance::HostColor;
    let mut h = remote_harness(cx);
    h.attach_showing("hello");
    let _ = h.helm.commands();
    assert_eq!(h.frame(), None, "off by default");
    assert!(h.cx.debug_bounds(HOST_FRAME_ID).is_none());
    let _ = h.fake().received();
    h.color_frames(true);
    redraw(h.cx);
    assert_eq!(
        h.frame(),
        Some(HostColor::Default),
        "the host's color, here the default"
    );
    assert!(h.cx.debug_bounds(HOST_FRAME_ID).is_some(), "drawn");
    assert!(
        !h.fake()
            .received()
            .iter()
            .any(|request| matches!(request, FakeTerminalRequest::Resize { .. })),
        "the terminal's size does not change"
    );
    h.host_color("teal");
    assert_eq!(
        h.frame(),
        Some(HostColor::Teal),
        "a host color change applies at once"
    );
    h.color_frames(false);
    assert_eq!(h.frame(), None, "and turning it off takes the frame away");
    assert!(h.cx.debug_bounds(HOST_FRAME_ID).is_none());
}

/// The frame follows the preference as the store holds it when the terminal opens, not only its changes after: a
/// session opened while the preference is already on is framed from its first frame. Specified: a remote session's
/// terminal made with the preference on and its host colored teal draws the teal frame at once.
#[gpui::test]
fn a_terminal_opened_with_the_preference_on_is_framed(cx: &mut TestAppContext) {
    use farhelm_proto::host_appearance::HostColor;
    let hosts: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../farhelm-helm/http-contract/host-list.json"
    ))
    .unwrap();
    let entries: Vec<_> = hosts["hosts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|host| {
            let mut host = host.clone();
            if host["id"] == 2 {
                host["color"] = serde_json::json!("teal");
            }
            farhelm_proto::ui_channel::KeyedEntry {
                key: farhelm_proto::ui_channel::HostView::key(host["id"].as_i64().unwrap()),
                value: host,
            }
        })
        .collect();
    let keys: Vec<String> = entries.iter().map(|entry| entry.key.clone()).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    let more = vec![
        test_support::sessions(&["s1"]),
        farhelm_proto::ui_channel::CollectionState {
            collection: farhelm_proto::ui_channel::Collection::Hosts,
            meta: test_support::meta(&keys),
            entries,
        },
        farhelm_proto::ui_channel::CollectionState {
            collection: farhelm_proto::ui_channel::Collection::Preferences,
            meta: test_support::meta(&[farhelm_proto::ui_channel::PREFERENCES_KEY]),
            entries: vec![farhelm_proto::ui_channel::KeyedEntry {
                key: farhelm_proto::ui_channel::PREFERENCES_KEY.into(),
                value: serde_json::json!({ "color_terminal_frames": true }),
            }],
        },
    ];
    let mut h = harness_connected(cx, false, more);
    h.attach_showing("hello");
    assert_eq!(h.frame(), Some(HostColor::Teal));
    assert!(
        h.cx.debug_bounds(HOST_FRAME_ID).is_some(),
        "drawn from the start"
    );
}

// ---- Mouse reporting -----------------------------------------------------------------------------------------------

/// A mouse report as the terminal sends it, with no modifier held.
fn report(kind: MouseEventKind, button: i8, (x, y): (u16, u16)) -> (GridInput, bool) {
    (
        GridInput::Mouse {
            kind,
            button,
            x,
            y,
            mods: 0,
        },
        true,
    )
}

/// A program that turned mouse reporting on gets the presses and releases of every button at the cell under the
/// pointer, in each mode; drag mode adds moves while a button is held and motion mode moves with none, each only
/// in a cell the last report did not name (the format leaves de-duplication to the client). Nothing the program did not ask for
/// is sent: click mode gets no moves. These are what full-screen programs (editors, agents) build their mouse
/// support on.
#[gpui::test]
fn the_program_gets_the_mouse_reports_its_mode_asks_for(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Move, Up};
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (a, b) = (h.at_cell(2, 1), h.at_cell(6, 1));
    let none = Modifiers::none();

    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    h.cx.simulate_mouse_down(a, MouseButton::Left, none);
    h.cx.simulate_mouse_move(b, MouseButton::Left, none);
    h.cx.simulate_mouse_up(b, MouseButton::Left, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [report(Down, 0, (2, 1)), report(Up, 0, (6, 1))],
        "click mode: no moves"
    );
    h.cx.simulate_mouse_down(a, MouseButton::Right, none);
    h.cx.simulate_mouse_up(a, MouseButton::Right, none);
    h.cx.simulate_mouse_down(a, MouseButton::Middle, none);
    h.cx.simulate_mouse_up(a, MouseButton::Middle, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [
            report(Down, 2, (2, 1)),
            report(Up, 2, (2, 1)),
            report(Down, 1, (2, 1)),
            report(Up, 1, (2, 1))
        ],
        "the right and middle buttons too"
    );

    h.fake().output(mouse_mode("drag"));
    settle(h.cx);
    let b_again = b + point(px(1.), px(0.));
    h.cx.simulate_mouse_down(a, MouseButton::Left, none);
    h.cx.simulate_mouse_move(b, MouseButton::Left, none);
    h.cx.simulate_mouse_move(b_again, MouseButton::Left, none);
    h.cx.simulate_mouse_up(b, MouseButton::Left, none);
    h.cx.simulate_mouse_move(a, None, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [
            report(Down, 0, (2, 1)),
            report(Move, 0, (6, 1)),
            report(Up, 0, (6, 1))
        ],
        "drag mode: one move per new cell while held, none without a button"
    );

    // Two buttons held: moves report the lower one (left before right), whichever the platform says was pressed last.
    h.cx.simulate_mouse_down(a, MouseButton::Left, none);
    h.cx.simulate_mouse_down(a, MouseButton::Right, none);
    h.cx.simulate_mouse_move(b, MouseButton::Right, none);
    h.cx.simulate_mouse_up(b, MouseButton::Right, none);
    h.cx.simulate_mouse_up(b, MouseButton::Left, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [
            report(Down, 0, (2, 1)),
            report(Down, 2, (2, 1)),
            report(Move, 0, (6, 1)),
            report(Up, 2, (6, 1)),
            report(Up, 0, (6, 1))
        ],
        "a chord drags with the left button"
    );

    h.fake().output(mouse_mode("motion"));
    settle(h.cx);
    h.cx.simulate_mouse_move(b, None, none);
    h.cx.simulate_mouse_move(b_again, None, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [report(Move, -1, (6, 1))],
        "motion mode: moves with no button"
    );
}

/// A press that goes to the program is input like a key: it clears the local selection and brings a scrolled view
/// back to the live screen. A release the window never saw (the button let go outside it) is still reported, at the
/// next move, so the program is not left thinking a button is held.
#[gpui::test]
fn a_reported_press_clears_the_selection_and_a_lost_release_is_reported(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Up};
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::none());
    let selection = |h: &mut Harness| {
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).selection())
    };
    assert!(
        selection(&mut h).is_some(),
        "the premise: something is selected"
    );
    h.fake().output(mouse_mode("drag"));
    settle(h.cx);
    let at = h.at_cell(3, 0);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(selection(&mut h), None, "the report cleared the selection");
    h.cx.simulate_mouse_move(at, None, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.sent(), [report(Down, 0, (3, 0)), report(Up, 0, (3, 0))]);
}

/// A press reported to the program is input like a key, so a view scrolled back into the history comes back to the
/// live screen with it: the program acts on the cell of the screen it drew, and a view left in the history would show
/// the user something other than what the click acted on. The old app's "a mouse report brings a scrolled-back view to
/// the bottom and clears the selection, as input does"; the selection half is also the test above's.
///
/// Specified: with history scrolled back by the wheel and a selection made while reporting was off, turning click
/// reporting on leaves the view scrolled back; a click then sends a press and a release at the clicked cell, the view
/// follows the live screen again, and the selection is gone.
#[gpui::test]
fn a_reported_press_brings_a_scrolled_back_view_to_the_live_screen(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Up};
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::none());
    assert!(
        h.selection().is_some(),
        "the premise: something is selected"
    );
    h.wheel(1.);
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    assert!(
        !h.following(),
        "the premise: scrolled back, and turning reporting on leaves the view there"
    );
    assert_eq!(h.sent(), [], "the premise: nothing sent yet");
    assert!(
        h.selection().is_some(),
        "the premise: the wheel and the mode change left the selection"
    );

    let at = h.at_cell(2, 1);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.sent(), [report(Down, 0, (2, 1)), report(Up, 0, (2, 1))]);
    assert!(
        h.following(),
        "the report brought the view to the live screen"
    );
    assert_eq!(h.selection(), None);
}

/// Under mouse reporting the wheel on the ordinary screen is the program's, not a scroll through the terminal's own
/// history: an editor or agent with mouse support scrolls its own content and would otherwise see nothing while the
/// terminal scrolled lines the program does not know about. The alternate screen's wheel has a test of its own; this is
/// the primary screen's, where history exists and the wrong owner would visibly scroll it (the wheel step of the old
/// app's "the mouse reports cells only in the modes the program asked for").
///
/// Specified: on the primary screen with 40 history lines and click reporting on, a notch toward older lines and one
/// toward newer each send one wheel report, buttons 0 and 1, at the cell under the pointer, and the view keeps
/// following the live screen.
#[gpui::test]
fn the_wheel_under_mouse_reporting_is_the_programs_on_the_primary_screen(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_with_history(40);
    let owner = h.owner.clone();
    h.choose(&owner);
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    assert!(
        h.read(|owner, cx| owner.committed.as_ref().unwrap().read(cx).scroll_position())
            .scrollable(),
        "the premise: there is history the wheel could scroll"
    );
    h.wheel(1.);
    h.wheel(-1.);
    let under = h
        .read(|owner, cx| owner.cell_at(point(px(200.), px(200.)), cx))
        .unwrap();
    assert_eq!(
        h.sent(),
        [
            report(MouseEventKind::Wheel, 0, under),
            report(MouseEventKind::Wheel, 1, under)
        ]
    );
    assert!(h.following(), "the terminal's own history did not scroll");
}

/// A drag the program follows can leave the terminal and be let go outside it, and the program must still hear where
/// it went and that it ended: an editor extending a selection to the bottom of its window, or a pane divider being
/// dragged, would otherwise be left with its button held. Positions outside the grid are reported at its nearest cell,
/// the old app's clamp. Once released, a move with no button is no longer a drag.
///
/// Specified: under drag reporting, a press at cell (1, 0), a move to (4, 2), a move within that cell, a move to a
/// point below and right of the grid, a release there, and then a move with no button over the grid send exactly a
/// press at (1, 0), a move at (4, 2), and a move and a release at the grid's last cell.
#[gpui::test]
fn a_reported_drag_released_outside_the_grid_ends_at_its_last_cell(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Move, Up};
    let mut h = harness(cx);
    h.attach_showing("hello world");
    h.fake().output(mouse_mode("drag"));
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    let (cols, rows) = h.read(|owner, cx| {
        let view = owner.committed.as_ref().unwrap().read(cx);
        (view.cols, u16::try_from(view.rows.len()).unwrap())
    });
    let last = (cols - 1, rows - 1);
    let outside = h.at_cell(last.0, last.1) + point(px(300.), px(200.));
    assert!(
        outside.x > px(PANE.0) && outside.y > px(PANE.1),
        "the premise: {outside:?} is outside the terminal's pane"
    );
    let none = Modifiers::none();
    let (a, b, over) = (h.at_cell(1, 0), h.at_cell(4, 2), h.at_cell(2, 2));
    h.cx.simulate_mouse_down(a, MouseButton::Left, none);
    h.cx.simulate_mouse_move(b, MouseButton::Left, none);
    h.cx.simulate_mouse_move(b + point(px(1.), px(0.)), MouseButton::Left, none);
    h.cx.simulate_mouse_move(outside, MouseButton::Left, none);
    h.cx.simulate_mouse_up(outside, MouseButton::Left, none);
    h.cx.simulate_mouse_move(over, None, none);
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [
            report(Down, 0, (1, 0)),
            report(Move, 0, (4, 2)),
            report(Move, 0, last),
            report(Up, 0, last)
        ]
    );
}

/// A program's mouse reporting is part of its terminal state, so a reattach that replays a screen with reporting on
/// brings the reports back at once, without the program having to ask again (the client half of the old app's
/// `mouse-modes-restored-on-reattach`; that the supervisor's replay carries the mode is the supervisor's to test).
#[gpui::test]
fn mouse_reporting_survives_a_reattach(cx: &mut TestAppContext) {
    use MouseEventKind::{Down, Up};
    let mut h = harness(cx);
    h.attach_showing("hello world");
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    let owner = h.owner.clone();
    h.choose(&owner);
    let at = h.at_cell(1, 0);
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.sent(),
        [report(Down, 0, (1, 0)), report(Up, 0, (1, 0))],
        "the premise: reporting is on before the reconnect"
    );
    h.fake().drop_link();
    let asked = h.fake().attach();
    for message in screen(asked.cols, asked.rows, "again", serde_json::Value::Null) {
        h.fake().output(message);
    }
    // The supervisor's replay carries the program's mode in its state, as for any other attach.
    h.fake().output(mouse_mode("click"));
    h.fake().replay_complete();
    settle(h.cx);
    assert!(
        h.read(|owner, _| owner.is_live()),
        "the premise: live again"
    );
    h.cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.sent(), [report(Down, 0, (1, 0)), report(Up, 0, (1, 0))]);
}

// ---- Paste ---------------------------------------------------------------------------------------------------------

/// This platform's chords for the terminal action `action`, as the keymap binds them.
fn chords_of(action: &dyn gpui::Action) -> Vec<&'static str> {
    crate::actions::TERMINAL_KEYS
        .iter()
        .filter_map(|key| Some((key.chord(THIS_PLATFORM)?, key.bind?)))
        .filter(|(chord, bind)| bind(chord).action().partial_eq(action))
        .map(|(chord, _)| chord)
        .collect()
}

/// This platform's paste chords, as the keymap binds them.
fn paste_chords() -> Vec<&'static str> {
    chords_of(&actions::Paste)
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

/// A paste is input the user sends, like a key, so it clears the local selection: the highlight would otherwise stay
/// on text the program may already have redrawn, and the next copy would take that stale range (the paste half of the
/// old app's "typing and pasting clear the selection"; typing is `a_drag_selects_and_copies_and_a_click_does_not`'s).
///
/// Specified: with a drag's selection on the screen, a paste sends the clipboard's text as one paste and leaves no
/// selection.
#[gpui::test]
fn a_paste_clears_the_selection(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.attach_showing("hello world");
    let owner = h.owner.clone();
    h.choose(&owner);
    let (from, to) = (h.at_cell(0, 0), h.at_cell(5, 0));
    h.drag(from, to, Modifiers::none());
    assert!(
        h.selection().is_some(),
        "the premise: something is selected"
    );
    h.set_clipboard("pasted");
    h.cx.dispatch_action(actions::Paste);
    settle(h.cx);
    assert_eq!(h.sent(), [(paste("pasted", true), true)]);
    assert_eq!(h.selection(), None, "the paste cleared the selection");
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
    // The replay's own state message says the program is asking, as a long-running program's replay does. A separate
    // message after it would be one event too many for one turn: the stream shows each reconnect wait and the attempt
    // after it, so the drop, the attempt, the attachment and this replay already take a whole batch.
    let mut replay = screen(asked.cols, asked.rows, "again", serde_json::Value::Null);
    replay[1] = focus_mode(true);
    for message in replay {
        h.fake().output(message);
    }
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
/// as the terminal's own command (paging, select all) must be the terminal's in the keymap, except Shift+PageUp and
/// Shift+PageDown with further modifiers held, which the keymap leaves to the terminal's listener on purpose: it pages
/// them there (`program_input_routes_keys_to_the_program_or_text_input` pins that).
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
        KeyRoute::Page(_) | KeyRoute::Nothing => false,
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
    assert_eq!(
        on("ctrl-shift-pageup"),
        KeyRoute::Page(-1),
        "a page key with more modifiers pages, which the keymap leaves to the listener"
    );
    assert_eq!(on("alt-shift-pagedown"), KeyRoute::Page(1));
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

// ---- Text size -----------------------------------------------------------------------------------------------------

/// SPEC.md, "Terminal experience": a text size step reaches "every open terminal's text size together, hidden tabs
/// included", and "each terminal is resized so the program inside sees its new rows and columns". A terminal that
/// redrew at the new size without telling its program would leave the program drawing for the old grid, wrapping and
/// cropping wrongly. Specified: with two live terminals, one step up draws both at the font measured at 15 points and
/// sends each fake stream exactly one resize, to the size that font fits in its own pane; a redraw sends nothing more.
#[gpui::test]
fn a_text_size_step_resizes_every_open_terminal(cx: &mut TestAppContext) {
    let mut h = harness_with(cx, true);
    h.attach_showing("first");
    let (second, second_opened) = h.second.clone().unwrap();
    let asked = second_opened.borrow_mut().as_mut().unwrap().0.attach();
    for message in screen(asked.cols, asked.rows, "second", serde_json::Value::Null) {
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
    assert_eq!(
        h.fake().received(),
        [],
        "the premise: both are live at their fitted size"
    );
    assert_eq!(
        second_opened.borrow_mut().as_mut().unwrap().0.received(),
        []
    );

    h.cx.update(|_, cx| super::super::text_size::step(1, cx));
    settle(h.cx);
    let larger =
        h.cx.update(|_, cx| CellMetrics::measure(super::super::TextSize::new(15), cx))
            .expect("the test font measures");
    let first = h.owner.clone();
    for (owner, name) in [(&first, "first"), (&second, "second")] {
        let (metrics, drawn) = h.cx.update(|_, cx| {
            let owner = owner.read(cx);
            (
                owner.metrics,
                owner.committed.as_ref().unwrap().read(cx).metrics,
            )
        });
        assert!(
            metrics == larger,
            "the {name} terminal fits its grid at 15 points"
        );
        assert!(drawn == larger, "and its screen draws at 15 points");
    }
    let fits = |pane: (f32, f32)| {
        fit(
            larger,
            pane.0 - 2. * PANE_PADDING,
            pane.1 - 2. * PANE_PADDING,
            1.,
        )
        .unwrap()
    };
    let resize = |size: (u16, u16)| FakeTerminalRequest::Resize {
        cols: size.0,
        rows: size.1,
    };
    assert_eq!(h.fake().received(), [resize(fits(PANE))]);
    assert_eq!(
        second_opened.borrow_mut().as_mut().unwrap().0.received(),
        [resize(fits(SECOND_PANE))]
    );
    redraw(h.cx);
    assert_eq!(h.fake().received(), [], "no second resize");
}

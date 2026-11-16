//! The file links' contract, against the helm client's fake terminal and fake transfers: what a hover on a printed path
//! or a program's `file://` hyperlink asks the host, what its display shows while checking and once answered, when a
//! hover counts as new, and which clicks a check admits. One test per sentence of SPEC.md's file-path bullet in this
//! scope, ported from the old app's `terminal-files.test.js` and `e2e/tests/terminal-files.spec.ts` (each test names
//! the cases it replaces).
//!
//! The terminal is a real attached owner in a window from the shared harness, so the display opens through the layer
//! manager as in the app; the fake transfers hold each check's answer until the test gives it, which is how a test sees
//! the checking state and whether leaving abandoned a check.

use std::cell::RefCell;
use std::rc::Rc;

use farhelm_grid_model::GridOutput;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::fake::transfers::{FakeAnswer, FakeTransfer, FakeTransfers};
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_proto::grid::{GridInput, MouseEventKind};
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionState, KeyedEntry, PREFERENCES_KEY, Preferences,
};
use gpui::{
    AppContext as _, Bounds, Context, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, PlatformInput, Point, Render, Styled, TestAppContext,
    VisualTestContext, Window, div, point, px,
};

use super::*;
use crate::session;
use crate::terminal::CellMetrics;
use crate::terminal::attached::{AttachedTerminal, OpenStream, TerminalTarget};
use crate::test_support::{self, redraw, settle};
use crate::{root, theme};

// ---- Pure rules ------------------------------------------------------------------------------------------------------

/// Sizes are what the user weighs before a transfer, and their units must agree with the host's decimal 100 MB limit:
/// a file just over it must never read as under "100 MB". Specified (the old "file sizes and tiers use decimal
/// boundaries including the exact cap"): whole bytes under 1000, then one decimal with `.0` dropped; the tiers switch
/// at exactly 1 MB and 10 MB, the limit itself is still large rather than refused, and every refusal is red whatever its
/// size.
#[test]
fn sizes_and_tiers_use_decimal_boundaries_including_the_exact_limit() {
    let ready = |size| DownloadFileInfo {
        path: "/work/f".into(),
        size: Some(size),
        status: DownloadFileStatus::Ready,
    };
    for (bytes, text, tier) in [
        (0, "0 B", SizeTier::Small),
        (999, "999 B", SizeTier::Small),
        (4800, "4.8 KB", SizeTier::Small),
        (999_999, "1000 KB", SizeTier::Small),
        (1_000_000, "1 MB", SizeTier::Medium),
        (8_400_000, "8.4 MB", SizeTier::Medium),
        (10_000_000, "10 MB", SizeTier::Large),
        (87_000_000, "87 MB", SizeTier::Large),
        (100_000_000, "100 MB", SizeTier::Large),
    ] {
        assert_eq!(format_size(bytes), text);
        assert_eq!(SizeTier::of(&ready(bytes)), tier, "{bytes} bytes");
    }
    assert_eq!(format_size(100_000_001), "100 MB");
    assert_eq!(format_size(2_500_000_000), "2.5 GB");
    // Half up on the exact quotient, where the old `toFixed` read the float just below it as "1.1 KB".
    assert_eq!(format_size(1150), "1.2 KB");
    for status in [
        DownloadFileStatus::TooLarge,
        DownloadFileStatus::Folder,
        DownloadFileStatus::NotFound,
        DownloadFileStatus::NotReadable,
        DownloadFileStatus::NotRegular,
    ] {
        let refused = DownloadFileInfo { status, ..ready(1) };
        assert_eq!(
            SizeTier::of(&refused),
            SizeTier::Danger,
            "a refusal overrides its size"
        );
    }
    for palette in [&theme::DARK, &theme::LIGHT] {
        assert_eq!(SizeTier::Danger.stripe(palette), palette.danger);
        assert_eq!(
            SizeTier::Danger.words(palette),
            palette.danger,
            "refusals are red"
        );
        assert_eq!(SizeTier::Small.stripe(palette), palette.ok);
        assert_eq!(SizeTier::Medium.stripe(palette), palette.info);
        assert_eq!(
            SizeTier::Medium.words(palette),
            palette.fg_1,
            "a medium size is neutral"
        );
        assert_eq!(SizeTier::Large.stripe(palette), palette.warn);
        assert_eq!(
            SizeTier::Neutral.stripe(palette),
            palette.fg_2,
            "checking is neutral"
        );
    }
}

/// The icon tells kinds of file apart and claims nothing about opening: an unknown extension stays generic. Specified
/// (the old "file icons distinguish documents, PDFs, images, archives and folders"): case-insensitive extensions, the
/// last one deciding, and a folder's icon whatever its name.
#[test]
fn icons_distinguish_documents_pdfs_images_archives_and_folders() {
    let ready = &DownloadFileStatus::Ready;
    assert_eq!(file_icon("/work/notes.MD", ready), '\u{f15c}');
    assert_eq!(file_icon("/work/main.rs", ready), '\u{f15c}');
    assert_eq!(file_icon("/work/report.pdf", ready), '\u{f1c1}');
    assert_eq!(file_icon("/work/shot.png", ready), '\u{f1c5}');
    assert_eq!(file_icon("/work/files.tar.gz", ready), '\u{f1c6}');
    assert_eq!(
        file_icon("/work/notes.md", &DownloadFileStatus::Folder),
        '\u{f07b}'
    );
    assert_eq!(file_icon("/work/notes.txt.command", ready), '\u{f15b}');
}

/// Only the helm decides whether a modifier-click may open a file, and the hint must name this platform's modifier.
/// Specified (the hint half of the old "hover advertises open only for native admitted types"): a ready file always
/// says a click downloads; the opening hint appears only when the helm offered opening (`can_open`), as ⌘ on a Mac and
/// Ctrl elsewhere; a refusal has no hints.
#[test]
fn hints_offer_opening_only_where_the_helm_does_with_the_platforms_modifier() {
    let stat = |can_open, status| FileStat {
        host: "build-box".into(),
        can_open,
        info: DownloadFileInfo {
            path: "/work/notes.txt".into(),
            size: Some(4800),
            status,
        },
    };
    let ready = DownloadFileStatus::Ready;
    for (can_open, platform, hint) in [
        (
            Some(true),
            Platform::Mac,
            Some(" · click to download · ⌘-click to open"),
        ),
        (
            Some(true),
            Platform::Linux,
            Some(" · click to download · Ctrl-click to open"),
        ),
        (Some(false), Platform::Mac, Some(" · click to download")),
        (None, Platform::Linux, Some(" · click to download")),
    ] {
        assert_eq!(
            hints(&stat(can_open, ready.clone()), platform).as_deref(),
            hint
        );
    }
    assert_eq!(
        hints(&stat(Some(true), DownloadFileStatus::Folder), Platform::Mac),
        None
    );
}

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// The window's root: the terminal in a fixed 816 by 496 pane, wired through [`root`] so its display opens as a layer.
struct Pane {
    owner: Entity<AttachedTerminal>,
    root: root::Attachment,
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = div().w(px(816.)).h(px(496.)).child(self.owner.clone());
        root::wrap(div().size_full().child(pane), &self.root, window, cx)
    }
}

/// An attached terminal of session `s1`, drawn and checking through the fake transfers, with the helm's side of its
/// stream and the test's side of its checks.
struct Harness<'a> {
    cx: &'a mut VisualTestContext,
    owner: Entity<AttachedTerminal>,
    stream: Rc<RefCell<Option<FakeTerminal>>>,
    /// The size the stream attached at, once [`Harness::show`] attached it.
    size: (u16, u16),
    transfers: FakeTransfers,
    /// The UI channel's helm: it keeps the UI session the terminal was made for current, and pushes preferences.
    helm: FakeHelm,
}

/// A connected app with the terminal drawn in its own window, its UI session's transfers the fake's (served through
/// the connection, the path the app's own take) and its preferences row empty, as a fresh helm pushes it. Its stream is
/// opened (the pane has been measured) but not attached; [`Harness::show`] attaches it.
fn harness(cx: &mut TestAppContext) -> Harness<'_> {
    test_support::install(cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    let (transfers, served) = FakeTransfers::new();
    cx.update(|cx| connection.update(cx, |connection, _| connection.serve_transfers(served)));
    helm.connect(vec![CollectionState {
        collection: Collection::Preferences,
        meta: test_support::meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::json!({}),
        }],
    }]);
    cx.run_until_parked();
    let session = cx.update(|cx| connection.read(cx).session().cloned().unwrap());
    let stream: Rc<RefCell<Option<FakeTerminal>>> = Rc::default();
    let slot = stream.clone();
    let (pane, cx) = test_support::open_view(cx, |window, cx| {
        let open: OpenStream = Box::new(move |attach| {
            let (fake, handle, events) = FakeTerminal::open(&attach);
            *slot.borrow_mut() = Some(fake);
            (handle, events)
        });
        let target = TerminalTarget {
            session: "s1".into(),
            tab: None,
            lease: AttachmentLease::new(),
        };
        let metrics = CellMetrics::new(10., 22., 16.).unwrap();
        let owner = cx.new(|cx| AttachedTerminal::new(&session, target, metrics, open, cx));
        Pane {
            owner,
            root: root::attach(window, cx),
        }
    });
    let owner = cx.update(|_, cx| pane.read(cx).owner.clone());
    Harness {
        cx,
        owner,
        stream,
        size: (0, 0),
        transfers,
        helm,
    }
}

/// Row `line` of a screen, holding `text` from column 0; with `link`, all of it is hyperlink `1`.
fn row(line: u64, text: &str, link: bool) -> serde_json::Value {
    let run = if link {
        serde_json::json!([0, text.len(), text, [0, -1, -1], 0])
    } else {
        serde_json::json!([0, text.len(), text, [0, -1, -1]])
    };
    serde_json::json!([line, {"w": false, "r": [run]}])
}

/// A frame committing revision `rev` of a `cols` by `rows` screen.
fn frame(rev: u64, (cols, rows): (u16, u16)) -> GridOutput {
    serde_json::from_value(
        serde_json::json!({"t":"frame","rev":rev,"cols":cols,"rows":rows,
        "cursor":{"x":0,"y":0,"visible":true,"shape":"block","blink":false}}),
    )
    .unwrap()
}

/// A ready file's answer.
fn ready(path: &str, size: u64) -> FileStat {
    FileStat {
        host: "build-box".into(),
        can_open: Some(true),
        info: DownloadFileInfo {
            path: path.into(),
            size: Some(size),
            status: DownloadFileStatus::Ready,
        },
    }
}

/// The tier names the status line can carry as its debug selector.
const TIERS: [&str; 5] = [
    "terminal-file-status-neutral",
    "terminal-file-status-small",
    "terminal-file-status-medium",
    "terminal-file-status-large",
    "terminal-file-status-danger",
];

impl Harness<'_> {
    /// Attach and replay a complete screen with `rows` (see [`row`]), whose hyperlink `1`, if any, goes to `link`.
    fn show(&mut self, rows: Vec<serde_json::Value>, link: Option<&str>) {
        let asked = self
            .stream
            .borrow_mut()
            .as_mut()
            .expect("the premise: the measured terminal opened its stream")
            .attach();
        let size = (asked.cols, asked.rows);
        self.size = size;
        let links = link.map_or(serde_json::json!([]), |uri| serde_json::json!([["1", uri]]));
        let messages = [
            serde_json::json!({"t":"hello","cols":size.0,"rows":size.1,"history_cap":0}),
            serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
                "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
            serde_json::json!({"t":"rows","rows":rows,"links":links}),
        ];
        {
            let mut stream = self.stream.borrow_mut();
            let stream = stream.as_mut().unwrap();
            for message in messages {
                stream.output(serde_json::from_value(message).unwrap());
            }
            stream.output(frame(1, size));
            stream.replay_complete();
        }
        settle(self.cx);
    }

    /// Live output: `rows` rewritten, committed as revision `rev`, with hyperlink `1` going to `link`.
    fn rewrite(&mut self, rows: Vec<serde_json::Value>, link: Option<&str>, rev: u64) {
        let links = link.map_or(serde_json::json!([]), |uri| serde_json::json!([["1", uri]]));
        let message = serde_json::json!({"t":"rows","rows":rows,"links":links});
        let size = self.size;
        {
            let mut stream = self.stream.borrow_mut();
            let stream = stream.as_mut().unwrap();
            stream.output(serde_json::from_value(message).unwrap());
            stream.output(frame(rev, size));
        }
        settle(self.cx);
        redraw(self.cx);
    }

    /// The middle of cell (`col`, `row`) in the window: the grid sits inside the pane's 8 pixels of padding, in 10 by
    /// 22 pixel cells.
    fn at(&mut self, col: u16, row: u16) -> Point<Pixels> {
        let grid = test_support::frame(self.cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("terminal-grid"))
            .expect("the premise: the terminal's grid is drawn")
            .bounds
            .origin;
        grid + point(
            px(8. + f32::from(col) * 10. + 1.),
            px(8. + (f32::from(row) + 0.5) * 22.),
        )
    }

    /// Rest the pointer on `at` and let the display open (it opens after the frame that saw the hover).
    fn rest_on(&mut self, at: Point<Pixels>) {
        self.cx.simulate_mouse_move(at, None, Modifiers::none());
        settle(self.cx);
        redraw(self.cx);
    }

    /// A click at `at`, where the pointer already rests.
    fn click(&mut self, at: Point<Pixels>) {
        self.cx
            .simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        self.cx
            .simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        settle(self.cx);
        redraw(self.cx);
    }

    /// The checks the terminal started since the last call, by the path each asked about, all for session `s1`.
    fn checks(&mut self) -> Vec<(String, FakeAnswer<FileStat>)> {
        self.transfers
            .take()
            .into_iter()
            .map(|transfer| match transfer {
                FakeTransfer::Stat {
                    session,
                    path,
                    answer,
                } => {
                    assert_eq!(
                        session, "s1",
                        "a check asks about the terminal's own session"
                    );
                    (path, answer)
                }
                other => panic!("only checks go out on a hover: {other:?}"),
            })
            .collect()
    }

    /// The one check started since the last call, which must have asked about `path`.
    fn check(&mut self, path: &str) -> FakeAnswer<FileStat> {
        let mut checks = self.checks();
        assert_eq!(checks.len(), 1, "one check went out");
        let (asked, answer) = checks.remove(0);
        assert_eq!(asked, path);
        answer
    }

    /// Give a check its answer and draw the result.
    fn answer(&mut self, answer: FakeAnswer<FileStat>, stat: FileStat) {
        answer.answer(Ok(stat));
        settle(self.cx);
        redraw(self.cx);
    }

    /// Whether the file display is drawn.
    fn shown(&mut self) -> bool {
        self.cx.debug_bounds(PANEL_ID).is_some()
    }

    /// The accessible name of the display's line drawn under `id`.
    fn line(&mut self, id: &str) -> Option<String> {
        test_support::label(self.cx, id)
    }

    /// The tier the status line shows, by its old `data-tier` name.
    fn tier(&mut self) -> Option<&'static str> {
        TIERS
            .into_iter()
            .find(|selector| self.cx.debug_bounds(selector).is_some())
            .map(|selector| selector.trim_start_matches("terminal-file-status-"))
    }

    /// The resolved paths of the clicks a check admitted so far.
    fn admitted(&mut self) -> Vec<String> {
        let owner = self.owner.clone();
        self.cx
            .update(|_, cx| owner.read(cx).files().read(cx).admitted.clone())
    }

    /// The refused click's explanation at the top of the terminal, if one shows.
    fn refusal(&mut self) -> Option<String> {
        test_support::label(self.cx, "terminal-link-refusal")
    }

    /// Whether the owner draws the pointing hand over the link the pointer rests on, as its render asks
    /// (`AttachedTerminal::points_at_link`; the test platform does not report the cursor it was asked for).
    fn pointer(&mut self) -> bool {
        let owner = self.owner.clone();
        self.cx.update(|_, cx| owner.read(cx).points_at_link(cx))
    }

    /// The program turns its mouse reporting to `mode` (`click`, or `off`).
    fn mouse_mode(&mut self, mode: &str) {
        let state = serde_json::json!({"t":"state","mouse":mode,"focus":false,"alt_screen":false,
            "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null});
        self.stream
            .borrow_mut()
            .as_mut()
            .unwrap()
            .output(serde_json::from_value(state).unwrap());
        settle(self.cx);
    }

    /// The Terminal settings' link choice (`intercept`, `both` or `program`), as the helm pushes it.
    fn link_click(&mut self, mode: &str) {
        self.helm.push(CollectionChanges {
            collection: Collection::Preferences,
            meta: test_support::meta(&[PREFERENCES_KEY]),
            upsert: vec![KeyedEntry {
                key: PREFERENCES_KEY.into(),
                value: serde_json::json!({ "terminal_link_click": mode }),
            }],
            remove: Vec::new(),
        });
        settle(self.cx);
    }

    /// The kinds of the mouse reports the program got since the last call.
    fn reports(&mut self) -> Vec<MouseEventKind> {
        self.stream
            .borrow_mut()
            .as_mut()
            .unwrap()
            .received()
            .into_iter()
            .filter_map(|request| match request {
                FakeTerminalRequest::Input {
                    input: GridInput::Mouse { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .collect()
    }

    /// Rest on `at`, the printed path `path`, and answer its check: a ready file of `size` bytes at `path`, resolved
    /// against `/work` when relative.
    fn checked(&mut self, at: Point<Pixels>, path: &str, size: u64) {
        self.rest_on(at);
        let answer = self.check(path);
        let resolved = if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/work/{path}")
        };
        self.answer(answer, ready(&resolved, size));
    }
}

// ---- Hover and check -------------------------------------------------------------------------------------------------

/// Hovering is the moment the user learns what a click would fetch, so the display must say it is checking at once and
/// then show exactly what the host found. Specified (SPEC.md: "Hover immediately underlines the path and shows a
/// checking spinner, then the host, full resolved path and size"; the old "hover preserves both path pieces", and the
/// e2e "file hover and complete browser download" up to its click): resting on a printed relative path asks the
/// session's host about it as printed and shows "Checking…" with the path, neutral; the answer shows the file's name,
/// `host:directory` of the resolved path, its decimal size and the hints; a click then is admitted for the resolved
/// path.
#[gpui::test]
fn a_hover_shows_checking_then_the_hosts_answer_and_a_click_is_admitted(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "see notes.txt now", false)], None);
    let on = h.at(5, 0);
    h.rest_on(on);
    let answer = h.check("notes.txt");
    assert!(h.shown(), "the display shows while checking");
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
    assert_eq!(h.line("terminal-file-path").as_deref(), Some("notes.txt"));
    assert_eq!(h.tier(), None, "checking has no size tier");

    h.answer(answer, ready("/work/notes.txt", 4800));
    assert_eq!(h.line("terminal-file-name").as_deref(), Some("notes.txt"));
    assert_eq!(
        h.line("terminal-file-directory").as_deref(),
        Some("build-box:/work/")
    );
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("4.8 KB"));
    let hint = h.line("terminal-file-hint").unwrap();
    assert!(hint.starts_with(" · click to download · "), "{hint:?}");
    assert_eq!(h.tier(), Some("small"));

    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
    assert_eq!(h.refusal(), None);
}

/// Evidence belongs to one hover: a check answered for an earlier hover must not stand for a later one, and a pending
/// check must not outlive the pointer (SPEC.md: "Each hover checks afresh ... Leaving cancels it"). Specified (the old
/// "real departure aborts and hides, and a later same-path hover checks afresh", and the e2e "leaving cancels lookup
/// and the next hover checks again"): leaving while checking abandons the request and hides the display; the next
/// hover on the same path asks again, and its answer shows.
#[gpui::test]
fn each_hover_checks_afresh_and_leaving_cancels(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "fresh.txt", false)], None);
    let (on, off) = (h.at(2, 0), h.at(40, 5));
    h.rest_on(on);
    let first = h.check("fresh.txt");
    h.rest_on(off);
    assert!(first.abandoned(), "leaving dropped the check");
    assert!(!h.shown(), "and its display");

    h.rest_on(on);
    let second = h.check("fresh.txt");
    assert!(!second.abandoned());
    h.answer(second, ready("/work/fresh.txt", 8));
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("8 B"));
}

/// A program can change the text over the same cells and keep the old target, so a click must be judged on the text
/// the check was shown for, never on a cached target. Specified (SPEC.md: "A partial redraw that changes the link text
/// withdraws transfer and opening permission even if the terminal library retains its old target"; the e2e "changed
/// OSC display text starts a fresh hover at the same cells", and the old "changed and unknown OSC display text cancels
/// old evidence" and "partial wrapped redraw invalidates file transfer evidence"): a `file://` hyperlink relabelled over
/// the same cells and target is a new hover with a fresh check; a click before that check answers is not admitted and
/// says nothing; once it answers, a click is admitted.
#[gpui::test]
fn a_relabel_withdraws_the_click_until_checked_again(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let target = "file:///work/report.txt";
    h.show(vec![row(0, "OLD-LABEL", true)], Some(target));
    let on = h.at(3, 0);
    h.rest_on(on);
    let first = h.check("/work/report.txt");
    h.answer(first, ready("/work/report.txt", 17));
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("17 B"));

    h.rewrite(vec![row(0, "NEW-LABEL", true)], Some(target), 2);
    let second = h.check("/work/report.txt");
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
    h.click(on);
    assert_eq!(
        h.admitted(),
        Vec::<String>::new(),
        "the relabel withdrew the click"
    );
    assert_eq!(h.refusal(), None, "and a click while checking says nothing");
    h.answer(second, ready("/work/report.txt", 17));
    h.click(on);
    assert_eq!(h.admitted(), ["/work/report.txt"]);
}

// ---- Tiers and refusals ----------------------------------------------------------------------------------------------

/// A refused file must stay unclickable, and the display must say why in the old words, since that is all the user
/// gets. Specified (SPEC.md: "Files must be readable regular files no larger than 100 MB ... directories are refused",
/// and "refusals are red"; the refusal half of the e2e "file refusals, OSC file save and forbidden OSC schemes", and the
/// cap half of "file hover shows decimal size tiers and a red cap refusal"): a missing file, a folder, an unreadable
/// file, something that is not a regular file and a file one byte over the limit each show their refusal in red with
/// no hints, and a click on each is neither admitted nor explained; output on another row keeps the refused hover (the
/// refused half of the old "same-task redraw preserves answered ready and refused hovers"); a check the helm itself
/// refused shows the helm's words under the red stripe, admitting nothing.
#[gpui::test]
fn folders_unreadable_and_other_refusals_say_why_and_admit_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    use DownloadFileStatus::{Folder, NotFound, NotReadable, NotRegular, TooLarge};
    let cases = [
        ("missing.txt", NotFound, 0, "Not found"),
        ("folder.dir", Folder, 0, "Folder: not downloadable"),
        ("private.txt", NotReadable, 0, "Not readable"),
        (
            "fifo.sock",
            NotRegular,
            0,
            "Not a regular file: not downloadable",
        ),
        (
            "big.bin",
            TooLarge,
            100_000_001,
            "Too large to download: 100 MB (100 MB limit)",
        ),
    ];
    let mut rows: Vec<_> = (0u64..)
        .zip(cases.clone())
        .map(|(line, (name, ..))| row(line, name, false))
        .collect();
    rows.push(row(5, "gone.txt", false));
    h.show(rows, None);
    for (row, (name, status, size, words)) in (0u16..).zip(cases) {
        let at = h.at(1, row);
        h.rest_on(at);
        let answer = h.check(name);
        let mut stat = ready(&format!("/work/{name}"), size);
        stat.info.status = status;
        h.answer(answer, stat);
        assert_eq!(h.line("terminal-file-status").as_deref(), Some(words));
        assert_eq!(h.tier(), Some("danger"), "{name}");
        assert_eq!(
            h.line("terminal-file-hint"),
            None,
            "{name}: a refusal offers nothing"
        );
        h.click(at);
        assert_eq!(h.admitted(), Vec::<String>::new(), "{name}");
        assert_eq!(h.refusal(), None, "{name}: the display already says why");
    }
    h.rewrite(vec![row(7, "other output", false)], None, 2);
    assert!(
        h.checks().is_empty(),
        "the same refused link is the same hover"
    );
    assert_eq!(h.tier(), Some("danger"));

    let at = h.at(1, 5);
    h.rest_on(at);
    let answer = h.check("gone.txt");
    answer.answer(Err(TransferError::Refused {
        status: 404,
        message: "no such session".into(),
    }));
    settle(h.cx);
    redraw(h.cx);
    assert_eq!(
        h.line("terminal-file-status").as_deref(),
        Some("no such session")
    );
    assert_eq!(h.tier(), Some("danger"));
    h.click(at);
    assert_eq!(h.admitted(), Vec::<String>::new());
}

// ---- Admission -------------------------------------------------------------------------------------------------------

/// The display is the whole safeguard, so a click must not transfer anything the user could not fully see, and the
/// refusal's advice must work. Specified (SPEC.md: "A click cannot transfer or open a file while the full hover
/// disclosure does not fit in the window ... the click is refused with a visible explanation"; the old "clipping refuses
/// transfer and shows failure after its own click"): with a ready check whose display ran past the window's bottom edge
/// when last drawn, a click is not admitted and the top of the terminal says the old words; hovering again, at another
/// cell of the path, checks afresh and a click then is admitted.
#[gpui::test]
fn a_click_whose_file_details_are_not_fully_visible_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at(3, 0);
    h.rest_on(on);
    let answer = h.check("/work/report.pdf");
    h.answer(answer, ready("/work/report.pdf", 2000));
    let owner = h.owner.clone();
    h.cx.update(|_, cx| {
        let files = owner.read(cx).files().read(cx);
        let drawn = files.hover.as_ref().unwrap().drawn.clone();
        let (bounds, window) = drawn.get().expect("the premise: the display was drawn");
        drawn.set(Some((
            Bounds::new(point(bounds.origin.x, window.height - px(4.)), bounds.size),
            window,
        )));
    });
    h.click(on);
    assert_eq!(h.admitted(), Vec::<String>::new(), "nothing is admitted");
    assert_eq!(
        h.refusal().as_deref(),
        Some(FILE_REFUSAL),
        "and the terminal says why"
    );
    assert!(!h.shown(), "the display does not sit over the explanation");

    let again = h.at(5, 0);
    h.rest_on(again);
    let answer = h.check("/work/report.pdf");
    h.answer(answer, ready("/work/report.pdf", 2000));
    h.click(again);
    assert_eq!(h.admitted(), ["/work/report.pdf"]);
}

/// A drag across a link must never download it (SPEC.md: "Dragging across a terminal link never ... downloads a
/// file"), while a hand's small jitter must still count as a click. Specified (the old "file transfers refuse gestures
/// the terminal did not admit as clicks"): over a ready check, a press dragged past the jitter and released is not
/// admitted, nor is one that wanders off and comes back; a press released 3 pixels away is.
#[gpui::test]
fn a_drag_across_a_checked_path_is_never_admitted(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "build/output.zip done", false)], None);
    let on = h.at(4, 0);
    h.rest_on(on);
    let answer = h.check("build/output.zip");
    h.answer(answer, ready("/work/build/output.zip", 5));
    let far = on + point(px(30.), px(0.));
    for back in [far, on] {
        assert_eq!(
            h.line("terminal-file-status").as_deref(),
            Some("5 B"),
            "the premise: the check is ready before the drag"
        );
        h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
        h.cx.simulate_mouse_move(far, MouseButton::Left, Modifiers::none());
        h.cx.simulate_mouse_move(back, MouseButton::Left, Modifiers::none());
        h.cx.simulate_mouse_up(back, MouseButton::Left, Modifiers::none());
        settle(h.cx);
        assert_eq!(h.admitted(), Vec::<String>::new(), "a drag is no click");
        // Resting on the path again; should the drag have left it, its check was dropped, and the new one is answered.
        h.rest_on(on);
        for (_, answer) in h.checks() {
            h.answer(answer, ready("/work/build/output.zip", 5));
        }
    }
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("5 B"));
    let nudged = on + point(px(3.), px(0.));
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(nudged, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(nudged, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(
        h.admitted(),
        ["/work/build/output.zip"],
        "jitter is still a click"
    );
}

// ---- What a click is judged on ---------------------------------------------------------------------------------------

/// A check vouches for the text it was shown for, so what counts as "the same link" is the whole of a click's safety:
/// too loose, and a click transfers a file the user never saw checked; too strict, and redraws withdraw good clicks.
/// Specified: a hover's ready check admits a link with the same cells, the same file and the same label, whatever its
/// hyperlink id or the rows around it it reads; another start or end cell, another file or another label is not
/// admitted, and nothing is while the check is not a ready file.
#[test]
fn ready_for_requires_the_same_cells_file_and_label() {
    use farhelm_grid_model::{Position, Selection, SelectionMode};
    let span = |from: u16, to: u16| {
        vec![Selection::new(
            Position { line: 4, col: from },
            Position { line: 4, col: to },
            SelectionMode::Linear,
        )]
    };
    let checked = TerminalLink {
        kind: LinkKind::Osc { id: "1".into() },
        uri: "file:///work/a.txt".into(),
        ranges: span(0, 6),
        evidence: [4].into_iter().collect(),
        label: "REPORT".into(),
    };
    let mut files = FileLinks::new("s1".into(), None);
    let hover = |check| Hover {
        link: checked.clone(),
        asked: HostText::from_host("/work/a.txt"),
        platform: Platform::Linux,
        drawn: Drawn::default(),
        check,
        _stat: None,
    };
    files.hover = Some(hover(Check::Answered(ready("/work/a.txt", 3))));
    let with = |change: &dyn Fn(&mut TerminalLink)| {
        let mut link = checked.clone();
        change(&mut link);
        link
    };
    assert!(files.ready_for(&checked));
    assert!(files.ready_for(&with(&|link| link.kind = LinkKind::Osc { id: "2".into() })));
    assert!(files.ready_for(&with(
        &|link| link.evidence = [3, 4, 5].into_iter().collect()
    )));
    assert!(
        !files.ready_for(&with(&|link| link.ranges = span(1, 6))),
        "another start"
    );
    assert!(
        !files.ready_for(&with(&|link| link.ranges = span(0, 7))),
        "another end"
    );
    assert!(
        !files.ready_for(&with(&|link| link.uri = "file:///work/b.txt".into())),
        "another file"
    );
    assert!(
        !files.ready_for(&with(&|link| link.label = "REP0RT".into())),
        "another label"
    );
    let mut folder = ready("/work/a.txt", 3);
    folder.info.status = DownloadFileStatus::Folder;
    files.hover = Some(hover(Check::Answered(folder)));
    assert!(!files.ready_for(&checked), "a refusal admits nothing");
    files.hover = Some(hover(Check::Checking));
    assert!(!files.ready_for(&checked), "nor does a check in flight");
}

/// Output under a resting pointer is the everyday case: a redraw that leaves the path's text as it was must keep the
/// check (or every prompt redraw would restart it and refuse clicks), while one that changes the text over the same
/// cells must withdraw it (SPEC.md: "A partial redraw that changes the link text withdraws transfer and opening
/// permission"). Specified: with the pointer resting on a checked printed path, rewriting its own row with the same text,
/// then the row below, starts no check and a click is admitted each time; rewriting its cells with another path, with
/// the pointer still, starts a check of the new path, shows it checking without the pointing hand, and a click is not
/// admitted and says nothing.
#[gpui::test]
fn a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.show(
        vec![row(0, "notes.txt", false), row(1, "other", false)],
        None,
    );
    let on = h.at(2, 0);
    h.checked(on, "notes.txt", 10);
    h.rewrite(vec![row(0, "notes.txt", false)], None, 2);
    assert!(
        h.checks().is_empty(),
        "the same text on its own row is the same hover"
    );
    h.click(on);
    h.rewrite(vec![row(1, "other", false)], None, 3);
    assert!(
        h.checks().is_empty(),
        "and so is the same text on the row below"
    );
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt", "/work/notes.txt"]);

    h.rewrite(vec![row(0, "plans.txt", false)], None, 4);
    let fresh = h.check("plans.txt");
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
    assert!(
        !h.pointer(),
        "no pointing hand until the new text is checked"
    );
    h.click(on);
    assert_eq!(h.admitted().len(), 2, "the relabel withdrew the click");
    assert_eq!(h.refusal(), None);
    drop(fresh);
}

/// The press and release find the link again from the live text, so output that changes the path between the last
/// frame and the click must decide the click, not the display still on screen. Specified: over a checked printed path,
/// output rewriting its cells with another path is delivered and a click follows at once, with no frame asked for in
/// between; the click is not admitted.
#[gpui::test]
fn a_click_right_after_output_changed_the_path_is_judged_on_the_new_text(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at(2, 0);
    h.checked(on, "notes.txt", 10);
    let size = h.size;
    {
        let mut stream = h.stream.borrow_mut();
        let stream = stream.as_mut().unwrap();
        let rows = serde_json::json!({"t":"rows","rows":[row(0, "plans.txt", false)],"links":[]});
        stream.output(serde_json::from_value(rows).unwrap());
        stream.output(frame(2, size));
    }
    h.cx.run_until_parked();
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.admitted(), Vec::<String>::new());
    drop(h.checks());
}

/// A check's answer changes what the display shows, and until the new display has painted, the user has not seen it:
/// where the display was drawn while it said "Checking…" says nothing about the answer. Specified: an answer arriving
/// leaves the display with no drawn position; a click pressed before the next frame is not admitted and is refused with
/// the explanation; hovering again, once the answer has painted, a click is admitted.
#[gpui::test]
fn a_click_before_the_answer_has_painted_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at(3, 0);
    h.rest_on(on);
    let pending = h.check("/work/report.pdf");
    // The answer, the press and the release in one update of the window, so no frame can come between them. Through
    // the test context's simulated events, a first version of this test saw the answered display drawn before the
    // press, which then was admitted, as it should be once the answer is on screen.
    let owner = h.owner.clone();
    h.cx.update(|window, cx| {
        let files = owner.read(cx).files().clone();
        files.update(cx, |files, cx| {
            files.answered(Ok(ready("/work/report.pdf", 2000)), cx);
            let drawn = &files.hover.as_ref().unwrap().drawn;
            assert_eq!(
                drawn.get(),
                None,
                "the answer forgot where the checking display was drawn"
            );
        });
        let down = MouseDownEvent {
            button: MouseButton::Left,
            position: on,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        };
        let up = MouseUpEvent {
            button: MouseButton::Left,
            position: on,
            modifiers: Modifiers::none(),
            click_count: 1,
        };
        window.dispatch_event(PlatformInput::MouseDown(down), cx);
        window.dispatch_event(PlatformInput::MouseUp(up), cx);
    });
    settle(h.cx);
    assert_eq!(h.admitted(), Vec::<String>::new());
    assert_eq!(h.refusal().as_deref(), Some(FILE_REFUSAL));
    drop(pending);

    let again = h.at(5, 0);
    h.checked(again, "/work/report.pdf", 2000);
    h.click(again);
    assert_eq!(h.admitted(), ["/work/report.pdf"]);
}

/// A click is judged on what the user pressed over: a press while the display still said "Checking…" was not made over
/// a disclosure, so it does nothing, even when the answer lands before the release, and it must not claim the display
/// was hidden either. Specified: resting on a printed path, a press while checking, the check's ready answer, then the
/// release: nothing is admitted and no explanation shows.
#[gpui::test]
fn an_answer_landing_between_press_and_release_leaves_the_click_silent(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at(2, 0);
    h.rest_on(on);
    let answer = h.check("notes.txt");
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    answer.answer(Ok(ready("/work/notes.txt", 10)));
    settle(h.cx);
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.admitted(), Vec::<String>::new());
    assert_eq!(h.refusal(), None);
}

/// SPEC.md: "Only a successful check ... admits a click (OSC hyperlinks may show a pointer earlier)", and the old app
/// gave a printed path the pointing hand only once its check was ready, so the hand never promises a click that would
/// do nothing. Specified: on a printed path, no hand while checking or after a refusal, and the hand once the file is
/// ready; on a program's `file://` hyperlink, the hand at once.
#[gpui::test]
fn the_pointing_hand_waits_for_a_ready_check_on_a_printed_path(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(
        vec![
            row(0, "notes.txt", false),
            row(1, "REPORT", true),
            row(2, "folder.dir", false),
        ],
        Some("file:///work/report.txt"),
    );
    let printed = h.at(2, 0);
    h.rest_on(printed);
    let answer = h.check("notes.txt");
    assert!(!h.pointer(), "checking");
    h.answer(answer, ready("/work/notes.txt", 10));
    assert!(h.pointer(), "ready");

    let folder = h.at(2, 2);
    h.rest_on(folder);
    let answer = h.check("folder.dir");
    let mut refused = ready("/work/folder.dir", 0);
    refused.info.status = DownloadFileStatus::Folder;
    h.answer(answer, refused);
    assert!(!h.pointer(), "refused");

    let hyperlink = h.at(2, 1);
    h.rest_on(hyperlink);
    let _checking = h.check("/work/report.txt");
    assert!(
        h.pointer(),
        "a program's hyperlink shows the hand while checking"
    );
}

// ---- Where file clicks differ from web links -------------------------------------------------------------------------

/// The old app never withheld a press on a file link from a program under mouse reporting: only web links could be
/// withheld (terminal.js at the reference commit passes `link.web` to `interceptLinkPress`), and its release still
/// downloaded the file. Porting that, not redesigning it, means a program and Farhelm both get the click. Specified:
/// with the program reporting clicks, a click on a checked printed path is admitted, and the program hears the press and
/// the release.
#[gpui::test]
fn under_mouse_reporting_a_file_click_is_admitted_and_still_reported(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    h.mouse_mode("click");
    let on = h.at(2, 0);
    h.checked(on, "notes.txt", 10);
    drop(h.reports());
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
    let reports = h.reports();
    assert!(
        reports.contains(&MouseEventKind::Down) && reports.contains(&MouseEventKind::Up),
        "the program heard the press and the release: {reports:?}"
    );
}

/// The Terminal settings' link choice is about web links (SPEC.md: it lets Farhelm open "their http(s) target" or not),
/// and the old app's file links downloaded without reading it (terminal.js at the reference commit: only `activateWeb`
/// passes the choice on). Specified: with the choice set to leave clicks to the program, a click on a checked printed
/// path is still admitted.
#[gpui::test]
fn a_file_click_is_farhelms_whatever_the_link_choice(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    h.link_click("program");
    let choice = h.cx.update(|_, cx| {
        session::current_store(cx).and_then(|store| {
            let collections = store.read(cx).collections();
            let preferences = collections.get::<Preferences>(PREFERENCES_KEY)?;
            preferences.terminal_link_click.clone()
        })
    });
    assert_eq!(
        choice.as_deref(),
        Some("program"),
        "the premise: clicks are left to the program"
    );
    let on = h.at(2, 0);
    h.checked(on, "notes.txt", 10);
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
}

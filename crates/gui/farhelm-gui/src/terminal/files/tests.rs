//! The file links' contract, against the helm client's fake terminal and fake transfers: what a hover on a printed path
//! asks the host, what becomes visible when the file is ready, when a hover counts as new, and which clicks
//! a check admits. A program's `file://` hyperlink is no file link (it is inert, which
//! `attached::tests::a_hyperlink_that_is_not_a_web_address_is_inert` pins), so it appears here only to show it asks
//! nothing. One test per sentence of SPEC.md's file-path bullet in this
//! scope, ported from the old app's `terminal-files.test.js` and `e2e/tests/terminal-files.spec.ts` (each test names
//! the cases it replaces).
//!
//! The terminal is a real attached owner in a window from the shared harness, so the display opens through the layer
//! manager as in the app; the fake transfers hold each check's answer until the test gives it, which is how a test sees
//! the checking state and whether leaving abandoned a check.

use std::ops::{Deref, DerefMut};
use std::time::Duration;

use farhelm_grid_model::LinkKind;
use farhelm_helm_client::fake::transfers::{FakeAnswer, FakeTransfer};
use farhelm_helm_client::{OpenOutcome, SavedDownload};
use farhelm_proto::grid::{GridInput, MouseEventKind};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    TestAppContext, point, px,
};

use super::*;
use crate::session;
use crate::terminal::attached;
use crate::terminal::attached::tests::{FITTED, Harness, frame, mouse_mode, resizes};
use crate::test_support::{self, advance, redraw, settle};
use crate::theme;

// ---- Pure rules ------------------------------------------------------------------------------------------------------

/// Sizes are what the user weighs before a transfer, and their units must agree with the host's decimal 100 MB limit:
/// a file just over it must never read as under "100 MB". Specified (the old "file sizes and tiers use decimal
/// boundaries including the exact cap"): whole bytes under 1000, then one decimal with `.0` dropped; the tiers switch
/// at exactly 1 MB and 10 MB, and the limit itself is still large.
#[test]
fn sizes_and_tiers_use_decimal_boundaries_including_the_exact_limit() {
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
        assert_eq!(SizeTier::of(Some(bytes)), tier, "{bytes} bytes");
    }
    assert_eq!(format_size(100_000_001), "100 MB");
    assert_eq!(format_size(2_500_000_000), "2.5 GB");
    // Half up on the exact quotient, where the old `toFixed` read the float just below it as "1.1 KB".
    assert_eq!(format_size(1150), "1.2 KB");
    assert_eq!(SizeTier::of(None), SizeTier::Neutral);
    for palette in [&theme::DARK, &theme::LIGHT] {
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
            "an unknown size is neutral"
        );
    }
}

/// The icon tells kinds of file apart and claims nothing about opening: an unknown extension stays generic. Specified
/// by case-insensitive extensions, with the last one deciding.
#[test]
fn icons_distinguish_documents_pdfs_images_and_archives() {
    assert_eq!(file_icon("/work/notes.MD"), '\u{f15c}');
    assert_eq!(file_icon("/work/main.rs"), '\u{f15c}');
    assert_eq!(file_icon("/work/report.pdf"), '\u{f1c1}');
    assert_eq!(file_icon("/work/shot.png"), '\u{f1c5}');
    assert_eq!(file_icon("/work/files.tar.gz"), '\u{f1c6}');
    assert_eq!(file_icon("/work/notes.txt.command"), '\u{f15b}');
}

/// Only the helm decides whether a modifier-click may open a file, and the hint must name this platform's modifier.
/// Specified (the hint half of the old "hover advertises open only for native admitted types"): a ready file always
/// says a click downloads; the opening hint appears only when the helm offered opening (`can_open`), as ⌘ on a Mac and
/// Ctrl elsewhere.
#[test]
fn hints_offer_opening_only_where_the_helm_does_with_the_platforms_modifier() {
    // Over the copy cap, so that only the opening hint varies here; the copy hint has its own test below.
    let stat = |can_open| FileStat {
        can_open,
        ..ready("/work/notes.txt", 10_000_001)
    };
    for (can_open, platform, hint) in [
        (
            Some(true),
            Platform::Mac,
            " · click to download · ⌘-click to open",
        ),
        (
            Some(true),
            Platform::Linux,
            " · click to download · Ctrl-click to open",
        ),
        (Some(false), Platform::Mac, " · click to download"),
        (None, Platform::Linux, " · click to download"),
    ] {
        assert_eq!(hints(&stat(can_open), platform), hint);
    }
}

/// The copy hint promises a copy that can work, so it must not appear where the click is refused before reading:
/// over the 10 MB cap, or for an image on Linux, which copies text only. It must not be withheld from a file whose
/// contents only the click can judge (a binary under the cap is refused then, loudly). The maintainer asked for the
/// words "copy contents", not just "copy". Specified: a file of exactly the cap (10,000,000 bytes) gets the hint and
/// one byte more does not; a file of unknown size does not, since the hover cannot promise it fits; on a Mac an image
/// gets it and on Linux it does not, while text, an SVG and an unknown binary do on both; the modifier is ⇧⌘ on a Mac
/// and Shift-Ctrl elsewhere, after the opening hint.
#[test]
fn the_copy_hint_appears_only_where_a_copy_can_be_attempted() {
    let stat = |path: &str, size: Option<u64>| FileStat {
        can_open: Some(false),
        info: DownloadFileInfo {
            path: path.into(),
            size,
            status: DownloadFileStatus::Ready,
        },
        ..ready(path, 0)
    };
    let mac = " · click to download · ⇧⌘-click to copy contents";
    let linux = " · click to download · Shift-Ctrl-click to copy contents";
    let none = " · click to download";
    for (path, size, platform, hint) in [
        ("/w/a.txt", Some(10_000_000), Platform::Mac, mac),
        ("/w/a.txt", Some(10_000_001), Platform::Mac, none),
        ("/w/a.txt", None, Platform::Mac, none),
        ("/w/a.txt", Some(0), Platform::Linux, linux),
        ("/w/shot.PNG", Some(10), Platform::Mac, mac),
        ("/w/shot.PNG", Some(10), Platform::Linux, none),
        ("/w/logo.svg", Some(10), Platform::Linux, linux),
        ("/w/data.bin", Some(10), Platform::Linux, linux),
    ] {
        assert_eq!(hints(&stat(path, size), platform), hint, "{path} {size:?}");
    }
    let open = FileStat {
        can_open: Some(true),
        ..stat("/w/a.txt", Some(10))
    };
    assert_eq!(
        hints(&open, Platform::Mac),
        " · click to download · ⌘-click to open · ⇧⌘-click to copy contents"
    );
}

/// A click on a file has three outcomes, and each chord must mean one of them and nothing else: copying reads a file
/// into the clipboard, so it takes exactly Shift and the opening modifier, while opening keeps ignoring other modifiers
/// as the old app did, except Shift, which now means a copy. Specified: on each platform a plain click and Shift alone
/// download; the opening modifier (Cmd on a Mac, Ctrl elsewhere), alone or with Option/Alt, opens; that modifier with
/// Shift copies; with Shift and anything more (the other platform's modifier, Option/Alt, Fn) it downloads; and the
/// other platform's opening modifier with Shift does not copy.
#[test]
fn each_click_chord_names_exactly_one_action() {
    use ClickAction::{Copy, Download, Open};
    let none = Modifiers::none();
    let shift = Modifiers {
        shift: true,
        ..none
    };
    let alt = Modifiers { alt: true, ..none };
    let function = Modifiers {
        function: true,
        ..none
    };
    for platform in [Platform::Mac, Platform::Linux] {
        // `opening` adds this platform's opening modifier to a chord, and `other` the other platform's.
        let opening = |chord: Modifiers| match platform {
            Platform::Mac => Modifiers {
                platform: true,
                ..chord
            },
            Platform::Linux => Modifiers {
                control: true,
                ..chord
            },
        };
        let other = |chord: Modifiers| match platform {
            Platform::Mac => Modifiers {
                control: true,
                ..chord
            },
            Platform::Linux => Modifiers {
                platform: true,
                ..chord
            },
        };
        let with = |a: Modifiers, b: Modifiers| Modifiers {
            alt: a.alt || b.alt,
            function: a.function || b.function,
            ..a
        };
        for (modifiers, action) in [
            (none, Download),
            (shift, Download),
            (opening(none), Open),
            (opening(alt), Open),
            (opening(shift), Copy),
            (opening(with(shift, alt)), Download),
            (opening(with(shift, function)), Download),
            (other(opening(shift)), Download),
            (other(shift), Download),
        ] {
            assert_eq!(
                ClickAction::of(modifiers, platform),
                action,
                "{platform:?} {modifiers:?}"
            );
        }
    }
}

/// What reaches the clipboard is the maintainer's explicit list, so the classifier must take exactly it and refuse
/// the rest loudly, never guessing: an image extension decides the kind, so a mislabelled image is refused rather than
/// copied as text, and nothing but valid UTF-8 without NUL is text. Specified: text, text with escape sequences and an
/// SVG copy as text on both platforms, an empty file as empty text; each listed raster extension (any case) holding its
/// format's signature copies as that image on a Mac, and as nothing on Linux; a PNG name over text, an AVIF and a PDF
/// (binary) are refused, as are invalid UTF-8 and text holding a NUL byte.
#[test]
fn only_text_and_listed_images_reach_the_clipboard() {
    use copy::{Clip, classify};
    use gpui::{Image, ImageFormat};
    for platform in [Platform::Mac, Platform::Linux] {
        for (path, bytes) in [
            ("/w/notes.md", b"# notes\n".to_vec()),
            ("/w/build.log", b"\x1b[31merror\x1b[0m\n".to_vec()),
            (
                "/w/logo.svg",
                b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
            ),
            ("/w/empty.txt", Vec::new()),
            ("/w/caf\u{e9}.txt", "caf\u{e9}\n".as_bytes().to_vec()),
        ] {
            let text = String::from_utf8(bytes.clone()).unwrap();
            assert_eq!(
                classify(path, bytes, platform),
                Ok(Clip::Text(text)),
                "{path}"
            );
        }
        for (path, bytes) in [
            ("/w/shot.png", b"not a png at all".to_vec()),
            ("/w/photo.avif", b"\0\0\0\x1cftypavif".to_vec()),
            ("/w/doc.pdf", b"%PDF-1.7\n\xe2\xe3\xcf\xd3\n".to_vec()),
            ("/w/bad.txt", vec![0xff, 0xfe, b'a']),
            ("/w/nul.txt", b"a\0b".to_vec()),
        ] {
            assert!(
                classify(path, bytes, platform).is_err(),
                "{path} on {platform:?}"
            );
        }
    }
    let webp = [b"RIFF".as_slice(), b"\x10\0\0\0", b"WEBPVP8 "].concat();
    for (path, format, bytes) in [
        (
            "/w/a.png",
            ImageFormat::Png,
            b"\x89PNG\r\n\x1a\n....".to_vec(),
        ),
        ("/w/a.JPG", ImageFormat::Jpeg, b"\xff\xd8\xff\xe0".to_vec()),
        ("/w/a.jpeg", ImageFormat::Jpeg, b"\xff\xd8\xff\xdb".to_vec()),
        ("/w/a.webp", ImageFormat::Webp, webp),
        ("/w/a.gif", ImageFormat::Gif, b"GIF89a..".to_vec()),
        ("/w/a.bmp", ImageFormat::Bmp, b"BM......".to_vec()),
        ("/w/a.tif", ImageFormat::Tiff, b"II*\0....".to_vec()),
        ("/w/a.tiff", ImageFormat::Tiff, b"MM\0*....".to_vec()),
        ("/w/a.ico", ImageFormat::Ico, b"\0\0\x01\0..".to_vec()),
    ] {
        assert_eq!(
            classify(path, bytes.clone(), Platform::Mac),
            Ok(Clip::Image(Image::from_bytes(format, bytes.clone()))),
            "{path}"
        );
        assert!(
            classify(path, bytes, Platform::Linux).is_err(),
            "{path} on Linux"
        );
    }
    // A JPEG's bytes under a PNG's name are refused: the extension, not a sniff, says what the file must be.
    assert!(classify("/w/a.png", b"\xff\xd8\xff\xe0".to_vec(), Platform::Mac).is_err());
}

/// A copy that cannot succeed must fail at once with words, before any bytes cross from the host. Specified: on Linux
/// an image is refused before reading; a file the hover measured over 10,000,000 bytes is refused, naming 10 MB; a
/// file of exactly the cap, and one of unknown size (whose read still enforces the cap), may be read.
#[test]
fn hopeless_copies_are_refused_before_reading() {
    use copy::refused_before_reading as refused;
    assert!(refused("/w/a.png", Some(10), Platform::Linux).is_some());
    assert_eq!(refused("/w/a.png", Some(10), Platform::Mac), None);
    assert_eq!(
        refused("/w/a.txt", Some(10_000_001), Platform::Mac).as_deref(),
        Some("the file is larger than 10 MB")
    );
    assert_eq!(refused("/w/a.txt", Some(10_000_000), Platform::Linux), None);
    assert_eq!(refused("/w/a.txt", None, Platform::Mac), None);
}

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// One copy's read as the terminal asked for it: the path, the cap, and the answer it waits on until a test gives it.
type CopyRead = (String, usize, Option<FakeAnswer<Vec<u8>>>);

/// The terminal's own test harness ([`attached::tests::harness`]: an attached owner of session `s1` in a window, its
/// stream and its UI session's transfers the helm client's fakes), with the saves the terminal has asked for so far.
/// Steps the terminal's harness already has (resting on a cell, a click with modifiers, the link choice) are its own.
struct Files<'a> {
    h: Harness<'a>,
    /// Every save the terminal asked for so far, in order: the path, whether it asked to open the save, and the answer
    /// the save waits on (taken by [`Files::save`]).
    downloads: Vec<(String, bool, Option<FakeAnswer<SavedDownload>>)>,
    /// Every copy's read the terminal asked for so far, in order: the path, the cap it asked the client to hold, and
    /// the answer the read waits on (taken by [`Files::read_back`]).
    copies: Vec<CopyRead>,
}

impl<'a> Deref for Files<'a> {
    type Target = Harness<'a>;
    fn deref(&self) -> &Harness<'a> {
        &self.h
    }
}

impl<'a> DerefMut for Files<'a> {
    fn deref_mut(&mut self) -> &mut Harness<'a> {
        &mut self.h
    }
}

/// The terminal's harness, its stream opened (the pane has been measured) but not attached; [`Files::show`] attaches
/// it.
fn harness(cx: &mut TestAppContext) -> Files<'_> {
    Files {
        h: attached::tests::harness(cx),
        downloads: Vec::new(),
        copies: Vec::new(),
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
const TIERS: [&str; 4] = [
    "terminal-file-status-neutral",
    "terminal-file-status-small",
    "terminal-file-status-medium",
    "terminal-file-status-large",
];

impl Files<'_> {
    /// Attach and replay a complete screen with `rows` (see [`row`]), whose hyperlink `1`, if any, goes to `link`.
    /// The attach is the [`FITTED`] size, which [`Files::rewrite`] and the tests' layouts (a cut path's head four
    /// columns short of the edge) count on.
    fn show(&mut self, rows: Vec<serde_json::Value>, link: Option<&str>) {
        let links = link.map_or(serde_json::json!([]), |uri| serde_json::json!([["1", uri]]));
        self.attach_rows(serde_json::json!(rows), links);
        let asked = self
            .opened
            .borrow()
            .as_ref()
            .map(|(_, asked)| (asked.cols, asked.rows));
        assert_eq!(
            asked,
            Some(FITTED),
            "the premise: the terminal attached at the fitted size"
        );
    }

    /// Live output: `rows` rewritten, committed as revision `rev`, with hyperlink `1` going to `link`.
    fn rewrite(&mut self, rows: Vec<serde_json::Value>, link: Option<&str>, rev: u64) {
        let links = link.map_or(serde_json::json!([]), |uri| serde_json::json!([["1", uri]]));
        let message = serde_json::json!({"t":"rows","rows":rows,"links":links});
        self.fake().output(serde_json::from_value(message).unwrap());
        let frame = frame(rev, FITTED.0, FITTED.1);
        self.fake().output(serde_json::from_value(frame).unwrap());
        settle(self.cx);
        redraw(self.cx);
    }

    /// A plain click at `at`, where the pointer already rests, and the frame after it, in which a display a press
    /// closed comes back.
    fn click(&mut self, at: Point<Pixels>) {
        self.click_with(at, Modifiers::none());
    }

    /// [`Files::click`] with `modifiers` held: the terminal harness's press and release, and the frame after them.
    fn click_with(&mut self, at: Point<Pixels>, modifiers: Modifiers) {
        self.h.click_with(at, modifiers);
        redraw(self.cx);
    }

    /// The checks the terminal started since the last call, by the path each asked about, all for session `s1`. Saves
    /// and copies it started meanwhile are kept in [`Files::downloads`] and [`Files::copies`].
    fn checks(&mut self) -> Vec<(String, FakeAnswer<FileStat>)> {
        let mut checks = Vec::new();
        for transfer in self.transfers.take() {
            match transfer {
                FakeTransfer::Stat {
                    session,
                    path,
                    answer,
                } => {
                    assert_eq!(
                        session, "s1",
                        "a check asks about the terminal's own session"
                    );
                    checks.push((path, answer));
                }
                FakeTransfer::Download {
                    session,
                    path,
                    open,
                    answer,
                } => {
                    assert_eq!(session, "s1", "a save asks the terminal's own session");
                    self.downloads.push((path, open, Some(answer)));
                }
                FakeTransfer::DownloadBytes {
                    session,
                    path,
                    max_bytes,
                    answer,
                } => {
                    assert_eq!(session, "s1", "a copy asks the terminal's own session");
                    self.copies.push((path, max_bytes, Some(answer)));
                }
                other => panic!("a file link starts checks, saves and copies only: {other:?}"),
            }
        }
        checks
    }

    /// Answer the latest save the terminal asked for, which must not have been answered yet, and draw the outcome. No
    /// check may be in flight: collecting the saves takes the checks too.
    fn save(&mut self, result: Result<SavedDownload, TransferError>) {
        assert!(
            self.checks().is_empty(),
            "the premise: no check is in flight"
        );
        let answer = self
            .downloads
            .last_mut()
            .and_then(|(_, _, answer)| answer.take())
            .expect("the premise: a save waits for its answer");
        answer.answer(result);
        settle(self.cx);
        redraw(self.cx);
    }

    /// The status line at the top of the terminal, as it reads.
    fn status(&mut self) -> Option<String> {
        test_support::label(self.cx, "terminal-file-message-line")
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

    /// The resolved paths of the clicks a check admitted so far: the saves the terminal asked the helm for.
    fn admitted(&mut self) -> Vec<String> {
        // A check drained here would be dropped, which answers it as a lost connection.
        assert!(
            self.checks().is_empty(),
            "the premise: no check is in flight"
        );
        self.downloads
            .iter()
            .map(|(path, ..)| path.clone())
            .collect()
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

    /// Inspect all three affordances together at an intentionally pending or unsuccessful check boundary.
    fn assert_silent_hover(&mut self) {
        assert!(!self.shown(), "no file disclosure");
        assert!(!self.pointer(), "no pointing hand");
        let owner = self.owner.clone();
        assert_eq!(
            self.cx.update(|_, cx| owner.read(cx).underlined_rows(cx)),
            []
        );
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

/// A printed path checks silently. Once ready, the underline and pointing hand accompany the three-line disclosure,
/// which gives the user the resolved path they are about to download.
#[gpui::test]
fn a_hover_is_silent_until_the_host_confirms_the_file_is_ready(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "see notes.txt now", false)], None);
    let on = h.at_cell(5, 0);
    h.rest_on(on);
    let answer = h.check("notes.txt");
    h.assert_silent_hover();

    h.answer(answer, ready("/work/notes.txt", 4800));
    assert!(h.shown());
    assert!(h.pointer());
    let owner = h.owner.clone();
    assert_eq!(
        h.cx.update(|_, cx| owner.read(cx).underlined_rows(cx)),
        [(0, 4..13)]
    );
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
    let (on, off) = (h.at_cell(2, 0), h.at_cell(40, 5));
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

// ---- Tiers and refusals ----------------------------------------------------------------------------------------------

/// Failed checks and non-downloadable paths leave no affordance or notice. A redraw that preserves a settled check
/// must also stay quiet on the wire, since a resting pointer can sit over a refused path through many frames.
#[gpui::test]
fn unsuccessful_file_checks_stay_silent_and_admit_nothing(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    use DownloadFileStatus::{Folder, NotFound, NotReadable, NotRegular, TooLarge};
    let cases = [
        ("missing.txt", NotFound, 0),
        ("folder.dir", Folder, 0),
        ("private.txt", NotReadable, 0),
        ("fifo.sock", NotRegular, 0),
        ("big.bin", TooLarge, 100_000_001),
    ];
    let mut rows: Vec<_> = (0u64..)
        .zip(cases.clone())
        .map(|(line, (name, ..))| row(line, name, false))
        .collect();
    rows.push(row(5, "gone.txt", false));
    h.show(rows, None);
    for (row, (name, status, size)) in (0u16..).zip(cases) {
        let at = h.at_cell(1, row);
        h.rest_on(at);
        let answer = h.check(name);
        let mut stat = ready(&format!("/work/{name}"), size);
        stat.info.status = status;
        h.answer(answer, stat);
        h.assert_silent_hover();
        h.click(at);
        assert_eq!(h.admitted(), Vec::<String>::new(), "{name}");
        assert_eq!(h.refusal(), None, "{name}");
    }
    h.rewrite(vec![row(7, "other output", false)], None, 2);
    assert!(
        h.checks().is_empty(),
        "the same refused link is the same hover"
    );
    h.assert_silent_hover();

    let at = h.at_cell(1, 5);
    h.rest_on(at);
    let answer = h.check("gone.txt");
    answer.answer(Err(TransferError::Lost("connection reset".into())));
    settle(h.cx);
    redraw(h.cx);
    h.assert_silent_hover();
    h.click(at);
    assert_eq!(h.admitted(), Vec::<String>::new());
    assert_eq!(h.refusal(), None);
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
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let answer = h.check("/work/report.pdf");
    h.answer(answer, ready("/work/report.pdf", 2000));
    h.stage_off_window();
    h.click(on);
    assert_eq!(h.admitted(), Vec::<String>::new(), "nothing is admitted");
    assert_eq!(
        h.refusal().as_deref(),
        Some(FILE_REFUSAL),
        "and the terminal says why"
    );
    assert!(!h.shown(), "the display does not sit over the explanation");

    let again = h.at_cell(5, 0);
    h.rest_on(again);
    let answer = h.check("/work/report.pdf");
    h.answer(answer, ready("/work/report.pdf", 2000));
    h.click(again);
    assert_eq!(h.admitted(), ["/work/report.pdf"]);
}

/// A file panel held back over its own pointer still explains a refused transfer and replaces any earlier result.
/// Rewriting its path starts a new check, even if output later puts the original text back under the resting pointer.
#[gpui::test]
fn a_file_panel_held_back_over_its_pointer_explains_the_refusal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.cx.simulate_resize(gpui::size(px(520.), px(300.)));
    h.show(
        vec![
            row(0, "/work/report.pdf", false),
            row(1, "notes.txt", false),
        ],
        None,
    );
    let previous = h.at_cell(3, 1);
    h.checked(previous, "notes.txt", 10);
    h.click(previous);
    h.save(Ok(saved(
        "/home/u/Downloads/notes.txt",
        OpenOutcome::NotAsked,
    )));
    assert!(h.status().is_some(), "an earlier save is still on screen");
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let answer = h.check("/work/report.pdf");
    let resolved = format!("/work/{}report.pdf", "deep/".repeat(420));
    h.answer(answer, ready(&resolved, 2000));
    let owner = h.owner.clone();
    assert!(
        h.cx.update(|_, cx| owner.read(cx).file_disclosure_held_back("/work/report.pdf")),
        "the panel painted over the pointer and was held back"
    );
    assert!(!h.shown(), "the file panel is gone");
    h.click(on);
    assert_eq!(
        h.admitted(),
        ["/work/notes.txt"],
        "no new transfer was admitted"
    );
    assert_eq!(h.refusal().as_deref(), Some(FILE_REFUSAL));

    h.rewrite(vec![row(0, "/work/plans.pdf", false)], None, 2);
    let intervening = h.check("/work/plans.pdf");
    h.rewrite(vec![row(0, "/work/report.pdf", false)], None, 3);
    assert!(intervening.abandoned(), "the old hover's check was retired");
    let pending = h.check("/work/report.pdf");
    h.click(on);
    assert!(
        !pending.abandoned(),
        "the replacement check is still outstanding"
    );
    assert_eq!(h.refusal(), None, "the pending check stays silent");
    assert_eq!(h.status(), None, "the earlier save does not come back");
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
}

/// A covering popover may disappear on mouse down. The release preserves the frame's refusal even when the blocker
/// overlaps only the stripe of an otherwise visible file panel.
#[gpui::test]
fn a_covered_file_panel_refuses_the_transfer_even_if_the_press_dismisses_the_cover(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.show(vec![row(0, "report.pdf", false)], None);
    let on = h.at_cell(3, 0);
    h.checked(on, "report.pdf", 2000);
    assert!(h.pointer(), "the host check found a ready file");
    let panel = test_support::bounds(h.cx, "terminal-file-disclosure");
    h.cover_at(
        point(panel.left().ceil() - px(15.), panel.top().ceil()),
        false,
    );
    let cover = test_support::bounds(h.cx, "terminal-test-cover");
    assert!(panel.intersects(&cover) && !cover.contains(&on));
    assert!(
        cover.right() - panel.left() < px(3.),
        "only the stripe is covered"
    );
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert!(
        h.cx.debug_bounds("terminal-test-cover").is_none(),
        "the press closed the cover"
    );
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.admitted(), Vec::<String>::new());
    assert_eq!(
        h.refusal().as_deref(),
        Some(
            "cannot download: full file details are covered by another Farhelm surface; move the pointer and hover again"
        )
    );
}

/// A drag across a link must never download it (SPEC.md: "Dragging across a terminal link never ... downloads a
/// file"), while a hand's small jitter must still count as a click. Specified (the old "file transfers refuse gestures
/// the terminal did not admit as clicks"): over a ready check, a press dragged past the jitter and released is not
/// admitted, nor is one that wanders off and comes back; a press released 3 pixels away is.
#[gpui::test]
fn a_drag_across_a_checked_path_is_never_admitted(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "build/output.zip done", false)], None);
    let on = h.at_cell(4, 0);
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
/// Specified: a hover's ready check on a printed path admits a link with the same cells and the same path, whatever
/// rows around it it reads; another start or end cell, or other text over the same cells, is not admitted, and nothing
/// is while the check is not a ready file.
#[test]
fn ready_for_requires_the_same_cells_and_file() {
    use farhelm_grid_model::{Position, Selection, SelectionMode};
    let span = |from: u16, to: u16| {
        vec![Selection::new(
            Position { line: 4, col: from },
            Position { line: 4, col: to },
            SelectionMode::Linear,
        )]
    };
    let checked = TerminalLink {
        kind: LinkKind::Path,
        uri: "/work/a.txt".into(),
        ranges: span(0, 11),
        evidence: [2, 3, 4, 5, 6].into_iter().collect(),
        label: String::new(),
        osc_label_ambiguous: false,
        unseen: false,
    };
    let mut files = FileLinks::new("s1".into(), None);
    let hover = |check| Hover {
        link: checked.clone(),
        platform: Platform::Linux,
        check,
        staged: false,
        _stat: None,
    };
    files.hover = Some(hover(Check::Ready(ready("/work/a.txt", 3))));
    let with = |change: &dyn Fn(&mut TerminalLink)| {
        let mut link = checked.clone();
        change(&mut link);
        link
    };
    assert!(files.ready_for(&checked));
    assert!(files.ready_for(&with(
        &|link| link.evidence = [3, 4, 5].into_iter().collect()
    )));
    assert!(
        !files.ready_for(&with(&|link| link.ranges = span(1, 11))),
        "another start"
    );
    assert!(
        !files.ready_for(&with(&|link| link.ranges = span(0, 12))),
        "another end"
    );
    assert!(
        !files.ready_for(&with(&|link| link.uri = "/work/b.txt".into())),
        "other text over the same cells"
    );
    let mut folder = ready("/work/a.txt", 3);
    folder.info.status = DownloadFileStatus::Folder;
    files.hover = Some(hover(Check::from_stat(folder)));
    assert!(!files.ready_for(&checked), "a refusal admits nothing");
    files.hover = Some(hover(Check::Checking));
    assert!(!files.ready_for(&checked), "nor does a check in flight");
}

/// Output under a resting pointer is the everyday case: a redraw that leaves the path's text as it was must keep the
/// check (or every prompt redraw would restart it and refuse clicks), while one that changes the text over the same
/// cells must withdraw it (SPEC.md: "A partial redraw that changes the link text withdraws transfer and opening
/// permission"). Specified: with the pointer resting on a checked printed path, rewriting its own row with the same text,
/// then the row below, starts no check and a click is admitted each time; rewriting its cells with another path, with
/// the pointer still, starts a silent check of the new path and leaves a click there unadmitted.
#[gpui::test]
fn a_rewrite_under_the_pointer_withdraws_the_check_only_when_the_paths_text_changes(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.show(
        vec![row(0, "notes.txt", false), row(1, "other", false)],
        None,
    );
    let on = h.at_cell(2, 0);
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
    h.assert_silent_hover();
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
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    let rows = serde_json::json!({"t":"rows","rows":[row(0, "plans.txt", false)],"links":[]});
    h.fake().output(serde_json::from_value(rows).unwrap());
    let frame = frame(2, FITTED.0, FITTED.1);
    h.fake().output(serde_json::from_value(frame).unwrap());
    h.cx.run_until_parked();
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    let _fresh = h.check("plans.txt");
    assert_eq!(h.admitted(), Vec::<String>::new());
}

/// The host can answer before the first disclosure frame. A click in that interval stays silent because the user
/// has yet to see the target. Once the same answer has painted, it admits a click.
#[gpui::test]
fn a_click_before_the_answer_has_painted_stays_silent(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let pending = h.check("/work/report.pdf");
    // Place the disclosure with one draw, then dispatch the click before the deferred popover paints.
    let owner = h.owner.clone();
    h.cx.update(|window, cx| {
        let files = owner.read(cx).files().clone();
        files.update(cx, |files, cx| {
            files.answered(Ok(ready("/work/report.pdf", 2000)), cx);
            let link = files.tracking().expect("the premise: the check is tracked");
            assert!(files.ready_for(link), "the premise: the host answered");
        });
        window.draw(cx).clear(cx);
        let drawn = owner
            .read(cx)
            .disclosure_drawn()
            .expect("the frame placed the disclosure");
        assert!(drawn.get().is_none(), "the popover has yet to paint");
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
    assert_eq!(h.refusal(), None);
    h.answer(pending, ready("/work/report.pdf", 2000));
    assert!(h.shown(), "the premise: the ready answer painted");
    h.click(on);
    assert_eq!(h.admitted(), ["/work/report.pdf"]);
}

/// A press while the stat is pending cannot gain permission from an answer that lands before release.
#[gpui::test]
fn an_answer_landing_between_press_and_release_leaves_the_click_silent(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    let answer = h.check("notes.txt");
    h.assert_silent_hover();
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    answer.answer(Ok(ready("/work/notes.txt", 10)));
    settle(h.cx);
    redraw(h.cx);
    assert!(
        h.pointer(),
        "the premise: the file is ready before mouse-up"
    );
    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    assert_eq!(h.admitted(), Vec::<String>::new());
    assert_eq!(h.refusal(), None);
}

/// Restoring the same printed path can resolve to a different file. A press against the earlier disclosure cannot
/// authorize the replacement, even after the new disclosure has painted at the same cells.
#[gpui::test]
fn a_replacement_file_check_cannot_inherit_a_press(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    let original = h.check("notes.txt");
    h.answer(original, ready("/work/first/notes.txt", 10));
    assert_eq!(
        h.line("terminal-file-directory").as_deref(),
        Some("build-box:/work/first/"),
        "the premise: the original target painted before the press"
    );
    h.cx.simulate_mouse_down(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);

    h.rewrite(vec![row(0, "plans.txt", false)], None, 2);
    let intervening = h.check("plans.txt");
    h.rewrite(vec![row(0, "notes.txt", false)], None, 3);
    assert!(intervening.abandoned());
    let replacement = h.check("notes.txt");
    h.answer(replacement, ready("/work/second/notes.txt", 20));
    assert_eq!(
        h.line("terminal-file-directory").as_deref(),
        Some("build-box:/work/second/"),
        "the premise: the replacement target painted before release"
    );

    h.cx.simulate_mouse_up(on, MouseButton::Left, Modifiers::none());
    settle(h.cx);
    redraw(h.cx);
    assert!(h.admitted().is_empty());
    assert_eq!(h.refusal(), None);

    h.click(on);
    assert_eq!(h.admitted(), ["/work/second/notes.txt"]);
}

/// SPEC.md: "Only a downloadable file gets an underline, a pointing hand, and a disclosure", and the old app gave a
/// printed path the pointing hand only once its check was ready, so the hand never promises a click that would do
/// nothing. The old app also gave a
/// program's `file://` hyperlink the hand at once; that hyperlink is now inert. Specified: on a printed path, no hand
/// while checking or after a refusal, and the hand once the file is ready; resting on the cells of a program's
/// `file://` hyperlink labelled with a path-shaped `report.txt` (the grid model finds the hyperlink there), no link, no
/// hand and no check at all, so neither the hyperlink nor its path-shaped label is a file link.
#[gpui::test]
fn the_pointing_hand_waits_for_a_ready_check_on_a_printed_path(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(
        vec![
            row(0, "notes.txt", false),
            row(1, "report.txt", true),
            row(2, "folder.dir", false),
        ],
        Some("file:///work/report.txt"),
    );
    let printed = h.at_cell(2, 0);
    h.rest_on(printed);
    let answer = h.check("notes.txt");
    assert!(!h.pointer(), "checking");
    h.answer(answer, ready("/work/notes.txt", 10));
    assert!(h.pointer(), "ready");

    let folder = h.at_cell(2, 2);
    h.rest_on(folder);
    let answer = h.check("folder.dir");
    let mut refused = ready("/work/folder.dir", 0);
    refused.info.status = DownloadFileStatus::Folder;
    h.answer(answer, refused);
    assert!(!h.pointer(), "refused");

    assert_eq!(
        h.link_under((2, 1)),
        (
            false,
            false,
            false,
            false,
            Some("file:///work/report.txt".to_owned())
        ),
        "over the hyperlink's cells, no link, underline, hand or display"
    );
    assert!(
        h.checks().is_empty(),
        "a program's file hyperlink asks nothing"
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
    h.fake().output(mouse_mode("click"));
    settle(h.cx);
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    drop(h.sent());
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
    let reports: Vec<MouseEventKind> = h
        .sent()
        .into_iter()
        .filter_map(|(input, _)| match input {
            GridInput::Mouse { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
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
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
}

// ---- Downloads and the status line -----------------------------------------------------------------------------------

impl Files<'_> {
    /// The latest save the terminal asked for: its path and whether it asked to open the save.
    fn last_save(&mut self) -> (String, bool) {
        assert!(
            self.checks().is_empty(),
            "the premise: no check is in flight"
        );
        let (path, open, _) = self
            .downloads
            .last()
            .expect("the premise: a save was asked for");
        (path.clone(), *open)
    }
}

/// A save into Downloads under the name the helm chose, where the user can find it.
fn saved(path: &str, open: OpenOutcome) -> SavedDownload {
    SavedDownload {
        path: path.into(),
        open,
    }
}

/// The click is the whole point of a file link: it must ask the helm to save the very file the check disclosed, and
/// then say where it went, since the helm, not the user, picks a free name. Specified (SPEC.md: "The desktop app saves
/// into Downloads under a unique name and shows where it went ... Plain click only downloads"; the save half of the e2e
/// "file hover and complete browser download", and the old "modifier opening is native-only and save outcomes survive
/// opener refusal" for the plain case): a plain click asks the session's host for the resolved path, without opening,
/// and the status line says "Downloading…", then "Saved to" the path the helm answered; a second click whose save the
/// helm put under a free name shows that name.
#[gpui::test]
fn a_click_saves_into_downloads_and_the_line_says_where(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    h.click(on);
    assert_eq!(h.last_save(), ("/work/notes.txt".to_owned(), false));
    assert_eq!(h.status().as_deref(), Some("Downloading…"));
    h.save(Ok(saved(
        "/home/u/Downloads/notes.txt",
        OpenOutcome::NotAsked,
    )));
    assert_eq!(
        h.status().as_deref(),
        Some("Saved to /home/u/Downloads/notes.txt")
    );

    h.click(on);
    h.save(Ok(saved(
        "/home/u/Downloads/notes (1).txt",
        OpenOutcome::NotAsked,
    )));
    assert_eq!(
        h.status().as_deref(),
        Some("Saved to /home/u/Downloads/notes (1).txt")
    );
}

/// A success is news for a moment, while a failure is something the user must read, so they leave differently
/// (SPEC.md: "success fades after a few seconds, while a failure stays until the next click or download there").
/// Specified (the old "browser and native successes fade, while a failure waits for a terminal click" and "replacement
/// cancels an earlier notice deadline and retires a failure"): a success is gone five seconds after it showed, not
/// before; a later download replaces it and the earlier deadline does not take the new line away; a failure, in the
/// helm's words, stays past that deadline, a press anywhere in the terminal takes it away, and so does the next
/// download.
#[gpui::test]
fn a_success_fades_and_a_failure_stays_until_the_next_press_or_download(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let (on, blank) = (h.at_cell(2, 0), h.at_cell(40, 5));
    h.checked(on, "notes.txt", 10);
    h.click(on);
    h.save(Ok(saved(
        "/home/u/Downloads/notes.txt",
        OpenOutcome::NotAsked,
    )));
    advance(h.cx, Duration::from_millis(4_900));
    assert!(h.status().is_some(), "still shown just before five seconds");
    advance(h.cx, Duration::from_millis(100));
    assert_eq!(h.status(), None, "gone at five seconds");

    h.click(on);
    h.save(Ok(saved(
        "/home/u/Downloads/notes.txt",
        OpenOutcome::NotAsked,
    )));
    advance(h.cx, Duration::from_secs(3));
    h.click(on);
    advance(h.cx, Duration::from_secs(3));
    assert_eq!(
        h.status().as_deref(),
        Some("Downloading…"),
        "the old deadline went with its line"
    );

    h.save(Err(TransferError::Refused {
        status: 502,
        message: "the host went away".into(),
    }));
    let failed = "Download failed: the host went away";
    advance(h.cx, Duration::from_secs(10));
    assert_eq!(h.status().as_deref(), Some(failed), "a failure stays");
    h.click(on);
    assert_eq!(
        h.status().as_deref(),
        Some("Downloading…"),
        "the next download replaces it"
    );
    h.save(Err(TransferError::Lost("connection reset".into())));
    assert!(
        h.status()
            .is_some_and(|line| line.starts_with("Download failed: "))
    );
    h.rest_on(blank);
    assert!(h.status().is_some(), "moving the pointer leaves it");
    h.click(blank);
    assert_eq!(h.status(), None, "a press in the terminal takes it away");
}

/// Opening is the one thing a click can do beyond saving, so it must take the platform's own modifier and only what
/// the user was offered at hover; and an opening that fails must not undo the save (SPEC.md: "Cmd-click on macOS
/// (Ctrl-click elsewhere) also opens the completed save ... an opening failure leaves the save successful and says why
/// it was not opened"). Specified (the old "modifier opening is native-only and save outcomes survive opener refusal"
/// and "a native modified click saves without opening when hover offered only download"): off a Mac a Ctrl-click on a
/// file the helm offered opening asks to open, and the line adds "opened", or, when the helm could not, "not opened:"
/// and its reason after where the file was saved; Cmd does nothing there; on a Mac Cmd opens and Ctrl does not; a
/// modified click on a file the hover offered no opening for saves without asking to open and says opening was not
/// offered.
#[gpui::test]
fn the_modifier_click_opens_only_what_the_helm_offered(cx: &mut TestAppContext) {
    let (ctrl, cmd) = (
        Modifiers {
            control: true,
            ..Modifiers::none()
        },
        Modifiers {
            platform: true,
            ..Modifiers::none()
        },
    );
    let mut h = harness(cx);
    let owner = h.owner.clone();
    h.cx.update(|_, cx| owner.update(cx, |owner, _| owner.set_platform(Platform::Linux)));
    h.show(
        vec![row(0, "notes.txt", false), row(1, "data.bin", false)],
        None,
    );
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    let saved_to = "/home/u/Downloads/notes.txt";
    h.click_with(on, ctrl);
    assert_eq!(h.last_save(), ("/work/notes.txt".to_owned(), true));
    h.save(Ok(saved(saved_to, OpenOutcome::Opened)));
    assert_eq!(
        h.status().as_deref(),
        Some("Saved to /home/u/Downloads/notes.txt · opened")
    );
    h.click_with(on, ctrl);
    h.save(Ok(saved(
        saved_to,
        OpenOutcome::NotOpened("no app opens it".into()),
    )));
    assert_eq!(
        h.status().as_deref(),
        Some("Saved to /home/u/Downloads/notes.txt · not opened: no app opens it")
    );
    h.click_with(on, cmd);
    assert!(!h.last_save().1, "Cmd is not the modifier off a Mac");

    let other = h.at_cell(2, 1);
    h.rest_on(other);
    let answer = h.check("data.bin");
    let mut unlisted = ready("/work/data.bin", 10);
    unlisted.can_open = Some(false);
    h.answer(answer, unlisted);
    h.click_with(other, ctrl);
    assert_eq!(h.last_save(), ("/work/data.bin".to_owned(), false));
    h.save(Ok(saved(
        "/home/u/Downloads/data.bin",
        OpenOutcome::NotAsked,
    )));
    assert_eq!(
        h.status().as_deref(),
        Some("Saved to /home/u/Downloads/data.bin · not opened: opening was not offered")
    );

    // On a Mac. The hover takes the platform when its display next changes, so the pointer leaves and comes back.
    h.cx.update(|_, cx| owner.update(cx, |owner, _| owner.set_platform(Platform::Mac)));
    let away = h.at_cell(40, 5);
    h.rest_on(away);
    h.checked(on, "notes.txt", 10);
    h.click_with(on, cmd);
    assert!(h.last_save().1, "Cmd opens on a Mac");
    h.click_with(on, ctrl);
    assert!(!h.last_save().1, "Ctrl does not");
}

// ---- Copying the contents --------------------------------------------------------------------------------------------

/// Shift and the platform's opening modifier: the copy chord on `platform`.
fn copy_chord(platform: Platform) -> Modifiers {
    match platform {
        Platform::Mac => Modifiers {
            platform: true,
            shift: true,
            ..Modifiers::none()
        },
        Platform::Linux => Modifiers {
            control: true,
            shift: true,
            ..Modifiers::none()
        },
    }
}

impl Files<'_> {
    /// Make the terminal `platform`'s, before any hover takes its platform.
    fn on(&mut self, platform: Platform) {
        let owner = self.owner.clone();
        self.cx
            .update(|_, cx| owner.update(cx, |owner, _| owner.set_platform(platform)));
    }

    /// Every copy's read the terminal asked for so far: the path and the cap it asked for.
    fn copied(&mut self) -> Vec<(String, usize)> {
        assert!(
            self.checks().is_empty(),
            "the premise: no check is in flight"
        );
        self.copies
            .iter()
            .map(|(path, cap, _)| (path.clone(), *cap))
            .collect()
    }

    /// Answer the latest copy's read, which must not have been answered yet, and draw the outcome.
    fn read_back(&mut self, result: Result<Vec<u8>, TransferError>) {
        assert!(
            self.checks().is_empty(),
            "the premise: no check is in flight"
        );
        let answer = self
            .copies
            .last_mut()
            .and_then(|(_, _, answer)| answer.take())
            .expect("the premise: a copy waits for its read");
        answer.answer(result);
        settle(self.cx);
        redraw(self.cx);
    }

    /// The image on the clipboard, if it holds one.
    fn clipboard_image(&mut self) -> Option<gpui::Image> {
        let item = self.cx.read_from_clipboard()?;
        item.entries().iter().find_map(|entry| match entry {
            gpui::ClipboardEntry::Image(image) => Some(image.clone()),
            _ => None,
        })
    }
}

/// The copy chord exists to put a file's contents on the clipboard without a trip through Downloads, so it must read
/// the very file the hover disclosed, capped at 10 MB, save nothing, and say how it went in the status line the way a
/// save does (the maintainer's words: "Copying…", "Copied to clipboard", "Copy failed: …"). Specified: on a Mac,
/// Shift-Cmd-click on a checked path asks the host for the resolved path's bytes with a 10,000,000-byte cap and asks
/// for no save; the line says "Copying…"; once the bytes arrive the clipboard holds them as text and the line says
/// "Copied to clipboard", gone at five seconds as a save's success is; a refusal without words reads "Copy failed: Copy
/// refused", and stays past that; one with words shows them; a file found over the cap reads "the file is larger than
/// 10 MB"; and bytes that are neither text nor a listed image are refused with words, leaving the clipboard as it was.
#[gpui::test]
fn a_copy_click_puts_the_contents_on_the_clipboard_without_saving(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    h.click_with(on, copy_chord(Platform::Mac));
    assert_eq!(
        h.copied(),
        [("/work/notes.txt".to_owned(), 10_000_000)],
        "the disclosed file, capped"
    );
    assert_eq!(h.admitted(), Vec::<String>::new(), "nothing is saved");
    assert_eq!(h.status().as_deref(), Some("Copying…"));
    h.read_back(Ok(b"hello\n".to_vec()));
    assert_eq!(h.clipboard().as_deref(), Some("hello\n"));
    assert_eq!(h.status().as_deref(), Some("Copied to clipboard"));
    advance(h.cx, Duration::from_secs(5));
    assert_eq!(h.status(), None, "the success fades as a save's does");

    for (error, line) in [
        (
            TransferError::Refused {
                status: 404,
                message: String::new(),
            },
            "Copy failed: Copy refused",
        ),
        (
            TransferError::Refused {
                status: 404,
                message: "the file is no longer readable".into(),
            },
            "Copy failed: the file is no longer readable",
        ),
        (
            TransferError::TooLarge {
                max_bytes: 10_000_000,
            },
            "Copy failed: the file is larger than 10 MB",
        ),
    ] {
        h.click_with(on, copy_chord(Platform::Mac));
        h.read_back(Err(error));
        advance(h.cx, Duration::from_secs(10));
        assert_eq!(h.status().as_deref(), Some(line), "a failure stays");
    }

    h.click_with(on, copy_chord(Platform::Mac));
    h.read_back(Ok(b"\x7fELF\x02\x01\x01\0".to_vec()));
    assert_eq!(
        h.status().as_deref(),
        Some("Copy failed: the file is not text or a supported image")
    );
    assert_eq!(
        h.clipboard().as_deref(),
        Some("hello\n"),
        "a refused copy leaves the clipboard alone"
    );
}

/// Images copy as images on a Mac, where GPUI's clipboard takes them, and are refused at once on Linux, where it takes
/// text only, so no bytes cross for a copy that cannot land; the chord is Shift-Ctrl there, and Shift-Cmd is not.
/// Specified: on a Mac a checked `shot.png` holding a PNG copies as a PNG image; on Linux Shift-Ctrl on it reads
/// nothing and says only text can be copied, while Shift-Ctrl on a text file copies it, and Shift-Cmd on that file
/// copies nothing.
#[gpui::test]
fn images_copy_on_a_mac_and_are_refused_before_reading_on_linux(cx: &mut TestAppContext) {
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(
        vec![row(0, "shot.png", false), row(1, "notes.txt", false)],
        None,
    );
    let (image, text) = (h.at_cell(2, 0), h.at_cell(2, 1));
    h.checked(image, "shot.png", 16);
    h.click_with(image, copy_chord(Platform::Mac));
    h.read_back(Ok(png.clone()));
    assert_eq!(
        h.clipboard_image(),
        Some(gpui::Image::from_bytes(gpui::ImageFormat::Png, png))
    );
    assert_eq!(h.status().as_deref(), Some("Copied to clipboard"));

    // On Linux. The hover takes the platform when its display next changes, so the pointer leaves and comes back.
    h.on(Platform::Linux);
    let away = h.at_cell(40, 5);
    h.rest_on(away);
    h.checked(image, "shot.png", 16);
    h.click_with(image, copy_chord(Platform::Linux));
    assert_eq!(h.copied().len(), 1, "no new read for the image");
    assert_eq!(
        h.status().as_deref(),
        Some("Copy failed: only text can be copied here; images can be copied on macOS")
    );
    h.checked(text, "notes.txt", 6);
    h.click_with(text, copy_chord(Platform::Mac));
    assert_eq!(h.copied().len(), 1, "Shift-Cmd does not copy on Linux");
    assert_eq!(
        h.last_save(),
        ("/work/notes.txt".to_owned(), false),
        "it is a plain download there"
    );
    h.click_with(text, copy_chord(Platform::Linux));
    assert_eq!(
        h.copied().last().map(|(path, _)| path.as_str()),
        Some("/work/notes.txt")
    );
    h.read_back(Ok(b"notes\n".to_vec()));
    assert_eq!(h.clipboard().as_deref(), Some("notes\n"));
}

/// The clipboard must end up holding the file the user copied last, even when an earlier, larger file's read is still
/// on its way. Specified: two copies started one after the other (premise: both reads went out) leave the first read
/// abandoned; the second's bytes reach the clipboard; and the first, answered afterwards, changes nothing.
#[gpui::test]
fn only_the_latest_copy_reaches_the_clipboard(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(
        vec![row(0, "big.log", false), row(1, "notes.txt", false)],
        None,
    );
    let (big, small) = (h.at_cell(2, 0), h.at_cell(2, 1));
    h.checked(big, "big.log", 9_000_000);
    h.click_with(big, copy_chord(Platform::Mac));
    h.checked(small, "notes.txt", 6);
    h.click_with(small, copy_chord(Platform::Mac));
    assert_eq!(h.copied().len(), 2, "the premise: both reads went out");
    let first = h.copies[0].2.take().expect("the first read is unanswered");
    assert!(first.abandoned(), "the second copy abandoned the first");
    h.read_back(Ok(b"notes\n".to_vec()));
    assert_eq!(h.clipboard().as_deref(), Some("notes\n"));
    first.answer(Ok(b"big\n".to_vec()));
    settle(h.cx);
    redraw(h.cx);
    assert_eq!(h.clipboard().as_deref(), Some("notes\n"));
    assert_eq!(h.status().as_deref(), Some("Copied to clipboard"));
}

/// A file the hover already measured over the cap cannot be copied, so the click says so at once instead of moving up
/// to 10 MB to find out. Specified: a checked file of 10,000,001 bytes copied with the chord reads nothing and the line
/// says "Copy failed: the file is larger than 10 MB".
#[gpui::test]
fn a_file_the_hover_measured_over_the_cap_is_refused_before_reading(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(vec![row(0, "big.log", false)], None);
    let on = h.at_cell(2, 0);
    h.checked(on, "big.log", 10_000_001);
    h.click_with(on, copy_chord(Platform::Mac));
    assert_eq!(h.copied(), Vec::<(String, usize)>::new());
    assert_eq!(
        h.status().as_deref(),
        Some("Copy failed: the file is larger than 10 MB")
    );
}

/// Shift extends a selection, and a click on a link counts only with nothing selected, so without care an earlier
/// selection would grow to the path and silently swallow the copy. Specified: with text selected elsewhere on the
/// screen (premise: the selection is there once the path's check answered), the copy chord on the checked path clears
/// the selection and reads the file.
#[gpui::test]
fn an_earlier_selection_never_swallows_a_copy_click(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(
        vec![row(0, "notes.txt", false), row(1, "some other text", false)],
        None,
    );
    let (from, to) = (h.at_cell(0, 1), h.at_cell(8, 1));
    h.drag(from, to, Modifiers::none());
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    assert!(
        h.selection().is_some_and(|selection| !selection.is_empty()),
        "the premise: text is selected"
    );
    h.click_with(on, copy_chord(Platform::Mac));
    assert_eq!(h.selection(), None, "the copy cleared the selection");
    assert_eq!(h.copied(), [("/work/notes.txt".to_owned(), 10_000_000)]);
}

/// The wording a copy refused for file details not fully visible gets.
const COPY_REFUSAL: &str =
    "cannot copy: full file details are not visible; enlarge the window and hover again";

/// A copy refused because the file details were not fully visible must say "cannot copy", not "cannot download": the
/// user asked for a copy, and the refusal's words are the only feedback. This is the file check's refusal, for a display
/// that ran past the window's edge; the next test is the terminal's own, for a display held back over the pointer.
/// Specified: with a ready check whose display ran off the window, the copy chord reads nothing and the top of the
/// terminal says the copy wording.
#[gpui::test]
fn a_copy_refused_for_a_clipped_display_says_it_could_not_copy(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at_cell(3, 0);
    h.checked(on, "/work/report.pdf", 2000);
    h.stage_off_window();
    h.click_with(on, copy_chord(Platform::Mac));
    assert_eq!(h.copied(), Vec::<(String, usize)>::new());
    assert_eq!(h.refusal().as_deref(), Some(COPY_REFUSAL));
}

/// The terminal's own refusal, for a file display held back over its pointer, names the copy too (see the previous
/// test for why). Specified: with a display held back over its pointer, the copy chord reads nothing and the top of the
/// terminal says the copy wording.
#[gpui::test]
fn a_copy_refused_for_a_held_back_display_says_it_could_not_copy(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.on(Platform::Mac);
    h.cx.simulate_resize(gpui::size(px(520.), px(300.)));
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let answer = h.check("/work/report.pdf");
    let resolved = format!("/work/{}report.pdf", "deep/".repeat(420));
    h.answer(answer, ready(&resolved, 2000));
    let owner = h.owner.clone();
    assert!(
        h.cx.update(|_, cx| owner.read(cx).file_disclosure_held_back("/work/report.pdf")),
        "the premise: the panel painted over the pointer and was held back"
    );
    h.click_with(on, copy_chord(Platform::Mac));
    assert_eq!(h.copied(), Vec::<(String, usize)>::new());
    assert_eq!(h.refusal().as_deref(), Some(COPY_REFUSAL));
}

/// The status line sits over the top of the terminal, and the program must never be resized for it: a resize would
/// redraw the program's screen on every download. Specified (SPEC.md: the status "appears at the top of that
/// terminal"; the old message was positioned over the grid): showing the status line, and then a failure that stays,
/// asks the program for no new size and leaves the grid's area where it was.
#[gpui::test]
fn the_status_line_never_resizes_the_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    drop(resizes(&h.opened));
    let before = h.at_cell(0, 0);
    h.click(on);
    h.save(Err(TransferError::Lost("connection reset".into())));
    assert!(h.status().is_some(), "the premise: the line shows");
    assert_eq!(resizes(&h.opened), Vec::<(u16, u16)>::new());
    assert_eq!(h.at_cell(0, 0), before, "the grid did not move");
}

/// A TUI that breaks a long path itself still printed one file, and both halves must name it, check it once and save
/// it whole, or a click on the tail would save the wrong file or none (SPEC.md: "a path cut by a program's line break
/// links to the joined path from either half"). Specified (the e2e "both halves of a program-broken path disclose and
/// download the whole file"): a path whose head ends four columns short of the right edge, continued indented on the
/// next row, is checked once as the joined path, from its head; resting on the tail is the same hover and asks
/// nothing; a click on the tail saves the whole file.
#[gpui::test]
fn both_halves_of_a_cut_path_check_and_save_the_whole_file(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    // The head ends four columns short of the edge of the fitted width ([`Files::show`] checks the attach).
    let head = format!("{}/work/broken-", " ".repeat(usize::from(FITTED.0) - 17));
    h.show(
        vec![row(0, &head, false), row(1, "  file.txt", false)],
        None,
    );
    let (on_head, on_tail) = (h.at_cell(66, 0), h.at_cell(4, 1));
    h.checked(on_head, "/work/broken-file.txt", 29);
    h.rest_on(on_tail);
    assert!(h.checks().is_empty(), "the tail is the same hover");
    h.click(on_tail);
    assert_eq!(h.last_save(), ("/work/broken-file.txt".to_owned(), false));
}

impl Files<'_> {
    /// As if the destination display the pointer rests on had run past the bottom edge of the window's content area
    /// when last drawn, so the next click on its link is refused. No frame may come before that click, or the display
    /// records itself again.
    fn stage_off_window(&mut self) {
        let owner = self.owner.clone();
        self.cx.update(|_, cx| {
            let drawn = owner
                .read(cx)
                .disclosure_drawn()
                .expect("the premise: a display is shown");
            let mut panel = drawn.get().expect("the premise: the display was drawn");
            panel.bounds.origin.y = panel.area.bottom() - px(4.);
            drawn.set(Some(panel));
        });
    }
}

/// The terminal has one message line, and the newer of a refusal and a download's outcome must show there: a save that
/// fails while a refused click's explanation stands would otherwise stay hidden behind it and be cleared with it at the
/// next press, never seen (the old app's late message replaced the refusal). Specified: with a save in flight, a click
/// refused for a hidden display shows its explanation; the save then fails, and the failure shows in its place and
/// stays until the next press.
#[gpui::test]
fn a_save_failing_behind_a_refusal_shows_its_failure(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.checked(on, "notes.txt", 10);
    h.click(on);
    h.stage_off_window();
    h.click(on);
    assert_eq!(
        h.refusal().as_deref(),
        Some(FILE_REFUSAL),
        "the premise: the refusal shows"
    );
    h.save(Err(TransferError::Lost("connection reset".into())));
    assert_eq!(h.refusal(), None);
    let failed = h.status().expect("the failure shows");
    assert!(failed.starts_with("Download failed: "), "{failed:?}");
    advance(h.cx, Duration::from_secs(10));
    assert_eq!(h.status(), Some(failed), "and stays");
}

/// A refusal takes the message line from a download's outcome for good: once the refusal is cleared at the next press,
/// an older success must not come back as if it were news. Both refusals clear it, a file click's and a web link's.
/// Specified: a success shows; a file click refused for a hidden display shows the refusal, and after the next press
/// neither shows; again, with a program's web hyperlink refused for a hidden display.
#[gpui::test]
fn a_refusal_takes_the_status_lines_place(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(
        vec![row(0, "notes.txt", false), row(1, "click here", true)],
        Some("https://example.com/page"),
    );
    let (on, web, blank) = (h.at_cell(2, 0), h.at_cell(2, 1), h.at_cell(40, 5));
    for refused in [on, web] {
        h.checked(on, "notes.txt", 10);
        h.click(on);
        h.save(Ok(saved(
            "/home/u/Downloads/notes.txt",
            OpenOutcome::NotAsked,
        )));
        assert!(h.status().is_some(), "the premise: the success shows");
        h.rest_on(refused);
        h.stage_off_window();
        h.click(refused);
        assert!(h.refusal().is_some(), "the refusal shows");
        h.rest_on(blank);
        h.click(blank);
        assert_eq!(h.refusal(), None);
        assert_eq!(
            h.status(),
            None,
            "and the success it replaced does not come back"
        );
    }
}

/// A save is the user's request, not the hover's: moving the pointer on must not abandon it, while closing the
/// terminal must, so the helm removes what it staged rather than leaving a save nobody will hear about. Specified: a
/// save asked for and still in flight is not abandoned when the pointer leaves the link; it is once the terminal is
/// closed.
#[gpui::test]
fn a_save_outlives_the_hover_but_not_the_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "notes.txt", false)], None);
    let (on, away) = (h.at_cell(2, 0), h.at_cell(40, 5));
    h.checked(on, "notes.txt", 10);
    h.click(on);
    h.rest_on(away);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);
    let pending = |downloads: &Vec<(String, bool, Option<FakeAnswer<SavedDownload>>)>| {
        downloads[0]
            .2
            .as_ref()
            .expect("the save is unanswered")
            .abandoned()
    };
    assert!(!pending(&h.downloads), "leaving the link keeps the save");

    // The terminal leaves its window, which stays open, and the test's own handle goes too.
    h.close_terminal();
    let Files { h, downloads, .. } = h;
    let Harness { cx, owner, .. } = h;
    drop(owner);
    redraw(cx);
    assert!(pending(&downloads), "closing the terminal abandons it");
}

// ---- What the host names ---------------------------------------------------------------------------------------------

/// Why: the resolved path comes from the host, and it is the evidence the user reads before a click transfers a file.
/// A right-to-left override in a file name would reorder what they read (`invoice`, U+202E, `txt.exe` is drawn as
/// "invoiceexe.txt", an executable posing as text), so the display must show such characters rather than obey them.
/// Spec (old `grid-attach.spec.ts`, "a host's file name with format characters is shown with them visible"; SPEC.md:
/// "File disclosures ... make presentation-unsafe characters visible"): hovering a printed path the host resolves to
/// `/tmp/invoice` U+202E `txt.exe`, the name line is named "invoice<U+202E>txt.exe" and painted that way inside its own
/// direction isolate, and no painted line carries the raw override.
#[gpui::test]
fn a_hosts_file_name_with_format_characters_is_shown_with_them_visible(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "/tmp/a.txt", false)], None);
    let on = h.at_cell(2, 0);
    h.rest_on(on);
    let answer = h.check("/tmp/a.txt");
    h.answer(answer, ready("/tmp/invoice\u{202E}txt.exe", 10));
    let shown = "invoice<U+202E>txt.exe";
    assert_eq!(h.line("terminal-file-name").as_deref(), Some(shown));
    // The texts as painted, isolates and all: the session list's `painted` drops a sentence's isolates, which this
    // test needs to see.
    let painted: Vec<String> = test_support::frame(h.cx)
        .iter()
        .flat_map(|node| node.texts.iter().map(|text| text.text.to_string()))
        .collect();
    assert!(
        painted
            .iter()
            .any(|line| crate::host_text::tests::paints_isolated(line, shown)),
        "the name is painted escaped: {painted:?}"
    );
    assert!(
        painted.iter().all(|line| !line.contains('\u{202E}')),
        "the override itself is never painted: {painted:?}"
    );
}

// ---- Late answers and unrelated output -------------------------------------------------------------------------------

impl Files<'_> {
    /// Attach and replay a complete screen with `rows` (see [`row`]) and history lines `history` above it, numbered
    /// from 0, so the screen's first line is `history.len()`. The cap keeps them all.
    fn show_with_history(&mut self, history: &[&str], rows: Vec<serde_json::Value>) {
        let asked = self.fake().attach();
        assert_eq!(
            (asked.cols, asked.rows),
            FITTED,
            "the premise: the terminal attached at the fitted size"
        );
        let (cols, height) = FITTED;
        let replay = [
            serde_json::json!({"t":"hello","cols":cols,"rows":height,"history_cap":100}),
            serde_json::json!({"t":"state","mouse":"off","focus":false,"alt_screen":false,
                "default_fg":null,"default_bg":null,"history_trimmed":false,"exit":null}),
            serde_json::json!({"t":"rows","rows":rows,"links":[]}),
            frame(1, cols, height),
            history_message(history),
        ];
        for message in replay {
            self.fake().output(serde_json::from_value(message).unwrap());
        }
        self.fake().replay_complete();
        settle(self.cx);
        redraw(self.cx);
    }

    /// The link the current hover checks (or checked), as the grid model found it when the hover started.
    fn hovered_file_link(&mut self) -> Option<TerminalLink> {
        let owner = self.owner.clone();
        self.cx.update(|_, cx| {
            let files = owner.read(cx).files().read(cx);
            files.hover.as_ref().map(|hover| hover.link.clone())
        })
    }
}

/// A `history` message appending `lines` from line 0, as the first one after a `hello` or a `history_reset` may.
fn history_message(lines: &[&str]) -> serde_json::Value {
    let rows: Vec<_> = lines
        .iter()
        .map(|text| serde_json::json!({"w":false,"r":[[0,text.len(),text,[0,-1,-1]]]}))
        .collect();
    serde_json::json!({"t":"history","first":0,"rows":rows,"cols":FITTED.0})
}

/// Why: a check waits on the host, and a program writing meanwhile (an agent at work, a prompt's clock) must not void
/// it, or no file link would ever show while the terminal is busy; but an answer must never stand for a line whose
/// path changed while it was asked. Spec (old `grid-terminal.spec.ts`, "a late link answer still hovers after output
/// that did not touch its line"): resting on a printed path, with its check unanswered, a frame that only moves the
/// cursor and then a rewrite of another row start no check and keep the pending one; its answer then shows and a
/// click is admitted. The control: with a fresh check pending, a rewrite of the path's own row with another path drops
/// that check and starts one for the new path, which the display shows as checking. (The old control rewrote the line
/// with its old text, which the old grid treated as a new line; SPEC.md's file-path bullet makes the same text at the
/// same position the same hover, which the rewrite tests above pin, so the control here changes the path.)
#[gpui::test]
fn a_pending_check_survives_output_that_does_not_touch_its_line(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(
        vec![row(0, "notes.txt", false), row(2, "idle", false)],
        None,
    );
    let (on, away) = (h.at_cell(2, 0), h.at_cell(40, 5));
    h.rest_on(on);
    let pending = h.check("notes.txt");

    let cursor_only = serde_json::json!({"t":"frame","rev":2,"cols":FITTED.0,"rows":FITTED.1,
        "cursor":{"x":3,"y":2,"visible":true,"shape":"block","blink":false}});
    h.fake()
        .output(serde_json::from_value(cursor_only).unwrap());
    settle(h.cx);
    redraw(h.cx);
    h.rewrite(vec![row(2, "busy...", false)], None, 3);
    assert!(h.checks().is_empty(), "output elsewhere starts no check");
    assert!(!pending.abandoned(), "and keeps the one in flight");
    h.answer(pending, ready("/work/notes.txt", 10));
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("10 B"));
    h.click(on);
    assert_eq!(h.admitted(), ["/work/notes.txt"]);

    h.rest_on(away);
    h.rest_on(on);
    let pending = h.check("notes.txt");
    h.rewrite(vec![row(0, "plans.txt", false)], None, 4);
    assert!(pending.abandoned(), "a changed path drops its check");
    let _fresh = h.check("plans.txt");
    h.assert_silent_hover();
}

/// Why: a check vouches for the line it was asked about, and scrolling while it is pending puts another line under the
/// still pointer; using the answer then would show one file's details beside the text of another, which is exactly
/// what the display exists to rule out. Spec (old `grid-terminal.spec.ts`, "a late link answer is not used for a line
/// the view scrolled away from the pointer"): over a screen with history above it, resting on the printed path
/// `/work/asked.txt` on the second row and scrolling one line back while its check is pending drops that check and
/// checks `/work/above.txt`, now under the pointer; the dropped check's late answer changes nothing, and the display
/// stays silent while the new path checks, then shows its answer.
#[gpui::test]
fn a_pending_check_is_not_used_for_a_line_scrolled_away(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show_with_history(
        &["h0", "h1", "h2"],
        vec![
            row(0, "/work/above.txt", false),
            row(1, "/work/asked.txt", false),
        ],
    );
    let on = h.at_cell(3, 1);
    h.rest_on(on);
    let asked = h.check("/work/asked.txt");

    h.scroll_view(-1);
    assert!(!h.following(), "the premise: the view scrolled back");
    assert!(
        asked.abandoned(),
        "the scrolled-away line's check was dropped"
    );
    let above = h.check("/work/above.txt");
    asked.answer(Ok(ready("/work/asked.txt", 99)));
    settle(h.cx);
    redraw(h.cx);
    h.assert_silent_hover();
    h.answer(above, ready("/work/above.txt", 5));
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("5 B"));
}

/// Why: a history reset can rebuild the history with the same text at the same line numbers (a reflow that changed
/// nothing on those lines), so neither text nor numbers tell the old line from its replacement; a check asked about
/// the old line must still not stand for the new one, while a fresh hover over the replacement must work. Spec (old
/// `grid-terminal.spec.ts`, "a late history link answer is refused after a same-number same-text reset"): scrolled to
/// the top, resting on the printed path in history line 0, with its check pending, a `history_reset` and the same
/// three lines from line 0 return the view to the live screen and drop the check; scrolling back to the top finds a
/// link equal in text, cells and file to the first and checks it afresh; the dropped check's late answer changes
/// nothing, and the fresh one's shows.
///
/// NOTE: This departs from the old case, where the view stayed at the top through the reset with the check still
/// pending, and the old grid refused the late answer by comparing the stored line itself. Here a file hover is the
/// same hover whenever its link is equal (`FileLinks::follow` compares `hover.link == link`), and the replacement's
/// link is equal, so the only thing that drops the old check is the view letting go of its place in history on the
/// reset. SPEC.md does not ask for that; it is this app's viewport rule (line numbers no longer name the lines they
/// named). A change that keeps the view's place through a reset must give the hover a line identity that a reset
/// changes, not delete the `following` assertion below.
#[gpui::test]
fn a_pending_history_check_is_not_used_after_a_same_number_same_text_reset(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    let history = ["/work/history.txt", "second line", "third line"];
    h.show_with_history(&history, vec![row(0, "screen 0", false)]);
    h.scroll_view(-100);
    assert!(!h.following(), "the premise: the view shows history");
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let old = h.check("/work/history.txt");
    let asked = h.hovered_file_link();

    let reset = serde_json::json!({"t":"history_reset"});
    h.fake().output(serde_json::from_value(reset).unwrap());
    h.fake()
        .output(serde_json::from_value(history_message(&history)).unwrap());
    settle(h.cx);
    redraw(h.cx);
    assert!(
        h.following(),
        "the reset returned the view to the screen, which is what drops the old check: the replacement's link is \
         equal to the asked one, so a view that kept its place would keep the check (see the NOTE above)"
    );
    assert!(old.abandoned(), "and dropped the check of the old line");

    h.scroll_view(-100);
    let fresh = h.check("/work/history.txt");
    assert_eq!(
        h.hovered_file_link(),
        asked,
        "the premise: the replacement is the same link by text, cells and file"
    );
    old.answer(Ok(ready("/work/history.txt", 99)));
    settle(h.cx);
    redraw(h.cx);
    h.assert_silent_hover();
    h.answer(fresh, ready("/work/history.txt", 7));
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("7 B"));
}

/// Why: a printed path's link reads the rows around it (two above and two below, to rule out a chain of program cuts),
/// so output there is neighbor evidence, but output past a blank row cannot change the path or its cells, and treating
/// it as a change would restart the host check (and withdraw the click) every time a status line below redraws. Spec
/// (old `grid-attach.spec.ts`, "a container file hover survives unrelated neighboring output", both its variants):
/// with a printed path (on row 0, or `cut` across rows 0 and 1 as a program broke it), a blank row after it and
/// "status" on the next row, which lies in the link's evidence, a checked hover on the path's first row stays checked
/// when that status row is rewritten as "update": no check starts, the display keeps its answer, the pointing hand
/// stays, and a click saves the whole path's file.
fn hover_survives_output_past_a_blank_row(cx: &mut TestAppContext, cut: bool) {
    let mut h = harness(cx);
    let (rows, on, path, status_row) = if cut {
        // The head ends four columns short of the edge of the fitted width, as in the cut-path test above.
        let head = format!("{}/work/broken-", " ".repeat(usize::from(FITTED.0) - 17));
        let rows = vec![
            row(0, &head, false),
            row(1, "  file.txt", false),
            row(3, "status", false),
        ];
        (rows, (66, 0), "/work/broken-file.txt", 3)
    } else {
        let rows = vec![row(0, "/work/real.txt", false), row(2, "status", false)];
        (rows, (3, 0), "/work/real.txt", 2)
    };
    h.show(rows, None);
    let on = h.at_cell(on.0, on.1);
    h.checked(on, path, 29);
    let link = h.hovered_file_link().expect("the premise: a hover");
    let first = link.ranges[0].start.line;
    assert!(
        link.evidence.contains(&(first + status_row)),
        "the premise: the status row is part of the link's evidence: {:?}",
        link.evidence
    );

    h.rewrite(vec![row(status_row, "update", false)], None, 2);
    assert!(
        h.checks().is_empty(),
        "the rewrite past the blank row starts no check"
    );
    assert_eq!(h.line("terminal-file-status").as_deref(), Some("29 B"));
    assert!(h.pointer(), "the hand stays");
    h.click(on);
    assert_eq!(h.last_save(), (path.to_owned(), false));
}

/// [`hover_survives_output_past_a_blank_row`] for a path on one row (old `grid-attach.spec.ts`, "a container file hover
/// survives unrelated neighboring output (single row)").
#[gpui::test]
fn a_single_row_paths_hover_survives_output_past_a_blank_row(cx: &mut TestAppContext) {
    hover_survives_output_past_a_blank_row(cx, false);
}

/// [`hover_survives_output_past_a_blank_row`] for a path a program cut across two rows (old `grid-attach.spec.ts`, "a
/// container file hover survives unrelated neighboring output (cut)").
#[gpui::test]
fn a_cut_paths_hover_survives_output_past_a_blank_row(cx: &mut TestAppContext) {
    hover_survives_output_past_a_blank_row(cx, true);
}

//! The file links' contract, against the helm client's fake terminal and fake transfers: what a hover on a printed path
//! or a program's `file://` hyperlink asks the host, what its display shows while checking and once answered, when a
//! hover counts as new, and which clicks a check admits. One test per sentence of SPEC.md's file-path bullet in this
//! scope, ported from the old app's `terminal-files.test.js` and `e2e/tests/terminal-files.spec.ts` (each test names
//! the cases it replaces).
//!
//! The terminal is a real attached owner in a window from the shared harness, so the display opens through the layer
//! manager as in the app; the fake transfers hold each check's answer until the test gives it, which is how a test sees
//! the checking state and whether leaving abandoned a check.

use std::ops::{Deref, DerefMut};
use std::time::Duration;

use farhelm_helm_client::fake::transfers::{FakeAnswer, FakeTransfer};
use farhelm_helm_client::{OpenOutcome, SavedDownload};
use farhelm_proto::grid::{GridInput, MouseEventKind};
use farhelm_proto::ui_channel::{PREFERENCES_KEY, Preferences};
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, Point, TestAppContext,
    point, px,
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

/// The terminal's own test harness ([`attached::tests::harness`]: an attached owner of session `s1` in a window, its
/// stream and its UI session's transfers the helm client's fakes), with the saves the terminal has asked for so far.
/// Steps the terminal's harness already has (resting on a cell, a click with modifiers, the link choice) are its own.
struct Files<'a> {
    h: Harness<'a>,
    /// Every save the terminal asked for so far, in order: the path, whether it asked to open the save, and the answer
    /// the save waits on (taken by [`Files::save`]).
    downloads: Vec<(String, bool, Option<FakeAnswer<SavedDownload>>)>,
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
const TIERS: [&str; 5] = [
    "terminal-file-status-neutral",
    "terminal-file-status-small",
    "terminal-file-status-medium",
    "terminal-file-status-large",
    "terminal-file-status-danger",
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
    /// it started meanwhile are kept in [`Files::downloads`].
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
                other => panic!("a file link starts checks and saves only: {other:?}"),
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
    let on = h.at_cell(5, 0);
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
    let on = h.at_cell(3, 0);
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
        let at = h.at_cell(1, row);
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

    let at = h.at_cell(1, 5);
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
    let on = h.at_cell(3, 0);
    h.rest_on(on);
    let answer = h.check("/work/report.pdf");
    h.answer(answer, ready("/work/report.pdf", 2000));
    let owner = h.owner.clone();
    h.cx.update(|_, cx| {
        let files = owner.read(cx).files().read(cx);
        let drawn = files.hover.as_ref().unwrap().drawn.clone();
        let mut panel = drawn.get().expect("the premise: the display was drawn");
        panel.bounds.origin.y = panel.area.bottom() - px(4.);
        drawn.set(Some(panel));
    });
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

/// A successful host check makes a file eligible for download only while its disclosed path can be seen. A
/// covering popover may disappear on mouse down. The release must preserve that frame's download refusal,
/// even if the blocker overlaps only the card's stripe.
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
        staged: false,
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

/// A check's answer changes what the display shows, and until the new display has painted, the user has not seen it:
/// where the display was drawn while it said "Checking…" says nothing about the answer. Specified: an answer arriving
/// leaves the display with no drawn position; a click pressed before the next frame is not admitted and is refused with
/// the explanation; hovering again, once the answer has painted, a click is admitted.
#[gpui::test]
fn a_click_before_the_answer_has_painted_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.show(vec![row(0, "/work/report.pdf", false)], None);
    let on = h.at_cell(3, 0);
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

    let again = h.at_cell(5, 0);
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
    let on = h.at_cell(2, 0);
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

    let hyperlink = h.at_cell(2, 1);
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
    let Files { h, downloads } = h;
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
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
    assert_eq!(h.line("terminal-file-path").as_deref(), Some("plans.txt"));
}

/// Why: a check vouches for the line it was asked about, and scrolling while it is pending puts another line under the
/// still pointer; using the answer then would show one file's details beside the text of another, which is exactly
/// what the display exists to rule out. Spec (old `grid-terminal.spec.ts`, "a late link answer is not used for a line
/// the view scrolled away from the pointer"): over a screen with history above it, resting on the printed path
/// `/work/asked.txt` on the second row and scrolling one line back while its check is pending drops that check and
/// checks `/work/above.txt`, now under the pointer; the dropped check's late answer changes nothing, and the display
/// shows only the new path's check and then its answer.
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
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
    assert_eq!(
        h.line("terminal-file-path").as_deref(),
        Some("/work/above.txt")
    );
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
    assert_eq!(h.line("terminal-file-checking").as_deref(), Some(CHECKING));
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

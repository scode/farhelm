//! The attachments' contract, against the helm client's fake terminal and fake transfers: which flavor of a paste wins,
//! what a pasted or dropped file or image is uploaded as, when and how its host path is typed, what the strip says
//! while uploads run and after they fail, what a terminal that is not live refuses, and what a terminal that goes away
//! leaves behind. One test per sentence of SPEC.md's "Attachments", ported from the old app's
//! `crates/farhelm-ui/src/attachments.rs` tests, `crates/farhelm-ui/js-tests/clipboard-name.test.js`,
//! `e2e/tests/terminal-attachments.spec.ts` and the attachment cases of `e2e/grid-client/grid-attach.spec.ts` (each test
//! names the cases it replaces).
//!
//! The terminal is a real attached owner in a window from the shared harness, pasting through its own Paste action and
//! taking drops as the platform delivers them (the files entering the window, then released over the pane); the fake
//! transfers hold each upload's answer until the test gives it, which is how a test sees the state while an upload
//! runs.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use farhelm_helm_client::UploadSource;
use farhelm_helm_client::fake::FakeHelm;
use farhelm_helm_client::fake::terminal::{FakeTerminal, FakeTerminalRequest};
use farhelm_helm_client::fake::transfers::{FakeAnswer, FakeTransfer, FakeTransfers};
use farhelm_helm_client::terminal::AttachmentLease;
use farhelm_input_mapping::Platform;
use farhelm_proto::grid::GridInput;
use farhelm_proto::ui_channel::{Collection, CollectionState, KeyedEntry, PREFERENCES_KEY};
use gpui::{
    AppContext as _, ClipboardString, Context, Entity, ExternalPaths, FileDropEvent, FocusHandle,
    ImageFormat, IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext,
    Window, div, px,
};

use super::*;
use crate::actions::{self, THIS_PLATFORM};
use crate::session;
use crate::terminal::CellMetrics;
use crate::terminal::attached::{AttachedTerminal, OpenStream, RECONNECT_CARD_ID, TerminalTarget};
use crate::test_support::{self, keys, redraw, settle};
use crate::{focus, root};

// ---- Pure rules ------------------------------------------------------------------------------------------------------

/// A paste carrying several flavors at once is the common case (a Finder copy carries the file and its path as text; a
/// copied screenshot often carries a text label), and without one winner a paste would upload a file and type its path
/// too. Specified: file references beat image data, which beats text, for every combination, and a paste with nothing
/// in it is a no-op. Replaces the old Rust `precedence_is_file_then_image_then_text` and
/// `the_shipped_table_is_the_exact_eight_entry_wire_array` (the table existed only to ship the rule to JS).
#[test]
fn precedence_is_files_then_images_then_text() {
    for (has_file, has_image, has_text, expected) in [
        (true, true, true, Flavor::File),
        (true, true, false, Flavor::File),
        (true, false, true, Flavor::File),
        (true, false, false, Flavor::File),
        (false, true, true, Flavor::Image),
        (false, true, false, Flavor::Image),
        (false, false, true, Flavor::Text),
        (false, false, false, Flavor::None),
    ] {
        assert_eq!(
            flavor(has_file, has_image, has_text),
            expected,
            "file {has_file}, image {has_image}, text {has_text}"
        );
    }
}

/// The precedence is applied to what GPUI's clipboard read hands over, and the winner is taken whole: every file of a
/// file paste, every image of an image paste, every text entry of a text paste, and nothing of the losers. Specified: a
/// file beside its path's text attaches only the file; an image beside a text label attaches only the image; text that
/// looks like a path is text, its entries joined; empty text and an empty file list count as nothing. Replaces the
/// classification halves of the old spec's "a pasted image is uploaded under a generated name" (its text sibling not
/// pasted) and "pasted text that looks like a path arrives as text".
#[test]
fn a_paste_takes_its_winning_flavor_whole() {
    let file =
        |path: &str| ClipboardEntry::ExternalPaths(ExternalPaths(vec![PathBuf::from(path)].into()));
    let text = |text: &str| ClipboardEntry::String(ClipboardString::new(text.into()));
    let image = Image::from_bytes(ImageFormat::Png, vec![1, 2, 3]);
    let item = |entries| ClipboardItem { entries };

    assert_eq!(
        classify(&item(vec![
            file("/home/u/a.png"),
            text("/home/u/a.png"),
            file("/home/u/b.txt")
        ])),
        Paste::Attach(vec![
            Offered::File("/home/u/a.png".into()),
            Offered::File("/home/u/b.txt".into())
        ])
    );
    assert_eq!(
        classify(&item(vec![
            text("screenshot label"),
            ClipboardEntry::Image(image.clone())
        ])),
        Paste::Attach(vec![Offered::Image(image)])
    );
    assert_eq!(
        classify(&item(vec![text("/etc/"), text("hosts")])),
        Paste::Text("/etc/hosts".into())
    );
    assert_eq!(
        classify(&item(vec![
            text(""),
            ClipboardEntry::ExternalPaths(ExternalPaths(Vec::new().into()))
        ])),
        Paste::Nothing
    );
}

/// A pasted image's name is the only thing that tells the agent reading the path what the bytes are, so its extension
/// must follow the type. Specified by a literal table, not by rerunning the derivation: every type GPUI reads names its
/// usual extension (Netpbm's over-long subtype falls back to `bin`), and the old rule's shapes hold (parameters and case
/// ignored, a `+suffix` dropped, a vendor tree cut to its last segment, `x-` stripped, aliases applied, and `bin` for
/// anything that yields nothing usable). Replaces the old Rust `extensions_come_from_the_mime_type_for_any_image_subtype`
/// and `the_shipped_aliases_are_the_corrections_the_rule_applies`, and `clipboard-name.test.js`'s "extension derivation
/// covers normalized and hostile MIME spellings" (with the Rust alias table, so `image/x-icon` is `ico`).
#[test]
fn an_images_extension_comes_from_its_type() {
    for (format, extension) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpg"),
        (ImageFormat::Webp, "webp"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::Svg, "svg"),
        (ImageFormat::Bmp, "bmp"),
        (ImageFormat::Tiff, "tiff"),
        (ImageFormat::Ico, "ico"),
        (ImageFormat::Pnm, "bin"),
    ] {
        assert_eq!(
            image_extension_for(format.mime_type()),
            extension,
            "{format:?}"
        );
    }
    for (mime, extension) in [
        ("IMAGE/PNG; charset=binary", "png"),
        ("  image/gif  ", "gif"),
        ("image/avif", "avif"),
        ("image/heic", "heic"),
        ("image/jxl", "jxl"),
        ("image/vnd.microsoft.icon", "ico"),
        ("image/vnd.example.webp", "webp"),
        ("image/x-icon", "ico"),
        ("image/vnd.adobe.photoshop", "photoshop"),
        ("image/this-extension-is-far-too-long", "bin"),
        ("image/!!!", "bin"),
        ("image/", "bin"),
        ("application/octet-stream", "bin"),
        ("", "bin"),
    ] {
        assert_eq!(image_extension_for(mime), extension, "{mime}");
    }
}

/// The host path's directories come from where the user keeps Farhelm's state, which may hold a space, an apostrophe
/// or a `$`; typed bare, such a path reaches the agent as several words or, worse, gets expanded. Specified: a path of
/// only safe characters is typed bare; anything else, and the empty path, is single-quoted with each `'` escaped, and a
/// real `sh` reads every one of them back as exactly one word, unchanged. Replaces the old Rust
/// `paths_a_shell_would_mangle_are_the_ones_that_need_quoting` and the spec's "a path whose directories need quoting
/// arrives at the shell as one word" (there through a tab's real shell, here through `sh` directly).
#[test]
fn a_typed_path_is_one_shell_word() {
    let bare = "/home/dev/.local/state/farhelm/attachments/s-1/a-b_c.d@e%f+g=h:i,j.png";
    assert_eq!(shell_safe_path(bare), bare);
    for path in [
        "/Users/dev/Library/Application Support/farhelm/a.png",
        "/home/o'brien/state/a.png",
        "/tmp/fh attach '1'/attachments/s/shot.png",
        "/tmp/$(echo injected)/a.png",
        "/tmp/back\\slash/a.png",
        "/tmp/star*/a ~#!.png",
        "",
    ] {
        let quoted = shell_safe_path(path);
        assert!(quoted.starts_with('\''), "{path:?} is quoted: {quoted}");
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("set -- {quoted}; printf '%s|%s' \"$#\" \"$1\""))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sh read {quoted}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("1|{path}"));
    }
}

// ---- Harness ---------------------------------------------------------------------------------------------------------

/// The window's root: the terminal in a fixed 816 by 496 pane, below it a button that focus can move to and the
/// further terminals a test adds ([`Harness::add`]), each in a smaller pane, wired through [`root`] as the app's root
/// views are.
struct Pane {
    owner: Entity<AttachedTerminal>,
    elsewhere: FocusHandle,
    extra: Extra,
    root: root::Attachment,
}

/// The further terminals, which a test adds and takes away.
type Extra = Rc<RefCell<Vec<Entity<AttachedTerminal>>>>;

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = div()
            .debug_selector(|| MAIN_PANE.into())
            .w(px(816.))
            .h(px(496.))
            .child(self.owner.clone());
        let elsewhere = test_support::button("elsewhere", &self.elsewhere);
        let owners = self.extra.borrow().clone();
        let extra = owners.iter().map(|owner| {
            div()
                .debug_selector(|| EXTRA_PANE.into())
                .w(px(816.))
                .h(px(160.))
                .child(owner.clone())
        });
        let extra: Vec<_> = extra.collect();
        root::wrap(
            div()
                .size_full()
                .child(pane)
                .child(elsewhere)
                .children(extra),
            &self.root,
            window,
            cx,
        )
    }
}

/// The snapshot a fresh helm sends: an empty preferences row.
fn preferences() -> Vec<CollectionState> {
    vec![CollectionState {
        collection: Collection::Preferences,
        meta: test_support::meta(&[PREFERENCES_KEY]),
        entries: vec![KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value: serde_json::json!({}),
        }],
    }]
}

/// The selectors of the main terminal's pane and of an added terminal's (the tests add one at a time).
const MAIN_PANE: &str = "attachments-test-main-pane";
const EXTRA_PANE: &str = "attachments-test-extra-pane";

/// An attached terminal of session `s1`, drawn, live and focused, uploading through the fake transfers.
struct Harness<'a> {
    cx: &'a mut VisualTestContext,
    owner: Entity<AttachedTerminal>,
    /// The focus handle of the button beside the terminal.
    elsewhere: FocusHandle,
    stream: Rc<RefCell<Option<FakeTerminal>>>,
    transfers: FakeTransfers,
    /// The terminals a test added, drawn below the main one.
    extra: Extra,
    /// The UI session the terminals were made for, for terminals a test adds.
    session: Entity<session::UiSession>,
    /// The UI channel's helm: it keeps the UI session the terminal was made for current, and a test breaks and reopens
    /// the channel through it.
    helm: FakeHelm,
    /// The window's connection, which makes a new UI session after a reset.
    connection: Entity<session::Connection>,
}

/// A connected app with the terminal drawn in its own window, its UI session's transfers the fake's (served through
/// the connection before the snapshot that makes the session, as the app's own are), its stream attached showing a
/// blank screen, and focus on it, so the Paste action reaches it as the user's would.
fn harness(cx: &mut TestAppContext) -> Harness<'_> {
    let (transfers, served) = FakeTransfers::new();
    harness_with(cx, Some(served), transfers)
}

/// [`harness`], on a connection with no helm client and nothing served: a UI session with no transfers, which only
/// tests make.
fn unserved(cx: &mut TestAppContext) -> Harness<'_> {
    let (transfers, _) = FakeTransfers::new();
    harness_with(cx, None, transfers)
}

/// The harness, its UI session's transfers `served` (none when `None`), with the test's side of them, `transfers`.
fn harness_with(
    cx: &mut TestAppContext,
    served: Option<Transfers>,
    transfers: FakeTransfers,
) -> Harness<'_> {
    test_support::install(cx);
    let (mut helm, handle, events) = FakeHelm::new();
    let connection = cx.update(|cx| session::start(handle, events, None, cx));
    if let Some(served) = served {
        cx.update(|cx| connection.update(cx, |connection, _| connection.serve_transfers(served)));
    }
    helm.connect(preferences());
    cx.run_until_parked();
    let session = cx.update(|cx| connection.read(cx).session().cloned().unwrap());
    let stream: Rc<RefCell<Option<FakeTerminal>>> = Rc::default();
    let extra: Extra = Rc::default();
    let (made, drawn) = (session.clone(), extra.clone());
    let (pane, cx) = test_support::open_view(cx, |window, cx| {
        let owner = cx.new(|cx| terminal(&made, "s1", None, &stream, cx));
        Pane {
            owner,
            elsewhere: cx.focus_handle().tab_stop(true),
            extra: drawn,
            root: root::attach(window, cx),
        }
    });
    let (owner, elsewhere) = cx.update(|_, cx| {
        let pane = pane.read(cx);
        (pane.owner.clone(), pane.elsewhere.clone())
    });
    let mut h = Harness {
        cx,
        owner,
        elsewhere,
        stream,
        transfers,
        extra,
        session,
        helm,
        connection,
    };
    h.attach();
    let handle = h.cx.update(|_, cx| h.owner.read(cx).focus_handle().clone());
    h.cx.update(|window, cx| {
        let chosen = focus::Mark::take(cx);
        focus::choose_terminal(&handle, &chosen, window, cx);
    });
    settle(h.cx);
    h
}

/// An attached terminal of session `session` (its tab `tab`, or its agent terminal), whose opener leaves the fake
/// stream it opens in `stream`.
fn terminal(
    ui: &Entity<session::UiSession>,
    session: &str,
    tab: Option<&str>,
    stream: &Rc<RefCell<Option<FakeTerminal>>>,
    cx: &mut Context<AttachedTerminal>,
) -> AttachedTerminal {
    let slot = stream.clone();
    let open: OpenStream = Box::new(move |attach| {
        let (fake, handle, events) = FakeTerminal::open(&attach);
        *slot.borrow_mut() = Some(fake);
        (handle, events)
    });
    let target = TerminalTarget {
        session: session.into(),
        tab: tab.map(Into::into),
        lease: AttachmentLease::new(),
    };
    let metrics = CellMetrics::new(10., 22., 16.).unwrap();
    AttachedTerminal::new(ui, target, metrics, open, cx)
}

/// Attach `stream` (again, after a dropped link) and replay a blank screen, which makes `owner` live.
fn attach_stream(
    cx: &mut VisualTestContext,
    owner: &Entity<AttachedTerminal>,
    stream: &Rc<RefCell<Option<FakeTerminal>>>,
) {
    {
        let mut stream = stream.borrow_mut();
        let stream = stream
            .as_mut()
            .expect("the premise: the measured terminal opened its stream");
        let asked = stream.attach();
        for message in test_support::blank_screen(asked.cols, asked.rows) {
            stream.output(message);
        }
        stream.replay_complete();
    }
    settle(cx);
    assert!(
        cx.update(|_, cx| owner.read(cx).is_live()),
        "the premise: the terminal is live"
    );
}

/// The input a terminal's stream carried since the last call, with whether it reached its attachment.
fn sent_on(stream: &Rc<RefCell<Option<FakeTerminal>>>) -> Vec<(GridInput, bool)> {
    stream
        .borrow_mut()
        .as_mut()
        .unwrap()
        .received()
        .into_iter()
        .filter_map(|request| match request {
            FakeTerminalRequest::Input { input, delivered } => Some((input, delivered)),
            _ => None,
        })
        .collect()
}

/// The strip's lines in `owner`, as assistive technology reads them.
fn lines_of(cx: &mut VisualTestContext, owner: &Entity<AttachedTerminal>) -> Vec<String> {
    cx.update(|_, cx| {
        let attachments = owner.read(cx).attachments().clone();
        let lines = attachments.read(cx).lines_shown();
        lines.into_iter().map(String::from).collect()
    })
}

impl Harness<'_> {
    /// Attach the stream (again, after a dropped link) and replay a blank screen, which makes the terminal live.
    fn attach(&mut self) {
        let (owner, stream) = (self.owner.clone(), self.stream.clone());
        attach_stream(self.cx, &owner, &stream);
    }

    /// Add a live terminal of `session` (`tab`, or its agent terminal) below the main one, made for the same UI session
    /// and so uploading through the same transfers, and return it with its stream.
    fn add(
        &mut self,
        session: &str,
        tab: Option<&str>,
    ) -> (Entity<AttachedTerminal>, Rc<RefCell<Option<FakeTerminal>>>) {
        let stream: Rc<RefCell<Option<FakeTerminal>>> = Rc::default();
        let ui = self.session.clone();
        let owner = self
            .cx
            .update(|_, cx| cx.new(|cx| terminal(&ui, session, tab, &stream, cx)));
        self.extra.borrow_mut().push(owner.clone());
        redraw(self.cx);
        attach_stream(self.cx, &owner, &stream);
        (owner, stream)
    }

    /// Take the added terminals away and let them go, as a closed tab's or a left session's terminal goes.
    fn remove_added(&mut self) {
        self.extra.borrow_mut().clear();
        redraw(self.cx);
    }

    /// Drop `paths` on the middle of the pane drawn under `selector`, as the platform delivers a drop from a file
    /// manager: the files enter the window there, then are released.
    fn drop_files(&mut self, selector: &'static str, paths: Vec<PathBuf>) {
        let position = test_support::center(self.cx, selector);
        self.cx.simulate_event(FileDropEvent::Entered {
            position,
            paths: ExternalPaths(paths.into()),
        });
        self.cx.simulate_event(FileDropEvent::Submit { position });
        settle(self.cx);
    }

    /// Where the main terminal's grid is drawn.
    fn grid_bounds(&mut self) -> gpui::Bounds<gpui::Pixels> {
        test_support::frame(self.cx)
            .into_iter()
            .find(|node| node.element_id.as_deref() == Some("terminal-grid"))
            .expect("the premise: the grid is drawn")
            .bounds
    }

    /// Drop the stream's link, so the terminal reconnects and is not live.
    fn drop_link(&mut self) {
        self.stream.borrow_mut().as_mut().unwrap().drop_link();
        settle(self.cx);
        assert!(
            !self.cx.update(|_, cx| self.owner.read(cx).is_live()),
            "the premise: the dropped link left the terminal not live"
        );
    }

    /// Put `entries` on the clipboard and paste them, as Cmd+V or the menu's Paste does.
    fn paste(&mut self, entries: Vec<ClipboardEntry>) {
        self.cx.write_to_clipboard(ClipboardItem { entries });
        self.cx.dispatch_action(actions::Paste);
        settle(self.cx);
    }

    /// The input the terminal sent since the last call, with whether the stream carried each to its attachment.
    fn sent(&mut self) -> Vec<(GridInput, bool)> {
        sent_on(&self.stream)
    }

    /// The uploads started since the last call, as (what, proposed name, answer), all for session `s1`.
    fn uploads(&mut self) -> Vec<(UploadSource, String, FakeAnswer<String>)> {
        self.uploads_in("s1")
    }

    /// The uploads started since the last call, all for session `session`.
    fn uploads_in(&mut self, session: &str) -> Vec<(UploadSource, String, FakeAnswer<String>)> {
        self.transfers
            .take()
            .into_iter()
            .map(|transfer| match transfer {
                FakeTransfer::Upload {
                    session: asked,
                    source,
                    filename,
                    answer,
                } => {
                    assert_eq!(
                        asked, session,
                        "an upload goes to the terminal's own session"
                    );
                    (source, filename, answer)
                }
                other => panic!("only uploads go out on a paste: {other:?}"),
            })
            .collect()
    }

    /// The one upload started since the last call, which must be `source` proposed as `filename`, for session `s1`.
    fn upload(&mut self, source: UploadSource, filename: &str) -> FakeAnswer<String> {
        self.upload_in("s1", source, filename)
    }

    /// [`Harness::upload`], for session `session`.
    fn upload_in(
        &mut self,
        session: &str,
        source: UploadSource,
        filename: &str,
    ) -> FakeAnswer<String> {
        let mut uploads = self.uploads_in(session);
        assert_eq!(uploads.len(), 1, "one upload went out");
        let (asked, name, answer) = uploads.remove(0);
        assert_eq!((asked, name.as_str()), (source, filename));
        answer
    }

    /// Answer an upload and let its result arrive.
    fn answer(&mut self, answer: FakeAnswer<String>, result: Result<String, TransferError>) {
        answer.answer(result);
        settle(self.cx);
    }

    /// The attachment lines the terminal draws now, as assistive technology reads them, checked against the frame.
    fn lines(&mut self) -> Vec<String> {
        let owner = self.owner.clone();
        let lines = lines_of(self.cx, &owner);
        let drawn = test_support::label(self.cx, "terminal-attachment-status");
        assert_eq!(
            drawn,
            (!lines.is_empty()).then(|| lines.join("\n")),
            "the lines are drawn"
        );
        lines
    }
}

fn file(path: &str) -> ClipboardEntry {
    ClipboardEntry::ExternalPaths(ExternalPaths(vec![PathBuf::from(path)].into()))
}

fn text(text: &str) -> ClipboardEntry {
    ClipboardEntry::String(ClipboardString::new(text.into()))
}

/// The paste a typed path goes out as: one part, closing the paste.
fn typed(data: &str) -> GridInput {
    GridInput::Paste {
        data: data.into(),
        last: true,
    }
}

/// A real file of this checkout, so the directory check finds a regular file; the fake transfers never read it.
fn real_file() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
}

// ---- Pastes ----------------------------------------------------------------------------------------------------------

/// The whole point of an attachment is that the agent picks the path up with no manual copying, so a pasted file must
/// reach the host and its path the program, exactly once. Specified: a pasted file beside the text of its path uploads
/// once, the file itself under its own name, to the terminal's session; nothing is typed while it runs; once the host
/// answers, its path is typed as one closed paste (which the supervisor brackets for a program that asked), followed by
/// a space, focus staying where the user moved it meanwhile (a button outside the terminal), and no line is shown.
/// Replaces the paste half of the old spec's "a file dropped
/// on the agent terminal uploads and inserts its host path at the cursor", "a completed upload keeps focus where it was
/// and inserts through bracketed paste", and `clipboard-name.test.js`'s "a copied file keeps DataTransferItem
/// File.name".
#[gpui::test]
fn a_pasted_file_is_uploaded_and_its_host_path_typed(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let path = real_file();
    h.paste(vec![
        file(path.to_str().unwrap()),
        text(path.to_str().unwrap()),
    ]);
    let answer = h.upload(UploadSource::File(path), "Cargo.toml");
    assert_eq!(
        h.sent(),
        [],
        "nothing is typed while the upload runs, the path's text included"
    );
    // The user moves on while the upload runs, as the old test parked focus on the Hosts toggle.
    let elsewhere = h.elsewhere.clone();
    h.cx.update(|window, cx| {
        let chosen = focus::Mark::take(cx);
        focus::focus_control(&elsewhere, &chosen, window, cx);
    });
    settle(h.cx);
    assert!(
        test_support::focused(h.cx, &elsewhere) && !test_support::terminal_focused(h.cx),
        "the premise: focus left the terminal for the button"
    );

    h.answer(answer, Ok("/state/attachments/s1/Cargo.toml".into()));
    assert_eq!(
        h.sent(),
        [(typed("/state/attachments/s1/Cargo.toml "), true)],
        "typed though the terminal is not focused"
    );
    assert!(
        test_support::focused(h.cx, &elsewhere),
        "a completed upload leaves focus where it was"
    );
    assert_eq!(h.lines(), Vec::<String>::new());
    assert!(h.uploads().is_empty(), "one upload, once");
}

/// A pasted screenshot has no file name behind it, and its name is all the agent learns of its type. Specified: image
/// data beside a text label uploads its bytes, not the label, as `pasted-1.png`; the next pasted image is
/// `pasted-2.<its own extension>`; and the label is never typed. Replaces the old spec's "a pasted image is uploaded
/// under a generated name and its path inserted" and `clipboard-name.test.js`'s "a nameless clipboard image gets a
/// generated name" and "a freshly synthesized engine placeholder gets a generated name" (GPUI hands no placeholder
/// name, so the old freshness cases are superseded).
#[gpui::test]
fn a_pasted_image_is_uploaded_under_a_generated_name(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let png = Image::from_bytes(ImageFormat::Png, b"png bytes".to_vec());
    h.paste(vec![
        ClipboardEntry::Image(png),
        text("https://example.invalid/screenshot.png"),
    ]);
    let answer = h.upload(UploadSource::Bytes(b"png bytes".to_vec()), "pasted-1.png");
    h.answer(answer, Ok("/state/attachments/s1/pasted-1.png".into()));
    assert_eq!(
        h.sent(),
        [(typed("/state/attachments/s1/pasted-1.png "), true)]
    );

    let jpeg = Image::from_bytes(ImageFormat::Jpeg, b"jpeg bytes".to_vec());
    h.paste(vec![ClipboardEntry::Image(jpeg)]);
    let _held = h.upload(UploadSource::Bytes(b"jpeg bytes".to_vec()), "pasted-2.jpg");
}

/// SPEC.md: "pasted text that merely looks like a path is still text". Specified: a pasted absolute path is typed as a
/// paste, and nothing is uploaded or shown. Replaces the old spec's "pasted text that looks like a path arrives as
/// text, with nothing uploaded".
#[gpui::test]
fn pasted_text_that_looks_like_a_path_is_typed(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.paste(vec![text("/etc/hosts")]);
    assert_eq!(h.sent(), [(typed("/etc/hosts"), true)]);
    assert!(h.uploads().is_empty(), "text is not an attachment");
    assert_eq!(h.lines(), Vec::<String>::new());
}

/// SPEC.md rejects directories visibly in v1, and the failure to guard against is quietly typing the folder's path
/// instead. Specified: a pasted directory beside the text of its path uploads nothing and types nothing, and a line
/// names the directory and says it was not uploaded. Replaces the paste half of the old spec's "a dropped directory is
/// rejected visibly, uploading nothing and pasting nothing".
#[gpui::test]
fn a_pasted_directory_is_refused_visibly(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let folder = env!("CARGO_MANIFEST_DIR");
    h.paste(vec![file(folder), text(folder)]);
    assert!(
        h.uploads().is_empty(),
        "nothing directory-shaped reaches the helm"
    );
    assert_eq!(h.sent(), []);
    assert_eq!(
        h.lines(),
        ["farhelm-gui is a directory — this version attaches files only, so nothing was uploaded"]
    );
}

/// SPEC.md: "you can keep typing while it runs, and the path is inserted at whatever cursor position is current when
/// the transfer completes". The program's cursor is wherever the user's typing left it, in whichever attachment is
/// live then. Specified: a key typed while an upload runs goes to the program at once, the path follows it when the
/// upload completes; and a path whose upload completes after a reconnect goes to the new attachment, not the one that
/// was live at the paste. Replaces the old spec's "typing stays live during an upload, and the path lands at the cursor
/// position it finds".
#[gpui::test]
fn typing_goes_on_during_an_upload_and_the_path_lands_after_it(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let path = real_file();
    h.paste(vec![file(path.to_str().unwrap())]);
    let answer = h.upload(UploadSource::File(path.clone()), "Cargo.toml");
    keys(h.cx, "enter");
    let enter = GridInput::Key {
        key: "Enter".into(),
        code: String::new(),
        kc: 13,
        mods: 0,
        mac: THIS_PLATFORM == Platform::Mac,
    };
    assert_eq!(
        h.sent(),
        [(enter, true)],
        "typing reached the program during the upload"
    );
    h.answer(answer, Ok("/state/a".into()));
    assert_eq!(h.sent(), [(typed("/state/a "), true)]);

    h.paste(vec![file(path.to_str().unwrap())]);
    let answer = h.upload(UploadSource::File(path), "Cargo.toml");
    h.drop_link();
    h.attach();
    h.answer(answer, Ok("/state/b".into()));
    assert_eq!(
        h.sent(),
        [(typed("/state/b "), true)],
        "typed into the attachment live at completion"
    );
}

/// Uploads run one at a time (the old `send`), and one bad file must not cost the user the rest. Specified: two pasted
/// files upload in order, the second only once the first has answered; the first failing shows its line and the second
/// still uploads and is typed, each path its own word. Replaces the paste halves of the old spec's "a clipboard payload
/// of two real files uploads both, each under its own name", "two dropped files insert both paths, each its own word"
/// and "a failure on the first of two dropped files does not stop the second".
#[gpui::test]
fn pasted_files_upload_one_at_a_time_and_a_failure_stops_none(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let first = real_file();
    let second = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    h.paste(vec![ClipboardEntry::ExternalPaths(ExternalPaths(
        vec![first.clone(), second.clone()].into(),
    ))]);
    let answer = h.upload(UploadSource::File(first), "Cargo.toml");
    h.answer(
        answer,
        Err(TransferError::Refused {
            status: 404,
            message: "no such session".into(),
        }),
    );
    let answer = h.upload(UploadSource::File(second), "lib.rs");
    h.answer(answer, Ok("/state/lib.rs".into()));
    assert_eq!(h.sent(), [(typed("/state/lib.rs "), true)]);
    assert_eq!(
        h.lines(),
        [
            "Attaching Cargo.toml failed: no such session — it was not attached, and no path was inserted"
        ]
    );
}

/// Every failure must be visible and say whether the file was definitely not attached or that this cannot be told
/// (SPEC.md, "Attachments"), in the helm's own words where it sent some, drawn escaped and never read as a template.
/// Specified, one upload each: a refusal before publication (a 4xx) with its message, a server error with no words (its
/// status, outcome unknown), a lost answer (unknown), a success without a path (the old "no usable path", unknown), a
/// request that never reached the helm (not attached), and a local file that could not be read (not attached); none
/// types anything; all the lines stay until the next paste that attaches something, which replaces them. Replaces the
/// old spec's "an upload that fails surfaces the error and inserts no path", "a dead request and a malformed reply both
/// fail visibly", "error text carrying replacement tokens renders literally", "an item whose bytes cannot be read is
/// rejected before anything is uploaded" (here the helm client's read fails, before the body is complete) and "an upload
/// failure stays on screen" (the part within one terminal).
#[gpui::test]
fn each_failure_says_whether_the_file_was_attached(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let path = real_file();
    let path_text = path.to_str().unwrap().to_owned();
    // One paste of the same file six times, since a later paste would replace the earlier lines: the six uploads
    // run in turn, and each is failed a different way.
    h.paste(vec![ClipboardEntry::ExternalPaths(ExternalPaths(
        vec![path.clone(); 6].into(),
    ))]);
    for error in [
        TransferError::Refused {
            status: 413,
            message: "no\u{202E}space {name} $1 left".into(),
        },
        TransferError::Refused {
            status: 500,
            message: String::new(),
        },
        TransferError::Lost("connection reset".into()),
        TransferError::BadReply("missing field `path`".into()),
        TransferError::NotSent("connection refused".into()),
        TransferError::File("Permission denied".into()),
    ] {
        let answer = h.upload(UploadSource::File(path.clone()), "Cargo.toml");
        h.answer(answer, Err(error));
    }
    assert_eq!(h.sent(), [], "a failed upload types nothing");
    let failed = "Attaching Cargo.toml failed: ";
    let not = " — it was not attached, and no path was inserted";
    let unknown = " — whether it was attached cannot be told, and no path was inserted";
    assert_eq!(
        h.lines(),
        [
            format!("{failed}no<U+202E>space {{name}} $1 left{not}"),
            format!("{failed}The helm answered 500 with no message{unknown}"),
            format!("{failed}the connection to the helm was lost: connection reset{unknown}"),
            format!("{failed}The upload reply carried no usable path{unknown}"),
            format!("{failed}the request did not reach the helm: connection refused{not}"),
            format!("{failed}the file could not be read: Permission denied{not}"),
        ]
    );

    h.paste(vec![file(&path_text)]);
    assert_eq!(
        h.lines(),
        ["Attaching Cargo.toml…"],
        "the next paste that attaches replaces them with its own upload"
    );
    let _held = h.upload(UploadSource::File(path), "Cargo.toml");
}

/// A UI session with no transfers (a connection with no helm client, which only tests make) must still fail an
/// attachment visibly rather than let it vanish. Specified: a pasted file says the request did not reach the helm,
/// since there is no helm client, and that it was not attached; nothing is typed.
#[gpui::test]
fn with_no_helm_client_a_paste_fails_visibly(cx: &mut TestAppContext) {
    let mut h = unserved(cx);
    h.paste(vec![file(real_file().to_str().unwrap())]);
    assert_eq!(h.sent(), []);
    assert_eq!(
        h.lines(),
        [
            "Attaching Cargo.toml failed: the request did not reach the helm: this window has no helm client to attach \
             files through — it was not attached, and no path was inserted"
        ]
    );
}

/// A published attachment must never disappear silently, and a terminal that is not live has nowhere to type its path.
/// Specified: an upload that completes while the stream reconnects types nothing, then or once the terminal is live
/// again, and a line hands the user the path. Replaces the old spec's "an upload that outlives its socket reports the
/// path it could not insert" (the reconnect case; a terminal taken away mid-upload is
/// `a_closed_tab_reports_its_cut_off_uploads_in_the_agent_terminal`).
#[gpui::test]
fn an_upload_that_lands_with_no_live_terminal_shows_its_path(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let path = real_file();
    h.paste(vec![file(path.to_str().unwrap())]);
    let answer = h.upload(UploadSource::File(path), "Cargo.toml");
    h.drop_link();
    h.answer(answer, Ok("/state/attachments/s1/Cargo.toml".into()));
    h.attach();
    assert_eq!(
        h.sent(),
        [],
        "nothing typed into a later attachment the user did not paste into"
    );
    assert_eq!(
        h.lines(),
        [
            "Cargo.toml landed at /state/attachments/s1/Cargo.toml — the terminal was not taking input, so the path \
             was not inserted"
        ]
    );
}

// ---- Not live ----------------------------------------------------------------------------------------------------------

/// SPEC.md: an attachment must never disappear silently, and a terminal that is not live has nowhere to type a path.
/// The refusal is about the connection, so it must not outlive it, or it would sit on a working terminal. Specified: a
/// file pasted while the stream reconnects uploads nothing and shows the not-connected refusal in the strip; text pasted
/// then goes nowhere; once the terminal is live again the refusal goes. After the next lost link a dropped file is
/// refused the same way, and goes the same way. Replaces the paste and drop halves of the old spec's "a payload dropped
/// on a detached terminal is refused instead of uploaded", "the not-connected refusal clears when the terminal
/// reconnects", and the pane half of "a file dropped where no live terminal takes it is refused, never opened by the
/// page".
#[gpui::test]
fn a_paste_or_drop_into_a_terminal_that_is_not_live_is_refused_until_it_is_live(
    cx: &mut TestAppContext,
) {
    let mut h = harness(cx);
    h.drop_link();
    h.paste(vec![file(real_file().to_str().unwrap()), text("/home/u/a")]);
    assert!(h.uploads().is_empty(), "nothing is uploaded for a paste");
    assert_eq!(h.lines(), [DETACHED_TEXT]);
    h.paste(vec![text("typed")]);
    assert_eq!(h.sent(), [], "nothing is typed while not live");
    h.attach();
    assert_eq!(
        h.lines(),
        Vec::<String>::new(),
        "live again, the paste's refusal goes"
    );

    h.drop_link();
    h.drop_files(MAIN_PANE, vec![real_file()]);
    assert!(h.uploads().is_empty(), "nothing is uploaded for a drop");
    assert_eq!(h.lines(), [DETACHED_TEXT], "the drop's own refusal");
    h.attach();
    assert_eq!(
        h.lines(),
        Vec::<String>::new(),
        "live again, the drop's refusal goes"
    );
}

/// Going live takes away the refusal only: a failure beside it is about a file, not the connection, and must stay.
/// Specified: an upload started while live, then a paste refused after the link drops, then the running upload failing,
/// leave the refusal and the failure; once the terminal is live again only the failure remains.
#[gpui::test]
fn going_live_takes_away_the_refusal_and_keeps_a_failure(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.paste(vec![file(real_file().to_str().unwrap())]);
    let running = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    h.drop_link();
    h.paste(vec![file(real_file().to_str().unwrap())]);
    h.answer(
        running,
        Err(TransferError::Refused {
            status: 404,
            message: "no such session".into(),
        }),
    );
    let failed = "Attaching Cargo.toml failed: no such session — it was not attached, and no path was inserted";
    assert_eq!(h.lines(), [DETACHED_TEXT, failed], "the premise: both show");
    h.attach();
    assert_eq!(h.lines(), [failed]);
}

/// An inert terminal (a status card stands over it) takes nothing, and its veil covers the pane, so a drop lands on the
/// veil. Specified: a file dropped there is refused in the strip and uploads nothing, rather than vanishing; once the
/// veil lifts from the live terminal, the refusal goes. Focus moves off the terminal first, as the session view moves it
/// when it puts the card up: focus resting on an inert terminal, which takes nothing, is what the focus invariant the
/// harness checks after every step refuses.
#[gpui::test]
fn a_drop_on_an_inert_terminal_is_refused_until_the_veil_lifts(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let elsewhere = h.elsewhere.clone();
    h.cx.update(|window, cx| {
        let chosen = focus::Mark::take(cx);
        focus::focus_control(&elsewhere, &chosen, window, cx);
    });
    let owner = h.owner.clone();
    owner.update(h.cx, |owner, cx| owner.set_inert(true, cx));
    settle(h.cx);
    h.drop_files(MAIN_PANE, vec![real_file()]);
    assert!(h.uploads().is_empty());
    assert_eq!(h.lines(), [INERT_TEXT]);
    owner.update(h.cx, |owner, cx| owner.set_inert(false, cx));
    settle(h.cx);
    assert_eq!(h.lines(), Vec::<String>::new());
}

/// While its stream comes back, a terminal shows the reconnect card over its grid, and the card occludes the pointer,
/// so a drop on it never reaches the pane's own drop target under it; without a target of its own the file would be
/// lost without a word (SPEC.md: an attachment must never disappear silently). Specified: with the card drawn (the
/// premise, checked), a file dropped on the card itself uploads nothing and the strip shows the not-connected refusal.
#[gpui::test]
fn a_drop_on_the_reconnect_card_is_refused(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.drop_link();
    redraw(h.cx);
    assert!(
        h.cx.debug_bounds(RECONNECT_CARD_ID).is_some(),
        "the premise: the reconnect card is drawn over the terminal"
    );
    h.drop_files(RECONNECT_CARD_ID, vec![real_file()]);
    assert!(h.uploads().is_empty(), "nothing is uploaded for it");
    assert_eq!(h.lines(), [DETACHED_TEXT]);
}

// ---- Drops -----------------------------------------------------------------------------------------------------------

/// A drop is the other way to attach, and several files must reach the program as several words, in the order dropped,
/// or a shell sees one impossible path. Specified: two files dropped on the terminal upload one at a time under their
/// own names, and each path is typed as its own paste with its trailing space, first then second. Replaces the old
/// spec's "a file dropped on the agent terminal uploads and inserts its host path at the cursor" and "two dropped files
/// insert both paths, each its own word", and grid-attach's "a drop on a grid terminal uploads the file and pastes its
/// path without navigating".
#[gpui::test]
fn two_dropped_files_are_typed_as_two_words_in_order(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let second = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    h.drop_files(MAIN_PANE, vec![real_file(), second.clone()]);
    let answer = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    h.answer(answer, Ok("/state/a/Cargo.toml".into()));
    let answer = h.upload(UploadSource::File(second), "lib.rs");
    h.answer(answer, Ok("/state/a/lib.rs".into()));
    assert_eq!(
        h.sent(),
        [
            (typed("/state/a/Cargo.toml "), true),
            (typed("/state/a/lib.rs "), true)
        ]
    );
}

/// SPEC.md sends a drop to the terminal it lands on, a tab's included; the agent terminal beside it must not get the
/// path. Specified: a file dropped on a tab's pane, with the agent terminal drawn beside it, is uploaded once and its
/// path typed into the tab's stream only; a directory dropped there is refused in the tab's strip, the agent
/// terminal's staying empty. Replaces the old spec's "a file dropped on a tab inserts its path in that tab, leaving the
/// agent terminal untouched" and "an attachment failure stays in the terminal that received it".
#[gpui::test]
fn a_drop_on_a_tab_reaches_only_that_tab(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let (tab, tab_stream) = h.add("s1", Some("t1"));
    h.drop_files(EXTRA_PANE, vec![real_file()]);
    let answer = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    h.answer(answer, Ok("/state/t/Cargo.toml".into()));
    assert_eq!(
        sent_on(&tab_stream),
        [(typed("/state/t/Cargo.toml "), true)]
    );
    assert_eq!(h.sent(), [], "the agent terminal beside it gets nothing");
    h.drop_files(EXTRA_PANE, vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))]);
    assert_eq!(
        lines_of(h.cx, &tab),
        ["farhelm-gui is a directory — this version attaches files only, so nothing was uploaded"]
    );
    let main = h.owner.clone();
    assert_eq!(lines_of(h.cx, &main), Vec::<String>::new());
}

/// SPEC.md rejects dropped directories visibly in v1. Specified: a dropped directory uploads nothing, types nothing,
/// and its line names it. Replaces the drop half of the old spec's "a dropped directory is rejected visibly, uploading
/// nothing and pasting nothing".
#[gpui::test]
fn a_dropped_directory_is_refused_visibly(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.drop_files(MAIN_PANE, vec![PathBuf::from(env!("CARGO_MANIFEST_DIR"))]);
    assert!(h.uploads().is_empty());
    assert_eq!(h.sent(), []);
    assert_eq!(
        h.lines(),
        ["farhelm-gui is a directory — this version attaches files only, so nothing was uploaded"]
    );
}

// ---- The strip -------------------------------------------------------------------------------------------------------

/// A strip that took room from the grid would resize the program twice per upload, reflowing its scrollback and
/// redrawing a full-screen agent. Specified: while the strip shows, the grid is drawn at exactly its size and place
/// before, and no resize reaches the stream. Replaces the old spec's "the attachment indicator never resizes the
/// terminal".
#[gpui::test]
fn the_strip_never_resizes_the_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    redraw(h.cx);
    let before = h.grid_bounds();
    h.stream.borrow_mut().as_mut().unwrap().received();
    h.paste(vec![file(real_file().to_str().unwrap())]);
    let _held = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    redraw(h.cx);
    assert_eq!(
        h.lines(),
        ["Attaching Cargo.toml…"],
        "the premise: the strip shows"
    );
    assert_eq!(h.grid_bounds(), before);
    let resized = h
        .stream
        .borrow_mut()
        .as_mut()
        .unwrap()
        .received()
        .into_iter()
        .any(|request| matches!(request, FakeTerminalRequest::Resize { .. }));
    assert!(!resized, "no resize went out");
}

/// The busy line must describe what is still running, not what started last, or it names a file that already landed.
/// Specified: two pasted files read "Attaching 2 files…" while the first runs and the second waits, then the second's
/// own name once the first is done, and nothing once both are. Replaces the old spec's "the indicator names the upload
/// still running, not the one that just finished" and the busy half of "typing stays live during an upload".
#[gpui::test]
fn the_strip_names_what_is_running_and_counts_what_waits(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let second = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    h.paste(vec![ClipboardEntry::ExternalPaths(ExternalPaths(
        vec![real_file(), second.clone()].into(),
    ))]);
    assert_eq!(h.lines(), ["Attaching 2 files…"]);
    let answer = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    h.answer(answer, Ok("/state/Cargo.toml".into()));
    assert_eq!(h.lines(), ["Attaching lib.rs…"]);
    let answer = h.upload(UploadSource::File(second), "lib.rs");
    h.answer(answer, Ok("/state/lib.rs".into()));
    assert_eq!(h.lines(), Vec::<String>::new());
}

// ---- A terminal going away -------------------------------------------------------------------------------------------

/// SPEC.md: an attachment must never disappear silently, and a closed tab takes its terminal with it. Specified: a tab
/// closed while one dropped file uploads and another waits abandons the running request, and the session's agent
/// terminal then shows a line for each: the running one's outcome cannot be told, the waiting one was never sent.
/// Replaces the old spec's "uploads cut off by a reconnect are reported by the remounted terminal" (here a terminal
/// goes away whole; a stream's reconnect keeps its uploads, as
/// `typing_goes_on_during_an_upload_and_the_path_lands_after_it` shows).
#[gpui::test]
fn a_closed_tab_reports_its_cut_off_uploads_in_the_agent_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let (tab, _stream) = h.add("s1", Some("t1"));
    let second = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    h.drop_files(EXTRA_PANE, vec![real_file(), second]);
    let running = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    drop(tab);
    h.remove_added();
    assert!(
        running.abandoned(),
        "the request was abandoned with its terminal"
    );
    assert_eq!(
        h.lines(),
        [
            "Attaching Cargo.toml was interrupted — whether it was attached cannot be told, and no path was inserted",
            "Attaching lib.rs was interrupted before it was sent, so nothing was uploaded",
        ]
    );
}

/// A failure the user has not read yet, and an upload cut off by leaving the session, must still be there when the
/// session's terminal is back, or the file vanishes without a word. Specified: a session's agent terminal that goes
/// away (the session left) with a failure on its strip and an upload running leaves both, the upload as cut off; the
/// session's next agent terminal shows them, in that order. Replaces the old spec's "an upload failure stays on screen
/// across a reconnect" (here across the terminal being made again) and "leaving mid-upload and reopening leaves exactly
/// one set of hooks" (a reopened terminal uploads nothing twice: it never saw the first upload).
#[gpui::test]
fn a_failure_and_a_cut_off_upload_outlive_their_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let (owner, _stream) = h.add("s2", None);
    let second = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"));
    h.drop_files(EXTRA_PANE, vec![real_file(), second.clone()]);
    let answer = h.upload_in("s2", UploadSource::File(real_file()), "Cargo.toml");
    h.answer(
        answer,
        Err(TransferError::Refused {
            status: 500,
            message: "no space left on device".into(),
        }),
    );
    let _running = h.upload_in("s2", UploadSource::File(second), "lib.rs");
    drop(owner);
    h.remove_added();

    let (owner, _stream) = h.add("s2", None);
    assert_eq!(
        lines_of(h.cx, &owner),
        [
            "Attaching Cargo.toml failed: no space left on device — whether it was attached cannot be told, and no \
             path was inserted",
            "Attaching lib.rs was interrupted — whether it was attached cannot be told, and no path was inserted",
        ]
    );
    assert!(h.uploads_in("s2").is_empty(), "nothing is uploaded again");
    let main = h.owner.clone();
    assert_eq!(
        lines_of(h.cx, &main),
        Vec::<String>::new(),
        "another session's terminal shows nothing of it"
    );
}

/// A reset of the window's connection retires its UI session and every terminal with it (SPEC_impl.md, "One failure
/// path: the reset"), and an upload running then must neither go on unseen nor vanish (SPEC.md, "Attachments").
/// Specified: breaking the channel while an upload runs retires the old UI session and abandons the request; once the
/// channel is back, the session's new agent terminal shows the upload as cut off, its outcome not to be told.
#[gpui::test]
fn a_reset_reports_a_cut_off_upload_in_the_new_agent_terminal(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    let old = h.session.clone();
    h.paste(vec![file(real_file().to_str().unwrap())]);
    let running = h.upload(UploadSource::File(real_file()), "Cargo.toml");

    h.helm.break_channel();
    settle(h.cx);
    assert!(
        h.cx.update(|_, cx| old.read(cx).is_retired()),
        "the premise: the reset retired the old UI session"
    );
    assert!(running.abandoned(), "the request went with the terminal");

    h.helm.connect(preferences());
    settle(h.cx);
    let connection = h.connection.clone();
    let new =
        h.cx.update(|_, cx| connection.read(cx).session().cloned())
            .expect("the premise: the channel is back with a new UI session");
    assert!(new != old, "the premise: a new UI session");
    h.session = new;
    let (owner, _stream) = h.add("s1", None);
    assert_eq!(
        lines_of(h.cx, &owner),
        [
            "Attaching Cargo.toml was interrupted — whether it was attached cannot be told, and no path was inserted"
        ]
    );
}

/// The lines a reset leaves must not stay for the life of the app once their session is gone, while those of a session
/// still there must wait for it (SPEC_impl.md, "One failure path: the reset"). Specified: a reset cutting off an upload
/// in session `s1` and one in `s2` leaves lines for both; the next snapshot's complete listing names `s1` only, and
/// with it `s2`'s lines go while `s1`'s stay, so `s1`'s next agent terminal shows its line and `s2`'s shows nothing.
#[gpui::test]
fn after_a_reset_the_lines_of_a_session_gone_from_the_listing_go(cx: &mut TestAppContext) {
    let mut h = harness(cx);
    h.paste(vec![file(real_file().to_str().unwrap())]);
    let _s1 = h.upload(UploadSource::File(real_file()), "Cargo.toml");
    let (_other, _stream) = h.add("s2", None);
    h.drop_files(EXTRA_PANE, vec![real_file()]);
    let _s2 = h.upload_in("s2", UploadSource::File(real_file()), "Cargo.toml");

    h.helm.break_channel();
    settle(h.cx);
    let carried = |h: &mut Harness<'_>, key: &str| {
        h.cx.update(|_, cx| {
            cx.try_global::<Carried>()
                .is_some_and(|carried| carried.0.contains_key(key))
        })
    };
    assert!(
        carried(&mut h, "s1") && carried(&mut h, "s2"),
        "the premise: the reset left lines for both sessions"
    );

    let mut snapshot = preferences();
    snapshot.push(test_support::sessions(&["s1"]));
    h.helm.connect(snapshot);
    settle(h.cx);
    assert!(
        !carried(&mut h, "s2"),
        "a session the listing no longer has loses its lines"
    );
    assert!(carried(&mut h, "s1"), "a session still listed keeps them");

    let connection = h.connection.clone();
    h.session =
        h.cx.update(|_, cx| connection.read(cx).session().cloned())
            .expect("the premise: the channel is back with a new UI session");
    h.remove_added();
    let (s2, _stream) = h.add("s2", None);
    assert_eq!(lines_of(h.cx, &s2), Vec::<String>::new());
    h.remove_added();
    let (s1, _stream) = h.add("s1", None);
    assert_eq!(
        lines_of(h.cx, &s1),
        [
            "Attaching Cargo.toml was interrupted — whether it was attached cannot be told, and no path was inserted"
        ]
    );
}

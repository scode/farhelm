//! Attachments: a file or an image pasted or dropped into a session terminal is uploaded to the session's host, and the
//! host path it lands at is typed at the program's cursor (SPEC.md, "Attachments"; the old app's `installAttachments`
//! in terminal.js and the rules in `crates/farhelm-ui/src/attachments.rs`, both at the reference commit).
//!
//! Here's the TLDR of the rules this module keeps, each a decision the old app made and SPEC.md pins:
//!
//! - One winning flavor per paste ([`Flavor`]): file references, then image data, then plain text. A clipboard holding
//!   a file and the text of its path (a Finder copy does) attaches the file and types nothing else; text that merely
//!   looks like a path is text. There is no path heuristic anywhere to get wrong.
//! - A directory is refused visibly ([`DIRECTORY_TEXT`]); nothing of it is uploaded.
//! - Uploads run one at a time, in the order pasted (the old `send`). The bytes move on the helm client's runtime and
//!   this side only awaits the answer, so typing goes on meanwhile.
//! - The path is typed when its upload completes, into whatever attachment is live then, as a paste (so the supervisor
//!   brackets it for a program that asked) and shell-quoted ([`shell_safe_path`]), one word with a trailing space.
//! - Every failure is shown in the strip at the bottom right of the terminal ([`strip`]), which takes no room from the
//!   grid, and says whether the upload was definitely not attached or its outcome cannot be told
//!   ([`TransferError::upload_outcome`]). A failure stays until the next paste or drop this module acts on, never on a
//!   timer, and outlives its terminal ([`Carried`]).
//! - Files or an image offered to a terminal that is not live are refused ([`DETACHED_TEXT`]), and the refusal goes
//!   once the terminal is live again.
//! - A terminal that goes away with uploads running or waiting preserves each outcome ([`Attachments::hand_over`]).
//!   The next terminal can report them while the session stays open. Leaving the session puts them in the notice
//!   center so the user learns about the interruption even after the session is gone.
//!
//! The terminal's owner ([`super::attached`]) hands a classified paste or a drop to [`Attachments::attach`] and types
//! each [`Uploaded`] event's path itself, because only it knows which attachment is live when the upload completes.
//! Drops reach a terminal through its own pane only; a file dropped anywhere else in the window opens nothing, since a
//! native window has no default action for a drop (the old page needed a window-wide guard, because a browser's
//! default opened the file).
//!
//! Uploads go through the UI session's transfers, which the terminal's owner hands over as it makes this. Only a
//! connection with no helm client has none, which the app never makes; there every upload fails visibly, saying so
//! ([`NO_HELM_CLIENT`]). Tests serve the fake through the connection, as the app's own transfers are served.
//!
//! On Wayland, GPUI turns local URIs from a file manager's clipboard into file entries. Offers with only web links
//! can still supply image or text entries. X11 uses GPUI's synchronous read, which reports text and images.

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use farhelm_helm_client::{TransferError, Transfers, UploadOutcome};
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    AnyElement, App, ClipboardEntry, ClipboardItem, Context, ElementId, EventEmitter, Global,
    Image, InteractiveElement as _, IntoElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, div, px, relative,
};

use crate::components::Role;
use crate::host_text::{HostSentence, HostText};
use crate::local_state::{self, LocalState};
use crate::notices::{self, InterruptedUploads, Notice};
use crate::session;
use crate::theme;

// ---- The rules ---------------------------------------------------------------------------------------------------------

/// The prefix of the name a pasted image is uploaded under: `pasted-<n>.<ext>`, the old app's shape.
///
/// GPUI hands a pasted image over as bytes and a format, with no file name behind it, so every pasted image gets a
/// generated name. The old app's test of whether a browser-made `image.png` was really a fresh screenshot does not come
/// back: there is no name to judge. Uniqueness is not this name's job: the counter is per terminal, so two terminals can
/// both paste `pasted-1.png`, and the supervisor publishes the second under another name.
const GENERATED_NAME_PREFIX: &str = "pasted-";

/// What is typed after each host path: one space, so a second attachment's path never fuses onto the first's, and the
/// cursor ends where the user's next word goes (the old `PATH_SEPARATOR`, whose docs tell why a trailing separator
/// beats one placed between paths).
const PATH_SEPARATOR: &str = " ";

/// Every character a host path may contain and still be typed into a shell unquoted (the old `SHELL_SAFE_PATH_CHARS`).
///
/// The supervisor sanitizes the file name, but the directories above it come from where the user keeps Farhelm's state,
/// which may hold a space (`Application Support`), an apostrophe or a `$`. Conservative on purpose: `~`, `#` and `!` are
/// safe inside a word for most shells and are quoted anyway, because a needless pair of quotes costs nothing and a
/// missing one costs the user the attachment.
const SHELL_SAFE_PATH_CHARS: &str = concat!(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    "abcdefghijklmnopqrstuvwxyz",
    "0123456789",
    "_@%+=:,./-",
);

/// The extension of an image whose type names nothing usable: not an image type, or a subtype that reduces to nothing
/// or to something too long to be an extension. `bin` rather than a plausible `png`, because naming unknown bytes after
/// a format they probably are not misleads whoever reads the path.
const FALLBACK_EXTENSION: &str = "bin";

/// The longest extension [`image_extension_for`] derives before giving up: past this a vendor subtype is noise glued to
/// a file name, not a file type.
const MAX_DERIVED_EXTENSION: usize = 12;

/// Image subtypes whose usual extension is not the subtype itself, as (derived token, extension).
const EXTENSION_ALIASES: &[(&str, &str)] = &[("jpeg", "jpg"), ("icon", "ico")];

/// One paste's winning interpretation: SPEC.md's flavors, plus the paste with nothing to act on.
///
/// Exactly one applies to a paste, which is what keeps a clipboard holding a file and its path from both uploading the
/// file and typing the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    /// One or more actual files (a directory among them, which is then refused).
    File,
    /// Image bytes with no file behind them: a screenshot on the clipboard.
    Image,
    /// Plain text, including text that looks like a path.
    Text,
    /// Nothing at all: not a failure, and not reported.
    None,
}

/// SPEC.md's precedence, applied to what a paste carries: file references beat image data, which beats text (the old
/// `classify`, unchanged).
pub(crate) fn flavor(has_file: bool, has_image: bool, has_text: bool) -> Flavor {
    if has_file {
        Flavor::File
    } else if has_image {
        Flavor::Image
    } else if has_text {
        Flavor::Text
    } else {
        Flavor::None
    }
}

/// What a paste does, once its flavor is decided.
#[derive(Debug, PartialEq)]
pub(crate) enum Paste {
    /// Type the clipboard's text as a paste.
    Text(String),
    /// Attach these, in order.
    Attach(Vec<Offered>),
    /// Do nothing.
    Nothing,
}

/// One thing a paste offers to attach.
#[derive(Debug, PartialEq)]
pub(crate) enum Offered {
    /// A local file (or directory) the clipboard refers to.
    File(PathBuf),
    /// Image bytes, in the format GPUI read them in.
    Image(Image),
}

/// Classify the clipboard `item` a paste read: its files when it holds any, otherwise its images, otherwise its text
/// (every text entry, joined). Empty text counts as no text, and an empty file list as no file, as in the old app.
///
/// The vendored macOS read never reports a file and an image together (a copied file comes with its path as text), so
/// the old note about an image left out beside a file does not come back: an image here is never a file of the user's.
pub(crate) fn classify(item: &ClipboardItem) -> Paste {
    let mut files = Vec::new();
    let mut images = Vec::new();
    let mut text = String::new();
    for entry in item.entries() {
        match entry {
            ClipboardEntry::ExternalPaths(paths) => {
                files.extend(paths.paths().iter().cloned().map(Offered::File));
            }
            ClipboardEntry::Image(image) => images.push(Offered::Image(image.clone())),
            ClipboardEntry::String(string) => text.push_str(string.text()),
        }
    }
    match flavor(!files.is_empty(), !images.is_empty(), !text.is_empty()) {
        Flavor::File => Paste::Attach(files),
        Flavor::Image => Paste::Attach(images),
        Flavor::Text => Paste::Text(text),
        Flavor::None => Paste::Nothing,
    }
}

/// The file extension a pasted image of MIME type `mime` is named with (the old `image_extension_for`): the subtype
/// reduced to a plausible token (parameters and a `+suffix` dropped, a vendor tree cut to its last segment, a leading
/// `x-` stripped, anything not alphanumeric removed), then corrected by [`EXTENSION_ALIASES`]. Derived rather than
/// looked up, so any `image/*` gets a sensible name; [`FALLBACK_EXTENSION`] when nothing usable is left.
///
/// GPUI names its formats' types (`ImageFormat::mime_type`), so a pasted PNG is `png`, a JPEG `jpg`, an icon
/// (`image/ico`) `ico`. Its Netpbm type, `image/x-portable-anymap`, reduces to a token over the length bound and is
/// named `bin`: the old rule, kept rather than special-cased for a format no clipboard is known to offer.
pub(crate) fn image_extension_for(mime: &str) -> String {
    let normalized = mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let Some(subtype) = normalized.strip_prefix("image/") else {
        return FALLBACK_EXTENSION.to_owned();
    };
    let subtype = subtype.split('+').next().unwrap_or("");
    let subtype = subtype.rsplit('.').next().unwrap_or("");
    let subtype = subtype.strip_prefix("x-").unwrap_or(subtype);
    let token: String = subtype
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    if token.is_empty() || token.len() > MAX_DERIVED_EXTENSION {
        return FALLBACK_EXTENSION.to_owned();
    }
    EXTENSION_ALIASES
        .iter()
        .find(|(from, _)| *from == token)
        .map_or(token, |(_, to)| (*to).to_owned())
}

/// `path` as it can be typed at a shell as one word: bare when every character is in [`SHELL_SAFE_PATH_CHARS`],
/// otherwise in POSIX single quotes, each embedded `'` closed, escaped and reopened (`'\''`). An empty path is quoted
/// too, so it cannot vanish from the command line (the old `shellSafePath`).
pub(crate) fn shell_safe_path(path: &str) -> String {
    if !path.is_empty() && path.chars().all(|c| SHELL_SAFE_PATH_CHARS.contains(c)) {
        return path.to_owned();
    }
    format!("'{}'", path.replace('\'', r"'\''"))
}

// ---- Wording ---------------------------------------------------------------------------------------------------------

/// After a directory's name: the old `DIRECTORY_TEXT`.
const DIRECTORY_TEXT: &str =
    " is a directory — this version attaches files only, so nothing was uploaded";

/// The end of a failure line when nothing was attached: the old `FAILED_TEXT`'s ending, with SPEC.md's required
/// statement that the upload is definitely not published.
const NOT_ATTACHED_TEXT: &str = " — it was not attached, and no path was inserted";

/// The end of a failure line when the request went out and its answer was lost: the host may or may not have kept the
/// file, and nothing here can tell which, so the line says exactly that and leans neither way (SPEC.md: a failure
/// "must distinguish a definitely unpublished upload from one whose publication outcome is unknown").
const UNKNOWN_TEXT: &str = " — whether it was attached cannot be told, and no path was inserted";

/// The reason for a refusal the helm sent no words with: the old `HTTP_STATUS_TEXT`.
const HTTP_STATUS_TEXT: &str = "The helm answered {status} with no message";

/// The reason for a success whose answer carried no path: the old `NO_PATH_TEXT`.
const NO_PATH_TEXT: &str = "The upload reply carried no usable path";

/// Why every upload fails for a terminal whose UI session has no transfers: only a connection with no helm client has
/// none, which the app never makes; tests that serve no transfers do.
const NO_HELM_CLIENT: &str = "this window has no helm client to attach files through";

/// Files or an image offered to a disconnected terminal: refused before anything is uploaded, since
/// no path could be typed (the old `DETACHED_TEXT`). It goes once the terminal is live again ([`Attachments::live`]).
pub(crate) const DETACHED_TEXT: &str =
    "This terminal is not connected, so nothing was attached — reconnect and try again";

/// A veil keeps a terminal from taking a drop even while its stream is still live. The veil is the "Session
/// unavailable" card, which shows exactly while the terminals are inert, so the line names that state in the card's
/// own word rather than leaving the user to guess why the drop did nothing.
const INERT_TEXT: &str = "This session is unavailable, so nothing was attached";

/// Between the name and the host path of a finished upload whose path could not be typed (the old `LANDED_TEXT`'s
/// start). The file is published, so the line names the path.
const LANDED_AT: &str = " landed at ";

/// The end of that line. The old line said "terminal detached" and "copy it from here"; this one is shown for a
/// terminal that is reconnecting or inert too, and the strip's text cannot be selected, so it says neither.
const LANDED_NOT_TYPED: &str = " — the terminal was not taking input, so the path was not inserted";

/// The end of the line for an upload the terminal's going away stopped before it started (the old
/// `INTERRUPTED_UNSENT_TEXT`'s ending): nothing reached the helm.
const INTERRUPTED_UNSENT_TEXT: &str =
    " was interrupted before it was sent, so nothing was uploaded";

/// The end of the line for an upload the terminal's going away stopped after it started. The old line said the file
/// "may have reached the host anyway"; like every unknown outcome here, this one says only that it cannot be told.
const INTERRUPTED_UNKNOWN_TEXT: &str =
    " was interrupted — whether it was attached cannot be told, and no path was inserted";

// ---- The strip -------------------------------------------------------------------------------------------------------

/// The strip at the bottom right of a terminal (the old `.attach-status`): `busy` says what is uploading, then come
/// `lines`, the failures and refusals, oldest first. `None` when there is nothing to say.
///
/// Overlaid on the terminal rather than placed beside it, because a line that took room from the grid would resize the
/// program twice per upload, reflowing its scrollback and making a full-screen agent redraw (the old stylesheet's
/// reason, which holds the same here). Bottom right and as wide as its text, at most 80% of the terminal, because the
/// cursor's row is at the bottom and its right end is almost always empty. It takes no pointer input: it has no
/// listener, so a selection dragged into its corner reaches the terminal, as the old `pointer-events: none` made it.
fn strip(busy: Option<HostSentence>, lines: &[HostSentence], cx: &App) -> Option<AnyElement> {
    if busy.is_none() && lines.is_empty() {
        return None;
    }
    let palette = theme::palette(cx);
    let label: SharedString = busy
        .iter()
        .chain(lines)
        .map(HostSentence::accessible_name)
        .collect::<Vec<_>>()
        .join("\n")
        .into();
    let busy = busy.map(|busy| div().text_color(theme::color(palette.fg_dim)).child(busy));
    let lines = lines
        .iter()
        .cloned()
        .map(|line| div().text_color(theme::color(palette.danger)).child(line));
    Some(
        div()
            .id(STRIP_ID)
            .debug_selector(|| STRIP_ID.into())
            .role(Role::Status)
            .aria_label(label)
            .absolute()
            .right_0()
            .bottom_0()
            .max_w(relative(0.8))
            .px(px(10.))
            .py(px(2.))
            .flex()
            .flex_col()
            .bg(theme::color(palette.bg_2))
            .text_color(theme::color(palette.fg_0))
            .text_size(theme::metrics::TEXT_SM)
            .children(busy)
            .children(lines)
            .into_any_element(),
    )
}

/// The strip's element id, and the selector tests and the bridge find it by.
pub(crate) const STRIP_ID: &str = "terminal-attachment-status";

/// A new line's element id. Process-wide, because lines move between terminals ([`Carried`]) and two with one id in
/// one frame would be one accessible node.
fn line_id() -> ElementId {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    (
        "terminal-attachment-line",
        NEXT.fetch_add(1, Ordering::Relaxed),
    )
        .into()
}

// ---- The queue -------------------------------------------------------------------------------------------------------

/// A finished upload, for the terminal's owner to type: [`Uploaded::typed`] at the cursor of the attachment live now,
/// or, with none live, [`Attachments::not_typed`].
#[derive(Clone, Debug)]
pub(crate) struct Uploaded {
    /// The name the upload was shown under: the file's own, or the generated one.
    name: HostText,
    /// The attachment's host path, as the helm reported it.
    path: String,
}

impl Uploaded {
    /// What to type: the path shell-quoted, then the separator.
    pub(crate) fn typed(&self) -> String {
        shell_safe_path(&self.path) + PATH_SEPARATOR
    }
}

/// Upload results left by a retired terminal, keyed by the same session key as the window selection.
///
/// The old app kept a pane's messages on its status element, which outlived the terminal a reconnect or a restart
/// remounted into it (SPEC.md: an attachment must never disappear silently). Here a terminal goes away whole when its
/// session is closed, its tab closes or the window's connection resets. Lines that were already shown wait for that
/// session's next terminal. An interrupted upload either follows the still-open session to its next terminal or
/// becomes a notice when the selection moves on. A reset keeps the selection through its disconnected window.
///
/// A store that proves a session is gone can prune its already-shown lines ([`prune_carried`]). The app-wide
/// observer owns interrupted entries, so an incomplete or filtered listing cannot hide these uploads.
#[derive(Default)]
pub(super) struct Carried(HashMap<String, CarriedLines>);

impl Global for Carried {}

/// The already visible failures and the structured interruptions waiting for their first delivery.
#[derive(Default)]
struct CarriedLines {
    lines: Vec<HostSentence>,
    interrupted: Vec<Interrupted>,
}

impl CarriedLines {
    fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.interrupted.is_empty()
    }
}

/// An abandoned upload retains the names as they were offered and how far Farhelm got before retiring its terminal.
struct Interrupted {
    name: HostText,
    title: Option<HostText>,
    sent: bool,
}

/// Watch uploads and selection for the app's lifetime, even while the window has no terminals.
pub(crate) fn install_carried(cx: &mut App) {
    cx.set_global(Carried::default());
    cx.observe_global::<Carried>(post_departed_uploads).detach();
    cx.observe_global::<LocalState>(post_departed_uploads)
        .detach();
}

/// Move interruptions from departed sessions to the notice center. The terminal can still take its older strip lines.
fn post_departed_uploads(cx: &mut App) {
    let selected = local_state::selected(cx);
    let Some(carried) = cx.try_global::<Carried>() else {
        return;
    };
    let departed: Vec<_> = carried
        .0
        .iter()
        .filter(|(key, entry)| selected.as_ref() != Some(*key) && !entry.interrupted.is_empty())
        .map(|(key, _)| key.clone())
        .collect();
    if departed.is_empty() {
        return;
    }
    let mut notices_to_post = Vec::new();
    let carried = cx.global_mut::<Carried>();
    for key in departed {
        let Some(entry) = carried.0.get_mut(&key) else {
            continue;
        };
        let (sent, unsent): (Vec<_>, Vec<_>) = std::mem::take(&mut entry.interrupted)
            .into_iter()
            .partition(|upload| upload.sent);
        for (sent, uploads) in [(true, sent), (false, unsent)] {
            if !uploads.is_empty() {
                // The session row may have arrived after the first file was offered. Use the first available
                // captured title so the notice can name the session even after the row goes away.
                let title = uploads
                    .iter()
                    .filter_map(|upload| upload.title.as_ref())
                    .find(|title| !title.raw().is_empty())
                    .cloned();
                let files = uploads.into_iter().map(|upload| upload.name).collect();
                notices_to_post.push(Notice::InterruptedUpload(Rc::new(InterruptedUploads::new(
                    title, files, sent,
                ))));
            }
        }
    }
    carried.0.retain(|_, entry| !entry.is_empty());
    for notice in notices_to_post {
        notices::post(notice, cx);
    }
}

/// Prune already-shown lines when the current store proves their session gone. Interrupted uploads belong to the
/// selection observer, even if disappearance in the listing and terminal retirement happen in the same update.
fn prune_carried(cx: &mut App) {
    let Some(carried) = cx.try_global::<Carried>() else {
        return;
    };
    if carried.0.is_empty() {
        return;
    }
    let Some(store) = session::current_store(cx) else {
        return;
    };
    let collections = store.read(cx).collections();
    let gone: Vec<String> = carried
        .0
        .iter()
        .filter(|(key, entry)| {
            !entry.lines.is_empty() && session::listing_proves_gone(collections, key)
        })
        .map(|(key, _)| key.clone())
        .collect();
    if gone.is_empty() {
        return;
    }
    let carried = cx.default_global::<Carried>();
    for key in gone {
        if let Some(entry) = carried.0.get_mut(&key) {
            entry.lines.clear();
        }
    }
    carried.0.retain(|_, entry| !entry.is_empty());
}

/// The attachments of one terminal: the uploads waiting and the one running, and the lines its strip shows. One per
/// terminal, made with it; dropping it, or retiring it with its terminal ([`Attachments::hand_over`]), drops the
/// running upload, which abandons the request, and leaves a line for every upload that did not finish.
pub(crate) struct Attachments {
    /// The session's served id, which the upload goes to.
    session: String,
    /// Where uploads go: the UI session's transfers, `None` only without a helm client ([`NO_HELM_CLIENT`]).
    transfers: Option<Transfers>,
    /// The number the next pasted image's generated name takes.
    pasted: u64,
    queue: VecDeque<Queued>,
    /// The upload in flight.
    running: Option<Running>,
    /// The lines on screen, oldest first: every failure since the last paste or drop acted on, and a refusal.
    lines: Vec<Line>,
    /// Set once the terminal went away; from then on nothing is started or taken.
    retired: bool,
    /// Hands the lines over when the entity goes, and (for an agent terminal) takes those carried for it.
    _carry: Vec<Subscription>,
    /// Prunes the carried lines of gone sessions whenever the current UI session's store changes ([`prune_carried`]).
    _prune: session::ObservingCurrent,
}

/// One upload waiting its turn.
struct Queued {
    /// The name it is shown under in every line about it.
    name: HostText,
    /// Capture this while the row still exists, before a Replace or Delete can remove it.
    title: Option<HostText>,
    /// The name proposed to the host: the file's own, or the generated one; empty lets the supervisor choose.
    filename: String,
    source: Source,
}

/// The upload in flight.
struct Running {
    name: HostText,
    title: Option<HostText>,
    /// Whether its request was handed to the helm client, after which its outcome is unknown if it is cut off.
    sent: bool,
    /// Dropping it abandons the request.
    _task: Task<()>,
}

/// One line of the strip.
struct Line {
    sentence: HostSentence,
    /// The not-connected refusal, which goes once the terminal is live and never moves to another terminal.
    refusal: bool,
}

/// Where a queued upload's bytes come from.
enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// How a queued upload ended.
enum Outcome {
    Uploaded(String),
    Directory,
    Failed(TransferError),
}

impl EventEmitter<Uploaded> for Attachments {}

impl Attachments {
    /// The attachments of a terminal of session `session`, uploading through `transfers`. The terminal can take the
    /// session's carried lines itself when a terminal set does not choose a recipient for them.
    pub(crate) fn new(
        session: String,
        takes_carried: bool,
        transfers: Option<Transfers>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut carry = vec![cx.on_release(|attachments, cx| attachments.hand_over(cx))];
        if takes_carried {
            carry.push(cx.observe_global::<Carried>(|attachments, cx| {
                attachments.take_carried(true, cx);
            }));
        }
        let prune = session::observe_current(cx, |_: &mut Self, cx| prune_carried(cx));
        prune_carried(cx);
        let mut attachments = Attachments {
            session,
            transfers,
            pasted: 0,
            queue: VecDeque::new(),
            running: None,
            lines: Vec::new(),
            retired: false,
            _carry: carry,
            _prune: prune,
        };
        if takes_carried {
            attachments.take_carried(true, cx);
        }
        attachments
    }

    /// Attach what a paste or a drop offered, in order, behind any upload still waiting. The lines before it go: a
    /// line stays on screen until the user does something that supersedes it, and offering files is that (the old
    /// `accept`). A generated image name is taken now, so names follow the order offered.
    pub(crate) fn attach(&mut self, offered: Vec<Offered>, cx: &mut Context<Self>) {
        if self.retired {
            return;
        }
        self.lines.clear();
        let title = session::current_store(cx).and_then(|store| {
            store
                .read(cx)
                .collections()
                .get::<SessionRow>(&self.session)
                .map(|row| HostText::from_host(row.info.title.as_str()))
        });
        for offered in offered {
            let queued = match offered {
                Offered::File(path) => {
                    let filename = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let shown = if filename.is_empty() {
                        path.to_string_lossy().into_owned()
                    } else {
                        filename.clone()
                    };
                    Queued {
                        name: HostText::from_host(shown),
                        title: title.clone(),
                        filename,
                        source: Source::File(path),
                    }
                }
                Offered::Image(image) => {
                    self.pasted += 1;
                    let filename = format!(
                        "{GENERATED_NAME_PREFIX}{}.{}",
                        self.pasted,
                        image_extension_for(image.format.mime_type())
                    );
                    Queued {
                        name: HostText::from_host(filename.clone()),
                        title: title.clone(),
                        filename,
                        source: Source::Bytes(image.bytes),
                    }
                }
            };
            self.queue.push_back(queued);
        }
        cx.notify();
        self.next(cx);
    }

    /// Files or an image were offered to a terminal that could not take their path: say so instead, replacing the
    /// lines before it, as the old app's refusal did. Nothing is uploaded.
    pub(crate) fn refuse(&mut self, inert: bool, cx: &mut Context<Self>) {
        let reason = if inert { INERT_TEXT } else { DETACHED_TEXT };
        self.lines.clear();
        self.lines.push(Line {
            sentence: HostSentence::new(line_id()).text(reason),
            refusal: true,
        });
        cx.notify();
    }

    /// The terminal is live again: the not-connected refusal, which asked for exactly this, goes. Every other line
    /// stays.
    pub(crate) fn live(&mut self, cx: &mut Context<Self>) {
        let before = self.lines.len();
        self.lines.retain(|line| !line.refusal);
        if self.lines.len() != before {
            cx.notify();
        }
    }

    /// Start the next waiting upload, unless one is running: one at a time, as the old `send` ran them.
    fn next(&mut self, cx: &mut Context<Self>) {
        if self.running.is_some() || self.retired {
            return;
        }
        let Some(queued) = self.queue.pop_front() else {
            return;
        };
        let (session, transfers) = (self.session.clone(), self.transfers.clone());
        let Queued {
            name,
            title,
            filename,
            source,
        } = queued;
        let shown = name.clone();
        let task = cx.spawn(async move |attachments, cx| {
            let outcome = match (source, transfers) {
                (_, None) => Outcome::Failed(TransferError::NotSent(NO_HELM_CLIENT.into())),
                (Source::File(path), Some(transfers)) => {
                    // Asked off the main thread, as every file read in the GUI is, so a slow or stuck disk never holds
                    // the window. A path that cannot be read is no directory: its upload fails with the reason.
                    let probe = path.clone();
                    let directory = cx
                        .background_executor()
                        .spawn(
                            async move { std::fs::metadata(probe).is_ok_and(|meta| meta.is_dir()) },
                        )
                        .await;
                    if directory {
                        Outcome::Directory
                    } else {
                        let upload = transfers.upload(&session, path, &filename);
                        let _ = attachments.update(cx, |attachments, _| attachments.sent());
                        ended(upload.await)
                    }
                }
                (Source::Bytes(bytes), Some(transfers)) => {
                    let upload = transfers.upload_bytes(&session, bytes, &filename);
                    let _ = attachments.update(cx, |attachments, _| attachments.sent());
                    ended(upload.await)
                }
            };
            let _ = attachments.update(cx, |attachments, cx| {
                attachments.finished(name, outcome, cx)
            });
        });
        self.running = Some(Running {
            name: shown,
            title,
            sent: false,
            _task: task,
        });
    }

    /// The running upload's request went to the helm client: from now on, cut off, its outcome is unknown.
    fn sent(&mut self) {
        if let Some(running) = &mut self.running {
            running.sent = true;
        }
    }

    /// An upload ended: hand a path to the owner to type, or show the failure, then start the next.
    fn finished(&mut self, name: HostText, outcome: Outcome, cx: &mut Context<Self>) {
        self.running = None;
        match outcome {
            Outcome::Uploaded(path) => cx.emit(Uploaded { name, path }),
            Outcome::Directory => {
                let line = HostSentence::new(line_id())
                    .host(&name)
                    .text(DIRECTORY_TEXT);
                self.push(line);
            }
            Outcome::Failed(error) => self.push(failure_line(&name, &error)),
        }
        cx.notify();
        self.next(cx);
    }

    /// The owner had no live attachment to type `uploaded`'s path into: show the path instead, so the published file
    /// is not lost from view.
    pub(crate) fn not_typed(&mut self, uploaded: &Uploaded, cx: &mut Context<Self>) {
        let path = HostText::from_host(uploaded.path.as_str());
        let line = HostSentence::new(line_id())
            .host(&uploaded.name)
            .text(LANDED_AT)
            .mono(&path)
            .text(LANDED_NOT_TYPED);
        self.push(line);
        cx.notify();
    }

    fn push(&mut self, sentence: HostSentence) {
        self.lines.push(Line {
            sentence,
            refusal: false,
        });
    }

    /// The terminal is going away (retired with its window's connection, or dropped): stop the running upload, which
    /// abandons its request, and leave the original upload facts for the still-open terminal or the notice center.
    /// Already-shown failures wait for the session's next terminal. A refusal was specific to this terminal.
    /// Once only; nothing starts afterwards.
    pub(crate) fn hand_over(&mut self, cx: &mut App) {
        if std::mem::replace(&mut self.retired, true) {
            return;
        }
        let lines: Vec<HostSentence> = self
            .lines
            .drain(..)
            .filter(|line| !line.refusal)
            .map(|line| line.sentence)
            .collect();
        let interrupted: Vec<_> = self
            .running
            .take()
            .map(|running| Interrupted {
                name: running.name,
                title: running.title,
                sent: running.sent,
            })
            .into_iter()
            .chain(self.queue.drain(..).map(|queued| Interrupted {
                name: queued.name,
                title: queued.title,
                sent: false,
            }))
            .collect();
        if !lines.is_empty() || !interrupted.is_empty() {
            let carried = cx.default_global::<Carried>();
            let entry = carried.0.entry(self.session.clone()).or_default();
            entry.lines.extend(lines);
            entry.interrupted.extend(interrupted);
        }
    }

    /// Take this terminal's session's old strip lines. Only a visible terminal of the current selection can
    /// report an interruption for the first time. Read first because updating the global notifies its observers.
    pub(super) fn take_carried(&mut self, shown: bool, cx: &mut Context<Self>) {
        let can_report =
            shown && local_state::selected(cx).as_deref() == Some(self.session.as_str());
        let waiting = cx
            .try_global::<Carried>()
            .and_then(|carried| carried.0.get(&self.session))
            .is_some_and(|entry| {
                !entry.lines.is_empty() || (can_report && !entry.interrupted.is_empty())
            });
        if self.retired || !waiting {
            return;
        }
        let carried = cx.default_global::<Carried>();
        let Some(entry) = carried.0.get_mut(&self.session) else {
            return;
        };
        let lines = std::mem::take(&mut entry.lines);
        // A retiring or hidden terminal must leave the facts for the notice center or a visible replacement.
        let interrupted = if can_report {
            std::mem::take(&mut entry.interrupted)
        } else {
            Vec::new()
        };
        if entry.is_empty() {
            carried.0.remove(&self.session);
        }
        for sentence in lines {
            self.push(sentence);
        }
        for upload in interrupted {
            self.push(interrupted_line(&upload.name, upload.sent));
        }
        cx.notify();
    }

    /// What the strip's first line says: the upload running, by name, or how many are running and waiting.
    fn busy(&self) -> Option<HostSentence> {
        let running = self.running.as_ref()?;
        let line = HostSentence::new(("terminal-attachment-busy", 0usize));
        Some(match self.queue.len() {
            0 => line.text("Attaching ").host(&running.name).text("…"),
            waiting => line.text(format!("Attaching {} files…", waiting + 1)),
        })
    }

    /// The strip's lines, as assistive technology reads them (each host value escaped): the busy line first, then the
    /// failures and refusals, oldest first. Test builds only; the strip names itself the same way.
    #[cfg(test)]
    pub(crate) fn lines_shown(&self) -> Vec<SharedString> {
        self.busy()
            .iter()
            .chain(self.lines.iter().map(|line| &line.sentence))
            .map(HostSentence::accessible_name)
            .collect()
    }

    /// The strip, for the owner to draw over the bottom of its terminal; `None` when there is nothing to say.
    pub(crate) fn strip(&self, cx: &App) -> Option<AnyElement> {
        let lines: Vec<HostSentence> = self
            .lines
            .iter()
            .map(|line| line.sentence.clone())
            .collect();
        strip(self.busy(), &lines, cx)
    }
}

/// The line for an upload of `name` that failed with `error`: the old `FAILED_TEXT`, with the reason drawn as the helm's
/// own words, escaped (they may come from another machine), and the ending saying what became of the file.
fn failure_line(name: &HostText, error: &TransferError) -> HostSentence {
    let reason = match error {
        TransferError::Refused { message, .. } if !message.is_empty() => message.clone(),
        TransferError::Refused { status, .. } => {
            HTTP_STATUS_TEXT.replace("{status}", &status.to_string())
        }
        TransferError::BadReply(_) => NO_PATH_TEXT.to_owned(),
        other => other.to_string(),
    };
    let ending = match error.upload_outcome() {
        UploadOutcome::NotPublished => NOT_ATTACHED_TEXT,
        UploadOutcome::Unknown => UNKNOWN_TEXT,
    };
    HostSentence::new(line_id())
        .text("Attaching ")
        .host(name)
        .text(" failed: ")
        .host(&HostText::from_host(reason))
        .text(ending)
}

/// The line for an upload of `name` its terminal's going away cut off, `sent` or not yet.
fn interrupted_line(name: &HostText, sent: bool) -> HostSentence {
    let ending = if sent {
        INTERRUPTED_UNKNOWN_TEXT
    } else {
        INTERRUPTED_UNSENT_TEXT
    };
    HostSentence::new(line_id())
        .text("Attaching ")
        .host(name)
        .text(ending)
}

/// An upload's answer as an [`Outcome`].
fn ended(answer: Result<String, TransferError>) -> Outcome {
    match answer {
        Ok(path) => Outcome::Uploaded(path),
        Err(error) => Outcome::Failed(error),
    }
}

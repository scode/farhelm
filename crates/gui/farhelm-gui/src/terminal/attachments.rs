//! Attachments by paste: a file or an image pasted into a session terminal is uploaded to the session's host, and the
//! host path it lands at is typed at the program's cursor (SPEC.md, "Attachments"; the old app's `installAttachments`
//! in terminal.js and the rules in `crates/farhelm-ui/src/attachments.rs`, both at the reference commit).
//!
//! NOTE: This covers pastes only. Drops, the status strip at the bottom of the terminal (with its "Attaching …" line),
//! and the report of uploads cut off when the terminal goes away come in the next change, which lands together with
//! this one. Until then a terminal dropped mid-upload (a reset, a closed tab) abandons its uploads without a word, and
//! failures show in the overlay at the top of the terminal, where a refused link click's explanation already sits, in
//! the old failure line's red. A file or an image pasted into a terminal that is not live is refused on that same
//! refusal line ([`DETACHED_TEXT`]).
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
//! - Every failure is shown, and says whether the upload was definitely not attached or its outcome cannot be told
//!   ([`TransferError::upload_outcome`]). A failure stays until the next paste this module acts on, never on a timer.
//!
//! The terminal's owner ([`super::attached`]) hands a classified paste to [`Attachments::attach`] and types each
//! [`Uploaded`] event's path itself, because only it knows which attachment is live when the upload completes.
//!
//! Uploads go through the UI session's transfers, which the terminal's owner hands over as it makes this. Only a
//! connection with no helm client has none, which the app never makes; there every upload fails visibly, saying so
//! ([`NO_HELM_CLIENT`]). Tests serve the fake through the connection, as the app's own transfers are served.
//!
//! NOTE: The vendored GPUI clipboard on Wayland never reports a file list, so a file copied in a Linux file manager
//! pastes whatever text the file manager offers (usually its path), as text. That is the platform read's limit, not
//! this module's; X11 falls back to GPUI's synchronous read, which reports text and images only too.

#[cfg(test)]
mod tests;

use std::collections::VecDeque;
use std::path::PathBuf;

use farhelm_helm_client::{TransferError, Transfers, UploadOutcome};
use gpui::{
    AnyElement, App, ClipboardEntry, ClipboardItem, Context, EventEmitter, Image,
    InteractiveElement as _, IntoElement as _, ParentElement as _, Pixels, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, div, px,
};

use crate::components::Role;
use crate::host_text::{HostSentence, HostText};
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

/// A paste of files or an image into a terminal that is not live, or is inert: refused before anything is uploaded,
/// since no path could be typed (the old `DETACHED_TEXT`). Shown by the owner on its refusal line.
pub(crate) const DETACHED_TEXT: &str =
    "This terminal is not connected, so nothing was attached — reconnect and try again";

/// Between the name and the host path of a finished upload whose path could not be typed (the old `LANDED_TEXT`'s
/// start). The file is published, so the line names the path.
const LANDED_AT: &str = " landed at ";

/// The end of that line. The old line said "terminal detached" and "copy it from here"; this one is shown for a
/// terminal that is reconnecting or inert too, and the overlay's text cannot be selected, so it says neither.
const LANDED_NOT_TYPED: &str = " — the terminal was not taking input, so the path was not inserted";

/// The tallest the failure lines are drawn: the refused click's line's bound, three lines of the small text with its
/// padding.
const STATUS_MAX_HEIGHT: Pixels = px(54.);

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

/// The attachments of one terminal: the uploads waiting and the one running, and the failure lines on screen. One per
/// terminal, made and dropped with it; dropping it drops the running upload, which abandons the request.
pub(crate) struct Attachments {
    /// The session's served id, which the upload goes to.
    session: String,
    /// Where uploads go: the UI session's transfers, `None` only without a helm client ([`NO_HELM_CLIENT`]).
    transfers: Option<Transfers>,
    /// The number the next pasted image's generated name takes.
    pasted: u64,
    queue: VecDeque<Queued>,
    /// The upload in flight. Dropping it abandons the request, whose outcome is then unknown on the host.
    running: Option<Task<()>>,
    /// The lines on screen: every failure since the last paste acted on, in order.
    failures: Vec<HostSentence>,
    /// How many lines this terminal has shown, which numbers the next line's element id.
    lines: usize,
}

/// One upload waiting its turn.
struct Queued {
    /// The name it is shown under in every line about it.
    name: HostText,
    /// The name proposed to the host: the file's own, or the generated one; empty lets the supervisor choose.
    filename: String,
    source: Source,
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
    /// The attachments of a terminal of session `session`, uploading through `transfers`.
    pub(crate) fn new(session: String, transfers: Option<Transfers>) -> Self {
        Attachments {
            session,
            transfers,
            pasted: 0,
            queue: VecDeque::new(),
            running: None,
            failures: Vec::new(),
            lines: 0,
        }
    }

    /// Attach what a paste offered, in order, behind any upload still waiting. The earlier paste's failures go: a
    /// failure stays on screen until the user does something that supersedes it, and a paste that attaches something
    /// is that (the old `accept`). A generated image name is taken now, so names follow the order pasted.
    pub(crate) fn attach(&mut self, offered: Vec<Offered>, cx: &mut Context<Self>) {
        self.failures.clear();
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

    /// Start the next waiting upload, unless one is running: one at a time, as the old `send` ran them.
    fn next(&mut self, cx: &mut Context<Self>) {
        if self.running.is_some() {
            return;
        }
        let Some(queued) = self.queue.pop_front() else {
            return;
        };
        let (session, transfers) = (self.session.clone(), self.transfers.clone());
        self.running = Some(cx.spawn(async move |attachments, cx| {
            let Queued {
                name,
                filename,
                source,
            } = queued;
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
                        ended(transfers.upload(&session, path, &filename).await)
                    }
                }
                (Source::Bytes(bytes), Some(transfers)) => {
                    ended(transfers.upload_bytes(&session, bytes, &filename).await)
                }
            };
            let _ = attachments.update(cx, |attachments, cx| {
                attachments.finished(name, outcome, cx)
            });
        }));
    }

    /// An upload ended: hand a path to the owner to type, or show the failure, then start the next.
    fn finished(&mut self, name: HostText, outcome: Outcome, cx: &mut Context<Self>) {
        self.running = None;
        match outcome {
            Outcome::Uploaded(path) => cx.emit(Uploaded { name, path }),
            Outcome::Directory => {
                let line = self.line().host(&name).text(DIRECTORY_TEXT);
                self.failures.push(line);
            }
            Outcome::Failed(error) => {
                let line = self.failure_line(&name, &error);
                self.failures.push(line);
            }
        }
        cx.notify();
        self.next(cx);
    }

    /// The owner had no live attachment to type `uploaded`'s path into: show the path instead, so the published file
    /// is not lost from view.
    pub(crate) fn not_typed(&mut self, uploaded: &Uploaded, cx: &mut Context<Self>) {
        let path = HostText::from_host(uploaded.path.as_str());
        let line = self
            .line()
            .host(&uploaded.name)
            .text(LANDED_AT)
            .mono(&path)
            .text(LANDED_NOT_TYPED);
        self.failures.push(line);
        cx.notify();
    }

    /// A new line, under an id of its own.
    fn line(&mut self) -> HostSentence {
        self.lines += 1;
        HostSentence::new(("terminal-attachment-line", self.lines))
    }

    /// The line for an upload of `name` that failed with `error`: the old `FAILED_TEXT`, with the reason drawn as the
    /// helm's own words, escaped (they may come from another machine), and the ending saying what became of the file.
    fn failure_line(&mut self, name: &HostText, error: &TransferError) -> HostSentence {
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
        self.line()
            .text("Attaching ")
            .host(name)
            .text(" failed: ")
            .host(&HostText::from_host(reason))
            .text(ending)
    }

    /// The lines on screen, as assistive technology reads them (each host value escaped), oldest first.
    pub(crate) fn lines_shown(&self) -> Vec<SharedString> {
        self.failures
            .iter()
            .map(HostSentence::accessible_name)
            .collect()
    }

    /// The failure lines, for the owner to draw in its top overlay; `None` when there is nothing to say. Drawn as the
    /// refused click's line is (`AttachedTerminal::refusal_line`), in the old failure line's red (`.attach-error`), and
    /// bounded to [`STATUS_MAX_HEIGHT`] so a pile of failures cannot cover the terminal: past it the stack scrolls, so
    /// no failure is cut off unseen.
    pub(crate) fn status_line(&self, cx: &App) -> Option<AnyElement> {
        if self.failures.is_empty() {
            return None;
        }
        let palette = theme::palette(cx);
        let label = self.lines_shown().join("\n");
        Some(
            div()
                .id("terminal-attachment-status")
                .debug_selector(|| "terminal-attachment-status".into())
                .role(Role::Status)
                .aria_label(label)
                .px(px(6.))
                .py(px(4.))
                .max_h(STATUS_MAX_HEIGHT)
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .bg(theme::color(palette.bg_2))
                .text_color(theme::color(palette.danger))
                .text_size(theme::metrics::TEXT_SM)
                .children(self.failures.iter().cloned())
                .into_any_element(),
        )
    }
}

/// An upload's answer as an [`Outcome`].
fn ended(answer: Result<String, TransferError>) -> Outcome {
    match answer {
        Ok(path) => Outcome::Uploaded(path),
        Err(error) => Outcome::Failed(error),
    }
}

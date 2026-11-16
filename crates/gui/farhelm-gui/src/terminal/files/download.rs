//! What an admitted click on a file link does: save the file into the user's Downloads through the helm, open the
//! completed save on the platform's modifier-click, and say how it went on the status line at the top of the terminal
//! (SPEC.md, "Terminal experience", the file-path bullet; the old app's `FileLinks.download` and `message` in
//! terminal-files.js at the reference commit).
//!
//! NOTE: Saving and opening both happen in the helm. The desktop app hands its embedded helm the Downloads folder and a
//! file opener, so a download here only asks for the save, with `open` for a modifier-click, and reports where the file
//! went: the helm picks the no-clobber name (`report (1).pdf`), publishes no partial file, checks the saved file's
//! type against the shared list of types to open, and opens it. No GUI code touches the file.
//!
//! The status line ports the old one message per terminal: "Downloading…" while a save runs, "Saved to …" when it is
//! done (gone after [`SUCCESS_SHOWN`]), and "Download failed: …" until the next press in the terminal or the next
//! download there. It shares its place with a refused click's explanation, and the newer of the two shows: a refusal
//! clears the status ([`FileLinks::clear_status`]), and a new status tells the owner to put a refusal away
//! ([`StatusShown`]), as the old message and refusal replaced each other in one element.

use std::time::Duration;

use farhelm_helm_client::{OpenOutcome, SavedDownload, TransferError};
use farhelm_input_mapping::Platform;
use gpui::{AnyElement, App, Context, EventEmitter, Modifiers};

use super::{FileLinks, failure};
use crate::host_text::{HostSentence, HostText};
use crate::terminal::disclosure::message_line;
use crate::ticker;

/// How long a successful save's line stays: the old message's five seconds.
const SUCCESS_SHOWN: Duration = Duration::from_secs(5);

/// The element id of the status line ([`FileLinks::status_line`]), whose accessible name is what it says: tests and
/// the bridge read it by this.
const LINE_ID: &str = "terminal-file-message-line";

/// The id of the sentence inside the status line, which a host sentence needs.
const SENTENCE_ID: &str = "terminal-file-message";

/// What the status line says about the latest download, and how long it stays.
pub(super) struct Status {
    line: HostSentence,
    /// Whether it is a failure, which stays until the next press in the terminal; progress and a success do not
    /// answer to a press.
    failure: bool,
    /// Takes a success away after [`SUCCESS_SHOWN`]; dropping it, with the status, cancels that.
    _fade: Option<ticker::Deadline>,
}

/// How a status line leaves: a save in progress when its outcome replaces it, a success after [`SUCCESS_SHOWN`], a
/// failure at the next press or download.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leaves {
    Replaced,
    Fading,
    AtPress,
}

/// Emitted whenever the status line shows something new: the owner puts a refused click's explanation away for it, as
/// the old message replaced a refusal in their one element.
pub(crate) struct StatusShown;

impl EventEmitter<StatusShown> for FileLinks {}

/// Whether `modifiers` hold the platform's opening modifier: Cmd on a Mac, Ctrl elsewhere, as the old app read
/// `metaKey` or `ctrlKey`. Other modifiers held beside it do not matter, as they did not there.
pub(super) fn opening(modifiers: Modifiers, platform: Platform) -> bool {
    match platform {
        Platform::Mac => modifiers.platform,
        Platform::Linux => modifiers.control,
    }
}

impl FileLinks {
    /// Save `path` (the host's resolved path, which the host reopens) into Downloads, asking the helm to open the save
    /// as well when the click `requested` it (held the opening modifier, [`opening`]) and the check offered opening
    /// (`can_open`): a list that grew after the hover cannot add an opening the user was not offered, though the helm
    /// may still refuse one it was.
    pub(super) fn download(
        &mut self,
        path: String,
        can_open: bool,
        requested: bool,
        cx: &mut Context<Self>,
    ) {
        // A check succeeded, so the session has transfers; without them there is nothing to ask.
        let Some(transfers) = self.transfers.clone() else {
            return;
        };
        let open = requested && can_open;
        let saving = transfers.download(&self.session, &path, open);
        let id = self.next_download;
        self.next_download += 1;
        let task = cx.spawn(async move |files, cx| {
            let saved = saving.await;
            // Dropping the file links drops this task with the rest, which abandons the save (the helm then removes
            // what it staged). The terminal holds them, and so does an open file display's popover, so a closed
            // terminal's save is abandoned once that popover has closed too.
            let _ = files.update(cx, |files, cx| {
                files.downloads.remove(&id);
                files.saved(saved, open, requested, cx);
            });
        });
        self.downloads.insert(id, task);
        self.show(
            HostSentence::new(SENTENCE_ID).text("Downloading…"),
            Leaves::Replaced,
            cx,
        );
    }

    /// A save finished: say where the file went and what came of opening it, in the old app's words, or why it
    /// failed. A modifier-click on a file the check offered no opening for saves and says so.
    fn saved(
        &mut self,
        saved: Result<SavedDownload, TransferError>,
        open: bool,
        requested: bool,
        cx: &mut Context<Self>,
    ) {
        match saved {
            Ok(saved) => {
                let line = HostSentence::new(SENTENCE_ID)
                    .text("Saved to ")
                    .host(&HostText::from_host(saved.path));
                let line = match (open, saved.open) {
                    (true, OpenOutcome::Opened) => line.text(" · opened"),
                    (true, OpenOutcome::NotOpened(why)) => {
                        line.text(" · not opened: ").host(&HostText::from_host(why))
                    }
                    // Asked to open, and the helm said nothing about it: no reason to give, and no claim either way.
                    (true, OpenOutcome::NotAsked) => line.text(" · not opened"),
                    (false, _) if requested => line.text(" · not opened: opening was not offered"),
                    (false, _) => line,
                };
                self.show(line, Leaves::Fading, cx);
            }
            Err(error) => {
                let line = HostSentence::new(SENTENCE_ID)
                    .text("Download failed: ")
                    .host(&failure(&error));
                self.show(line, Leaves::AtPress, cx);
            }
        }
    }

    /// Show `line` on the status line, replacing whatever it said (and cancelling that line's fade), until it `leaves`;
    /// the owner then puts a refusal away for it ([`StatusShown`]).
    fn show(&mut self, line: HostSentence, leaves: Leaves, cx: &mut Context<Self>) {
        let fade = (leaves == Leaves::Fading).then(|| {
            let files = cx.weak_entity();
            ticker::after(SUCCESS_SHOWN, cx, move |cx| {
                let _ = files.update(cx, |files, cx| files.clear_status(cx));
            })
        });
        self.status = Some(Status {
            line,
            failure: leaves == Leaves::AtPress,
            _fade: fade,
        });
        cx.emit(StatusShown);
        cx.notify();
    }

    /// Take the status line away: its success faded, or a refused click's explanation takes its place.
    pub(crate) fn clear_status(&mut self, cx: &mut Context<Self>) {
        if self.status.take().is_some() {
            cx.notify();
        }
    }

    /// A press in the terminal: a failure's line goes, as the old app's did on the next click there.
    pub(crate) fn pressed(&mut self, cx: &mut Context<Self>) {
        if self.status.as_ref().is_some_and(|status| status.failure) {
            self.clear_status(cx);
        }
    }

    /// The status line, for the owner to draw at the top of the terminal, where a refused click's explanation goes:
    /// the old `.terminal-file-message`, as the owner's refusal line draws it too, clear of the newest output at the
    /// bottom and at most three lines tall. It takes no pointer and no room: the owner overlays it.
    pub(crate) fn status_line(&self, cx: &App) -> Option<AnyElement> {
        let status = self.status.as_ref()?;
        let label = status.line.accessible_name();
        Some(message_line(LINE_ID, label, status.line.clone(), cx))
    }
}

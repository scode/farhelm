//! Hovering a printed file path checks it on the session's host. Only a downloadable file gets a disclosure and a
//! click action (SPEC.md, "Terminal experience"). The grid model only recognizes web destinations in OSC 8 links, so
//! a program's `file://` hyperlink never reaches this module.
//!
//! The grid model recognizes the links ([`farhelm_grid_model::TerminalLink::file_path`]); a link grants nothing. Only
//! the host can say what a path names, so each hover asks it afresh (a stat through the helm client's
//! [`Transfers`]). A successful check discloses the host, the full resolved path and the size.
//!
//! - One hover, one check. A hover is the link as the grid model finds it under the pointer: its file (the path as
//!   printed) and its cells. A redraw that finds the same link again is the same hover and keeps its check; a changed
//!   link (other text over the same cells, a path that grew, other cells) is a new hover and checks again. Leaving
//!   drops the check, and dropping its future abandons the request on the helm.
//! - Only a successful check admits a click, and only for the link it checked: the owner finds the link again from the
//!   live text at the release, and a click counts only when that link's cells and file are still the checked one's
//!   ([`FileLinks::ready_for`]). The press must have a painted answer already. A click that starts before paint or
//!   while a check is pending or unsuccessful stays silent.
//! - The display is the safeguard. A click whose display was outside the window or covered by a pointer-blocking
//!   surface is refused with the old app's explanation.
//! - A printed path gets an underline and a pointing hand only once its check succeeded.
//! - A staged run's host (agent builds with a pins file only) is never asked: the check is answered at once from the
//!   pins file ([`crate::agent_startup::staged_file_stat`]), and a click on such an answer saves nothing, since the
//!   file it names exists on no host.
//!
//! The terminal's owner ([`super::attached`]) draws the display through the same non-focusing popover and anchor as a
//! web link's destination display, so it opens by the pointer, closes on a press, comes back after it and is judged
//! fully visible exactly as that one is. The popover holds a snapshot of the ready check and a fresh paint witness.
//!
//! An admitted click saves the file into Downloads, and opens the save on the platform's modifier-click
//! ([`download`]); with Shift added to that modifier it copies the file's contents into the clipboard instead
//! ([`copy`]). The status line at the top of the terminal says how either went.
//!
//! Checks go through the UI session's transfers (`UiSession::transfers`), which the terminal hands over as it is made;
//! tests serve fake ones through the connection, the same path (`Connection::serve_transfers`).

mod copy;
mod download;
pub(in crate::terminal) use download::ClickAction;
pub(crate) use download::StatusShown;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use farhelm_grid_model::TerminalLink;
use farhelm_helm_client::{FileStat, TransferError, Transfers};
use farhelm_input_mapping::Platform;
#[cfg(test)]
use farhelm_proto::DownloadFileInfo;
use farhelm_proto::DownloadFileStatus;
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, Modifiers,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Task, Window,
    div, px,
};

use super::disclosure::{self, Drawn, Hidden};
use crate::assets::TERMINAL_FONT;
use crate::components::Role;
use crate::host_text::{HostSentence, HostText};
use crate::theme::{self, Palette, Token};

#[cfg(test)]
const FILE_REFUSAL: &str = Hidden::NotVisible.file_words(ClickAction::Download);

/// The display's element id, which tests and the bridge find it by.
const PANEL_ID: &str = "terminal-file-disclosure";

/// The file checks and downloads of one terminal. Dropping the entity drops its outstanding transfers.
pub(crate) struct FileLinks {
    /// The session's served id, which the helm resolves a path against (its working directory and home).
    session: String,
    /// The UI session's transfers. A session with no helm client can only answer with a staged stat.
    transfers: Option<Transfers>,
    hover: Option<Hover>,
    /// What the status line at the top of the terminal says about the latest download or copy ([`download`],
    /// [`copy`]).
    status: Option<download::Status>,
    /// The saves in flight, by the number each was started under. Dropping one abandons it, and the helm then removes
    /// what it staged; each removes itself when its answer comes.
    downloads: HashMap<u64, Task<()>>,
    /// The number the next save is started under.
    next_download: u64,
    /// The copy in flight, if any ([`copy`]). There is at most one: a new copy replaces it, which abandons its read.
    copying: Option<Task<()>>,
}

/// One hover over a file link, with its check.
struct Hover {
    /// The link as the grid model found it when the hover started: what a click must still find.
    link: TerminalLink,
    /// Whose modifier opens the save on a click. The disclosure's hint takes the owner's platform when it opens.
    platform: Platform,
    check: Check,
    /// Whether `check` came from a staged run's pins file rather than the host (agent builds only): the file it names
    /// exists on no host, so a click on it downloads nothing.
    staged: bool,
    /// The stat in flight. Dropping it, with the hover, abandons the request.
    _stat: Option<Task<()>>,
}

/// How far a hover's check got.
enum Check {
    Checking,
    Ready(FileStat),
    /// Retain a settled negative result for this hover. Redraws must not keep asking about the same path.
    NotActionable,
}

impl Check {
    /// Only a downloadable answer carries file details into the UI.
    fn from_stat(stat: FileStat) -> Self {
        if stat.info.status == DownloadFileStatus::Ready {
            Self::Ready(stat)
        } else {
            Self::NotActionable
        }
    }
}

impl FileLinks {
    /// The file links of a terminal of session `session`, checking through `transfers`.
    pub(crate) fn new(session: String, transfers: Option<Transfers>) -> Self {
        FileLinks {
            session,
            transfers,
            hover: None,
            status: None,
            downloads: HashMap::new(),
            next_download: 0,
            copying: None,
        }
    }

    /// The link whose check belongs to the current hover, including a pending or settled negative check.
    pub(super) fn tracking(&self) -> Option<&TerminalLink> {
        self.hover.as_ref().map(|hover| &hover.link)
    }

    /// Follow the hovered file independently of its popover. Leaving the link drops its task and abandons the request.
    pub(crate) fn follow(
        &mut self,
        hovered: Option<TerminalLink>,
        platform: Platform,
        cx: &mut Context<Self>,
    ) {
        if self.tracking() == hovered.as_ref() {
            return;
        }
        self.hover = None;
        let Some((path, link)) = hovered.and_then(|link| Some((link.file_path()?, link))) else {
            return;
        };
        // A staged run's host is never asked: its answer comes from the pins file (`agent_startup`, "Staged host
        // values"), at once, as if the host had answered.
        let staged = crate::agent_startup::staged_file_stat(&self.session, &path, cx);
        let was_staged = staged.is_some();
        let (check, stat) = match (&self.transfers, staged) {
            (_, Some(stat)) => (Check::from_stat(stat), None),
            (Some(transfers), None) => {
                let stat = transfers.stat(&self.session, &path);
                let task = cx.spawn(async move |files, cx| {
                    let answer = stat.await;
                    let _ = files.update(cx, |files, cx| files.answered(answer, cx));
                });
                (Check::Checking, Some(task))
            }
            (None, None) => (Check::NotActionable, None),
        };
        let ready = matches!(check, Check::Ready(_));
        self.hover = Some(Hover {
            link,
            platform,
            check,
            staged: was_staged,
            _stat: stat,
        });
        if ready {
            cx.notify();
        }
    }

    /// The host answered the current hover's check. A replaced hover's answer never gets here: dropping its task
    /// cancelled it.
    fn answered(&mut self, answer: Result<FileStat, TransferError>, cx: &mut Context<Self>) {
        let Some(hover) = &mut self.hover else {
            return;
        };
        hover.check = answer.map_or(Check::NotActionable, Check::from_stat);
        if matches!(hover.check, Check::Ready(_)) {
            cx.notify();
        }
    }

    /// Whether a click on `link` may transfer a file: the current hover's check found a ready file, and `link`, as the
    /// owner found it from the live text, is still the link that hover checked. Still the same means the same cells and
    /// the same file. A printed path's file is its text, so a partial redraw that changed the text over the same cells
    /// withdraws the click: the user has not seen the new file checked.
    pub(super) fn ready_for(&self, link: &TerminalLink) -> bool {
        self.ready_stat(link).is_some()
    }

    /// A ready answer for these cells and path, which a new disclosure snapshots when it opens.
    pub(super) fn ready_stat(&self, link: &TerminalLink) -> Option<&FileStat> {
        let hover = self.hover.as_ref()?;
        match &hover.check {
            Check::Ready(stat)
                if link.same_range(&hover.link) && link.file_path() == hover.link.file_path() =>
            {
                Some(stat)
            }
            _ => None,
        }
    }

    /// A click on the same checked link saves the disclosed file, with its opening modifier also opens the save
    /// ([`download`]), and with Shift added copies its contents into the clipboard instead ([`copy`]); see
    /// [`ClickAction`]. The owner calls this only when the press and release refer to the same painted disclosure.
    /// At release an observed cover still refuses the click, though a hidden panel is normal after a press closes it,
    /// and the refusal names what the click asked for. If the host check no longer applies to these cells and path,
    /// the click stays silent.
    pub(super) fn clicked(
        &mut self,
        link: &TerminalLink,
        pressed: Result<(), Hidden>,
        released: Result<(), Hidden>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> Option<&'static str> {
        if !self.ready_for(link) {
            return None;
        }
        let hover = self.hover.as_ref()?;
        let platform = hover.platform;
        let action = ClickAction::of(modifiers, platform);
        // A staged answer names a file on no host, so its click transfers nothing.
        let file = match &hover.check {
            Check::Ready(stat) if !hover.staged => Some((
                stat.info.path.clone(),
                stat.info.size,
                stat.can_open == Some(true),
            )),
            _ => None,
        };
        let refusal = match (pressed, released) {
            // The mouse-down frame remains authoritative even if pressing dismissed the cover.
            (Err(Hidden::Covered), _) | (Ok(()), Err(Hidden::Covered)) => Hidden::Covered,
            (Err(Hidden::NotVisible), Ok(())) => return None,
            (Err(hidden), Err(_)) => hidden,
            (Ok(()), _) => {
                if let Some((path, size, can_open)) = file {
                    match action {
                        ClickAction::Copy => self.copy(path, size, platform, cx),
                        ClickAction::Download | ClickAction::Open => {
                            self.download(path, can_open, action == ClickAction::Open, cx);
                        }
                    }
                }
                return None;
            }
        };
        self.clear_status(cx);
        Some(refusal.file_words(action))
    }
}

/// The status line's reason for a failed download or copy. Use the helm's words when it sent a refusal message, and
/// `refused` (the operation's own "… refused") when it sent a refusal without any.
fn failure(error: &TransferError, refused: &str) -> HostText {
    match error {
        TransferError::Refused { message, .. } if !message.is_empty() => {
            HostText::from_host(message.as_str())
        }
        TransferError::Refused { .. } => HostText::from_host(refused),
        other => HostText::from_host(other.to_string()),
    }
}

// ---- What the display shows ------------------------------------------------------------------------------------------

/// The display's left stripe, and the size's color, by what the check found (the old `sizeTier` and its `data-tier`
/// styles in app.css). The thresholds use decimal bytes, like the host's download limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SizeTier {
    /// The host reported a downloadable file without a size.
    Neutral,
    /// Under 1 MB: green stripe and size.
    Small,
    /// 1 MB up to 10 MB: blue stripe, neutral size.
    Medium,
    /// 10 MB up to the 100 MB limit, which itself is allowed: amber stripe and size.
    Large,
}

impl SizeTier {
    fn of(size: Option<u64>) -> SizeTier {
        match size {
            None => SizeTier::Neutral,
            Some(size) if size < 1_000_000 => SizeTier::Small,
            Some(size) if size < 10_000_000 => SizeTier::Medium,
            Some(_) => SizeTier::Large,
        }
    }

    /// The stripe's color: `--fg-2`, `--ok`, `--info` and `--warn`, as the old tiers set
    /// `border-left-color`.
    fn stripe(self, palette: &Palette) -> Token {
        match self {
            SizeTier::Neutral => palette.fg_2,
            SizeTier::Small => palette.ok,
            SizeTier::Medium => palette.info,
            SizeTier::Large => palette.warn,
        }
    }

    /// The size's color: green and amber for their tiers, and the display's own text
    /// color otherwise, as the old `.terminal-file-size` inherited it.
    fn words(self, palette: &Palette) -> Token {
        match self {
            SizeTier::Small => palette.ok,
            SizeTier::Large => palette.warn,
            SizeTier::Neutral | SizeTier::Medium => palette.fg_1,
        }
    }

    /// The tier's name in the old `data-tier` attribute, which the status line's debug selector carries for tests.
    fn name(self) -> &'static str {
        match self {
            SizeTier::Neutral => "neutral",
            SizeTier::Small => "small",
            SizeTier::Medium => "medium",
            SizeTier::Large => "large",
        }
    }
}

/// `bytes` in decimal units, as the old `formatSize`: whole bytes under 1000, then KB, MB and GB to one decimal with a
/// trailing `.0` dropped. Decimal units agree with the host's 100,000,000-byte limit.
///
/// The tenths round half up on exact integers. The old code rounded the floating-point quotient with `toFixed`, which
/// can differ for a quotient that floating point stores just below a half (1,150 bytes read "1.1 KB" there and "1.2 KB"
/// here); every size its tests pinned reads the same.
fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let (unit, suffix) = if bytes < 1_000_000 {
        (1_000_u64, "KB")
    } else if bytes < 1_000_000_000 {
        (1_000_000, "MB")
    } else {
        (1_000_000_000, "GB")
    };
    let tenths = (u128::from(bytes) * 20 + u128::from(unit)) / (u128::from(unit) * 2);
    match tenths % 10 {
        0 => format!("{} {suffix}", tenths / 10),
        tenth => format!("{}.{tenth} {suffix}", tenths / 10),
    }
}

/// The action hints after a ready file's size: a click downloads; where the helm offers opening (only the desktop
/// app's helm has an opener, and only for a type on the shared list), the platform's modifier-click opens; and where a
/// copy could succeed, that modifier with Shift copies the contents.
///
/// A copy is offered for a file of known size within [`copy::COPY_MAX_BYTES`], except an image on Linux, which copies
/// text only. It is not withheld from other binary files by extension: no list of such extensions is ever complete, and
/// the copy itself reads the bytes and refuses what is neither text nor a supported image, saying so.
fn hints(stat: &FileStat, platform: Platform) -> String {
    let mut hints = String::from(" · click to download");
    if stat.can_open == Some(true) {
        hints.push_str(match platform {
            Platform::Mac => " · ⌘-click to open",
            Platform::Linux => " · Ctrl-click to open",
        });
    }
    let copyable = stat
        .info
        .size
        .is_some_and(|size| size <= copy::COPY_MAX_BYTES)
        && !(platform == Platform::Linux && copy::is_image_path(&stat.info.path));
    if copyable {
        hints.push_str(match platform {
            Platform::Mac => " · ⇧⌘-click to copy contents",
            Platform::Linux => " · Shift-Ctrl-click to copy contents",
        });
    }
    hints
}

/// The bundled Nerd Font symbol for a file's broad kind (the old `fileIcon`): a PDF, an image, an archive, a
/// text or source file, or a generic file. It names a kind, never whether the file may be opened; only the helm
/// decides that.
fn file_icon(path: &str) -> char {
    let name = path.rsplit('/').next().unwrap_or(path);
    let extension = name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => '\u{f1c1}',
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif" => '\u{f1c5}',
        "zip" | "gz" | "xz" | "bz2" | "tar" | "7z" | "zst" => '\u{f1c6}',
        "txt" | "md" | "rs" | "js" | "ts" | "py" | "html" | "css" | "json" | "toml" | "yaml"
        | "yml" | "sh" | "c" | "h" => '\u{f15c}',
        _ => '\u{f15b}',
    }
}

/// One line of the app's own words, under `id` with them as its accessible name, so tests and the bridge read it as
/// drawn.
fn words(id: &'static str, text: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    let text = text.into();
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text.clone())
        .child(text)
}

/// The ready stat for one popover. The owner supplies a fresh witness so only this panel's paint can admit a click.
pub(super) struct FileDisclosureView {
    pub(super) stat: FileStat,
    pub(super) platform: Platform,
    pub(super) drawn: Drawn,
}

impl Render for FileDisclosureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        panel(&self.stat, self.platform, self.drawn.clone(), window, cx)
    }
}

/// The file disclosure uses the web-link display's floating surface with a 3-pixel stripe in the size tier's color.
/// Its three lines show the file's icon and name, `host:directory`, and the size with the action hints. Every host
/// value goes through the host-text element, so hidden characters show and repeated spaces stay. Long paths wrap rather than elide: the full
/// path is the evidence a click needs, and a display too tall for the window is not fully visible, which refuses the
/// click.
fn panel(
    stat: &FileStat,
    platform: Platform,
    drawn: Drawn,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let palette = theme::palette(cx);
    let tier = SizeTier::of(stat.info.size);
    let panel = disclosure::surface(PANEL_ID, window, cx)
        .border_l(px(3.))
        .border_color(theme::color(tier.stripe(palette)))
        .children(answer_lines(stat, platform, tier, palette));
    disclosure::witnessed(panel, drawn).into_any_element()
}

/// The three size tiers for ready file disclosures, with the opening hint on the small file.
#[cfg(test)]
pub(crate) fn golden_displays(window: &Window, cx: &App) -> AnyElement {
    let answers = [
        ("/home/dev/notes/plan.md", 4_800, true),
        ("/home/dev/report.pdf", 8_400_000, false),
        ("/home/dev/dump.tar.gz", 87_000_000, false),
    ];
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(8.))
        .children(answers.into_iter().map(|(path, size, can_open)| {
            let stat = FileStat {
                host: "build-box".into(),
                can_open: Some(can_open),
                info: DownloadFileInfo {
                    path: path.into(),
                    size: Some(size),
                    status: DownloadFileStatus::Ready,
                },
            };
            panel(&stat, Platform::Mac, Drawn::default(), window, cx)
        }))
        .into_any_element()
}

/// The three lines of a host's answer (see [`panel`]). The path is split at its last slash for presentation only:
/// together the name and directory lines still show every character of the resolved path.
fn answer_lines(
    stat: &FileStat,
    platform: Platform,
    tier: SizeTier,
    palette: &Palette,
) -> Vec<AnyElement> {
    let path = stat.info.path.as_str();
    let split = path.rfind('/').map_or(0, |slash| slash + 1);
    let (directory, name) = path.split_at(split);
    let icon = file_icon(path);
    let size = stat
        .info
        .size
        .map_or_else(|| "Size unknown".into(), format_size);
    vec![
        div()
            .flex()
            .text_color(theme::color(palette.fg_0))
            .font_weight(FontWeight::SEMIBOLD)
            // The Nerd Font symbol keeps its face beside the interface face, as the old icon span did.
            .child(
                div()
                    .flex_none()
                    .font_family(TERMINAL_FONT)
                    .child(format!("{icon} ")),
            )
            .child(HostSentence::new("terminal-file-name").host(&HostText::from_host(name)))
            .into_any_element(),
        div()
            .text_color(theme::color(palette.fg_2))
            .child(
                HostSentence::new("terminal-file-directory")
                    .host(&HostText::from_host(stat.host.as_str()))
                    .text(":")
                    .host(&HostText::from_host(directory)),
            )
            .into_any_element(),
        div()
            .flex()
            .flex_wrap()
            .child(
                words("terminal-file-status", size)
                    .debug_selector(|| format!("terminal-file-status-{}", tier.name()))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(tier.words(palette))),
            )
            .child(words("terminal-file-hint", hints(stat, platform)))
            .into_any_element(),
    ]
}

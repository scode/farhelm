//! File links in a session terminal: hovering a file path a program printed checks it on the session's host and shows
//! the host's answer before any click may transfer the file (SPEC.md, "Terminal experience", the file-path bullet; the
//! old app's `FileLinks` in terminal-files.js at the reference commit). A program's `file://` hyperlink, which the old
//! app also checked, is inert like every hyperlink that is not a web address: the terminal finds no link over it, so it
//! never reaches this module.
//!
//! The grid model recognizes the links ([`farhelm_grid_model::TerminalLink::file_path`]); a link grants nothing. Only
//! the host can say what a path names, so each hover asks it afresh (a stat through the helm client's
//! [`Transfers`]), and the hover display shows what it said: the host, the full resolved path and the size, or the
//! refusal. Here's the TLDR of the rules this module keeps:
//!
//! - One hover, one check. A hover is the link as the grid model finds it under the pointer: its file (the path as
//!   printed) and its cells. A redraw that finds the same link again is the same hover and keeps its check; a changed
//!   link (other text over the same cells, a path that grew, other cells) is a new hover and checks again. Leaving
//!   drops the check, and dropping its future abandons the request on the helm.
//! - Only a successful check admits a click, and only for the link it checked: the owner finds the link again from the
//!   live text at the release, and a click counts only when that link's cells and file are still the checked one's
//!   ([`FileLinks::ready_for`]). A click pressed while checking, even one whose answer lands before the
//!   release, or on a path the host refused, does nothing. One pressed over a ready answer the display has not painted
//!   yet is refused as not visible: a fresh answer forgets where the display was drawn.
//! - The display is the safeguard. A click whose display was outside the window or covered by a pointer-blocking
//!   surface is refused with the old app's explanation.
//! - A printed path shows the pointing hand only once its check succeeded; a web link shows it at once.
//! - A staged run's host (agent builds with a pins file only) is never asked: the check is answered at once from the
//!   pins file ([`crate::agent_startup::staged_file_stat`]), and a click on such an answer saves nothing, since the
//!   file it names exists on no host.
//!
//! The terminal's owner ([`super::attached`]) draws the display through the same non-focusing popover and anchor as a
//! web link's destination display, so it opens by the pointer, closes on a press, comes back after it and is judged
//! fully visible exactly as that one is; this module owns what the display shows (this entity is the popover's
//! content) and the check behind it.
//!
//! An admitted click saves the file into Downloads, and opens the save on the platform's modifier-click; the status
//! line at the top of the terminal says how it went ([`download`]).
//!
//! Checks go through the UI session's transfers (`UiSession::transfers`), which the terminal hands over as it is made;
//! tests serve fake ones through the connection, the same path (`Connection::serve_transfers`).

mod download;
pub(crate) use download::StatusShown;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use farhelm_grid_model::{LinkKind, TerminalLink};
use farhelm_helm_client::{FileStat, TransferError, Transfers};
use farhelm_input_mapping::Platform;
use farhelm_proto::{DownloadFileInfo, DownloadFileStatus};
use gpui::{
    AnyElement, App, Context, FontWeight, InteractiveElement, IntoElement, Modifiers,
    ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement as _, Styled, Task,
    Window, div, prelude::FluentBuilder as _, px,
};

use super::disclosure::{self, Drawn, Hidden};
use crate::assets::TERMINAL_FONT;
use crate::components::{Role, Spinner, spinner_icon};
use crate::host_text::{HostSentence, HostText};
use crate::theme::{self, Palette, Token};
use crate::ticker;

#[cfg(test)]
const FILE_REFUSAL: &str = Hidden::NotVisible.file_words();

/// The display's element id, which tests and the bridge find it by.
const PANEL_ID: &str = "terminal-file-disclosure";

/// The old display's first line while the host has not answered.
const CHECKING: &str = "Checking…";

/// Why every check fails for a terminal whose UI session has no transfers: only a connection with no helm client has
/// none, which the app never makes; tests that serve no transfers do.
const NO_HELM_CLIENT: &str = "this window has no helm client to check files through";

/// The shared spinner's size here: the old checking ring's 10 pixels plus its 2-pixel border on each side.
const SPINNER_SIZE: Pixels = px(14.);

/// The file links of one terminal: the hover the pointer rests on, its check, and the display that shows it. One per
/// terminal, made with it and dropped with it, which drops any check in flight.
pub(crate) struct FileLinks {
    /// The session's served id, which the helm resolves a path against (its working directory and home).
    session: String,
    /// Where checks go: the UI session's transfers, `None` only without a helm client ([`NO_HELM_CLIENT`]).
    transfers: Option<Transfers>,
    hover: Option<Hover>,
    /// The checking ring's turn, held only while the display shows a check in an active window.
    spinner: Option<Spinner>,
    /// What the status line at the top of the terminal says about the latest download ([`download`]).
    status: Option<download::Status>,
    /// The saves in flight, by the number each was started under. Dropping one abandons it, and the helm then removes
    /// what it staged; each removes itself when its answer comes.
    downloads: HashMap<u64, Task<()>>,
    /// The number the next save is started under.
    next_download: u64,
}

/// One hover over a file link, with its check.
struct Hover {
    /// The link as the grid model found it when the hover started: what a click must still find.
    link: TerminalLink,
    /// The path as it was asked about, as printed, shown while checking or after a failure, before the host has named
    /// the file itself.
    asked: HostText,
    /// Whose modifier the opening hint names.
    platform: Platform,
    /// Where the display was last drawn, which a click checks it against; the owner's for the display it opened.
    drawn: Drawn,
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
    /// The host answered: a ready file, or its refusal (a folder, a file over the limit, ...).
    Answered(FileStat),
    /// The request failed, in the helm's words (or the transport's).
    Failed(HostText),
}

impl FileLinks {
    /// The file links of a terminal of session `session`, checking through `transfers`.
    pub(crate) fn new(session: String, transfers: Option<Transfers>) -> Self {
        FileLinks {
            session,
            transfers,
            hover: None,
            spinner: None,
            status: None,
            downloads: HashMap::new(),
            next_download: 0,
        }
    }

    /// The pointer rests on `hovered`, drawn into `drawn`, or on no file link: keep the hover if it is the same link,
    /// otherwise drop it, with its check, and start a fresh one for the new link. The owner calls this whenever its
    /// destination display changes, which is whenever the hovered link changes and when the display comes back after a
    /// press, with that display's `drawn`.
    pub(crate) fn follow(
        &mut self,
        hovered: Option<(TerminalLink, Drawn)>,
        platform: Platform,
        cx: &mut Context<Self>,
    ) {
        let hovered = hovered.and_then(|(link, drawn)| Some((link.file_path()?, link, drawn)));
        let Some((path, link, drawn)) = hovered else {
            if self.hover.take().is_some() {
                cx.notify();
            }
            return;
        };
        if let Some(hover) = &mut self.hover
            && hover.link == link
        {
            hover.drawn = drawn;
            hover.platform = platform;
            return;
        }
        // A staged run's host is never asked: its answer comes from the pins file (`agent_startup`, "Staged host
        // values"), at once, as if the host had answered.
        let staged = crate::agent_startup::staged_file_stat(&self.session, &path, cx);
        let was_staged = staged.is_some();
        let (check, stat) = match (&self.transfers, staged) {
            (_, Some(stat)) => (Check::Answered(stat), None),
            (Some(transfers), None) => {
                let stat = transfers.stat(&self.session, &path);
                let task = cx.spawn(async move |files, cx| {
                    let answer = stat.await;
                    let _ = files.update(cx, |files, cx| files.answered(answer, cx));
                });
                (Check::Checking, Some(task))
            }
            (None, None) => (
                Check::Failed(failure(&TransferError::NotSent(NO_HELM_CLIENT.into()))),
                None,
            ),
        };
        self.hover = Some(Hover {
            link,
            asked: HostText::from_host(path),
            platform,
            drawn,
            check,
            staged: was_staged,
            _stat: stat,
        });
        cx.notify();
    }

    /// The host answered the current hover's check. A replaced hover's answer never gets here: dropping its task
    /// cancelled it.
    fn answered(&mut self, answer: Result<FileStat, TransferError>, cx: &mut Context<Self>) {
        let Some(hover) = &mut self.hover else {
            return;
        };
        hover.check = match answer {
            Ok(stat) => Check::Answered(stat),
            Err(error) => Check::Failed(failure(&error)),
        };
        // Where the display was drawn was where it said "Checking…": until it has painted the answer, a click must
        // not count as made over a visible answer, so it fails closed ([`disclosure::visibility`] of nothing).
        hover.drawn.set(None);
        cx.notify();
    }

    /// Whether a click on `link` may transfer a file: the current hover's check found a ready file, and `link`, as the
    /// owner found it from the live text, is still the link that hover checked. Still the same means the same cells and
    /// the same file. A printed path's file is its text, so a partial redraw that changed the text over the same cells
    /// withdraws the click: the user has not seen the new file checked.
    pub(super) fn ready_for(&self, link: &TerminalLink) -> bool {
        self.hover.as_ref().is_some_and(|hover| {
            matches!(&hover.check, Check::Answered(stat) if stat.info.status == DownloadFileStatus::Ready)
                && link.same_range(&hover.link)
                && link.file_path() == hover.link.file_path()
        })
    }

    /// Whether `link` shows the pointing hand: a printed path once its check succeeded, every other link (a web link,
    /// the only other kind the terminal finds) at once.
    pub(crate) fn shows_pointer(&self, link: &TerminalLink) -> bool {
        link.kind != LinkKind::Path || self.ready_for(link)
    }

    /// A click on the same checked link saves the disclosed file, and with its opening modifier also opens the save
    /// ([`download`]). The press must have seen the whole disclosure. At release an observed cover still refuses
    /// the click, though a missing panel is normal after the press closes its popover.
    ///
    /// A click with no ready check, now or at its press, is silent, as the old app's was: the display already says
    /// why, and a check that answered between the press and the release was not one the user pressed over.
    pub(super) fn clicked(
        &mut self,
        link: &TerminalLink,
        pressed: Result<(), Hidden>,
        ready_at_press: bool,
        released: Result<(), Hidden>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> Option<&'static str> {
        if !ready_at_press || !self.ready_for(link) {
            return None;
        }
        let refusal = match (pressed, released) {
            // The mouse-down frame remains authoritative even if pressing dismissed the cover.
            (Err(Hidden::Covered), _) | (Ok(()), Err(Hidden::Covered)) => Hidden::Covered,
            (Err(Hidden::NotVisible), Ok(())) => return None,
            (Err(hidden), Err(_)) => hidden,
            (Ok(()), _) => {
                if let Some(hover) = &self.hover
                    && !hover.staged
                    && let Check::Answered(stat) = &hover.check
                {
                    let (path, can_open) = (stat.info.path.clone(), stat.can_open == Some(true));
                    let requested = download::opening(modifiers, hover.platform);
                    self.download(path, can_open, requested, cx);
                }
                return None;
            }
        };
        self.clear_status(cx);
        Some(refusal.file_words())
    }
}

/// A failed check's line: the helm's own words for a refusal (the old app's "Download refused" when it sent none), the
/// failure's description otherwise.
fn failure(error: &TransferError) -> HostText {
    match error {
        TransferError::Refused { message, .. } if !message.is_empty() => {
            HostText::from_host(message.as_str())
        }
        TransferError::Refused { .. } => HostText::from_host("Download refused"),
        other => HostText::from_host(other.to_string()),
    }
}

// ---- What the display shows ------------------------------------------------------------------------------------------

/// The display's left stripe, and the size's color, by what the check found (the old `sizeTier` and its `data-tier`
/// styles in app.css). Size sets it by exact decimal byte thresholds; any refusal or failure overrides the size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SizeTier {
    /// Still checking, or a ready file whose size the host did not report: the neutral stripe.
    Neutral,
    /// Under 1 MB: green stripe and size.
    Small,
    /// 1 MB up to 10 MB: blue stripe, neutral size.
    Medium,
    /// 10 MB up to the 100 MB limit, which itself is allowed: amber stripe and size.
    Large,
    /// A refusal or a failed check: the red stripe for both, red words only for the host's refusal of the file.
    Danger,
}

impl SizeTier {
    /// The tier of a host's answer.
    fn of(info: &DownloadFileInfo) -> SizeTier {
        match (&info.status, info.size) {
            (DownloadFileStatus::Ready, None) => SizeTier::Neutral,
            (DownloadFileStatus::Ready, Some(size)) if size < 1_000_000 => SizeTier::Small,
            (DownloadFileStatus::Ready, Some(size)) if size < 10_000_000 => SizeTier::Medium,
            (DownloadFileStatus::Ready, Some(_)) => SizeTier::Large,
            _ => SizeTier::Danger,
        }
    }

    /// The stripe's color: `--fg-2`, `--ok`, `--info`, `--warn` and `--danger`, as the old tiers set
    /// `border-left-color`.
    fn stripe(self, palette: &Palette) -> Token {
        match self {
            SizeTier::Neutral => palette.fg_2,
            SizeTier::Small => palette.ok,
            SizeTier::Medium => palette.info,
            SizeTier::Large => palette.warn,
            SizeTier::Danger => palette.danger,
        }
    }

    /// The size's (or refusal's) color: green and amber for their tiers, red for a refusal, and the display's own text
    /// color otherwise, as the old `.terminal-file-size` inherited it.
    fn words(self, palette: &Palette) -> Token {
        match self {
            SizeTier::Small => palette.ok,
            SizeTier::Large => palette.warn,
            SizeTier::Danger => palette.danger,
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
            SizeTier::Danger => "danger",
        }
    }
}

/// `bytes` in decimal units, as the old `formatSize`: whole bytes under 1000, then KB, MB and GB to one decimal with a
/// trailing `.0` dropped. Decimal units agree with the host's 100,000,000-byte limit, so a file just over it never
/// reads as less than "100 MB" (the refusal names the limit beside it).
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

/// The status line's words for a host's answer: the size of a ready file, or its refusal, in the old app's words.
fn status_words(info: &DownloadFileInfo) -> String {
    let size = info.size.map(format_size);
    match &info.status {
        DownloadFileStatus::Ready => size.unwrap_or_else(|| "Size unknown".into()),
        DownloadFileStatus::NotFound => "Not found".into(),
        DownloadFileStatus::Folder => "Folder: not downloadable".into(),
        DownloadFileStatus::TooLarge => match size {
            Some(size) => format!("Too large to download: {size} (100 MB limit)"),
            None => "Too large to download (100 MB limit)".into(),
        },
        DownloadFileStatus::NotReadable => "Not readable".into(),
        DownloadFileStatus::NotRegular => "Not a regular file: not downloadable".into(),
    }
}

/// The action hints after a ready file's size: a click downloads, and where the helm offers opening (only the desktop
/// app's helm has an opener, and only for a type on the shared list), the platform's modifier-click opens.
fn hints(stat: &FileStat, platform: Platform) -> Option<String> {
    if stat.info.status != DownloadFileStatus::Ready {
        return None;
    }
    let mut hints = String::from(" · click to download");
    if stat.can_open == Some(true) {
        hints.push_str(match platform {
            Platform::Mac => " · ⌘-click to open",
            Platform::Linux => " · Ctrl-click to open",
        });
    }
    Some(hints)
}

/// The bundled Nerd Font symbol for a file's broad kind (the old `fileIcon`): a folder, a PDF, an image, an archive, a
/// text or source file, or a generic file. It names a kind, never whether the file may be opened; only the helm
/// decides that.
fn file_icon(path: &str, status: &DownloadFileStatus) -> char {
    if *status == DownloadFileStatus::Folder {
        return '\u{f07b}';
    }
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

impl Render for FileLinks {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let checking = self
            .hover
            .as_ref()
            .is_some_and(|hover| matches!(hover.check, Check::Checking));
        let spins = checking && ticker::is_active(window, cx);
        if spins != self.spinner.is_some() {
            self.spinner = spins.then(|| Spinner::start(window, cx));
        }
        let step = self.spinner.as_ref().map_or(0, |spinner| spinner.step(cx));
        match &self.hover {
            Some(hover) => panel(hover, step, window, cx),
            None => div().into_any_element(),
        }
    }
}

/// The display for `hover`, the old `.terminal-file-target`: the web-link display's floating surface, with a 3-pixel
/// left stripe in its tier's color. Its three lines, once the host answered, are the file's icon and name in the bright
/// face, then `host:directory`, then the size or refusal with the action hints; every host value goes through the
/// host-text element, so hidden characters show and repeated spaces stay. Long paths wrap rather than elide: the full
/// path is the evidence a click needs, and a display too tall for the window is not fully visible, which refuses the
/// click.
fn panel(hover: &Hover, step: u8, window: &Window, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    let tier = match &hover.check {
        Check::Checking => SizeTier::Neutral,
        Check::Answered(stat) => SizeTier::of(&stat.info),
        Check::Failed(_) => SizeTier::Danger,
    };
    let lines: Vec<AnyElement> = match &hover.check {
        Check::Checking => vec![
            spinner_icon(step, SPINNER_SIZE, theme::color(palette.fg_1)).into_any_element(),
            words("terminal-file-checking", CHECKING).into_any_element(),
            HostSentence::new("terminal-file-path")
                .host(&hover.asked)
                .into_any_element(),
        ],
        Check::Failed(message) => vec![
            HostSentence::new("terminal-file-path")
                .host(&hover.asked)
                .into_any_element(),
            // Ordinary text under the red stripe, as the old app showed a failed check (a refusal's words are red).
            div()
                .debug_selector(|| format!("terminal-file-status-{}", SizeTier::Danger.name()))
                .child(HostSentence::new("terminal-file-status").host(message))
                .into_any_element(),
        ],
        Check::Answered(stat) => answer_lines(stat, hover.platform, tier, palette),
    };
    let panel = disclosure::surface(PANEL_ID, window, cx)
        .border_l(px(3.))
        .border_color(theme::color(tier.stripe(palette)))
        .children(lines);
    disclosure::witnessed(panel, hover.drawn.clone()).into_any_element()
}

/// Every kind of display, stacked, for the goldens' scene: checking, a small, medium and large ready file (the small
/// one offering a Mac's ⌘-click), the limit refusal, a folder, and a failed check. Test builds only.
#[cfg(test)]
pub(crate) fn golden_displays(window: &Window, cx: &App) -> AnyElement {
    use DownloadFileStatus::{Folder, Ready, TooLarge};
    // The answered ones: what was asked, the resolved path, its size and status, and whether the helm offers opening.
    let answers = [
        (
            "notes/plan.md",
            "/home/dev/notes/plan.md",
            4_800,
            Ready,
            true,
        ),
        (
            "report.pdf",
            "/home/dev/report.pdf",
            8_400_000,
            Ready,
            false,
        ),
        (
            "dump.tar.gz",
            "/home/dev/dump.tar.gz",
            87_000_000,
            Ready,
            false,
        ),
        ("big.bin", "/home/dev/big.bin", 100_000_001, TooLarge, false),
        ("src", "/home/dev/src", 4_096, Folder, false),
    ];
    let answered = answers
        .into_iter()
        .map(|(asked, path, size, status, can_open)| {
            let info = DownloadFileInfo {
                path: path.into(),
                size: Some(size),
                status,
            };
            let stat = FileStat {
                host: "build-box".into(),
                can_open: Some(can_open),
                info,
            };
            (asked, Check::Answered(stat))
        });
    let lost = failure(&TransferError::Lost("connection reset".into()));
    let hovers: Vec<Hover> = std::iter::once(("notes/plan.md", Check::Checking))
        .chain(answered)
        .chain(std::iter::once(("gone.txt", Check::Failed(lost))))
        .map(|(asked, check)| Hover {
            link: TerminalLink {
                kind: LinkKind::Path,
                uri: asked.into(),
                ranges: Vec::new(),
                evidence: Default::default(),
                label: String::new(),
                osc_label_ambiguous: false,
            },
            asked: HostText::from_host(asked),
            platform: Platform::Mac,
            drawn: Drawn::default(),
            check,
            staged: false,
            _stat: None,
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(8.))
        .children(hovers.iter().map(|hover| panel(hover, 0, window, cx)))
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
    let icon = file_icon(path, &stat.info.status);
    let ready = stat.info.status == DownloadFileStatus::Ready;
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
                words("terminal-file-status", status_words(&stat.info))
                    .debug_selector(|| format!("terminal-file-status-{}", tier.name()))
                    .when(ready, |status| status.font_weight(FontWeight::SEMIBOLD))
                    .text_color(theme::color(tier.words(palette))),
            )
            .children(hints(stat, platform).map(|hints| words("terminal-file-hint", hints)))
            .into_any_element(),
    ]
}

//! A session row's three marks: where the session runs (the host mark, in the locality slot), which agent it runs (the
//! agent mark) and what its launch says about permission checks (the permission shield), the last two side by side in
//! the first line's agent track.
//!
//! SPEC.md, "Session list", ported from the old app's `list/row.rs` (`launch_badge`, `agent_badge`),
//! `list/shared.rs` (`session_locality`), `icons.rs` (`HostMark`, `HarnessIcon`) and the stylesheet's
//! `.session-agent`, `.sidebar-glyph`, `.harness-glyph` and `.permission-glyph` rules at the reference commit.
//!
//! Every mark is a status mark (a picture with words, nothing to press): its drawing is decorative, and its words are
//! its accessible name, with hover help that explains it. What each mark says is decided by pure functions here
//! ([`locality`], [`Description::of`] and [`RowMarks::of`]), which the tests read without a window; drawing only
//! lays the decisions out. All three come from what the helm stored, never from reading the command: the host mark
//! from the registry, the agent mark and the shield from the session's launch. Which mark and name an agent type has
//! is the GUI's per-harness vocabulary in [`crate::harness`]; nothing here compares harnesses. Which shield a launch
//! earns is [`crate::permission`]'s, kept apart so the agent approval cards can show the same classification. The
//! session pane's details bar and the quick switcher's rows draw the same host mark beside the session's host
//! ([`HostMark::of`] and [`host_mark`]), so none of them disagrees about where a session runs. A session in a disposable
//! environment of its own draws its provider's mark in the locality slot instead ([`environment_mark`]), in its row and
//! the details bar; the quick switcher keeps the host mark there, as the old switcher drew no environment mark.
//!
//! The agent mark and the shield describe the pair together, the way the old row did: one description names the
//! agent, what the launch's permission means and the stored command, since the shield alone cannot say which of
//! several modes it stands for. As in the old row, the track is one node for assistive technology, named by that
//! description, so a screen reader hears it once per row; the two pictures inside it are decorative, and hovering
//! either (or the gap between them) shows the description, as the old track's and both its glyphs' tooltips did.
//!
//! The description quotes host values (the command, custom flags), so its hover help is paragraphs of host sentences
//! ([`HoverHelp::paragraphs`]), drawn in the layer manager's paragraph tooltip: wide, a line between paragraphs and no
//! cut, as the old app drew help with paragraphs. Each paragraph that holds a host value holds it after all of its own
//! words and no line break (`Launched with: <command>` is a paragraph of its own), since a sentence lays its parts out
//! as one wrapping row. The accessible name is the same paragraphs with a blank line between them: the old text
//! exactly.

use farhelm_helm_client::Collections;
use farhelm_proto::SessionLaunch;
use farhelm_proto::host_appearance::{HostColor, HostIcon};
use farhelm_proto::ui_channel::{Availability, Collection, HostView, SessionRow};
use gpui::{InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div, px, svg};

use crate::assets::Icon;
use crate::components::{HoverHelp, Role, StatusMark};
use crate::environment_ui::ProviderUi;
use crate::harness;
use crate::host_text::{HostSentence, HostText};
use crate::hosts::identity;
use crate::marks::HarnessGlyph;
use crate::permission::words;
use crate::permission::{Shield, observed_yolo_mode, permission};
use crate::theme::{self, Palette, Token, metrics};

/// The id suffix of a row's host mark, after the row's open control id (`session-row-<key>-host`).
pub(super) const HOST_ID_SUFFIX: &str = "-host";

/// The id suffix of a row's agent track, the one node that names the agent mark and the shield together
/// (`session-row-<key>-agent`).
pub(super) const AGENT_ID_SUFFIX: &str = "-agent";

/// The id suffix the shield's picture is selected by in tests (`session-row-<key>-permission`); the shield has no node
/// of its own for assistive technology.
pub(super) const PERMISSION_ID_SUFFIX: &str = "-permission";

/// `.sidebar-glyph`'s box: each of the agent track's two cells is this wide and the row's line this tall. The mark is
/// drawn in a square of the cell's width, centred, as the old `viewBox 0 0 12 12` in a 10 by 12 box drew it.
const GLYPH_WIDTH: f32 = 10.;
const GLYPH_HEIGHT: f32 = 12.;

/// `.session-agent`'s `column-gap`, between the agent mark and the shield.
const AGENT_TRACK_GAP: f32 = 2.;

// ---- The host mark --------------------------------------------------------------------------------------------------

/// Where a session runs, as far as the registry can say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Locality {
    /// On the helm's own machine: the session's host is the registry's local host.
    Local,
    /// On another machine: the registry was read, and the session's host is not its local host.
    Remote,
}

impl Locality {
    /// The word assistive technology reads for the mark (the old hidden `Local` / `Remote`).
    pub(super) const fn word(self) -> &'static str {
        match self {
            Locality::Local => "Local",
            Locality::Remote => "Remote",
        }
    }

    /// What the word means, as the mark's hover help says it.
    pub(super) const fn help(self) -> &'static str {
        match self {
            Locality::Local => "Local: on this machine, where the helm runs",
            Locality::Remote => "Remote: on another machine, reached over SSH",
        }
    }
}

/// Where `row`'s session runs: local when the registry lists its host as the local one, remote when the registry was
/// read and its host is another (listed or not: the registry always lists its own local host), and `None` when the
/// registry was never read (the host list has not arrived, or its first read failed), the same test the host rail
/// makes. SPEC.md, "Session list": "A session whose host cannot yet be placed either way marks neither, rather than
/// guessing." The row's host name is shown either way; only the mark waits.
pub(super) fn locality(row: &SessionRow, collections: &Collections) -> Option<Locality> {
    if let Some(host) = collections.get::<HostView>(&HostView::key(row.host)) {
        return Some(if host.is_local() {
            Locality::Local
        } else {
            Locality::Remote
        });
    }
    collections
        .meta(Collection::Hosts)
        .is_some_and(|meta| meta.availability == Availability::Available || !meta.order.is_empty())
        .then_some(Locality::Remote)
}

/// The shape and color `row`'s host mark draws in: the host's own choices from the registry (the helm's defaults for
/// a host nobody changed: the red laptop for the local host, the cloud in the ordinary foreground for the others), or
/// those defaults' remote pair for a host the registry does not list. A remote host may choose the red laptop: the
/// shape is identity, never locality, which is the mark's word and hover's to say.
fn appearance(row: &SessionRow, collections: &Collections) -> (HostIcon, HostColor) {
    collections
        .get::<HostView>(&HostView::key(row.host))
        .map_or_else(Default::default, |host| (host.icon, host.color))
}

// ---- The description ------------------------------------------------------------------------------------------------

/// One piece of a description: the app's own words, or a host value (the command, custom flags), drawn in the
/// monospace face and escaped.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Words(String),
    Value(HostText),
}

/// The agent mark's and the shield's shared description, SPEC.md's hover and screen-reader text for the pair, built
/// once per row as paragraphs of pieces and given two ways: as the accessible name ([`Description::name`], the old
/// text exactly, paragraphs separated by blank lines) and as hover help ([`Description::help`], the same paragraphs as
/// host sentences). Ported word for word from the old `launch_badge` and `agent_badge`:
///
/// - An agent launch: `<agent> — <the launcher's permission help for its stored permission>`, then
///   `Custom permission flags: <flags>` when it has custom flags, then `Launched with: <the stored command>`. Guarded's
///   help names the agent's mode and its flags, and every help ends with the caveat that the agent's own rules can
///   loosen it.
/// - A command launch: `<its declared agent, or Command> — <its assertion> — <the command>`; a legacy session:
///   `Command — Unclassified … — <the command>`.
/// - With status-line evidence of YOLO, the observation comes first (a red shield's text must not open with the safer
///   launch mode), then `Launched as:` and the launch's own description, unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Description {
    paragraphs: Vec<Vec<Piece>>,
}

impl Description {
    /// `launch`'s description, with the observation first when `screen_yolo` says one was made.
    pub(super) fn of(launch: &SessionLaunch, screen_yolo: bool) -> Description {
        let said = |text: String| vec![Piece::Words(text)];
        let command = Piece::Value(HostText::from_host(launch.display_command()));
        let mut paragraphs = Vec::new();
        if let Some(mode) = observed_yolo_mode(launch, screen_yolo) {
            paragraphs.push(said(format!(
                "YOLO observed on the agent's status line: {mode}. Best effort; allow rules, hooks and managed policy \
                 are not visible."
            )));
        }
        let launched_as = if paragraphs.is_empty() {
            ""
        } else {
            "Launched as: "
        };
        match launch {
            SessionLaunch::Agent { selection, .. } => {
                let stored = selection.harness.stored_permission(selection.permissions);
                // The launcher's help is itself paragraphs (a heading and its explanation, Guarded's flags, the
                // caveat); they stay paragraphs of the description.
                let help = words::permission_help(selection.harness, stored);
                let mut help_paragraphs = help.split("\n\n");
                let first = help_paragraphs.next().unwrap_or_default();
                paragraphs.push(said(format!(
                    "{launched_as}{} — {first}",
                    harness::name(selection.harness)
                )));
                paragraphs.extend(help_paragraphs.map(|paragraph| said(paragraph.to_owned())));
                if let Some(flags) = &selection.permission_flags {
                    paragraphs.push(vec![
                        Piece::Words("Custom permission flags: ".to_owned()),
                        Piece::Value(HostText::from_host(farhelm_proto::join_permission_flags(
                            flags,
                        ))),
                    ]);
                }
                paragraphs.push(vec![Piece::Words("Launched with: ".to_owned()), command]);
            }
            SessionLaunch::Command(command_launch) => {
                let name = command_launch.agent.map_or("Command", harness::name);
                let assertion = if command_launch.yolo {
                    "YOLO, as asserted when the command was launched, by the user or by an agent — not checked by \
                     Farhelm"
                } else {
                    "Not YOLO, as asserted when the command was launched, by the user or by an agent — not checked \
                     by Farhelm"
                };
                paragraphs.push(vec![
                    Piece::Words(format!("{launched_as}{name} — {assertion} — ")),
                    command,
                ]);
            }
            SessionLaunch::Legacy { .. } => paragraphs.push(vec![
                Piece::Words(format!(
                    "{launched_as}Command — Unclassified — a command from before launch kinds, which Farhelm never \
                     classified — "
                )),
                command,
            ]),
        }
        Description { paragraphs }
    }

    /// The description as assistive technology reads it: the old text exactly, paragraphs separated by a blank line,
    /// each host value escaped as the screen shows it. It is also what the hover help's paragraphs read as one string.
    pub(super) fn name(&self) -> SharedString {
        // Only the sentences' accessible names are used, so they are never drawn and their ids never reach a frame.
        self.help("row-description")
            .iter()
            .map(HostSentence::accessible_name)
            .collect::<Vec<_>>()
            .join("\n\n")
            .into()
    }

    /// The description as hover help: one host sentence per paragraph, under `id` and the paragraph's index (each
    /// paragraph is a node of its own when drawn, so the ids must differ). The launcher's help keeps its heading's line
    /// break, which the paragraph tooltip draws; no host value ever follows a line break inside a paragraph.
    pub(super) fn help(&self, id: &str) -> Vec<HostSentence> {
        self.paragraphs
            .iter()
            .enumerate()
            .map(|(index, pieces)| {
                pieces.iter().fold(
                    HostSentence::new(SharedString::from(format!("{id}-{index}"))),
                    |sentence, piece| match piece {
                        Piece::Words(words) => sentence.text(words.clone()),
                        Piece::Value(value) => sentence.mono(value),
                    },
                )
            })
            .collect()
    }
}

// ---- What a row's marks are -----------------------------------------------------------------------------------------

/// Everything a row's three marks show, decided from the row and the registry (the drawing is [`host_mark`] and
/// [`agent_track`]), and what a test of a mark's look reads, since the frame's record says nothing of colors. Every ink
/// here is final: a selected row's marks keep their shapes, words and help, and all draw in the inverse `--on-fill`,
/// overriding a host's or an agent's identity color (SPEC.md, "Session list").
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RowMarks {
    /// The host mark, or `None` when the session's host cannot be placed.
    pub(super) host: Option<HostMark>,
    /// The agent mark's drawing and ink.
    pub(super) agent: (HarnessGlyph, Token),
    /// The shield's drawing and ink.
    pub(super) permission: (Shield, Token),
    /// The pair's shared description.
    pub(super) description: Description,
}

/// A host mark: where the session runs, the host's chosen shape, and its ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HostMark {
    pub(crate) locality: Locality,
    pub(super) icon: HostIcon,
    /// The host's chosen color, which [`identity::host_mark`] tints with.
    pub(super) color: HostColor,
    /// The ink drawn instead of that tint: `--on-fill` on the selected row, `None` elsewhere.
    pub(super) inverse: Option<Token>,
}

impl HostMark {
    /// The host mark of `row`'s session, drawn in `inverse` instead of the host's tint when given, or `None` when the
    /// session's host cannot be placed ([`locality`]). The row, the session pane's details bar and the quick
    /// switcher's rows all draw this one decision, so neither of the others marks a session the row leaves unmarked.
    pub(crate) fn of(
        row: &SessionRow,
        collections: &Collections,
        inverse: Option<Token>,
    ) -> Option<HostMark> {
        locality(row, collections).map(|locality| {
            let (icon, color) = appearance(row, collections);
            HostMark {
                locality,
                icon,
                color,
                inverse,
            }
        })
    }
}

impl RowMarks {
    /// `row`'s marks, `selected` or not.
    pub(super) fn of(
        row: &SessionRow,
        collections: &Collections,
        selected: bool,
        palette: &Palette,
    ) -> RowMarks {
        let launch = &row.info.launch;
        let glyph = match launch {
            SessionLaunch::Legacy { agent_kind, .. } => harness::kind_glyph(*agent_kind),
            SessionLaunch::Agent { .. } | SessionLaunch::Command(_) => launch
                .agent_type()
                .map_or(HarnessGlyph::Terminal, harness::glyph),
        };
        let shield = permission(launch, row.info.screen_yolo);
        let ink = |own: Token| if selected { palette.on_fill } else { own };
        RowMarks {
            host: HostMark::of(row, collections, selected.then_some(palette.on_fill)),
            // A mark with no identity color takes the agent track's foreground (the old `.session-agent`'s `--fg-1`).
            agent: (
                glyph,
                ink(harness::ink(glyph, palette).unwrap_or(palette.fg_1)),
            ),
            permission: (shield, ink(shield.ink(palette))),
            description: Description::of(launch, row.info.screen_yolo),
        }
    }
}

// ---- Where a mark's help shows ---------------------------------------------------------------------------------------

/// Where a mark's hover help shows. SPEC.md, "Session list": "Every session-row hover, including the summary card and
/// notification bell, sits just right of the sidebar", with a notch pointing back at its control and a thin ring on it,
/// while "tooltips outside session rows ... keep their placement without these cues". The host mark is drawn in a row,
/// in the session pane's details bar and in the quick switcher's rows, so whoever draws it says which it is.
///
/// The old app opted each row target in by hand (`data-tooltip-anchor="session-row"` on the open button, the dot, the
/// host, agent and permission marks, the pin, the age, the stale and ended marks, the bell and the actions button);
/// every one of those that is ported asks through here, so a mark added to the row later has one place to look. The
/// bell is the exception: its help is built in [`crate::notices_bell`], which asks for the same placement itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HelpPlace {
    /// Anywhere but a session row: the ordinary placement, no cues.
    Ordinary,
    /// In a session row, `selected` or not: beside the sidebar ([`HoverHelp::beside`]), the ring in the on-fill ink on
    /// the selected row's accent fill ([`HoverHelp::ring_on_fill`]), as the old `.session-row.selected
    /// .tooltip-target-ring` drew it.
    Row { selected: bool },
}

impl HelpPlace {
    /// `help`, asking for this placement. A round control adds its own radius after this
    /// ([`HoverHelp::ring_radius`]), which only matters beside the sidebar.
    pub(crate) fn place(self, help: HoverHelp) -> HoverHelp {
        match self {
            HelpPlace::Ordinary => help,
            HelpPlace::Row { selected: false } => help.beside(),
            HelpPlace::Row { selected: true } => help.ring_on_fill(),
        }
    }
}

// ---- Drawing --------------------------------------------------------------------------------------------------------

/// The debug selector a drawn mark's picture carries: the mark's id and what it draws
/// (`session-row-<key>-agent:icons/harness-claude.svg`). The driver record does not say which picture an element
/// painted, so this is how a test tells an actual drawing from a mark that only has the right words, as the old
/// `data-glyph` did. Callers build it inside `debug_selector`'s closure, which runs only in builds that record
/// selectors.
pub(super) fn picture_selector(mark_id: &str, picture: &str) -> String {
    format!("{mark_id}:{picture}")
}

/// The locality slot's mark: the host's shape at the slot's size, named `Local` or `Remote` with what that means as
/// its hover help, shown where `place` says (beside the sidebar in a session row).
pub(crate) fn host_mark(
    row_id: &str,
    mark: HostMark,
    palette: &Palette,
    place: HelpPlace,
) -> impl IntoElement {
    let id = SharedString::from(format!("{row_id}{HOST_ID_SUFFIX}"));
    let drawn = identity::host_mark(mark.icon, mark.color, metrics::ROW_SLOT, palette);
    // The identity tint gives way to the inverse ink on the selected row's fill (the old `.host-kind-icon` rule's
    // `!important` over the inline tint).
    let drawn = match mark.inverse {
        Some(ink) => drawn.text_color(theme::color(ink)),
        None => drawn,
    };
    // The host shapes' stable words name the picture here, as the old mark's `data-glyph` did.
    let selector_id = id.clone();
    StatusMark::new(
        id,
        mark.locality.word(),
        place.place(HoverHelp::tooltip(mark.locality.help())),
        Role::Image,
        mark.locality.word(),
    )
    .content(drawn.debug_selector(move || picture_selector(&selector_id, mark.icon.as_str())))
}

/// The locality slot's mark for a session in its own environment ([`super::environment`]), in place of the host mark:
/// the provider's mark at the slot's size in `ink`, named by the provider's word (`Sandbox`, `Container`) with what
/// that means as its hover help, shown where `place` says. Its id is the host mark's, so the slot's mark is found the
/// same way whichever it draws. The old row drew it in place of `SessionHostIcon`, with the word visually hidden.
pub(crate) fn environment_mark(
    row_id: &str,
    ui: ProviderUi,
    ink: Token,
    place: HelpPlace,
) -> impl IntoElement {
    let id = SharedString::from(format!("{row_id}{HOST_ID_SUFFIX}"));
    let icon = ui.mark();
    let selector_id = id.clone();
    StatusMark::new(
        id,
        ui.locality_label(),
        place.place(HoverHelp::tooltip(ui.mark_help())),
        Role::Image,
        ui.locality_label(),
    )
    .content(
        svg()
            .path(icon.path())
            .size(metrics::ROW_SLOT)
            .flex_none()
            .text_color(theme::color(ink))
            .debug_selector(move || picture_selector(&selector_id, icon.path())),
    )
}

/// A suspended environment session's mark under `id` ([`super::environment`]): the pause glyph in `ink`, then
/// "Suspended" when `worded`, named "Suspended" either way, with the old badge's hover help shown where `place` says.
/// The row draws it where a stale row's cause would be (worded on its ended line, the glyph alone after a compact row's
/// title) and the details bar where the age would be (worded), so the two never draw it differently. Never drawn for a
/// stopped container: it is not suspended. A status mark: nothing to press.
pub(crate) fn suspended_mark(
    id: SharedString,
    worded: bool,
    ink: Token,
    place: HelpPlace,
) -> impl IntoElement {
    use super::environment::{SUSPENDED, SUSPENDED_HELP};
    let content = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .child(cell(id.clone(), Icon::Pause, ink));
    let content = if worded {
        content.child(SUSPENDED)
    } else {
        content
    };
    div().flex_none().child(
        StatusMark::new(
            id,
            SUSPENDED,
            place.place(HoverHelp::tooltip(SUSPENDED_HELP)),
            Role::Status,
            SUSPENDED,
        )
        .ink(ink)
        .content(content),
    )
}

/// A session's mark under `id` while its environment is being deleted ([`super::environment::Deleting`]): the host's
/// phase words ("Deleting…", "Deleting failed") in `ink`, named by them, with the provider's reason why every action is
/// unavailable as the hover help, shown where `place` says. The old `.status-badge.environment-deleting`, words alone
/// with no glyph. The row draws it in place of its stale cause on the ended line and the details bar in the age's
/// place, as [`suspended_mark`] is drawn, so the two never draw it differently. A status mark: nothing to press.
pub(crate) fn deleting_mark(
    id: SharedString,
    deleting: super::environment::Deleting,
    ink: Token,
    place: HelpPlace,
) -> impl IntoElement {
    div().flex_none().child(
        StatusMark::new(
            id,
            deleting.label,
            place.place(HoverHelp::tooltip(deleting.help)),
            Role::Status,
            deleting.label,
        )
        .ink(ink),
    )
}

/// The first line's agent track (`.session-agent`): the agent mark, then the shield, each in its own 10-pixel cell, as
/// one status mark named by the pair's description and helped by it, shown where `place` says. As wide as its two
/// cells, so every row's track lines up.
pub(super) fn agent_track(row_id: &str, marks: &RowMarks, place: HelpPlace) -> impl IntoElement {
    let id = SharedString::from(format!("{row_id}{AGENT_ID_SUFFIX}"));
    let (glyph, agent_ink) = marks.agent;
    let (shield, shield_ink) = marks.permission;
    let name = marks.description.name();
    let help = place.place(HoverHelp::paragraphs(
        marks.description.help(&format!("{id}-help")),
    ));
    let pair = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(AGENT_TRACK_GAP))
        .child(cell(id.clone(), harness::icon(glyph), agent_ink))
        .child(cell(
            SharedString::from(format!("{row_id}{PERMISSION_ID_SUFFIX}")),
            shield.icon(),
            shield_ink,
        ));
    div().flex_none().child(
        StatusMark::new(id, name.clone(), help, Role::Image, name)
            .described(false)
            .content(pair),
    )
}

/// One `.sidebar-glyph` cell: `icon` in `ink`, centred in a 10 by 12 box, its picture selected in tests by `picture_id`.
/// The agent track's two cells, and a compact row's ended and stale marks (`super::row`), which the old sheet drew in
/// the same box.
pub(super) fn cell(picture_id: SharedString, icon: Icon, ink: Token) -> impl IntoElement {
    div()
        .w(px(GLYPH_WIDTH))
        .h(px(GLYPH_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon.path())
                .size(px(GLYPH_WIDTH))
                .flex_none()
                .text_color(theme::color(ink))
                .debug_selector(move || picture_selector(&picture_id, icon.path())),
        )
}

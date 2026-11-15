//! A session's status as its row shows it: a colored mark for a live session, the full wording for an ended one, and
//! nothing for a status nobody has classified yet.
//!
//! Ported from the old app's `status.rs` (`status_badge`) and the stylesheet's `.status-badge` and `.status-dot` rules
//! at the reference commit. SPEC.md, "Status", is the rule set. A live session draws only a mark, because a word per row
//! would crowd the sidebar; its shape and color together say which status it is (running a filled circle, waiting a
//! triangle, idle with new output a filled diamond, plain idle a ring), so the list can be read without telling hues
//! apart. The word still reaches assistive technology, through the row's description. An ended session keeps its words
//! visible instead (an exit code, a stop annotation, a launch failure's detail are facts a mark cannot carry), on a line
//! of their own under the title. `Unknown` is plumbing, not a status, and draws nothing at all.
//!
//! The running mark pulses ([`pulses`]): its opacity steps down to a shallow minimum and back every two seconds
//! ([`pulse_opacity`]), at the ticker's shared pulse phase, so every running dot in the list steps together. The pulse
//! is a claim about the present, so a running session on a host that is not connected draws a grey, still mark, as
//! SPEC.md asks.
//!
//! A session on a host the helm cannot reach (a stale row) keeps its last-known status and also says why it is only
//! last-known: a neutral cause such as `Waiting for supervisor` or `Host needs update` ([`stale_cause`]), from the
//! host's connection state and from whether the host list was read at all. SPEC.md, "Session list": such sessions stay
//! in the list "with a neutral cause mark ... rather than vanishing". Ported from the old app's
//! `hosts::session_unavailable_cause`. The cause is the app's own wording, never the host's: retry counts and the
//! transport's error text belong to the session view's card, so a cause stays put while the helm keeps retrying.
//!
//! Other surfaces show the same badge ([`badge`]): the quick switcher, the menu header and the session pane's details
//! bar. The bar keeps an ended status on one line and cuts it short, which a wrapping sentence cannot do, so it lays out
//! the wording's parts itself ([`ended_words`]), decided here like every other form of it.
//!
//! Hover help and accessible descriptions here are the app's own words. A help that ever needs to quote a host value
//! (an annotation, a launch failure's detail) goes through [`HoverHelp::sentence`](crate::components::HoverHelp), which
//! escapes it; the wording a row shows on screen draws its host parts through [`Line`] for the same reason.

use farhelm_helm_client::Collections;
use farhelm_proto::SessionStatus;
use farhelm_proto::tensorlake::LifecycleDirection;
use farhelm_proto::ui_channel::{Availability, Collection, HostStateView, HostView, SessionRow};
use gpui::{
    Div, IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder as _, px,
    svg,
};

use crate::assets::Icon;
use crate::components::pulse_opacity;
use crate::host_text::{HostSentence, HostText, Line};
use crate::theme::{Palette, Token, metrics};

/// What a row shows for its status.
#[derive(Clone)]
pub(crate) enum Badge {
    /// A live session: a mark in the row's status column, and its word for assistive technology.
    Live(Live),
    /// An ended session: its full wording, drawn on a line of its own.
    Ended {
        kind: Ended,
        /// The words, which may quote the host (a stop annotation, a launch failure's detail).
        line: Line,
    },
}

/// The live statuses' marks. Each is drawn from its own shape (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Live {
    Running,
    Waiting,
    /// Idle with output newer than the last time a client had the session open.
    IdleUnseen,
    Idle,
}

/// The ended statuses, which differ in color: an exit is quiet, an interruption amber (nothing failed, but the user may
/// want to act), an error red (the agent never started).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ended {
    Exited,
    Interrupted,
    Error,
}

impl Live {
    /// The status word, which assistive technology reads instead of the mark.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Live::Running => "Running",
            Live::Waiting => "Waiting",
            Live::IdleUnseen => "Idle — new output",
            Live::Idle => "Idle",
        }
    }

    /// The mark's shape.
    fn icon(self) -> Icon {
        match self {
            Live::Running => Icon::StatusRunning,
            Live::Waiting => Icon::StatusWaiting,
            Live::IdleUnseen => Icon::StatusUnseen,
            Live::Idle => Icon::StatusIdle,
        }
    }

    /// The mark's color on an ordinary row: `.status-badge.running` and its siblings. Waiting takes the loudest live
    /// color, `--danger`, because it is the one status that asks for a person; unseen idle reuses `--info`.
    fn color(self, palette: &Palette) -> Token {
        match self {
            Live::Running => palette.ok,
            Live::Waiting => palette.danger,
            Live::IdleUnseen => palette.info,
            Live::Idle => palette.fg_1,
        }
    }

    /// The mark's color on a row: its status color, except that on the selected row every mark turns `--on-fill`
    /// (the status hues are not calibrated against the solid selection), and that a row on a host that is not
    /// connected (`stale`) is grey whatever its status says. SPEC.md, "Status": a cached status is no longer evidence
    /// for any live color, so such a session "shows a grey, still dot" (the old sheet's `.session-row.stale
    /// .status-dot`, `--fg-2`). The shape stays, so the last-known status is still there to read.
    pub(crate) fn mark_color(self, palette: &Palette, selected: bool, stale: bool) -> Token {
        if selected {
            palette.on_fill
        } else if stale {
            palette.fg_2
        } else {
            self.color(palette)
        }
    }

    /// Whether this mark pulses on a row that is `stale` or not: only a running mark does, and only on a connected
    /// host. This is the one statement of that rule; [`pulses`] and [`Live::mark_opacity`] both ask here. The old
    /// sheet's `.session-row.stale` rule gives the reason: staleness is exactly the state in which nobody can claim
    /// something is happening right now, and motion reads as evidence, so a stale row's last-known running stands
    /// still.
    pub(crate) fn pulses(self, stale: bool) -> bool {
        self == Live::Running && !stale
    }

    /// The mark's opacity at pulse step `step` ([`crate::ticker::Pulsing::step`]): [`pulse_opacity`] at the palette's
    /// minimum for an ordinary row (`--status-pulse-min`) or the shallower one for the selected row
    /// (`--selected-status-pulse-min`), when the mark pulses ([`Live::pulses`]); full opacity for every other mark,
    /// whatever the step.
    pub(crate) fn mark_opacity(
        self,
        palette: &Palette,
        selected: bool,
        stale: bool,
        step: u64,
    ) -> f32 {
        if !self.pulses(stale) {
            return 1.;
        }
        let min = if selected {
            palette.selected_status_pulse_min
        } else {
            palette.status_pulse_min
        };
        pulse_opacity(step, min)
    }

    /// The mark, in [`Live::mark_color`] and at [`Live::mark_opacity`].
    pub(crate) fn mark(
        self,
        palette: &Palette,
        selected: bool,
        stale: bool,
        step: u64,
    ) -> impl IntoElement {
        let color = self.mark_color(palette, selected, stale);
        let opacity = self.mark_opacity(palette, selected, stale, step);
        svg()
            .path(self.icon().path())
            .size(metrics::STATUS_DOT)
            .flex_none()
            .text_color(crate::theme::color(color))
            .when(opacity < 1., |mark| mark.opacity(opacity))
    }
}

impl Ended {
    /// The status word alone, without the host's details: the part of an ended status the row's hover help can name.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Ended::Exited => "Exited",
            Ended::Interrupted => "Interrupted",
            Ended::Error => "Error",
        }
    }

    /// The wording's color on an ordinary row (`.status-badge.exited` and its siblings); `--on-fill` on the selected
    /// row.
    pub(crate) fn color(self, palette: &Palette, selected: bool) -> Token {
        if selected {
            return palette.on_fill;
        }
        match self {
            Ended::Exited => palette.fg_1,
            Ended::Interrupted => palette.warn,
            Ended::Error => palette.danger_strong,
        }
    }
}

impl Badge {
    /// The status word for the row's hover help and description: the live word, or the ended word without its host
    /// details, which the row already shows in full on its own line (help that quoted them would have to be a
    /// [`HoverHelp::sentence`](crate::components::HoverHelp) to stay escaped). `None` for a status that draws nothing.
    pub(crate) fn word(badge: Option<&Badge>) -> Option<&'static str> {
        match badge? {
            Badge::Live(live) => Some(live.word()),
            Badge::Ended { kind, .. } => Some(kind.word()),
        }
    }
}

/// Whether `row`'s mark pulses ([`Live::pulses`]): a running session on a connected host.
pub(crate) fn pulses(row: &SessionRow) -> bool {
    live(row).is_some_and(|live| live.pulses(row.stale))
}

/// The live mark `row` draws, or `None` for a status that draws no mark (an ended one, or one nobody classified).
/// Visible to the crate for the session pane's details bar, which draws the list's own mark.
pub(crate) fn live(row: &SessionRow) -> Option<Live> {
    match &row.info.status {
        SessionStatus::Running => Some(Live::Running),
        SessionStatus::Waiting => Some(Live::Waiting),
        SessionStatus::Idle if unseen(row) => Some(Live::IdleUnseen),
        SessionStatus::Idle => Some(Live::Idle),
        SessionStatus::Unknown
        | SessionStatus::Exited { .. }
        | SessionStatus::Interrupted
        | SessionStatus::Error { .. } => None,
    }
}

/// Whether the session has output nobody has looked at: its latest activity is newer than the stamp the helm recorded
/// the last time a client had it open, or nothing is recorded at all (never opened, or marked unread). SPEC.md,
/// "Status".
pub(crate) fn unseen(row: &SessionRow) -> bool {
    row.seen_activity_at
        .is_none_or(|seen| row.info.effective_activity() > seen)
}

/// What `row`'s status shows anywhere but its sidebar row (the quick switcher's rows, the menu header): an exit with no
/// code reads plain `Exited`. `id` names the ended line's sentence, which needs an element id when it quotes the host.
pub(crate) fn badge(row: &SessionRow, id: SharedString) -> Option<Badge> {
    badge_with(row, id, false)
}

/// What `row`'s status shows in its sidebar row: [`badge`], except that an exit with no code says so,
/// `Exited (code unknown)`. SPEC.md, "Status": an exit the supervisor did not witness shows "an explicit unknown
/// otherwise — never a guess", and the row is where the complete ended wording lives; the old app made that change on
/// the sidebar alone (`rows::session_row_detail`) and kept plain `Exited` in its summary card and menu header.
pub(crate) fn row_badge(row: &SessionRow, id: SharedString) -> Option<Badge> {
    badge_with(row, id, true)
}

/// [`badge`] and [`row_badge`]: `unknown_code` says whether an exit without a code names the code as unknown.
fn badge_with(row: &SessionRow, id: SharedString, unknown_code: bool) -> Option<Badge> {
    if let Some(live) = live(row) {
        return Some(Badge::Live(live));
    }
    let ended = ended_words(row, unknown_code)?;
    Some(Badge::Ended {
        kind: ended.kind,
        line: ended.line(id),
    })
}

/// An ended status's wording in its two parts: the app's words, and the one host value that may follow them (a stop
/// annotation, a launch failure's detail). [`Badge::Ended`]'s line is these parts as one sentence; a surface that has
/// to keep the wording on one line and cut the host value short first (the session pane's details bar) lays the parts
/// out itself, which a sentence, one text element, cannot do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EndedWords {
    pub(crate) kind: Ended,
    /// The app's words, with the separator before the host value when there is one: `Exited (code 0) — `.
    pub(crate) words: String,
    /// The host value after the words, if any.
    pub(crate) host: Option<HostText>,
}

impl EndedWords {
    /// The wording as one line, a sentence under `id` when it quotes the host.
    pub(crate) fn line(&self, id: SharedString) -> Line {
        match &self.host {
            Some(host) => HostSentence::new(id)
                .text(self.words.clone())
                .host(host)
                .into(),
            None => Line::from(self.words.clone()),
        }
    }
}

/// The wording of `row`'s ended status ([`badge`]'s when `unknown_code` is false, [`row_badge`]'s when true), or
/// `None` for a live status or one nobody classified. The one place the ended wording is decided, so the row, the
/// quick switcher, the menu header and the details bar cannot word an exit differently.
pub(crate) fn ended_words(row: &SessionRow, unknown_code: bool) -> Option<EndedWords> {
    match &row.info.status {
        SessionStatus::Unknown
        | SessionStatus::Running
        | SessionStatus::Waiting
        | SessionStatus::Idle => None,
        SessionStatus::Exited { exit_code } => {
            let mut words = "Exited".to_owned();
            match exit_code {
                Some(code) => words.push_str(&format!(" (code {code})")),
                None if unknown_code => words.push_str(" (code unknown)"),
                None => {}
            }
            let host = row.info.annotation.as_ref().map(|annotation| {
                words.push_str(" — ");
                HostText::from_host(annotation.as_str())
            });
            Some(EndedWords {
                kind: Ended::Exited,
                words,
                host,
            })
        }
        SessionStatus::Interrupted => Some(EndedWords {
            kind: Ended::Interrupted,
            words: "Interrupted".to_owned(),
            host: None,
        }),
        SessionStatus::Error { detail } => Some(EndedWords {
            kind: Ended::Error,
            words: "Error — ".to_owned(),
            host: Some(HostText::from_host(detail.as_str())),
        }),
    }
}

/// The ended wording, drawn as `.session-row-detail .status-badge`: 12-pixel text in the status's color that wraps
/// rather than truncating, since this is the one place an ended message must stay fully visible. A plain element, so
/// the row can lay it out beside a stale cause on one wrapping line.
pub(crate) fn ended_line(kind: Ended, line: Line, palette: &Palette, selected: bool) -> Div {
    div()
        .min_w_0()
        .text_size(metrics::TEXT_SM)
        .line_height(px(16.))
        .text_color(crate::theme::color(kind.color(palette, selected)))
        .child(line)
}

// ---- Stale rows: why a status is only last-known ---------------------------------------------------------------------

/// Where a stale row's host stands in the registry this window holds: the old app's `HostLookup`, which the cause is
/// read from. The registry read and the host's own state are different claims, so each has its own case: a failed read
/// says nothing about the host, and a successful read that lacks the host says it was removed.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HostLookup<'a> {
    /// No host list has arrived yet.
    Pending,
    /// The latest host list read failed, so nothing current can be said about any host.
    Failed,
    /// A successful read lists the host.
    Known(&'a HostView),
    /// A successful read does not list the host: it was removed from the registry.
    Absent,
}

impl<'a> HostLookup<'a> {
    /// The lookup of registry id `host` in `collections`.
    pub(crate) fn of(collections: &'a Collections, host: i64) -> Self {
        let Some(meta) = collections.meta(Collection::Hosts) else {
            return HostLookup::Pending;
        };
        if let Availability::Unavailable { .. } = meta.availability {
            return HostLookup::Failed;
        }
        match collections.get::<HostView>(&HostView::key(host)) {
            Some(host) => HostLookup::Known(host),
            None => HostLookup::Absent,
        }
    }
}

/// Why a session's status is only last-known, as its row names it: the old `session_unavailable_cause`, word for word.
///
/// A connected host still has stale rows for a moment while the helm's listing catches up (`Refreshing session`), and a
/// host refused at the handshake for running a newer protocol needs a newer helm, not an update, so it reads
/// `Host is too new` ([`crate::hosts::offers::runs_newer_version`] is that test, shared with the Hosts panel). The old
/// table's "unknown state" case has no counterpart: the port's host state is a closed type, so every state it can hold
/// has its own words here.
pub(crate) fn cause(lookup: HostLookup<'_>) -> &'static str {
    let host = match lookup {
        HostLookup::Pending => return "Checking host",
        HostLookup::Failed => return "Host state unavailable",
        HostLookup::Absent => return "Host removed",
        HostLookup::Known(host) => host,
    };
    match &host.state {
        HostStateView::Connecting { .. } | HostStateView::Unreachable { .. } => {
            "Waiting for supervisor"
        }
        HostStateView::Connected { .. } => "Refreshing session",
        HostStateView::VersionSkew { .. }
            if crate::hosts::offers::runs_newer_version(&host.state) =>
        {
            "Host is too new"
        }
        HostStateView::VersionSkew { .. } => "Host needs update",
        HostStateView::IdentityMismatch { .. } => "Host identity mismatch",
        HostStateView::IdentityUnverified { .. } => "Host identity unverified",
        HostStateView::Duplicate { .. } => "Duplicate host",
        HostStateView::Suspended { .. } => "Host suspended",
        HostStateView::Stopped { .. } => "Environment stopped",
        HostStateView::TensorlakeChanging { direction, .. } => match direction {
            LifecycleDirection::Suspending => "Host suspending",
            LifecycleDirection::Resuming => "Host resuming",
        },
        HostStateView::Retired { .. } => "Host connection stopped",
        HostStateView::Deleting { .. } => "Host being deleted",
    }
}

/// The cause `row` shows, or `None` for a row whose host is connected and current (not stale), which shows none.
pub(crate) fn stale_cause(row: &SessionRow, collections: &Collections) -> Option<&'static str> {
    row.stale
        .then(|| cause(HostLookup::of(collections, row.host)))
}

/// The cause's hover help: what the mark means for the row beside it (the old capsule's `data-tooltip`).
pub(crate) fn cause_help(cause: &str) -> String {
    format!("{cause}: this is the session's last-known state")
}

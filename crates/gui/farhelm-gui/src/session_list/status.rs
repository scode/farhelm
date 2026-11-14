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

use farhelm_proto::SessionStatus;
use farhelm_proto::ui_channel::SessionRow;
use gpui::{
    IntoElement, ParentElement, SharedString, Styled, div, prelude::FluentBuilder as _, px, svg,
};

use crate::assets::Icon;
use crate::host_text::{HostSentence, HostText, Line};
use crate::theme::{Palette, Token, metrics};
use crate::ticker;

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

    /// The mark's opacity at pulse step `step` ([`ticker::Pulsing::step`]): [`pulse_opacity`] at the palette's
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
    /// details (hover help is the app's own words only). `None` for a status that draws nothing.
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
fn live(row: &SessionRow) -> Option<Live> {
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

/// The running mark's opacity at pulse step `step` (`0..ticker::PULSE_STEPS`), dipping to `min_percent` at its
/// dimmest. The old `farhelm-status-pulse` keyframes fall linearly from 1 to the minimum and climb back, and `steps(4)`
/// holds each quarter of each half at the value where it starts, so the steps read full, three quarters of the way,
/// half way, a quarter of the way, the minimum, and back up: with a 60% minimum, 1.0, 0.9, 0.8, 0.7, 0.6, 0.7, 0.8,
/// 0.9.
pub(crate) fn pulse_opacity(step: u64, min_percent: u8) -> f32 {
    let half = ticker::PULSE_STEPS / 2;
    let step = step % ticker::PULSE_STEPS;
    let depth = if step <= half {
        step
    } else {
        ticker::PULSE_STEPS - step
    };
    let dip = 1. - f32::from(min_percent) / 100.;
    1. - dip * depth as f32 / half as f32
}

/// Whether the session has output nobody has looked at: its latest activity is newer than the stamp the helm recorded
/// the last time a client had it open, or nothing is recorded at all (never opened, or marked unread). SPEC.md,
/// "Status".
pub(crate) fn unseen(row: &SessionRow) -> bool {
    row.seen_activity_at
        .is_none_or(|seen| row.info.effective_activity() > seen)
}

/// What `row`'s status shows. `id` names the ended line's sentence, which needs an element id when it quotes the host.
pub(crate) fn badge(row: &SessionRow, id: SharedString) -> Option<Badge> {
    match &row.info.status {
        SessionStatus::Unknown => None,
        SessionStatus::Running | SessionStatus::Waiting | SessionStatus::Idle => {
            live(row).map(Badge::Live)
        }
        SessionStatus::Exited { exit_code } => {
            let mut words = "Exited".to_owned();
            if let Some(code) = exit_code {
                words.push_str(&format!(" (code {code})"));
            }
            let line = match &row.info.annotation {
                Some(annotation) => HostSentence::new(id)
                    .text(words)
                    .text(" — ")
                    .host(&HostText::from_host(annotation.as_str()))
                    .into(),
                None => Line::from(words),
            };
            Some(Badge::Ended {
                kind: Ended::Exited,
                line,
            })
        }
        SessionStatus::Interrupted => Some(Badge::Ended {
            kind: Ended::Interrupted,
            line: "Interrupted".into(),
        }),
        SessionStatus::Error { detail } => Some(Badge::Ended {
            kind: Ended::Error,
            line: HostSentence::new(id)
                .text("Error — ")
                .host(&HostText::from_host(detail.as_str()))
                .into(),
        }),
    }
}

/// The ended wording, drawn as `.session-row-detail .status-badge`: 12-pixel text in the status's color that wraps
/// rather than truncating, since this is the one place an ended message must stay fully visible.
pub(crate) fn ended_line(
    kind: Ended,
    line: Line,
    palette: &Palette,
    selected: bool,
) -> impl IntoElement {
    div()
        .min_w_0()
        .text_size(metrics::TEXT_SM)
        .line_height(px(16.))
        .text_color(crate::theme::color(kind.color(palette, selected)))
        .child(line)
}

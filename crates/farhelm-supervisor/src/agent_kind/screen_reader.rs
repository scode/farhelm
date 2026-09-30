//! Per-harness screen readers: what one sampled terminal screen says about
//! the agent behind it.
//!
//! The supervisor's ticker captures each live agent pane on a budgeted
//! round robin (`service::ticker`), and every classification of a live
//! session — the running/waiting/idle status on the wire, and the
//! transitions that order the session list — is derived from what a
//! [`ScreenReader`] concluded about those captures. The shared layer
//! consumes readings and nothing else: it holds no per-harness knowledge,
//! and a harness differs from another only in which reader
//! [`reader_for`] hands out.
//!
//! Two kinds of evidence exist, and a [`Reading`] records which one it
//! rests on:
//!
//! - CHANGE COUNTING, the generic rule every harness falls back to: a
//!   screen that keeps changing reads working, one that stayed unchanged
//!   for [`QUIET_SAMPLES_BEFORE_IDLE`] of its own consecutive comparisons
//!   reads idle. It needs no knowledge of the agent, and it cannot tell an
//!   agent asking a question from one that finished, so it never reads
//!   waiting.
//! - RECOGNIZED CONTENT ("anchored" readings): a harness reader that knows
//!   what its agent draws can say working or waiting from the screen
//!   itself, whatever the counts say.
//!
//! Readers are stateless. The history change counting needs (how many
//! samples, how long the screen stood still) is kept by the sampler in
//! `ticker::ActivitySample` and handed in as [`SampleCounts`]; keeping it
//! there is what lets a rename share that history and a relaunch start a
//! fresh one without any reader having to know about either.
//!
//! Robustness: readers run on arbitrary terminal bytes that survived a
//! lossy UTF-8 decode, on a supervisor that is also serving live
//! terminals. Wrong is cosmetic (SPEC.md, Status); panicking is not, so a
//! reader never indexes or slices by byte offset.

use super::{CLAUDE_QUESTION_PHRASES, CODEX_QUESTION_PHRASES, codex_comparison};
use super::{codex_working, looks_like_a_choice_prompt};
use farhelm_proto::AgentKind;

/// How many of a session's OWN consecutive comparisons must show an
/// unchanged screen before the generic rule reads it idle rather than
/// working.
///
/// Counted in samples, not seconds, and that is the whole design. The
/// sampler works through live sessions on a budgeted round robin
/// (`ticker::SAMPLE_TAIL_BUDGET`), so a session's real sampling period is
/// `ceil(live / budget) × interval` — unbounded in the number of live
/// sessions. Any wall-clock window would therefore be crossed by a pane
/// that changed at EVERY one of its own samples as soon as a host ran
/// enough sessions, turning "how busy is this host" into "this session is
/// idle". Counting the session's own observations makes the cadence cancel
/// out: the question is "how many times have I looked and seen nothing
/// new", which means the same thing at any population.
///
/// Three, for the reasons a shorter count is wrong rather than for a
/// timing: a screen can legitimately repeat for a sample or two while an
/// agent is working — between tool calls, on a spinner frame that renders
/// identically, on output that lands and is overwritten within one sample
/// gap — and a count of one would flip such a session to idle on every one
/// of those. Three consecutive silent looks is a pattern rather than a
/// coincidence. Nothing wants it much larger: at the production cadence
/// with a small fleet this is a handful of seconds, and an idle that takes
/// a minute to appear is not the signal the status column exists to give.
pub(crate) const QUIET_SAMPLES_BEFORE_IDLE: u64 = 3;

/// The live state one screen is evidence for.
///
/// Deliberately smaller than the wire `SessionStatus`: a screen is
/// evidence about what a live agent is doing, never about whether its
/// process exists. Exits, launch errors, and interrupted sessions come
/// from the lifecycle record and tmux's liveness verdict in
/// `service::status`, which consults a reading only for a live pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScreenState {
    /// The agent is producing output or showing that it is busy.
    Working,
    /// The agent shows a pending question or approval directed at the user.
    Waiting,
    /// The agent is alive and at rest.
    Idle,
}

/// One reader's conclusion about one sampled screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reading {
    /// What the screen says the agent is doing.
    pub(crate) state: ScreenState,
    /// Whether the conclusion rests on recognized screen content rather
    /// than on change counting alone.
    ///
    /// The sampler uses this to end the startup gap after a supervisor
    /// restart: a recognized screen is fresh evidence on its first
    /// sample, while change counting needs comparisons to mean anything.
    pub(crate) anchored: bool,
}

/// Before any screen has been read, a session reads as change counting
/// with no history does: working, unanchored — the conservative answer for
/// a pane nobody has looked at yet.
impl Default for Reading {
    fn default() -> Self {
        generic_reading(SampleCounts::default())
    }
}

/// The change-counting history the sampler keeps for one session run,
/// after folding the sample being read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SampleCounts {
    /// How many successful captures this run has folded, including the
    /// current one.
    pub(crate) samples: u64,
    /// How many consecutive comparisons, ending with the current one,
    /// found the screen unchanged.
    pub(crate) unchanged_streak: u64,
}

/// One sampled screen, as the reader sees it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Screen<'a> {
    /// The visible grid as captured, bounded by the reader's
    /// [`ScreenReader::capture_bytes`] (or the sampler's default).
    pub(crate) capture: &'a str,
    /// The bottom of the same grid, bounded to the sampler's retained
    /// tail size. Prompt recognition reads this, so a larger capture taken
    /// for another purpose cannot change what counts as the bottom of the
    /// screen.
    pub(crate) tail: &'a str,
}

/// What a harness knows about reading its own agent's screen.
///
/// Every method has the generic behavior as its default, so a harness
/// with no dedicated reader is simply one that overrides nothing.
pub(crate) trait ScreenReader: Sync {
    /// How many bytes of the visible grid this reader needs captured, or
    /// `None` for the sampler's default.
    fn capture_bytes(&self) -> Option<usize> {
        None
    }

    /// The text compared with the previous sample to decide whether the
    /// screen changed. The default compares the whole capture; a reader may
    /// remove a proven vendor redraw region that is not output.
    fn comparison(&self, capture: &str) -> String {
        capture.to_string()
    }

    /// Classify one screen given the run's change-counting history.
    fn read(&self, counts: SampleCounts, screen: &Screen<'_>) -> Reading {
        let _ = screen;
        generic_reading(counts)
    }
}

/// The generic change-counting rule, which every reader falls back to.
///
/// Idle needs at least two samples (one comparison) and a quiet streak of
/// [`QUIET_SAMPLES_BEFORE_IDLE`]; everything else reads working, which is
/// the conservative answer while the evidence is thin — a pane nobody has
/// compared yet, or one whose captures just recovered from a failure, is
/// not known to be at rest.
pub(crate) fn generic_reading(counts: SampleCounts) -> Reading {
    let idle = counts.samples >= 2 && counts.unchanged_streak >= QUIET_SAMPLES_BEFORE_IDLE;
    Reading {
        state: if idle {
            ScreenState::Idle
        } else {
            ScreenState::Working
        },
        anchored: false,
    }
}

/// The reader for one agent kind. Kinds without a dedicated reader get the
/// generic one, which is also what an explicitly non-integrated session
/// uses.
pub(crate) fn reader_for(kind: AgentKind) -> &'static dyn ScreenReader {
    match kind {
        AgentKind::Claude => &ClaudeReader,
        AgentKind::Codex => &CodexReader,
        AgentKind::Goose
        | AgentKind::Pi
        | AgentKind::Omp
        | AgentKind::Grok
        | AgentKind::Generic => &GenericReader,
    }
}

/// Change counting alone.
struct GenericReader;

impl ScreenReader for GenericReader {}

/// Claude Code: a recognized approval or question prompt reads waiting;
/// everything else is change counting.
struct ClaudeReader;

impl ScreenReader for ClaudeReader {
    fn read(&self, counts: SampleCounts, screen: &Screen<'_>) -> Reading {
        if looks_like_a_choice_prompt(screen.tail, CLAUDE_QUESTION_PHRASES) {
            return Reading {
                state: ScreenState::Waiting,
                anchored: true,
            };
        }
        generic_reading(counts)
    }
}

/// Codex: a recognized approval prompt reads waiting, the `Working (…)`
/// widget adjoining the composer reads working, and composer redraws are
/// masked out of change comparison.
struct CodexReader;

/// Codex's composer masking needs to see farther than the retained status
/// tail: changing three-byte sparkle cells near the final 4096-byte boundary
/// can otherwise move unrelated text into or out of comparison. This larger
/// capture is temporary; each sample retains only the capped comparison.
const CODEX_ACTIVITY_CAPTURE_BYTES: usize = 64 * 1024;

impl ScreenReader for CodexReader {
    fn capture_bytes(&self) -> Option<usize> {
        Some(CODEX_ACTIVITY_CAPTURE_BYTES)
    }

    fn comparison(&self, capture: &str) -> String {
        codex_comparison(capture)
    }

    /// A waiting prompt wins over the working widget: it names the
    /// interaction the user is presently asked for.
    fn read(&self, counts: SampleCounts, screen: &Screen<'_>) -> Reading {
        if looks_like_a_choice_prompt(screen.tail, CODEX_QUESTION_PHRASES) {
            return Reading {
                state: ScreenState::Waiting,
                anchored: true,
            };
        }
        if codex_working(screen.capture) {
            return Reading {
                state: ScreenState::Working,
                anchored: true,
            };
        }
        generic_reading(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(samples: u64, unchanged_streak: u64) -> SampleCounts {
        SampleCounts {
            samples,
            unchanged_streak,
        }
    }

    fn screen(text: &str) -> Screen<'_> {
        Screen {
            capture: text,
            tail: text,
        }
    }

    /// The generic rule's two edges: idle needs both a comparison and a
    /// full quiet streak, and nothing about it is anchored.
    ///
    /// Why it matters: this is the whole status for every harness without
    /// a dedicated reader, and the fallback for the ones with one.
    #[test]
    fn generic_reading_needs_a_comparison_and_a_full_quiet_streak() {
        assert_eq!(generic_reading(counts(0, 0)).state, ScreenState::Working);
        assert_eq!(
            generic_reading(counts(1, QUIET_SAMPLES_BEFORE_IDLE)).state,
            ScreenState::Working,
            "one sample has nothing to have compared against"
        );
        assert_eq!(
            generic_reading(counts(9, QUIET_SAMPLES_BEFORE_IDLE - 1)).state,
            ScreenState::Working
        );
        assert_eq!(
            generic_reading(counts(9, QUIET_SAMPLES_BEFORE_IDLE)).state,
            ScreenState::Idle
        );
        assert!(!generic_reading(counts(9, 9)).anchored);
    }

    /// Kinds without a dedicated reader ignore what their screen shows,
    /// even a screen another harness would read as waiting.
    ///
    /// Why it matters: per-harness knowledge must be reached only through
    /// that harness's reader, or a generic session would inherit another
    /// agent's heuristics.
    #[test]
    fn kinds_without_a_dedicated_reader_use_change_counting_alone() {
        let approval = "Do you want to proceed?\n❯ 1. Yes\n  2. No";
        for kind in [
            AgentKind::Goose,
            AgentKind::Pi,
            AgentKind::Omp,
            AgentKind::Grok,
            AgentKind::Generic,
        ] {
            let reader = reader_for(kind);
            assert_eq!(reader.capture_bytes(), None, "{kind:?}");
            assert_eq!(reader.comparison(approval), approval, "{kind:?}");
            assert_eq!(
                reader.read(counts(9, 9), &screen(approval)),
                generic_reading(counts(9, 9)),
                "{kind:?}"
            );
        }
    }
}

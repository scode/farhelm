//! Per-harness screen readers: what one sampled terminal screen says about
//! the agent behind it.
//!
//! The supervisor's ticker captures each live agent pane on a budgeted
//! round robin (`service::ticker`), and every classification of a live
//! session — the running/waiting/idle status on the wire, and the
//! transitions that order the session list, date its activity, and mark it
//! unseen — is derived from what a [`ScreenReader`] concluded about those
//! captures. The shared layer consumes readings and nothing else: it holds
//! no per-harness knowledge, and a harness differs from another only in
//! which reader [`reader_for`] hands out.
//!
//! Two kinds of evidence exist, and a [`Reading`] records which one it
//! rests on:
//!
//! - CHANGE COUNTING, the generic rule every harness falls back to: a
//!   screen that keeps changing reads working, one that stayed unchanged
//!   for [`QUIET_SAMPLES_BEFORE_IDLE`] of its own consecutive comparisons
//!   reads idle. It needs no knowledge of the agent, and it cannot tell an
//!   agent asking a question from one that finished, so it never reads
//!   waiting. Its weakness is the reason this module exists: agents redraw
//!   parts of their screen while idle (Claude Code's `/clear` hint, Codex's
//!   recap block and rate-limit footer), and change counting reads every
//!   such redraw as work.
//! - RECOGNIZED CONTENT ("anchored" readings): the Claude Code and Codex
//!   readers know what their agent draws — its spinner, its input box, the
//!   key-hint footer of every dialog that asks the user something — and
//!   answer from that, whatever the screen did since the last sample. The
//!   rules are anchored on wording and layout observed in real captures
//!   (`tests/fixtures/screens/`, re-captured by
//!   `scripts/capture-agent-screens.py`), preferring text the vendor shows
//!   the user as an instruction over decoration such as spinner glyphs.
//!
//! A dedicated reader that recognizes none of its anchors (a vendor UI
//! change, an older version) falls back to change counting for that sample
//! and says so ([`Reading::anchored`] false), which the sampler logs once
//! per run as likely rule drift. A screen it recognizes as carrying no
//! state at all — a model picker, a transcript view the user opened —
//! reads [`ScreenState::Unknown`], and the sampler keeps the previous
//! reading.
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

use super::{codex_working, is_codex_composer_padding};
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

/// How much of a sampled pane's screen is kept.
///
/// Enough for the bottom of a normal terminal — an 80x24 screen of dense
/// text is under 2 KiB — with headroom for a wide one, because the
/// readers have to see a whole approval prompt to recognize it. The cap
/// exists because this is held per session for as long as the session
/// lives, and a pane rendering a 500-column wall of text should not be
/// able to grow the supervisor's resident memory through it.
pub(crate) const SAMPLE_TAIL_BYTES: usize = 4096;

/// What one screen is evidence for.
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
    /// A recognized screen that says nothing about the agent's state (a
    /// menu or viewer the user opened). The sampler keeps the previous
    /// reading rather than storing this.
    Unknown,
}

/// One reader's conclusion about one sampled screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reading {
    /// What the screen says the agent is doing.
    pub(crate) state: ScreenState,
    /// Whether the conclusion rests on recognized screen content rather
    /// than on change counting alone.
    ///
    /// The sampler reads this three ways: a recognized screen is fresh
    /// evidence on its first sample (it ends the startup gap after a
    /// supervisor restart), a recognized working screen dates activity
    /// whether or not it changed, and an unanchored reading from a
    /// dedicated reader is the fallback that signals rule drift.
    pub(crate) anchored: bool,
}

impl Reading {
    /// A reading from recognized content.
    fn anchored(state: ScreenState) -> Self {
        Reading {
            state,
            anchored: true,
        }
    }
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
    /// The bottom of the visible grid, bounded to the sampler's retained
    /// tail size and trimmed of the blank rows a pane is padded out with.
    pub(crate) text: &'a str,
    /// The pane title the agent last set (OSC 0/2), or empty when the
    /// reader did not ask for it ([`ScreenReader::wants_title`]) or it
    /// could not be read.
    pub(crate) title: &'a str,
}

/// What a harness knows about reading its own agent's screen.
///
/// Every method has the generic behavior as its default, so a harness
/// with no dedicated reader is simply one that overrides nothing.
pub(crate) trait ScreenReader: Sync {
    /// Whether the sampler should fetch the pane title for this reader. A
    /// separate tmux call per sample, so only readers that use it ask.
    fn wants_title(&self) -> bool {
        false
    }

    /// Whether this reader recognizes its agent's screens at all. When it
    /// does, an unanchored reading is a fallback worth logging: the
    /// agent's UI no longer looks like what the rules were written for.
    fn has_screen_rules(&self) -> bool {
        false
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
#[warn(clippy::wildcard_enum_match_arm)]
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

/// The screen's non-blank lines, top to bottom, with trailing blanks
/// trimmed. Blank rows carry no anchor and would only make "the last few
/// lines" depend on how tall the pane is.
fn content_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// The last `count` entries of `lines` (all of them when there are fewer).
fn last_lines<'a>(lines: &'a [&'a str], count: usize) -> &'a [&'a str] {
    &lines[lines.len().saturating_sub(count)..]
}

/// Whether any of `lines` contains `needle`, ignoring ASCII case. Vendors
/// capitalize their key hints inconsistently ("Esc to cancel" in Claude
/// Code, "esc to cancel" in Codex), and the wording is the anchor, not the
/// case.
fn any_contains(lines: &[&str], needle: &str) -> bool {
    lines
        .iter()
        .any(|line| line.to_ascii_lowercase().contains(needle))
}

// ---------------------------------------------------------------------
// Change counting alone
// ---------------------------------------------------------------------

/// Change counting alone: every kind without a dedicated reader.
struct GenericReader;

impl ScreenReader for GenericReader {}

// ---------------------------------------------------------------------
// Claude Code (observed on 2.1.285)
// ---------------------------------------------------------------------

/// Claude Code's reader.
///
/// Claude draws its input box as a `❯` line directly below a horizontal
/// rule, with a user-configurable status area below it. While a turn runs,
/// a spinner line (`✶ Imagining… (2s · thinking)`, the glyph and verb
/// rotating) sits just above that box for the whole turn, tool runs
/// included. Every dialog that needs the user — permission prompts,
/// question forms, the folder-trust prompt — replaces the box and ends in
/// a key-hint footer containing "Esc to cancel".
///
/// Deliberate choices: background tasks announced at an idle prompt read
/// idle, because the agent is waiting on the user's next message, not
/// working on one; and the pane title is not read, because Claude keeps it
/// at `✳ <topic>` through a whole turn, so it says nothing about work.
struct ClaudeReader;

/// How far from the bottom a Claude dialog's key-hint footer may sit.
/// Dialogs put it on the last line; the slack covers a wrapped footer.
const CLAUDE_FOOTER_LINES: usize = 2;

/// How far above the input box's top rule the spinner line may sit.
/// Claude puts notices (a connector warning, a tmux hint, a tip) between
/// the two; six covers every observed arrangement without reaching into
/// transcript text.
const CLAUDE_SPINNER_LINES_ABOVE_BOX: usize = 6;

impl ScreenReader for ClaudeReader {
    fn has_screen_rules(&self) -> bool {
        true
    }

    fn read(&self, counts: SampleCounts, screen: &Screen<'_>) -> Reading {
        let lines = content_lines(screen.text);
        let Some(box_rule) = claude_input_box_rule(&lines) else {
            // Every dialog and viewer replaces the input box, so footers are
            // only looked for when the box is gone; with the box drawn, the
            // same words could be the user's own unsent draft.
            let footer = last_lines(&lines, CLAUDE_FOOTER_LINES);
            // Menus and viewers the user opened. Checked before the dialog
            // footer because the model picker ends in "Esc to cancel" too.
            if any_contains(footer, "showing detailed transcript")
                || any_contains(footer, "enter to set as default")
            {
                return Reading::anchored(ScreenState::Unknown);
            }
            if any_contains(footer, "esc to cancel") {
                return Reading::anchored(ScreenState::Waiting);
            }
            return generic_reading(counts);
        };
        let above_box = &lines[box_rule.saturating_sub(CLAUDE_SPINNER_LINES_ABOVE_BOX)..box_rule];
        if above_box.iter().any(|line| is_claude_spinner_line(line)) {
            return Reading::anchored(ScreenState::Working);
        }
        Reading::anchored(ScreenState::Idle)
    }
}

/// The index of the horizontal rule that opens Claude's input box: the
/// last `❯` line whose preceding line is a rule.
///
/// The last one, because Claude echoes submitted prompts into the
/// transcript with the same `❯` prefix; only the box has a rule directly
/// above it. Menus draw `❯` at a selected option, but never under a rule.
fn claude_input_box_rule(lines: &[&str]) -> Option<usize> {
    (1..lines.len())
        .rev()
        .find(|&index| lines[index].starts_with('❯') && lines[index - 1].starts_with('─'))
        .map(|index| index - 1)
}

/// Whether `line` is Claude's working spinner: one non-alphanumeric glyph,
/// a space, a one-word verb ending in `…`, then ` (` and the elapsed
/// seconds — `✶ Imagining… (2s · thinking)`.
///
/// The shape rather than a glyph list, because the glyphs rotate and have
/// changed between versions; the finished-turn line (`✻ Cogitated for 26s`)
/// carries no ellipsis and so does not match.
fn is_claude_spinner_line(line: &str) -> bool {
    let mut chars = line.trim_start().chars();
    let Some(glyph) = chars.next() else {
        return false;
    };
    if glyph.is_alphanumeric() || glyph.is_whitespace() || chars.next() != Some(' ') {
        return false;
    }
    let Some((verb, after)) = chars.as_str().split_once("… (") else {
        return false;
    };
    !verb.is_empty()
        && !verb.contains(char::is_whitespace)
        && after.starts_with(|c: char| c.is_ascii_digit())
}

// ---------------------------------------------------------------------
// Codex (observed on 0.159.0)
// ---------------------------------------------------------------------

/// Codex's reader.
///
/// Codex's composer is the last `›` line on screen, with only indented
/// rows below it: wrapped draft lines and one or two footer rows (a
/// configurable status line, key hints). While a turn runs,
/// Codex animates a Braille spinner at the start of the pane title for the
/// whole turn; the on-screen `Working (…)` widget appears during tool runs
/// but not while prose streams, so the title is the primary working
/// signal and the widget a second one for a host where the title could not
/// be read. When Codex needs the user it says so in the title
/// (`[ ! ] Action Required`, alternating with `[ . ]`) and on screen:
/// approvals end in "esc to cancel", its question form in "enter to
/// submit answer" or "enter to submit all", the folder-trust prompt in
/// "enter continue", and a question queued during a turn shows as
/// `? N question(s)` with a "to answer" hint.
///
/// The footer's rate-limit counters and the recap block Codex appends
/// after a stretch without focus sit above or below an unchanged composer,
/// so they read idle like the rest of an idle screen.
struct CodexReader;

/// How far from the bottom a Codex dialog's key-hint footer may sit. Dialogs
/// put it on the last line; the slack covers a wrapped footer. An idle
/// screen's last two lines are its own footer rows, never the composer, so
/// a draft cannot be mistaken for a dialog footer.
const CODEX_FOOTER_LINES: usize = 2;

/// How far from the bottom a queued question's two rows may sit: above the
/// composer and its footer rows.
const CODEX_QUEUED_QUESTION_LINES: usize = 8;

/// The title prefixes Codex alternates between while it needs the user.
/// Only the prefix counts: the rest of the title is the conversation topic,
/// which may contain the same words.
const CODEX_ACTION_REQUIRED_TITLES: [&str; 2] = ["[ ! ] Action Required", "[ . ] Action Required"];

impl ScreenReader for CodexReader {
    fn wants_title(&self) -> bool {
        true
    }

    fn has_screen_rules(&self) -> bool {
        true
    }

    fn read(&self, counts: SampleCounts, screen: &Screen<'_>) -> Reading {
        let lines = content_lines(screen.text);
        // Codex draws its dialogs as numbered menus in place of the composer
        // (`› 1. Yes, proceed`). With a plain draft in the composer instead,
        // no dialog is open, and footer words in its last rows are the
        // user's own text.
        let footer: Vec<&str> = if codex_dialog_may_be_open(&lines, screen.text) {
            last_lines(&lines, CODEX_FOOTER_LINES)
                .iter()
                .copied()
                .filter(|line| !line.starts_with('›'))
                .collect()
        } else {
            Vec::new()
        };
        if screen.text.contains("Select Model and Effort") && any_contains(&footer, "esc back") {
            return Reading::anchored(ScreenState::Unknown);
        }
        if CODEX_ACTION_REQUIRED_TITLES
            .iter()
            .any(|prefix| screen.title.starts_with(prefix))
            || any_contains(&footer, "esc to cancel")
            || any_contains(&footer, "enter to submit answer")
            || any_contains(&footer, "enter to submit all")
            || any_contains(&footer, "enter continue")
            || codex_queued_question(last_lines(&lines, CODEX_QUEUED_QUESTION_LINES))
        {
            return Reading::anchored(ScreenState::Waiting);
        }
        if screen
            .title
            .starts_with(|c: char| ('\u{2800}'..='\u{28ff}').contains(&c))
            || codex_working(screen.text)
        {
            return Reading::anchored(ScreenState::Working);
        }
        if codex_has_composer(&lines) {
            return Reading::anchored(ScreenState::Idle);
        }
        generic_reading(counts)
    }
}

/// Whether the bottom of a Codex screen shows a queued question waiting
/// for the user: a `? N question(s)` row followed by its "to answer" hint.
fn codex_queued_question(bottom: &[&str]) -> bool {
    bottom.windows(2).any(|pair| {
        let row = pair[0].trim_start();
        row.starts_with("? ") && row.contains(" question") && pair[1].contains("to answer")
    })
}

/// Whether a Codex dialog may be on screen.
///
/// Codex draws its dialogs as menus in place of the composer: the selected
/// option marked `› 1. …` with another numbered option below it
/// (`  2. …`) or, when the highlight sits on the last option, in the same
/// block of rows above it. A `›` row that is not such a menu is the composer holding a
/// draft, which Codex never shows beside a dialog, and so is a screen whose
/// last rows carry the idle composer's own `? for shortcuts` hint. With no
/// `›` row at all, a dialog may be open.
///
/// The block above is read in the raw grid (`raw`), not in `lines`, which
/// drops blank rows, and it ends at the first blank row: the composer always
/// has a blank padding row over it, so for a numbered draft the block is
/// empty, while a menu's options sit together in one block even when an
/// option wraps onto a continuation row. Looking past that blank row would
/// let a numbered list at the end of the transcript turn a draft into a
/// menu.
fn codex_dialog_may_be_open(lines: &[&str], raw: &str) -> bool {
    if last_lines(lines, CODEX_FOOTER_LINES)
        .iter()
        .any(|line| line.contains("for shortcuts"))
    {
        return false;
    }
    let Some(prompt) = lines.iter().rposition(|line| line.starts_with('›')) else {
        return true;
    };
    let is_option = |text: &str| {
        let digits = text.chars().take_while(char::is_ascii_digit).count();
        digits > 0 && text.chars().nth(digits) == Some('.')
    };
    let selected = lines[prompt]
        .strip_prefix('›')
        .unwrap_or(lines[prompt])
        .trim_start();
    let option_below = lines[prompt + 1..]
        .iter()
        .any(|line| is_option(line.trim_start()));
    let option_above = raw
        .lines()
        .rev()
        .skip_while(|line| !line.starts_with('›'))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .any(|line| is_option(line.trim_start()));
    is_option(selected) && (option_below || option_above)
}

/// Whether a Codex screen ends in its composer: the last `›` line, with
/// nothing below it but indented rows (wrapped draft lines, the footer
/// rows) and sparkle padding.
///
/// Layout rather than content, because the composer holds whatever the user
/// typed, a numbered list or a long wrapped paragraph included. The
/// transcript echoes submitted prompts with the same `›`, but an echo is
/// always followed by the agent's reply at column zero (`• …`), which is
/// what rules it out. The working check's `codex_composer_top` locates the
/// same composer more strictly, because it also needs the padding above the
/// prompt; see its docs for how the two differ. Codex menus mark their selected option with `›` as
/// well; the dialogs among them are recognized by their footers before this
/// is asked.
fn codex_has_composer(lines: &[&str]) -> bool {
    let Some(prompt) = lines.iter().rposition(|line| line.starts_with('›')) else {
        return false;
    };
    lines[prompt + 1..]
        .iter()
        .all(|line| line.starts_with(char::is_whitespace) || is_codex_composer_padding(line))
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

    fn read(kind: AgentKind, text: &str, title: &str) -> Reading {
        reader_for(kind).read(counts(9, 0), &Screen { text, title })
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
    /// even a screen Claude's reader would read as waiting, and ask for no
    /// title.
    ///
    /// Why it matters: per-harness knowledge must be reached only through
    /// that harness's reader, or a generic session would inherit another
    /// agent's rules.
    #[test]
    fn kinds_without_a_dedicated_reader_use_change_counting_alone() {
        let dialog = "Do you want to proceed?\n❯ 1. Yes\n  2. No\nEsc to cancel";
        for kind in [
            AgentKind::Goose,
            AgentKind::Pi,
            AgentKind::Omp,
            AgentKind::Grok,
            AgentKind::Generic,
        ] {
            let reader = reader_for(kind);
            assert!(
                !reader.wants_title() && !reader.has_screen_rules(),
                "{kind:?}"
            );
            assert_eq!(
                read(kind, dialog, "[ ! ] Action Required"),
                generic_reading(counts(9, 0)),
                "{kind:?}"
            );
        }
    }

    /// Claude's spinner is recognized by its shape, whatever the glyph, and
    /// the finished-turn line and ordinary prose are not.
    ///
    /// Why it matters: the glyphs rotate every frame and have changed
    /// between versions, so a glyph list would go stale; the finished-turn
    /// line sits in the same place and must read idle.
    #[test]
    fn claude_spinner_line_is_recognized_by_shape() {
        for line in [
            "✶ Imagining… (2s · thinking)",
            "· Imagining… (3s · ↓ 262 tokens)",
            "* Tempering… (48s · ↓ 2.9k tokens · thinking with high effort)",
            "  ✢ Slithering… (1m 4s · ↓ 224 tokens)",
        ] {
            assert!(is_claude_spinner_line(line), "{line:?}");
        }
        for line in [
            "✻ Cogitated for 26s · done 3:41 PM",
            "● Skillette is loaded… (not a spinner)",
            "Imagining… (2s)",
            "✶ Two words… (2s)",
            "✶ Imagining… (soon)",
            "",
        ] {
            assert!(!is_claude_spinner_line(line), "{line:?}");
        }
    }

    /// A Claude screen with none of the anchors falls back to change
    /// counting, unanchored.
    ///
    /// Why it matters: an unrecognized screen must not be guessed at from
    /// the rules; the fallback keeps the pre-reader behavior and is what the
    /// sampler reports as rule drift.
    #[test]
    fn unrecognized_claude_and_codex_screens_fall_back_to_change_counting() {
        for kind in [AgentKind::Claude, AgentKind::Codex] {
            assert_eq!(
                read(kind, "some shell output\n$ ", "sh"),
                generic_reading(counts(9, 0)),
                "{kind:?}"
            );
        }
    }

    /// Codex's title alone decides working while prose streams, and a
    /// waiting title wins over a working widget.
    ///
    /// Why it matters: Codex draws no on-screen working indicator while it
    /// streams a reply, so without the title a streaming turn reads idle;
    /// and a question queued mid-turn is the interaction the user is asked
    /// for, even while a tool still runs.
    #[test]
    fn codex_title_decides_streaming_work_and_waiting_wins() {
        let composer = "• Running the requested command.\n› Ask Codex to do anything\n  footer\n  ? for shortcuts";
        assert_eq!(
            read(AgentKind::Codex, composer, "⠏ ⠏ | codex").state,
            ScreenState::Working
        );
        assert_eq!(
            read(AgentKind::Codex, composer, "codex").state,
            ScreenState::Idle
        );
        let queued = "• Working (2s • esc to interrupt)\n• Queued follow-up inputs\n  ? 1 question\n    shift+← to answer\n› Ask Codex to do anything\n  footer";
        assert_eq!(
            read(AgentKind::Codex, queued, "⠙ task | codex").state,
            ScreenState::Waiting
        );
        assert_eq!(
            read(
                AgentKind::Codex,
                composer,
                "[ ! ] Action Required | task | codex"
            )
            .state,
            ScreenState::Waiting
        );
    }

    /// Codex's composer is recognized by where it sits, whatever the user
    /// typed into it, and a transcript echo of a submitted prompt is not it.
    ///
    /// Why it matters: an unrecognized composer falls back to change
    /// counting, and then every edit of an unsent draft and every sparkle
    /// frame reads as work — the idle noise this reader removes.
    #[test]
    fn codex_composer_is_recognized_by_layout_not_content() {
        assert!(codex_has_composer(&[
            "› Ask Codex to do anything",
            "  footer"
        ]));
        assert!(codex_has_composer(&[
            "› 1. Fix the bug",
            "  2. Then the docs",
            "  footer"
        ]));
        assert!(codex_has_composer(&["⠁", "› ⠂draft", "⠄", "  footer"]));
        assert!(codex_has_composer(&[
            "› a long draft that wraps",
            "  onto a second row",
            "  and a third row",
            "  and a fourth row",
            "  and a fifth row",
            "  and a sixth row",
            "  status line",
            "  ? for shortcuts",
        ]));
        assert!(
            !codex_has_composer(&["› Run this command", "• Running it now.", "  └ done"]),
            "a transcript echo is followed by the agent's reply"
        );
        assert!(!codex_has_composer(&["  footer only"]));
    }

    /// A Codex menu with its last option highlighted reads waiting even when
    /// the option above it wraps onto a continuation row, and with no title
    /// to go on.
    ///
    /// Why it matters: Codex is a first-class harness (SPEC.md "First-class
    /// harnesses"), and the permission dialog's second option is long enough
    /// to wrap in an 80-column pane. With the highlight on the last option,
    /// the row right above it is then that continuation, and a check of only
    /// that row read the dialog as an idle numbered draft. The title usually
    /// says "Action Required" too, but a reader must not depend on it: some
    /// hosts cannot read titles.
    #[test]
    fn codex_last_option_under_a_wrapped_option_reads_waiting() {
        let menu = concat!(
            "  $ sleep 20 && echo done > marker.txt\n",
            "\n",
            "  1. Yes, proceed (y)\n",
            "  2. Yes, and don't ask again for commands that start with\n",
            "     `sleep 20 && echo done` (p)\n",
            "› 3. No, and tell Codex what to do differently (esc)\n",
            "\n",
            "  Press enter to confirm or esc to cancel",
        );
        assert_eq!(read(AgentKind::Codex, menu, "").state, ScreenState::Waiting);
    }

    /// Text that merely mentions a dialog's words is not a dialog: a draft
    /// in either agent's input, and a conversation topic in Codex's title.
    ///
    /// Why it matters: a false waiting reading moves the session up the
    /// list, dates it, and can mark it unread — the exact noise the readers
    /// exist to remove.
    #[test]
    fn dialog_words_in_a_draft_or_topic_are_not_a_dialog() {
        let codex_draft =
            "output\n› Explain the Esc to cancel shortcut\n  status line\n  ? for shortcuts";
        for draft in [
            "output\n› Explain the shortcut\n  Esc to cancel\n  status line",
            "output\n› 1. Explain the shortcut\n  Esc to cancel\n  status line",
            "output\n› 1. Explain\n  2. the shortcut\n  Esc to cancel\n  ? for shortcuts",
            // A numbered list ending the transcript sits above the composer's
            // blank padding row, never directly above the draft, so it does
            // not make a numbered draft read as a highlighted last option.
            "1. first\n2. second\n\n› 3. Explain the shortcut\n  Esc to cancel\n  status line",
        ] {
            assert_eq!(
                read(AgentKind::Codex, draft, "task | codex").state,
                ScreenState::Idle,
                "a wrapped draft row in the footer position is still the user's text: {draft:?}"
            );
        }
        assert_eq!(
            read(AgentKind::Codex, codex_draft, "task | codex").state,
            ScreenState::Idle
        );
        let topic = "Fix Action Required banner | codex";
        assert_eq!(
            read(AgentKind::Codex, codex_draft, topic).state,
            ScreenState::Idle
        );
        assert_eq!(
            read(AgentKind::Codex, codex_draft, &format!("⠋ {topic}")).state,
            ScreenState::Working
        );
        for prefix in CODEX_ACTION_REQUIRED_TITLES {
            assert_eq!(
                read(
                    AgentKind::Codex,
                    codex_draft,
                    &format!("{prefix} | task | codex")
                )
                .state,
                ScreenState::Waiting,
                "{prefix}"
            );
        }
        let claude_draft =
            "● done\n────────\n❯ what does Esc to cancel do\n────────\n  user@host:~/work";
        assert_eq!(
            read(AgentKind::Claude, claude_draft, "").state,
            ScreenState::Idle
        );
    }
}

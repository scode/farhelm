# A Claude question with its last option highlighted may show as idle

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When Claude asks a multiple-choice question and the user moves the highlight to the last option, Farhelm may show the
session as idle instead of waiting on the user.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F52 / COR-CLAUDE-LAST-OPTION`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:334` — a Claude question with its last option highlighted
reads as Idle.

The supervisor decides whether a Claude Code session is working, waiting for the user, or idle partly by reading the
text on its screen (the "screen reader" in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`). For Claude it
first looks for Claude's input box, which it identifies as the last line starting with `❯` that sits directly under a
horizontal rule of `─` characters (`claude_input_box_rule`, lines 334-339). Only when it finds no input box does it look
at the bottom lines for a dialog footer such as "Esc to cancel", which means Waiting. If it does find a box, it looks
for Claude's spinner above it and otherwise reports Idle.

When Claude asks the user a multiple-choice question, the real captured screen
(`crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-question.txt`) draws the options with `❯`
marking the highlighted one, and the last option, `4. Chat about this`, sits directly under a full-width `─` rule. With
option 1 highlighted the reader works, because no `❯` row is under a rule. If the user arrows down to the last option,
that row becomes `❯ 4. Chat about this`, which matches the input-box shape exactly. The reader then takes the question
for the input box, never checks the "Esc to cancel" footer, finds no spinner above it, and reports Idle as a confident
reading.

So a session that is waiting on the user shows as idle and the user is not prompted to come back; and when the highlight
moves back up, the switch back to "waiting" is recorded as a new burst of work. This is the Claude counterpart of F13,
which is the same last-option problem in Codex's dialog detection. The one unconfirmed premise is that the highlighted
last option renders as `❯ 4.` starting at column 0; that is how option 1 renders in the capture, but no capture shows
option 4 highlighted.

The suggested fix is to require the box's full shape (rule, `❯` row, closing rule; every captured input box has the
closing rule, the question form does not), or to check for the dialog footer before accepting a box candidate, and to
add this highlighted-last-option variant as a fixture.

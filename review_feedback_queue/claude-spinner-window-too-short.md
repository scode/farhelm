# A long Claude task list may make a busy session show as idle

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When Claude shows a task list of more than a few items while working, Farhelm may show the session as idle for the rest
of the turn and record a spurious new start of work afterwards.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F53 / COR-CLAUDE-SPINNER-WINDOW`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:294` — the Claude spinner search window is a fixed 6 lines,
so a task list under the spinner reads as Idle mid-turn.

Once the supervisor's Claude screen reader (see F52) has found Claude's input box, it decides Working versus Idle by
looking for Claude's spinner line (for example `✢ Imagining… (7s · ↓ 262 tokens)`) above the box. It only looks a fixed
six non-blank lines up (`CLAUDE_SPINNER_LINES_ABOVE_BOX`, `screen_reader.rs:294`, used at `:320-324`); blank lines are
dropped before counting. The constant's comment says six covers every observed arrangement of notices between spinner
and box.

The real fixture `working-tool-2.txt` already has one warning line between the spinner and the box, which leaves room
for only four more. If five or more extra rows sit in between, the spinner falls outside the window and the reader
reports Idle as a confident reading for the rest of the turn. Claude Code draws its todo/task list (and queued user
messages) in exactly that space, below the spinner and above the input box, and a task list easily exceeds five rows.

When that happens a busy session shows idle, its activity time stops updating, and when the list shrinks and the spinner
re-enters the window the supervisor records a spurious new start of work. The open premise is the layout itself: it
matches how Claude Code draws task lists, but none of the captured screens in the repository shows one.

The suggested fix is either to skip task-list rows (lines with `⎿`, `☐`, `☒`, `✔`) when counting, or to search upward
until the first transcript line instead of a fixed count, and to capture a working screen with a task list as a fixture.

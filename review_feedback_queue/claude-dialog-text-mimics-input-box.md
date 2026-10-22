# Agent-written text inside a Claude question dialog can pass for the input box

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

NOTE: Dormant in this build. The only feature that would act on the mistake, agents sending prompts into other sessions,
is switched off.

When text the agent itself writes into a Claude Code question dialog contains a line starting with a horizontal rule
followed by a line starting with Claude's prompt arrow, Farhelm reads the dialog as an idle (or working) agent instead
of one waiting on the user. Today that only shows the wrong status. If agent prompts are turned back on, it becomes a
way for a delivered prompt to answer a question the user never saw, and it would not be fixed by the timing fix the
re-enable checklist names.

## Details

Found by the 2026-10-22 prioritized persistent review, group `207-screen-reader` (finding f1).

Reader defect: `ClaudeReader::read` (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:491-512`) consults the
dialog footer only when `claude_input_box_rule` (`screen_reader.rs:532-541`) finds no box. That function accepts any
column-0 `─` row directly above a column-0 `❯` row with any `─` row somewhere below. Claude's question dialog prints the
agent's question text at column 0 and already has a `─` separator below its options
(`crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.296/waiting-question.txt`), so a question whose text
contains those two rows reads Idle, or Working if a spinner-shaped line sits above it.

Reachability: `Supervisor::prompt_session` (`crates/farhelm-supervisor/src/service/core.rs:10818-10832`) is the consumer
that would paste a prompt plus Enter into the misread dialog. Its only production sender is the helm's agent-prompt
path, which is refused up front while `AGENT_PROMPTS_ENABLED` is `false`
(`crates/farhelm-helm/src/agent_requests.rs:315`, doc at 297-314). Same-account processes that could talk to the
supervisor directly are trusted by SPEC.md "Local authority and trust between hosts". So no untrusted party reaches the
dialog-answering consequence in this build. The live consequence is a wrong status while the dialog is up (and possibly
a missing waiting alert). Restart is unaffected in the direction that matters: waiting and idle agents both restart
without asking.

Why keep it: the re-enable checklist (TODO.md "Near term", "Re-assess letting agents prompt running sessions", around
171-179, and the `AGENT_PROMPTS_ENABLED` doc comment) names only the two-second staleness gap. This is a second,
independent way for a delivered message to answer a question the user never saw, triggered by agent-controlled dialog
content rather than timing.

How to verify: a reader unit test built from `waiting-question.txt` with the question text replaced by two rows, the
first starting with `─` and the second with `❯`; the reading comes out Idle instead of Waiting.

Fix sketch: do not accept an input-box candidate when the last rows carry Claude's dialog footer (`esc to cancel`
together with `enter to select`, `tab to amend` or `enter to confirm`), or require the box's closing rule to be followed
only by status-area rows.

Suggested bucket: other in this build (dormant); highest once agent prompts are re-enabled. Triage may prefer to fold
this into the TODO.md "Near term" item "Re-assess letting agents prompt running sessions" instead of keeping a queue
file.

Confidence: the misreading is likely, with the premise that Claude prints multi-line question text row by row at column
0 (the captured single-line question is at column 0). Dormancy is confirmed from `agent_requests.rs:315`.

Reviewer's proposed drop, kept for triage: dormant path. With `AGENT_PROMPTS_ENABLED = false` every agent prompt is
refused, and the remaining Restart effect is accepted by SPEC.md "Status" (around 1699-1700): "a wrong reading costs at
most a skipped or an extra confirmation".

Possible cover, for triage to judge: TODO.md "Near term", "Re-assess letting agents prompt running sessions" (not
Planned, and names a different trigger); SPEC.md "Status" prompt-delivery exception (around 1697-1698: "A mistaken
reading may therefore refuse that request or let it reach an unrecognized dialog"), which accepts an unrecognized dialog
rather than a recognized dialog misread through agent-controlled text; TRIAGE_OUTCOMES.md
`claude-last-option-reads-idle.md` (a related box-shape fix with a different trigger: a highlighted option under the
dialog's own rule, not agent-written text). SPEC.md "First-class harnesses" makes a clear, definite gap in Claude's
activity reading a defect; whether a screen shaped by unusual agent text counts as ordinary use is for triage.

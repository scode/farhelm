# A child Pi that carries Farhelm's reporter can replace a Pi session's Resume target

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

For Pi sessions, Farhelm accepts the conversation reported by any Pi inside the session that carries Farhelm's reporter,
not just the user's own Pi. If a Pi extension starts a child Pi that inherits Farhelm's reporter, the child's
conversation can become the session's Restart target (or withdraw the real one), and Restart then resumes the child's
conversation. This needs a third-party extension that passes Farhelm's reporter on to its child, so it is very unlikely.

## Details

Found by the 2026-10-22 prioritized persistent review, group `206-harness-non-first-class-2of2` (finding f4). Proposed
for drop at recording; the independent check disagreed and asked for it to be recorded.

This is the Pi counterpart of `nested-goose-report-replaces-resume-target.md`, which records the same missing ownership
check for Goose and cites the same SPEC.md sentence as a possible cover. Triage will probably want to decide the two
together.

Mechanism: the Pi reporter (`crates/farhelm-supervisor/assets/pi-conversation-v1.ts:11-59`) reports every
`session_start` and `agent_end` with no interactive-context gate. OMP's asset needed exactly that gate because OMP loads
extensions into delegated contexts (`omp-conversation-v1.ts:4-17, 54-60`). On the supervisor side, Pi reports go through
`report_conversation_legacy` (`crates/farhelm-supervisor/src/service/core.rs:15429-15510`), which runs no corridor for
Pi (`AgentKind::Pi => {}` in the arm at `core.rs:15458-15464`). Only the drain's pane anchoring applies, so any process
in the pane's tree can report. A Pi the user or agent starts directly does not load the reporter, because Farhelm
injects it only through the per-launch `-e` (`crates/farhelm-supervisor/src/pi_extension.rs:22-24`). The exposure is a
child Pi started with Farhelm's `-e <state>/integrations/pi/farhelm-conversation-v3.ts` passed on, for example by a
third-party sub-agent extension that copies the parent's extension arguments. The child inherits
`FARHELM_PI_REPORTER_EXE` and reports its own conversation, which is accepted with no ordering check (a report without a
file withdraws the previous offer, per the asset's own doc comment). Pi's pre-resume check only confirms that the file's
header matches the reported id, so the child's real session file passes.

Why the independent check did not accept the drop: SPEC.md "Durability and resume" (around 2278-2279: "Goose and Pi
retain their existing admission and resume rules; for them the discriminator alone adds no foreground protection") and
SPEC_impl.md "Interim ownership states" (around 2930: "Goose and Pi retain their existing acceptance behind the
discriminator gate") describe a missing protection; they do not accept its consequence. SPEC.md (around 2172-2175)
requires that "restart resumes exactly that conversation" and lists Pi among the integrations required at that level.

Uncovered remainder: a child Pi that loads Farhelm's `-e` reporter can replace or withdraw the session's Resume target,
and Restart then resumes the child's conversation.

How to verify: in a Pi session started by Farhelm, start a second Pi from inside it with the same `-e` argument and the
inherited environment, begin a conversation there, then check the session's captured conversation or Restart; it names
the child's conversation.

Fix sketch (the reviewer's): give Pi a foreground corridor like OMP's (the reporter's ancestry must show the one Pi
runtime at or directly under the pane), or have the asset ignore non-UI contexts if Pi exposes one.

Suggested bucket: highest by consequence (resuming the wrong conversation), with negligible likelihood: Pi has no
built-in sub-agents, and the trigger needs a third-party extension that forwards Farhelm's `-e`.

Confidence: likely. The missing check is confirmed in code. Unverified premise: that some Pi extension or workflow
starts a child Pi that also loads Farhelm's reporter.

Possible cover, for triage to judge: SPEC.md "Durability and resume" (the "Goose and Pi retain ..." sentence above);
SPEC_impl.md "Interim ownership states"; SPEC.md "First-class harnesses" ("a resume they accept is not the lost user
work this allowance excludes"). TODO.md "Maybe later", "Keep a nested Grok from erasing the session's conversation
switch", says "Claude, Codex, Pi and OMP are not affected: Farhelm adds their reporter to the one launch it builds, so a
copy of them started inside a session reports nothing"; that is not a coverage source, and this finding is the case
where a copy does carry the reporter. FILTER.md "Rare edge cases in harnesses without first-class support" does not
match, because it excludes resuming the wrong conversation.

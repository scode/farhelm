# Codex agent output shaped like a queued-question hint reads as waiting while the agent works

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

If a working Codex agent's recent output happens to contain a line starting with `? ` that mentions "question", followed
by a line containing "to answer", Farhelm shows the session as waiting on the user even while the agent is busy. A
Restart then stops the agent without asking, cutting off the running turn, and the user may get a spurious "waiting"
notification.

## Details

Found by the 2026-10-22 prioritized persistent review, group `207-screen-reader` (finding f3). Proposed for drop at
recording; the independent check disagreed and asked for it to be recorded. Sibling item for Claude's reader:
`claude-dialog-text-mimics-input-box.md`.

Mechanism: `CodexReader::read` (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:651-689`) checks every
waiting cue, including `codex_queued_question` (`screen_reader.rs:691-712`), before the working title spinner and the
Working widget (`screen_reader.rs:677-686`), so a waiting match overrides definite evidence of work.
`codex_queued_question` accepts any pair of rows within the last content rows above the last `›` line where the first,
after `trim_start` (`screen_reader.rs:708`), starts with `? ` and contains ` question`, and the next contains
`to answer`. Those rows are transcript output, which the agent controls through its prose or a tool's printed output. An
example would be an agent summarizing a review: `? 2 open questions` followed by `Reply to answer them later`.

Consequence: SPEC.md "Lifecycle operations" (around 1178-1184) restarts a waiting agent without asking, and the
supervisor applies the same rule from its own reading, so a busy agent is stopped unconfirmed. There is also a spurious
waiting alert after the usual delay.

Why the independent check did not accept the drop: SPEC.md "Status" (around 1699-1700) bounds the cost of a wrong
reading ("a wrong reading costs at most a skipped or an extra confirmation"), and SPEC.md around 1181-1182 accepts that
"Harnesses with weak activity detection may therefore restart a busy agent unasked", but that is about weak detectors,
not Codex. SPEC.md "First-class harnesses" (around 3441-3442): "Claude Code and Codex are the first-class harnesses. A
clear, definite gap in how Farhelm reads their activity ... is a defect and gets fixed." The uncovered remainder is that
in a first-class reader, agent-controlled transcript text overrides definite working evidence.

How to verify: a reader unit test with a Braille spinner title (or the Working widget) and transcript rows
`? 2 open questions` / `Reply to answer them later` above the composer; the reading comes out Waiting instead of
Working.

Fix sketch (the reviewer's): let definite working evidence (the title spinner) win over the queued-question cue, or
anchor the cue to Codex's own rendering (its exact `? N question(s)` glyph, indent and hint wording from a capture)
rather than to substrings.

Suggested bucket: other (the reviewer said high at most).

Confidence: likely for the code path (the ordering and substring match are confirmed). Unverified: that Codex renders
agent prose rows at that position without a prefix that defeats `starts_with("? ")` after `trim_start`. Codex prefixes
reply paragraphs with `• ` and indents continuation rows, so the trigger needs a continuation row or a tool-output row
that begins with `? `.

Possible cover, for triage to judge: SPEC.md "Status" and "Lifecycle operations" (quoted above); TRIAGE_OUTCOMES.md
`codex-last-option-reads-idle.md` and `claude-spinner-window-too-short.md`, whose principle is that only clear, definite
gaps in Claude Code or Codex activity detection are fixed. Whether a screen shaped by unusual agent text is a clear,
definite gap is for triage.

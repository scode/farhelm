# Claude scan fallback can claim a conversation that is not the session's own

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

Some Claude sessions get their conversation found by looking at Claude's saved records, because the agent could not
report it itself. Such a session can be tied to the wrong conversation. This happens when Claude also runs in the same
project directory outside that session: a `claude` in another terminal, `claude -p` from a script or a Codex session, or
a Farhelm Claude session that never took typed input. If the session's own first prompt arrives more than a minute after
its first keystroke, Farhelm records the other conversation as this session's. Restart then offers Resume, silently
opens the other conversation, and the user's new messages are appended to it.

## Details

Source: whole-codebase review, 2026-09-30, slice sup-tmux.

Reviewer's confidence: likely (traced end to end in code; not verified is how often in practice a foreign Claude record
lands in the session's 65 s window while the session's own record lands after it).

Reviewer's bucket suggestion: high (arguably highest: Resume opens another process's or person's conversation and
appends to it; treat it as highest if in doubt).

Possible cover for triage to check: TODO.md `## Maybe later`, "Consider dropping conversation-identity SCAN support and
keeping only the per-launch hook". That is a maybe, not a `Planned` item, and it does not name this flaw.

Additional caveats from the helper agent that traced this (restored by a later audit): a further trigger source is a
Farhelm Claude session deleted or fresh-restarted right after its first prompt, whose record then has no window in the
rival group; and the early-anchor trigger rests on an unverified premise, that Claude's startup terminal queries
actually get answered as the session's first input.

**Who is exposed.** Claude sessions with no accepted hook report fall back to the record scan (SPEC.md "Durability and
resume": "Claude retains scanning as its fallback when no report has been accepted"). That covers:

- argv that already passes `--settings` (`agent_kind/claude.rs:16`)
- a bare `--`
- the `FARHELM_AGENT_HOOKS` opt-out
- wrapper chains deeper than one level (SPEC.md ~1066)
- a hook that failed to report

**How the scan decides.**

- The window runs from 5 s before to 60 s after the first input byte tmux confirmed it delivered
  (`service/capture.rs:635`, `agent_kind/capture.rs:56`, `:86`).
- Candidates are the Claude records whose `cwd` field matches, minus ids another session in the group reported
  (`service/capture.rs:1258-1265`).
- `choose` treats a lone in-window candidate as a match (`agent_kind/capture.rs:521-528`).
- At the horizon, a complete scan commits that match (`service/capture.rs:1269-1303`).
- The only rival protection is the window-overlap check. It is built only from Farhelm entries of the same kind that
  have a first-input time (`service/capture.rs:1093-1111`). A record written by `claude` in another terminal, by a
  script's or a Codex session's `claude -p`, or by a Farhelm Claude session with no input never makes the result
  ambiguous by itself.

**Trigger.** The session's own record must fall outside its window while exactly one foreign record falls inside. Two
ordinary ways get there:

- The first prompt is submitted more than 60 s after the first keystroke. The code's own "too short" docs list
  dismissing a trust dialog, editing a long prompt, or pasting; it can also be input with no prompt ever submitted.
- The anchor starts early because the first "input" is the terminal's automatic reply to Claude's startup DSR/DA query
  (documented at `agent_kind/capture.rs:76-85`).

The foreign record is then committed durably, the session offers Resume, and restart runs
`claude … --resume <foreign-id>`.

**The documented acceptance is wrong.** `agent_kind/capture.rs:76-85` says "an early anchor can only ever exclude
candidates, never admit a record belonging to someone else". But the window has a fixed length, so an early anchor
shifts it rather than shrinking it. The shift drops the session's own later record and admits a foreign one. The "too
short" note at `:62-65` makes the same assumption, that a late record only costs an honest fresh-launch offer.
SPEC_impl.md ~1476 already withdrew this fallback for Codex for the same reason ("even a single matching rollout may
belong to a nested invocation"), and SPEC.md promises "restart resumes exactly that conversation".

**Coverage.**

- Not accepted by SPEC.md or SPEC_impl.md, which allow a claim only when correlation is unambiguous.
- Not in TODO `Planned`, the queue, or TRIAGE_OUTCOMES.md (grepped).
- `claude-scan-budget-never-settles.md` is a different issue (the scan budget).
- No FILTER.md filter applies; they all exclude resuming the wrong conversation.

**How to verify.** Use an unhooked Claude session, e.g. from a profile that already passes `--settings`.

1. Type one character at time T.
2. At T+10 s, run `claude -p hi` in the same directory.
3. Submit the real first prompt at T+90 s.

After T+70 s the `-p` conversation is committed and Resume opens it. The `plant_claude_record` fixtures in
`service/capture.rs` can reproduce this without a real Claude.

**Fix shape**, any one of:

- Drop the Claude scan fallback, as was done for Codex.
- Require stronger evidence before a scan claim, e.g. that the record's first prompt was typed into this pane.
- At minimum, refuse to commit when a record lands just outside the window while the session has none of its own.

Also correct the "never a wrong one" docs.

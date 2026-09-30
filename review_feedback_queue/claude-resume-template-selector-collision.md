# Claude's derived resume command collides with a `--continue`, `--resume` or `--` already in the launch

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

When a Claude session was started from a command line that already contains `--continue`, `--resume <id>` or a bare `--`
(for example a profile `claude --continue`), Farhelm's Restart builds the resume command by copying that command line
and adding its own `--resume <captured-id>`. The two selectors then collide. Depending on which one Claude honors,
Resume may open the folder's most recent conversation (possibly another session's) instead of the one Farhelm captured,
or, with `--`, put the resume flag where Claude reads it as prompt text and start a fresh conversation. Either way the
valid Resume offer can be lost or replaced.

## Details

Source: whole-codebase review, 2026-09-30. A helper of the supervisor tmux/capture slice dropped a broader candidate
("default resume templates replay original launch arguments") as covered by the spec and a TODO. Because it was
potentially highest, an independent fresh-context checker reviewed the drop and found it only partly covered; this item
is the uncovered part.

Reviewer's confidence: likely. Unverified premise: which of two conflicting selectors Claude honors (`--continue` plus a
later `--resume <id>`, or two `--resume` flags). The `--` case does not depend on that premise.

Possible cover for triage to check: SPEC.md:1139-1145 accepts that "every original argument is reusable and that the
launch has no initial prompt or launch-only option", and TODO.md:469-477 (under `## Maybe later`, not `## Planned`)
defers separating common, launch and resume arguments and says "existing resume/continue selectors or an end-of-options
marker must not collide with the generated resume command". The spec accepts replaying initial prompts and launch-only
options, which is why that part of the original candidate was dropped; it does not clearly accept a selector or `--`
collision, and it refuses the identical shape for OMP and Grok.

- **Code.** The Claude template at `crates/farhelm-supervisor/src/agent_kind/mod.rs:1451-1455` is
  `original_argv.to_vec()` with the resume arguments appended; nothing is stripped and nothing is refused. The Codex
  template at `mod.rs:1557-1561` has the same shape.
- **Contrast.** OMP strips its own session selectors, and a genuine `--` "refuses the CREATE when the derived OMP resume
  template would be appended behind it (the appended `--resume` would land in prompt position)" (SPEC.md:1113-1115,
  SPEC_impl.md:1686-1691). Grok refuses to derive a template when the retained argv already has a session selector or a
  real `--` (SPEC.md:1141-1142). Pi and Goose strip their own selectors (`strip_selectors`, `mod.rs:1396-1436`).
- **Claude selector collision.** Trigger: a Claude session created without an explicit resume template whose original
  invocation contains `--continue`/`-c` or `--resume`/`-r <id>`. The derived resume command becomes
  `claude … --continue … --resume <captured-id>` or `claude --resume <old> … --resume <captured-id>`. If `--continue` or
  the earlier `--resume` wins, Resume opens a different conversation than the captured one, and a new capture then
  replaces the valid offer.
- **Claude `--` collision.** Trigger: a Claude invocation containing a bare `--`. Hooks are skipped for it
  (SPEC_impl.md:1637), but "Claude's scan remains the fallback when no report has been accepted" (SPEC_impl.md:1639), so
  an identity can still be captured and a Resume offer can exist. The derived `claude -- <prompt> --resume <id>` puts
  `--resume <id>` in prompt position: Resume starts a fresh conversation instead of resuming, or the launch fails, and a
  fresh capture replaces the valid offer.
- **Codex.** With a selector already present (`codex resume X` giving `codex resume X resume <id>`), check the same
  collision at triage. A Codex `--` launch appears to capture nothing (Codex requires attributed reports and gets no
  hooks), so that sub-case is likely unreachable.
- **Pi and Goose `--`.** Their reporters are never attached to a launch containing `--` (`pi.rs:70`, `goose.rs:109`) and
  they never scan, so such a session never offers Resume; likely unreachable, worth confirming at triage.
- FILTER.md counts "losing or replacing a valid Resume offer" and "resuming the wrong conversation" as loss of
  user-owned work, so no filter applies. TRIAGE_OUTCOMES.md and the queue have no decision or item for this.

Verify: create a Claude session from `claude --continue` (or with a `--` and a prompt) without an explicit resume
template, let the scan or hook capture its conversation, and inspect the derived resume argv in the session snapshot;
then restart and observe which conversation opens.

Fix shape: treat Claude (and Codex) like OMP and Grok. Strip the vendor's own session selectors when deriving the
template, and refuse the create (or refuse to derive a template) when a genuine `--` would put the appended resume flag
in prompt position. Record the rule in SPEC.md next to the OMP and Grok sentences.

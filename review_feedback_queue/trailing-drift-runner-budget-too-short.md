# The drift runner's single 720-second per-case budget fits neither the conversation rotation category nor the Codex base case's added probe

Reviewed commit: 197fcf86ad658224b2de09ebf5fe4a3a1cc6ae55

## TLDR

When the maintainer runs the conversation rotation drift check on a slow vendor day, the runner can kill a Pi, OMP,
Codex or Muse case partway through and report it as a failed timeout without the drift verdict or the step-specific
diagnostic the test would have produced, even though every individual step was still within its own limit. The
maintainer then spends another round of real vendor turns re-running it, or misreads a slow day as vendor drift.

## Details

scripts/run-harness-drift.py:129-131 runs every case under `--timeout 720`, with a comment that sizes it for "a base
case's two turns and both composer startups, or a model selection case's launches one after another". The base worst
case under the test's own limits (crates/farhelm/tests/e2e/real_agent_capture/drift/mod.rs:30-31, START_LIMIT 90 s,
TURN_LIMIT 180 s) is about 2x180 + 2x90 = 540 s, inside the budget. The conversation_rotation category added by 197fcf86
runs every command of a harness in one case (conversation_rotation.rs rotate_session, around lines 1290-1374): a seed
turn, then per command a rotation (COMMAND_LIMIT 60 s, conversation_rotation.rs:42), a seed turn or capture wait, Stop,
Restart with a fresh composer startup and a recall turn. docs/harness-drift-tests.md (the conversation_rotation
paragraph) counts six turns and four startups for Pi and OMP, five turns for Codex and Muse. The sum of the per-step
limits is well past 720 s (six turns alone are 1080 s), so the recorder's timeout, not the test's own deadlines, decides
any slow run; the runner's read_verdict then reports "timed out" with no drift verdict (see test-run-harness-drift.py
test_missing_verdict_keeps_the_recorders_known_failure_reason). Neither commit touched run-harness-drift.py or its
budget comment. Verify by reading the three limits against the budget; a live slow run is not needed. A fix would size
the budget per category (for example scale it by the declared turn count) or document why 720 s still holds, and update
the comment.

### Triage notes

- Recorded bucket: `other`. Two routes (the new rotation category and the Codex base case's added process-tree probe) to
  one cause: the drift runner's fixed 720 s per-case budget.
- Suggested bucket (reviewer): other. Opt-in, manually run test suite; the cost is a spurious failure and wasted vendor
  turns, not product behavior.
- Confidence: likely. Unverified premise: that real Pi/OMP/Codex/Muse rotation runs come close enough to 720 s for this
  to fire in practice; typical turns with cheap models may finish well inside it. The mismatch between the per-step
  limits and the budget is confirmed from source.
- possible_cover: none
- proposed_drop: none

### Related reviewer records

#### g34/f1 (finding): Codex base drift case's added process-tree probe pushes its worst-case waits past the runner's 720 s per-case budget

Location: scripts/run-harness-drift.py:131

When someone runs the real-agent drift tests, a slow Codex run can now be killed by the runner's outer timeout before
the test's own limits fire. The case is then reported as FAILED with "recorder timed out before the test wrote a
verdict" rather than with which step stalled and the screen diagnostics. Before this commit the base case's inner limits
fit inside the budget.

scripts/run-harness-drift.py:131-133 gives each case `--timeout 720`. Its comment sizes that budget for "a base case's
two turns and both composer startups ... plus private fixture cleanup". The base case's inner limits
(drift/mod.rs:30-31) are START_LIMIT 90 s and TURN_LIMIT 180 s, plus a 30 s attach timeout (mod.rs:520). Before
d196d8e75f a hooked base case's worst case was about 2x(30+90+180) = 600 s, inside 720. d196d8e75f makes `execute`
(drift/base.rs, `execute`) run `command_tree_probe` after `base_session` for Codex, in a second supervisor. That probe
waits up to another 30 s to attach, 90 s in `ready`, 20 s in `run_in_shell_mode` (base.rs:263), 20 s in `type_text`
(mod.rs:664) and 180 s in `wait_for_chain` (base.rs:290). That adds up to about 340 s, so the Codex worst case is about
940 s, before supervisor startup and teardown. The comment and the budget were left as they were. The inner limits exist
so that a stall ends with a specific error and `terminal.diagnostics()`. A runner timeout discards that
(run-harness-drift.py:110-111) and reports FAILED even for a run that was merely slow. To verify, sum the inner
deadlines along `execute` for Codex and compare them with 720. A fix is to give Codex's base case a larger budget, or a
budget per category or case, or to shorten the probe's chain wait (the `!` command needs no model turn, so TURN_LIMIT is
generous), and to update the comment.

Suggested bucket: other. This is test tooling with no product effect, and it only matters for slow runs near several
limits at once.

Confidence: likely. The arithmetic is confirmed from source. Unverified premise: that a real Codex run gets slow enough
for the sum to cross 720 s (each wait usually ends far below its limit).

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g35/f1 (finding), group g35 "Guarded permission footers in the drift tests; rotation drift wiring", findings log
  line 1.
- g34/f1 (finding), group g34 "Real-agent drift tests: base turns, Codex process tree, private config", findings log
  line 5.

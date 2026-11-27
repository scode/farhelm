# No test pins that a Claude start command failing its checks still lets Restart resume

Reviewed commit: 4f0692f0387d6d35039f2e2125d2924569953d14

## TLDR

The fresh-start feature promises it can only ever add a fresh start where resuming would have failed, never make a
working Restart fail. One way it could break that promise, a stored start command that this build's checks refuse, has
no test, so a refactor could turn such a Restart into a refusal and nothing would notice.

## Details

SPEC_impl.md, "Restart when Claude saved nothing", says: "A start command that fails those checks only loses the fresh
option: the Restart resumes unchecked, with a warning in the supervisor log, as it did before the check existed." The
implementation is the match at crates/farhelm-supervisor/src/service/core.rs:12703-12717: an `Err` from
`fresh_restart_argv` (core.rs:9125) is logged and becomes `None`. The new test module
crates/farhelm-supervisor/src/service/core/tests/claude_restart.rs covers missing, saved, not-a-file,
saved-while-stopping, bare-id, Restart with and named-restart cases, but every fixture's start command is valid. A grep
across crates/ for `fresh_restart_argv`, `loses the fresh option` and the warning text turns up only the implementation.
So replacing the match with `self.fresh_restart_argv(executable_launch)?` (an easy simplification to make) would refuse
a Restart of a Claude session whose saved conversation resumed before this change, and every test would still pass. The
`conversation_saved` contract has a smaller gap of the same kind: its doc
(crates/farhelm-supervisor/src/agent_kind/mod.rs:2393-2405) and the SPEC_impl text say a symlink or FIFO where the
transcript should be also resumes, but only the directory case is tested
(`a_transcript_that_is_not_a_file_still_resumes`). Swapping `records::open_regular_file` for a call that follows
symlinks (e.g. `tokio::fs::try_exists`) would make a dangling symlink start fresh, and no test would fail. Fix: add a
`restart_claude` case whose stored launch has a start command the pre-stop checks refuse (inserted directly into the
store, e.g. a command launch whose start text fails `validate_command_placeholders`, or one whose start argv fails
`ensure_executable_argv`) with the transcript missing, and assert that the Restart succeeds and runs the resume command.
Optionally add a dangling-symlink `Transcript` variant.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other. This is a test-coverage gap for a safety property, and the current code behaves as
  specified. Realistic triggers (an old row whose start text this build refuses) are rare.
- Confidence: confirmed (the absence of a covering test, by grep and by reading the new module in full).
- possible_cover: none
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g17/f1 (finding), group g17 "Claude restart fresh: tests; Codex fork and app-server conversations", findings log
  line 1.

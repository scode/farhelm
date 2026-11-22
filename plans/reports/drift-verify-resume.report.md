# Verify that agent conversations survive Restart

## What this was about

An agent could open an empty conversation after Restart and still pass Farhelm's live drift test by accepting input. The
maintainer asked for evidence that the agent remembered the earlier conversation and that Farhelm could still resume it.

The test for each of the eight tracked agents now gives the agent a random test label, stops and restarts the session,
and asks the agent to recall the label. It also requires a fresh conversation acknowledgment after Restart and checks
that the saved resume command still names the original conversation.

## Things you should know

The test counts acknowledged reports only after the first run has stopped and its queued reports have settled. For
agents whose hooks can acknowledge a report without storing its conversation, the test also checks that the fresh
acknowledgment names the identity that Farhelm holds. The manual runner allows up to 12 minutes per agent for both
turns, startup, and cleanup.

The final wording calls the random word a test label and asks for a text-only reply. In earlier Codex trials, the agent
first asked for command approval and later declined to repeat the code word even though it showed the earlier turn and
reported the original conversation. Codex and OMP both passed with the final wording.

## Open questions and possible follow-ups

Run Claude Code in a trusted test workspace to verify both turns and the conversation report after Restart.

## PRs

- Check conversation recall and fresh hook evidence after Restart.
  [#4628](https://github.com/scode/farhelm/pull/4628/changes)

## Checks

- The four deterministic Restart verification tests passed under recorded pinned nextest, run
  `11dfba7d-b400-4e28-b488-c403109e74f0`. They cover both a successful recall and the conditions that must fail the
  check, including an unchanged resume command paired with an acknowledgment for a different conversation.
- Codex and OMP passed both turns and the conversation checks with the final wording, runs
  `2bdd4636-2aa2-4fc9-9723-e86092d71932` and `a2d75976-f04f-4532-9ad2-c1e15bac427f`. Claude Code 2.1.330 stopped at its
  workspace trust screen before either turn, run `e1108d37-9796-44c4-85b0-aab30e1b96dd`. The other five tracked agents
  were outside the live selection. Retained the earlier Codex failures `c9f54522-1d06-4a2a-855f-82991ada4b67` and
  `45a3b3b0-b6df-4905-a1dd-c2b9bcce452c`. OMP also passed the prior wording in run
  `ccf2dc7c-c1d8-4671-93b7-11f5e5c71c08`.
- Ran Rust formatting and diff-whitespace checks on the draft. After `34fe5baf` the only source change was a local
  binding rename that avoids reindenting the launch profiles, so the deterministic and live evidence still applies.
  Reused Clippy from `2ed8ac7f` because the other Rust edits change only prompt text, its fixture, and its comment.
  Reused nine passing Python runner stand-in tests from `4bcc3dc0`, run `9562f616-237e-4481-af4a-6dac3216f98f`. The
  runner and stand-in fixture have the same contents in the draft.
- Reused the isolated test-delay check and the Markdown formatting and instruction-reference checks on `4bcc3dc0`. The
  final edits add no delays and change no Markdown. The instruction check reported five existing reference warnings and
  zero errors.
- Skipped the broader Rust and live batteries. The deterministic tests cover the shared verifier and the two live passes
  cover the prompts and reporting in real terminals.

## Review gate

The independent source reviewer found that Restart could acknowledge a queued report from the first turn and that some
new acknowledgments could name a different conversation while Farhelm still offered the old resume command. The reviewer
verified both fixes and checked the final prompt lengths and echo exclusion. The commit wording passed a separate cold
read.

### Landing

#### What landed with it

The plan's PR (#4628) landed third in this round, after the container image and Restart cleanup fixes and before the
screen fixture scrubber. Nothing else in the round or on main since the plan was written touches the drift test or its
runner, and the rebase conflicted only in TODO.md, where each plan removes its own entry.

An independent reviewer found nothing to fix and confirmed the new check cannot pass by accident: the line the agent
must print to show it remembers the test label appears in neither prompt, only a whole line counts (so an echoed prompt
does not), such a line already on screen before the second turn is refused, and a conversation acknowledgment that named
a new conversation would change the saved resume command and fail the test.

#### Fixes made while landing

None.

#### Checks

The round landed changes to the helm's container recipe rebuilds and the supervisor's Restart cleanup, plus two test
tools, so the landing ran the Rust workspace suite, the lint runs and the two tools' own tests, once, on the four plans
stacked together. No desktop code changed (the new Rebuild wording in Settings and the launcher comes from the helm), so
the desktop app's tests, the end-to-end smoke test and the reference screenshots were not run. The worker runs were on a
separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally.

On the four plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `03875ce4-e83a-47da-87cb-b3c4ee02da69`): 8349 of 8350 passed, none printed a runtime `SKIPPED`, and the 86 tests
  marked ignored did not run, as on every run. The real Podman recipe build and launch test ran and passed. The one
  failure was the supervisor test that refuses session titles with control characters, which timed out after ten seconds
  waiting for its own create reply while the suite ran around it. It exercises neither supervisor startup nor Restart,
  the parts of the supervisor the Restart cleanup plan changes, it is not a known flake, and it then passed 30 times in
  a row on the same worker, each attempt a separate recorder run inside one worker session (worker session
  `687080cb-021d-499c-a209-af6f89149001`). One timeout is not enough to record it as a flake.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed. Lints are not runtime tests, so they have no recorder run.
- `python3 -B scripts/test-run-harness-drift.py`, `python3 -B scripts/test-capture-agent-screens.py`,
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`, locally: passed.

Skipped: the real-agent harness drift tests, which spend vendor turns and run only when asked; macOS, since nothing in
the round builds only on a Mac; CI, which runs only on demand.

#### Things you should know that the report does not say

- The report names only a Claude Code run as the follow-up, but more is unproven live. For Claude Code, Antigravity and
  Muse the test now requires the acknowledgment after Restart to name the stored conversation, and none of the three has
  passed that against the real agent; for Goose, Grok and Pi, the plain fresh-acknowledgment check has not run live
  either. Those five were not in the live selection, so a real harness test run that selects them (and gives Claude Code
  a trusted workspace) is what would prove them.
- The new unit tests all exercise the stricter check (the acknowledgment names the stored conversation); none exercises
  the plain fresh-acknowledgment check Goose, Grok, Pi and OMP use. Running the existing unit tests over both kinds
  would close that gap.

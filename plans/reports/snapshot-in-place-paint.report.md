## What this was about

Saving a terminal's screen and history for recovery briefly held two large buffers containing the same output. The
maintainer chose to reduce that peak for ordinary output while preserving the saved format. Some unusual screen states
cannot be recreated by replaying terminal output alone, such as a wide character with only one half erased; those cells
need separate saved corrections. Snapshots dominated by those cells remain the difficult case left for later, while the
ordinary text fixture uses almost entirely replayable output.

The encoder now reuses the output buffer for the final snapshot, shifting its contents in place to add the header.
Existing saved bytes stay compatible with running terminal hosts. The completed Near term TODO is removed, and the
requested Maybe later entry records the remaining cost of those cell corrections.

## Things you should know

On Linux, the ordinary-output test produced the same 3.55 MiB snapshot with peak growth of 3.7 MiB, down from 6.9 MiB.
That is about half the temporary memory in this fixture, not a reduction in the terminal's steady memory use. The new
assertion first failed against the unchanged encoder: 7,270,400 bytes of peak growth for a 3,726,399-byte blob. Its
measurement premise passed, so an earlier memory peak did not hide that growth.

A snapshot made mostly of cell corrections still needs their separate buffer: its 16 MiB blob raised the peak by 32.2
MiB, within the unchanged 36 MiB test allowance. The documented ceiling and size limit remain unchanged. Final snapshots
retain no spare allocation capacity. The changelog fragment uses `kind: none`, because this brief allocation improvement
changes no visible behavior.

## Open questions and possible follow-ups

Reducing the remaining peak for snapshots made mostly of cell corrections needs a separate design, now recorded under
Maybe later in TODO.md. The measurements here are Linux observations; they do not establish the same resident-memory
figures for another operating system or allocator.

## The PRs

- [#3209: lower terminal snapshot peak memory](https://github.com/scode/farhelm/pull/3209/changes) — one draft
  performance PR based on main.

## Checks run, reused and skipped

- Passed `cargo nextest run -p farhelm-terminal-emulator` through the controlled recorder: 124 passed, 20 ignored; run
  `c4935403-9a71-44fa-ba7b-fa39d9487e12`. The Linux run used pinned nextest, four global slots, zero retries and
  recorder tmux mode `none` because no tmux is used. This includes the exact-limit allocation regression and unchanged
  golden-byte, round-trip and fuzz tests. No fixtures were rewritten.
- Passed all seven release memory tests, each in its own process, through generic recording: run
  `d7d89a94-64f5-473e-8db1-a2fddf77afe2`. Child command:
  `cargo nextest run -p farhelm-terminal-emulator --release --test memory --run-ignored only --no-capture --retries 0`.
  The Linux run used pinned nextest, serial execution imposed by no-capture, zero retries and recorder tmux mode `none`.
  Generic recording preserves output and exit status; these seven passes were read from the retained test output, not
  controlled-run metadata.
- Retained the intentional pre-change assertion failure as run `992c8612-e4a4-41a8-96eb-4ba0a7a958c1`. Child command:
  `cargo nextest run -p farhelm-terminal-emulator --release --test memory --run-ignored only -E 'test(=typical_output_at_full_scrollback)' --no-capture --test-threads 1`.
  It ran one test and failed the new peak assertion; it is evidence of the regression test's sensitivity, not a passing
  run. Nextest reported that no-capture already implies serial execution and ignores the redundant thread flag.
- Both passing runtime runs covered base `3831ab172c2bbea1473efa248a9f73ef97846f1e` with tracked-diff SHA256
  `2003ee749a0d108458563528940f675c9f79a5fd86937114dd585b161f73878e` and complete source identity. Subsequent edits
  changed only comments, memory documentation, TODO entries and the changelog fragment, so runtime coverage still
  applies.
- Passed `cargo fmt --all -- --check`, `cargo clippy -p farhelm-terminal-emulator --all-targets -- -D warnings`,
  `dprint check TODO.md releasing/changelog.d/snapshot-in-place-paint.md`,
  `python3 releasing/check-changelog.py format`, and `<isolated-python> -B scripts/check-test-sleeps.py`: 423 delays
  inspected, none missing a rationale. The isolated interpreter used the repository's pinned parser requirements.
- Skipped browser, installer and unrelated workspace tests because this changes only the emulator's snapshot allocation.
  Ignored full-size and private-recording experiments and timing benchmarks were not needed beyond the ordinary crate
  tests and the complete memory suite for this allocation change. No instruction file or Markdown heading changed, so no
  agent-reference check was needed. No CI run was dispatched.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort passed the source change with no actionable
findings, including the test-authoring checklist. Commit wording received an independent cold read.

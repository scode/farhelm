## What this was about

Several choices could look saved after the helm refused them: terminal-frame colors, quick-launch naming, terminal
link-click behavior, trust-hint visibility and the quick-launch mark. The maintainer chose to report these failures and
discard the failed choice. The welcome card remains a deliberate silent exception; a failed dismissal can make it appear
on a later load.

## Things you should know

Settings now shows the failure beside the affected control and restores its saved choice. A newer click supersedes an
older result. Closing Settings drops its alert while shared choices still revert. Reopening during a pending trust-hint
save cannot keep that failed choice alive in the reopened dialog or the session's hint.

The quick-launch mark waits for its save, like number keys. The Templates dialog remains busy until the result and any
corrective read finish. A lost reply can follow a committed save, so a failure restores the earlier display and checks
the helm before claiming which mark is stored.

A failed Don't show again save brings back the trust hint with an error while its session is still shown. Leaving the
session drops that error. If reading the trust setting also fails, the error appears alone; the hint's explanation and
actions wait until the setting is known to be off. No failed write is retried or replayed.

Execution found two ways an older trust choice could return: reopening Settings before a pending save failed, and a read
started before a save finished arriving afterward. Both now refresh from the helm rather than retaining the old choice.
Overlapping clicks preserve the newest choice, and an older result cannot undo it. Errors stay beside the affected
control or session.

## Open questions and possible follow-ups

None required for this plan. Device-local sound, sidebar width and text-size saves remain outside its scope.

## PRs

- [#2713 — report failed Settings saves](https://github.com/scode/farhelm/pull/2713/changes).
- [#2714 — report failed quick-launch mark saves](https://github.com/scode/farhelm/pull/2714/changes).
- [#2715 — report failed trust-hint dismissals](https://github.com/scode/farhelm/pull/2715/changes).

## Checks run, reused and skipped

All runtime commands used `scripts/record-test-run.py`. Controlled Rust runs used four nextest slots and zero retries.
Browser runs used pinned tmux, Chromium and WebKit, three isolated stacks and zero retries. Successful records have
complete source and reports, retained output through EOF, no runtime `SKIPPED`, zero command/recorder exits and complete
cleanup.

- Preference queue and save-order tests: 40 passed in run `232fada3-141b-432e-9187-befb4a2438c2`, selecting
  `cargo nextest run -p farhelm-ui --lib -E 'test(api::preference_write_tests::) | test(api::save_order::)'`. Reused
  from source `c7be2a20` because later changes only add UI reporting, tests and prose and leave that queue unchanged.
- Settings and trust regressions: 78 distinct cases, 39 per engine, passed across complementary runs
  `25dc5bc1-c52d-4915-a78d-9bcee73a373e` (eight crossing-read cases) and `a5335392-14f6-4fc9-a9a6-fca4ad987c0f` (the
  other 70). These cover PR1's final behavior; later trust-band reporting receives its own final checks below.

  From `e2e`, the eight-case selection was
  `npx playwright test '(^|/)(settings|claude-trust)\.spec\.ts$' -g 'settings repairs a (Claude|Codex) trust read crossing a successful hint save|Claude trust band (discards a newer Settings overlay after its origin closes|repairs an idle read crossing a Settings dismissal save)'`.
  The complementary command selected `(^|/)(settings|claude-trust|workspace-trust)\.spec\.ts$` with `--grep-invert` and
  that same grep expression. Both used the same corrected PR1 source, later committed as `60562058`.
- Template binding refusals, quick-launch mark refusal and recovery, held/failed mark launch behavior and the restart
  boundary: 16 passed, eight per engine, run `73960f94-6061-4b6a-b43a-a6e8e7bc86a1`, covering PR2. Reused because PR3
  does not change those paths.

  From `e2e`:
  `npx playwright test '(^|/)(templates|quick-launch|restart-with)\.spec\.ts$' -g 'number keys move|a failed number-key save|a refused quick-launch mark|quick launch (discards a failed|honors a held) local mark|restart with refuses held shortcut batches and bolt templates'`.
  This covered the final PR2 files committed as `b5f46443`.
- Final session trust-hint lifecycle, refusal/recovery/navigation and welcome behavior: 36 passed, 18 per engine, run
  `73961841-b18b-4d46-afe5-f4be1039cc88`. From `e2e`:
  `npx playwright test '(^|/)(claude-trust|workspace-trust|welcome)\.spec\.ts$'`. This covers the final PR3 source
  committed as `6702488c`, including unchanged welcome dismissal remaining silent.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed for each PR, including the final band changes.
  `cargo fmt --all`, `dprint` on the changed specification/TODO files, and `python3 releasing/check-changelog.py format`
  passed. The isolated source-delay checker inspected 314 delays with zero missing rationales.
- `cargo build -p farhelm -p farhelm-fixtures` passed at final source `6702488c`; all outputs were already current. This
  identifies the same unchanged backend and fixture inputs used by the browser runs. The earlier prerequisite build's
  exact invocation was not retained, so it is not counted as a separate reproducible check. Each behavioral UI stage
  received a fresh `dx build --package farhelm-ui --platform web --release` from `crates/farhelm-ui`. PR2 reused its
  build across a source-reviewed equivalent `match` to `if let` correction.
- Retained run `75642bb0-e103-4231-9a76-637bd5523386` demonstrated the pre-fix reopened-dialog bug. Run
  `3d390011-6518-4d3f-9c8e-5f990952a76f` failed on new fixture assertions expecting HTTP 200 rather than the actual
  successful 204 and was interrupted; none of its partial passes is counted above. Those assertions were corrected
  before the successful eight-plus-70 runs. Three source-check attempts exhausted their time budget before the later
  complete pass. These same-session failures remain retained privately and are not latent-flake claims.
- Full Rust/browser batteries, desktop runtime, installer, provisioning and release gates were skipped: this stack
  changes client preference outcomes, and focused queue/UI coverage addresses its risks. No hosted CI was requested;
  ordinary PR pushes do not start it.

## Review gate outcome

Each PR received the required separate fresh-context `gpt-6.1-sol` review at high effort, with follow-ups to that PR's
same reviewer. PR1's review found a checkbox that could retain its clicked appearance after refusal and an older trust
read that could restore a stale choice; both were fixed and the reviewer cleared the final diff. Fresh scope assessments
kept those fixes within the required newest-choice and failed-save behavior. PR2 passed with no remaining findings,
including the final equivalent conditional cleanup. PR3 passed with no findings. Commit/PR wording received separate
cold reads. All PRs remain drafts for the landing monitor.

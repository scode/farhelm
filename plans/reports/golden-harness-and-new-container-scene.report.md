## What this was about

The screenshot comparison report could omit a screenshot that failed before it produced a picture. When a comparison
also found pixel changes, reviewers could see those changes and miss the gap. The page now shows which expected light or
dark appearances were missing or failed, even when there are no pixel diffs. The runner rejects incomplete coverage in
comparison and update mode, including when the renderer exits successfully.

The New container screenshot used to stop as soon as the Recipe field appeared. The maintainer wanted it to show the
settled build status for the Development recipe on the selected container host. The fixture now supplies that ready
state, and capture waits until the whole “Ready on Local (this machine)” line is visible.

## Things you should know

The review page uses a separate outcome for each appearance, so no saved result still identifies a gap if a renderer
dies mid-run. A failed comparison keeps its saved results in the retained run directory. The New container change is
confined to that scene. The full capture had the same 540 reference names, with only the two intended pictures changed.

## Open questions and possible follow-ups

None.

## PRs

- [#5011](https://github.com/scode/farhelm/pull/5011/changes) reports missing and failed golden renders.
- [#5014](https://github.com/scode/farhelm/pull/5014/changes) captures the settled New container status.

## Checks run, reused and skipped

- At the outcome-reporting revision `885408b4`, `python3 -B scripts/test-check-gui-goldens.py` passed all 25 stand-in
  cases (run `67d04fbb-ca46-4ec1-987c-74d2309addf9`). A real comparison with
  `python3 scripts/check-gui-goldens.py --scene anchored-prompt --jobs 1` passed in both appearances (run
  `66656d72-8da9-4846-9146-5a94dc330cbd`). The two native pixel review tests
  (`cargo nextest run --locked -p farhelm-gui --lib -E 'test(goldens::changes)'`, run
  `796bc439-74df-497a-be36-60c397cb03be`) and the selected update
  (`python3 scripts/check-gui-goldens.py --update --scene anchored-prompt --jobs 1`, run
  `2d998a24-7126-44a1-843d-ac5a06ed5075`) passed on `aeb34216`. Their output contracts carried forward, and the later
  live updates exercised the final renderer.
- For the New container scene, `python3 scripts/check-gui-goldens.py --update --scene launcher-new-container --jobs 1`
  (run `523e7e64-b53e-4312-9d4f-4b3a8e800546`) and `python3 scripts/check-gui-goldens.py --update` (run
  `16101a88-f9f1-47cb-913b-0222ca748040`) passed on `3796b0b7`. The selected and full captures matched byte for byte. I
  inspected both appearances, then verified all 540 blob IDs against the previous references. The final commit adds
  those two references and corrects which host value is printed on timeout. The successful capture path is identical.
- Rust formatting, changed Markdown formatting, whitespace checks and the test-delay check passed. The instruction
  checker reported an existing archive citation warning. Upstream changes to host provisioning and queue bookkeeping
  left the renderer and references untouched. The broader workspace suite was skipped because the changes affect
  screenshot testing and the New container fixture.

## Review gate

Both PRs passed independent source review with high-effort Sol. The outcome reporting fix now retains receipts on
interrupted comparisons and tests partial success from a renderer that exits zero. The scene fixture checks the actual
painted text and reports the observed container host, recipe and note when the staging budget runs out.

### Landing

#### What landed with it

Only this plan landed this round. Since the plan's PRs were based, main gained the What's new restyle (which replaced
its two screenshots and refactored the launcher's Launch button), the release headline checker, the supervisor's Restart
cleanup fix and two helm SSH sign-in fixes. The rebase applied without conflicts. The two New container screenshots were
captured before the launcher's button refactor landed, so the round's full screenshot comparison ran on the combined
code: every picture matched, those two included. The new coverage rule works from the scenes the renderer reports rather
than from the picture list, so the What's new pictures needed nothing from it, and both sides' picture list lines are
present.

#### Fixes made while landing

- **One sentence in the validation instructions became wrong.** It said a render with no differences produces no review
  page; a run with a missing or failed picture now produces one even without differences. Fixed inside
  [#5011](https://github.com/scode/farhelm/pull/5011/changes). The landing's independent review found nothing else to
  change before landing.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the plan's PRs rebased onto main as it stood at the start of the landing (`0ff64462`), which applied without
conflicts; every file's change is exactly what the executor pushed, apart from the one-sentence instructions fix inside
#5011, which changes no code.

- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally, with the plan's
  new runner (recorder run `275332ed-a746-48b2-9622-1f3c79e45d6a`): passed, every expected appearance present, including
  the two New container pictures the plan updated.
- The golden runner's stand-in tests, `python3 -B scripts/test-check-gui-goldens.py`, locally (recorder run
  `397d662d-f889-43f3-a3e4-26768def8b2a`): passed.
- The screenshot batch tool's test, including the committed scene list against the changed picture list (recorder run
  `b8317e4a-bc55-46ca-bc2c-9bc4a7f5837e`), locally: passed.
- The GUI crate's tests, `cargo nextest run -p farhelm-gui`, on the test worker (recorder run
  `d88d6bfd-b44a-474b-92f0-987e6ec0d46c`): all 3433 passed, including the review page's Rust tests. Only this crate's
  golden code changed among the Rust code.
- `cargo clippy --all-targets -- -D warnings`, on the test worker: passed.
- `python3 scripts/check-agents-md.py` (before and after the instructions fix),
  `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive, which main has had since an earlier triage record (#4978)
  landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused: none. The executor's runs were on an older base, and everything they covered ran again above.

Skipped: the workspace Rust suite, the desktop app's tests and clippy configurations and the end-to-end smoke, since no
app code changed (only the golden harness, which is test code in the GUI crate, and its Python runner); a Mac run, since
nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **A run whose test filter selects no scene now fails in comparison mode too**; before, it passed without comparing
  anything. That is deliberate, but worth knowing if a `--scene` pattern with a typo now stops a run.
- **Smaller points the review noticed.** Nothing pins the format of the per-appearance results the Python runner reads
  from the renderer, though a drift would fail every run loudly rather than hide a gap; when the New container scene
  stalls before the launcher opens, its timeout message still says it was waiting for the "Ready on" note, though the
  values it prints show the real step; and after an update the error line for an incomplete render names a directory
  whose result files were already removed, while the review page still lists the reasons. None needs a follow-up on its
  own.

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

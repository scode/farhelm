## What this was about

A removed container host could remain on a recipe card indefinitely if a build had failed there. Re-adding the host gave
it a new identity, so the old row stayed unusable. The card now drops removed hosts whose only record is a past failure.
Registered hosts still show their failure history. Removed hosts with active work or resources to clean up stay visible.

Previously, an edit to a Tensorlake recipe while the launcher's billing question was open could start a sandbox with a
different CPU, memory or disk size than the user had agreed to pay for. **Create and launch** now confirms the size
shown in the question. If the helm reads a different size at admission, it refuses before registering a host or
contacting Tensorlake. The launcher keeps the draft and shows the updated size when you press Launch again.

## Things you should know

Confirmed launches through the current environment route require a desktop app and helm that both support the size
field. In a mixed-version setup the launch is refused, with the draft still available. Clients of the legacy sandbox
endpoint can still send the confirmation flag alone. The deliberate Tensorlake skip setting and ordinary container
launches still send unconfirmed requests.

## Open questions and possible follow-ups

None.

## PRs

- #4980 hides removed container hosts with only a past build failure.
- #4988 binds Tensorlake launch confirmation to the size the launcher showed.

## Checks run, reused and skipped

- Recipe-place tests covered registered failures across recipe bases, omission of removed failure-only hosts, retained
  resources, active work and the recipe feed. All three passed in the helm test binary built from `ee2a91ca`'s source.
  Ran recorded
  `cargo nextest run -p farhelm-helm --lib -E 'test(/ui_channel_tests::.*recipe/) | test(a_place_jobs_admission_step_and_end_reach_the_connected_ui)'`.
  The module filter missed two cases, so ran those exact tests using the same compiled libtest binary,
  `recipe_places_keep_usable_history_and_single_provider_places` and
  `a_recipe_edit_reaches_the_connected_ui_and_the_reads_start_nothing`. Recorder IDs
  `5fb1c579-28fe-4ee3-9f10-7756687cf8a9` and `43fcc409-1c9c-4987-8b70-0eaabf386a22`.
- The focused helm and launcher run passed 38 tests against `290edcc0` plus a capture-only fixture. It checked a
  same-name edit before confirmation, refusal before registration or provider calls, matching-size creation, the legacy
  route, the skip setting and ordinary container launches. Ran recorded
  `cargo nextest run -p farhelm-helm -p farhelm-gui --lib` with filters for the sandbox and environment launcher tests,
  recipe session provisioning, request decoding and the container route. Recorder ID
  `3f7197f7-10ed-4d47-a3c9-181a9ccba2f2`.
- Inspected Linux light and dark captures from `af8c80b0`, the same product code with a temporary capture fixture. Both
  show the complete refusal, retained draft and enabled Launch button. Ran recorded
  `python3 scripts/check-gui-goldens.py --update --scene launcher-new-sandbox-checkout --jobs 2`. Recorder ID
  `0f277aba-48b2-46ac-983d-a094916db13a`.
- Reused recorded runtime and visual evidence on the final draft heads `13371629` and `3662ae60`. The rebase preserved
  both patches. The intervening code changes cover existing sandbox setup and repository-cache cleanup, outside the
  recipe feed, fresh session launch path and launcher behavior tested here.
- `cargo fmt --all -- --check`, targeted `dprint check`, `python3 releasing/check-changelog.py format` and
  `python -B scripts/check-test-sleeps.py` passed. The workspace battery and a live Tensorlake launch were skipped
  because the focused provider and GUI fixtures directly exercised the changed behavior.

## Review gate

Independent source review caught a recipe card regression when a registered host's build failure predated a base change.
The fix preserves that row and adds a test for it. Review of the consent change prompted a matching-size launch test
through the current route, a stronger check that the catalog edit arrived before the user answered, and documentation
corrections. The follow-up reviews found no remaining source issues.

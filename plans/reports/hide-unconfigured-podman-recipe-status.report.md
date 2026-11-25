## What this was about

Managed Ubuntu can run on Tensorlake or on a container host. Settings → Recipes used to show an unchecked or unavailable
Podman host and its Rebuild button even for a recipe that had never used it. The shared recipe now shows that host once
it can build there, or when it has build history, work or resources to show.

## Things you should know

- A recipe based on a container image still shows unavailable container hosts, since its base can only run there.
  Settings still checks for container hosts when the page opens. A usable check brings the host into the shared recipe's
  list.
- Rebuild is disabled while a shown place is unavailable. Its hover help gives the current reason while the row can
  still show a previous build error. A retained row stays readable after a recipe changes base, and the disabled button
  explains when the new base cannot run there.
- Tensorlake snapshot status has its own display. Launch placement still reads the provider's current checks and
  preserves its refusal text. Cleanup still follows the resources the recipe owns.

- The Podman triage changes how Farhelm checks host identity and cleans up recipes after host removal. This plan uses
  those checks to decide which status rows and Rebuild buttons to show.

## Open questions and possible follow-ups

The agreed scope is complete.

## PRs

- Draft at `6c29e95a` that hides incidental unavailable places and explains when Rebuild is disabled.
  [#4934](https://github.com/scode/farhelm/pull/4934/changes)

## Checks run, reused and skipped

The reported checks all ran on the exact draft revision `6c29e95a`.

- Focused nextest passed 32 Settings, launcher and helm tests with no runtime skips in recorder run
  `3150e5de-6f09-4009-9e91-75d5e40dc39e`. The cases cover the shared and single-provider place lists, history and live
  work, discovery, and the disabled and restored Rebuild states.

  ```sh
  cargo nextest run --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib -E '
    test(recipe_place)
    | test(a_place_jobs_admission_step_and_end_reach_the_connected_ui)
    | test(every_pushed_collection_round_trips_through_its_proto_type)
    | test(launch_joins_rebuild_and_only_success_replaces_the_working_pair)
    | test(launcher::environment_view_tests::)
  '
  ```

- `python3 scripts/check-gui-goldens.py` matched all four Linux reference images for Settings recipe places and New
  container in light and dark. Recorder run `9e66e911-6b2b-4c03-9f9c-93c3d699cf7e` used the same clean `6c29e95a`
  revision.
- Both `cargo clippy --locked --all-targets -- -D warnings` and `cargo clippy --locked -p farhelm --bins -- -D warnings`
  passed at `6c29e95a`.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`
  and `git diff --check 7efe4f5c..6c29e95a` passed. `python -B scripts/check-test-sleeps.py` found a written rationale
  for all 309 test delays.
- Kept the tested base after inspecting the later `16d47afc` change to main. That revision adds a separate terminal
  shortcut TODO.
- The full runtime battery was skipped. Targeted coverage exercises the place list, Settings controls and the launcher's
  use of the readiness reason.

## Review gate

The independent source review passed after the fix for incompatible historical places and the clarification of how
Tensorlake snapshot status is shown.

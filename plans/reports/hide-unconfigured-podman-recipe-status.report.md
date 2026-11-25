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

### Landing

#### What landed with it

Three plans landed this round, in this order: the shared Managed checkout repository history (#4928, #4938, #4944), this
one through its single PR, and then activating the previous terminal tab on close (#4940). This plan was written before
the Podman triage plan landed in the previous round, and that triage changed how Farhelm checks a container host's
identity, which host entry holds its claim, and how a recipe is deleted after its host is removed. The landing's review
read this plan against those changes and found no interaction that breaks either: this plan only decides which places a
shared recipe card and the launcher show, and it reads the triage's checks to do it, as its report says. The rebase
conflicted only in TODO.md, where both sides removed entries; I kept both, and every other file's change is exactly what
the executor pushed.

The review counted no blockers, one should-fix (below, left for you because the plan chose it on purpose) and no small
points for this plan.

#### Fixes made while landing

- None.

#### Checks

This round landed three plans in queue order: the shared Managed checkout repository history, hiding container hosts
that a shared recipe cannot use, and activating the previous terminal tab on close. All the checks below ran once on the
three plans' PRs stacked in that order on main as it stood at the start of the landing (`3c39d3ff`), with the conflict
resolutions described in the Landing section of the Managed checkout repository history report; the stack's top commit
was `1d46ce6e`. Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The only
later change is the landing fix [#4944](https://github.com/scode/farhelm/pull/4944/changes), which drops a stale example
from one SPEC.md sentence; after it I ran `dprint check` on SPEC.md again, and it passed. That final tree is what main
now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `f77a85f9-2e39-4531-b1e0-23ca4b4ca57e`): all 8660 tests passed. One test skipped itself at run time,
  `the_probed_launch_prefix_pins_every_load_bearing_flag`, because the worker's systemd user manager did not answer its
  probe at that moment; it checks the supervisor's launch-scope flags, which none of these plans touch, and it ran in
  the two previous rounds' runs, so its absence here is missing coverage unrelated to them. The plans changed the helm's
  store (a new schema step) and session creation, the protocol between helm and supervisor, the supervisor's request
  handlers, the recipe place list and recipe state, the Managed checkout end-to-end tests, and the GUI's launcher,
  Settings recipe cards and terminal tabs; the whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `b578690f-32d5-4cdc-9495-1e30c4d332e2`): all 184 passed. The desktop app builds the GUI crate these plans changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `dd8d7103-e0d0-4e3c-ba0a-b2c415fe833d`): 50 steps passed, one skipped by design on Linux (paste). It opens the
  launcher and drives terminal tabs and attach.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `a2119b48-b97f-4fbb-abfd-0aade7959e9d`): passed, including the launcher scenes whose test setup the landing merged by
  hand and the container checkout refusal scene the Podman triage added.
- The test of the screenshot comparison tool and its scene list, `python3 -B scripts/test-gui-screenshot-batch.py`,
  locally (recorder run `71c56adc-6730-4bc8-885a-764e395c5806`): passed. The first plan removed a golden scene and its
  list entry.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Start a session and Work in a session).
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main),
  `python -B scripts/check-test-sleeps.py` and `dprint check`, locally: passed.

Reused: none. The executors' runs were focused selections on older bases, and everything they covered ran again above.

Skipped: an older supervisor connecting to the upgraded helm (the first plan's report asks for it during release
testing, since the protocol version changed); the workspace doctests, since no documentation example changed; a Mac run,
since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **A container host that was removed after a failed build now shows on the shared recipe card for good.** If a launch
  preparation or a Rebuild failed on a container host where the recipe had no images yet, Farhelm keeps a record of that
  failure. Before this plan, such a host disappeared from the card once it was removed. Now the card keeps a row for it,
  reading "Unavailable place" with the old failure and a disabled Rebuild whose help says the place is no longer
  available, and nothing removes the row except deleting the recipe (a re-added host gets a new identity, so it cannot
  take the row over). Any failed first launch preparation or Rebuild on a container host leaves such a record. The
  plan's test asserts this row on purpose, as build history worth showing, so I did not change it at landing. The
  review's suggested fix is to show a place the provider no longer lists only while the recipe still has resources or
  running work there, or to drop failure-only records when the host is removed. The first keeps a removed host on the
  card while the recipe still has images or running work there, so their cleanup stays visible, and hides it once only
  the old error is left; the second loses that error when the host is removed, which matters little since a re-added
  host cannot pick it up anyway, but it changes host removal rather than the card. I recommend the first; say if you
  want a TODO entry.

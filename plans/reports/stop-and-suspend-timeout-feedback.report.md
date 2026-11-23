## What this was about

The desktop dropped both timeout replies to **Stop and suspend**. If the helm stopped waiting after 20 seconds while the
session was still stopping, there was nothing by the row to explain the wait. A timeout reported by the supervisor
itself was also silent, even though Farhelm would skip suspension.

The maintainer chose a neutral notice that leaves the outcome open. The desktop shows it in the session row, its menu
and the open session header. The notice says the action may still be running and that suspension requires a clean Stop.
When the supervisor reports a timeout, the desktop shows a Stop failure and Farhelm skips suspension.

## Things you should know

The notice can appear before the sandbox or container changes phase. It clears when the host reports that it is
suspended or deleting, or when you start another action on the same session. If the host is already suspended or
deleting when the deadline reply arrives, the notice is already obsolete. The UI follows the host even if the row is
filtered out, so resuming from Hosts does not bring the notice back. If the work fails after the reply without reaching
either host phase, the helm logs the late failure and the notice stays until the next session action or a connection
reset.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4668](https://github.com/scode/farhelm/pull/4668/changes) marks the helm's 20-second reply and distinguishes a
  supervisor timeout on the wire.
- [#4671](https://github.com/scode/farhelm/pull/4671/changes) shows the notice and failure, retires the notice from
  pushed host state, and updates the two screenshot references.

## Checks run, reused and skipped

The nextest commands below are abbreviated. Each run used the named test selection.

- `cargo nextest run --locked -p farhelm-gui --lib` selected the four cases for both replies and notice dismissal. All
  passed. Recorder `96894e4f-20c6-4410-9226-3a125627acb1` at `f280c3d8`.
- Reused 222 passes from `cargo nextest run --locked -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib` with the UI
  channel, Tensorlake host lifecycle, late failure logging and desktop outcome modules selected. Recorder
  `0f6e9754-cf37-492b-9db1-8bcf8abe44b8` at `a0b71165`. The marker, response handling and desktop behavior kept the
  tested patches. The last rebase changed Tensorlake's suspension path, which the next run covers on the final tree.
  Claude folder trust and opt-in drift tests were independent of these paths.
- `cargo nextest run --locked -p farhelm-helm --lib` selected the three routed Stop and suspend cases and the
  unconfirmed sandbox refusal after the Tensorlake identity rebase. All four passed on `70338f69`, recorder
  `fee85f47-f8b6-4abf-ab4d-7fb1076f278f`.
- `cargo clippy --locked --all-targets -- -D warnings` passed at `f280c3d8`. `cargo fmt --all -- --check`,
  `dprint check`, `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` passed.
  `python3 -B scripts/test-gui-screenshot-batch.py` passed all 47 cases in recorder
  `1ddfafce-5c8a-4aa4-8bf6-6358afe49e9c`.
- `python3 scripts/check-gui-goldens.py --update --scene stop-and-suspend-outcomes` produced the two revised pictures at
  `f280c3d8` in recorder `3e9c3433-d13a-4486-b461-9de23e2103b9`. The full Linux run with `--update --jobs 4` rendered
  408 images in recorder `4379a82f-7891-4ff1-8af8-f86c493191bb`. Only the two approved screenshots were added, and they
  matched the reviewed images byte for byte. The final rebase left desktop source, dependencies and reference manifest
  intact.
- A whole-workspace runtime run and real cloud provisioning were outside the affected behavior. The existing user guide
  photographs the Stop controls before any action is taken, so those images were reused. Visual evidence was captured on
  Linux.

## Review gate

Independent Sol code review passed on both changes. The first review corrected the timeout description in the spec and
helm log. The desktop test gives both timeout replies identical text so the marker alone must distinguish them. Opus
approved the two screenshots after the fixture ages and command were corrected. Only the pictured test data changed
before the second screenshot review.

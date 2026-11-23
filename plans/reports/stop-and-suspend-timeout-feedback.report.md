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

### Landing

#### What landed with it

The plan's two PRs (#4668, #4671) landed alone in this round, followed by one landing fix (#4676) that changes only code
comments. Nothing reached main between the plan's base and the round's start; while it landed, only two changes to
TODO.md went in, which touch nothing here. The plan was built on top of the previous round's change to Tensorlake
Suspend (#4670), which made Suspend and Resume refuse a host whose sandbox Farhelm cannot identify; nothing here undoes
or contradicts it.

The landing's independent reviewer confirmed that only the helm's own 20-second reply carries the new marker, so a
supervisor's own Stop timeout always shows as a failure, and that no other action whose reply the helm also limits to 20
seconds changes behavior. The notice clears once the host shows as suspended or deleting (and is never shown at all if
the host got there before the reply arrived), on the next action on the session, or when the desktop reconnects to the
helm. A later Resume cannot bring it back, and it clears even while the session is filtered out of the list. Every other
outcome line in the session list (refusals, partial results, their "Stop: " style prefixes, the selected row's colors)
looks as before. The marker needs no protocol version change: only the desktop app reads it, and the app only ever talks
to the helm built with it.

#### Fixes made while landing

- Rewriting the code comments around the 20-second reply of Stop and suspend and of a host's Suspend and Resume, the
  plan dropped the recorded reason those actions reply within it instead of answering "accepted" at once (their result
  reaches the desktop through the host state the helm pushes anyway). It also left some desktop comments describing the
  change instead of the code, one of them wrongly saying the new notice carries a "Stop: " prefix. #4676 restores the
  reason and corrects those comments. No behavior changes.

#### Checks

The plan changes the helm's Stop and suspend reply, the message format between the helm and the desktop app, and how the
desktop draws every session outcome line, with a new reference scene and an entry in the screenshot scene list. The
supervisor and the desktop app's outer program (the part that starts the window and the embedded helm) are untouched, so
the landing ran the helm's, the message format's and the desktop window's own test suites, the lints, the end-to-end
smoke test and every reference picture on the plan as delivered, then the lints again after the comment-only landing
fix. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH to
itself; the rest ran locally on x86_64 Linux. None of them reached a real sandbox or container host: the tests use
stand-ins, as the report says.

On the plan as delivered, rebased onto main:

- `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-gui`, on the test worker (recorder run
  `2b1df1c5-64d3-48b1-ae86-6c1d36df2798`): all 5463 passed, none printed a runtime `SKIPPED`.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `61849e6a-7899-4ef4-8dbc-3a8d781656e6`): all
  50 steps passed, one skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `7a51c5f5-25ac-4aef-a8a7-62e4d2f1a4e6`): every picture
  matched its reference, the new scene's light and dark pictures included; I also looked at the light one myself.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks), locally and not through
  the recorder, so it has no run id: all 47 cases passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4676), which changes only comments:

- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed (the first attempt could not reach the worker and ran nothing; the second ran).
- `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`, locally: passed.

Reused: every runtime suite above for the fix, since comments cannot change behavior. Skipped: the supervisor's tests
and the desktop app's outer program's tests, which nothing in the plan reaches; macOS, where nothing compiles
differently; a real sandbox or container host; CI, which runs only on demand.

#### Things you should know that the report does not say

- The notice is the helm's own text, so inside the session list it ends "Check the session list for the result", and at
  the sidebar's width it takes four lines under the row. The plan asked for that wording; a possible follow-up is a
  shorter text for the desktop.
- The plan adds one new reference scene, pictured in light and dark. It shows the notice on the selected row (in the
  row's selection colors) and in the open session's header, but not on an unselected row, so no picture shows the
  notice's neutral color in the list. Adding that row to the scene would be a small follow-up.

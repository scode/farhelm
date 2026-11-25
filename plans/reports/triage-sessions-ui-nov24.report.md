## What this was about

The desktop app could hide an interrupted upload, open Upgrade highlights after a sign-in question, or give no reason
when Open Announcements failed. The session list also needed to give screen readers the warning on a retrying host and
show the mute deadline at the waiting dot. This stack addresses those problems and the small presentation, bookkeeping
and test gaps from the November 24 triage.

## Things you should know

Every interrupted upload occurrence can be dismissed on its own, including repeated attempts for the same filename. Each
group of interrupted files uses the first captured, nonempty session name available for that group. If the selected
session settles without a terminal, the window reports its pending interruptions while the session stays open. A
reconnect that does restore the terminal can still report them there.

If a sign-in question took focus while Upgrade highlights was loading, the highlights stay closed. A failed Don’t show
again now points to the existing Upgrade highlights control under App in Settings, in the area the Settings plan also
touches. If Open Announcements fails, the Notifications panel stays open, shows why and keeps its retry action.

When a desktop window reconnects with a long notice stack, the newest warning stays in view. In Light appearance, Save
and destructive actions visibly respond to a pointer press. With Reduce Motion enabled, repeated animations stop leaving
an ever-growing list of stale entries behind.

The recovery-card and bell fixes reuse existing notice and panel behavior. I treated the roughly 100-line target per
outcome as approximate because the required screenshot fixtures brought those fixes to 134 and 159 changed lines,
respectively. The draft totals 297 lines within the roughly 300-line batch budget.

The upload change uses the terminal attachment handover and its existing tests, near the terminal triage plan's area.
The session toolbar and host rail changes are test assertions in the desktop UI. The screenshot catalog gains the
references for these views alongside the scenes maintained by the tooling plan.

## Open questions and possible follow-ups

Triage allowed accessibility and test-only changes here when they fit in about 20 lines. These three exceeded that
scope. Native screen readers still may omit menu shortcut metadata. A spoken fallback would need rules for modifier
glyphs and avoiding a repeated shortcut after the adapters expose it. The host status test keeps the names that the
desktop and helm use for setup phases in sync, but can miss a rename in the host manager. It would need a table built
from manager states. On Mac, a maximized relaunch should never flash the previous ordinary window size, and the next
Zoom should restore the remembered size and position. The existing capture measures one open window. Testing those
guarantees requires spanning a relaunch.

## The PRs

- [#4811](https://github.com/scode/farhelm/pull/4811/changes) preserves repeated interruptions and uses an available
  session name in grouped upload notices. Fixes `trailing-duplicate-interrupted-upload-notices-silently-dropped.md` and
  `trailing-interrupted-upload-empty-title-and-batch-title-drop.md`.
- [#4813](https://github.com/scode/farhelm/pull/4813/changes) keeps late highlights out of the way of sign-in and makes
  a failed opt-out visible. Fixes `upgrade-highlights-can-open-after-sign-in.md` and
  `upgrade-highlights-opt-out-refusal-is-hidden.md`.
- [#4816](https://github.com/scode/farhelm/pull/4816/changes) exposes retry warnings to screen readers and gives the
  muted waiting dot its own help, including the time of a timed mute. Fixes
  `sidebar-retry-warning-missing-accessible-name.md` and
  `trailing-muted-waiting-dots-bypass-ringless-status-tooltip-path.md`.
- [#4822](https://github.com/scode/farhelm/pull/4822/changes) surfaces interrupted uploads on recovery cards and failed
  announcement reads in the bell panel. Screenshot tests for the update reminder now use the same drawing path as the
  app. Fixes `trailing-interrupted-uploads-invisible-on-card-or-retired-session.md`,
  `trailing-reminder-frame-cfg-test-branch-in-render-path.md` and
  `trailing-bell-open-announcements-silent-on-read-failure.md`. Closes
  `menu-shortcuts-omitted-from-native-accessibility.md` with no change.
- [#4826](https://github.com/scode/farhelm/pull/4826/changes) restores the newest notice after reconnect, distinguishes
  button presses in Light and bounds animation bookkeeping under Reduce Motion. Tests check that status help points to
  its own mark, the condensed toolbar uses the expected heights and late announcement replies stay out of the way after
  the user moves on. Fixes `trailing-status-mark-tests-lost-their-per-target-geometry-check.md`,
  `trailing-overflowing-banner-stack-resets-to-oldest-after-reconnect.md`,
  `trailing-light-theme-primary-and-danger-buttons-identical-hover-held.md`,
  `trailing-ticker-leaks-dead-subscribers-under-reduced-motion.md`,
  `trailing-toolbar-650px-height-test-unguarded-else-branch.md` and `trailing-bell-stale-reply-guards-untested.md`.
  Closes `trailing-all-phase-fixture-does-not-compare-managers-phase-strings.md` and
  `trailing-maximized-relaunch-contract-has-no-first-frame-or-subsequent.md` with no change.

## Checks

Each draft ran `cargo nextest run --locked -p farhelm-gui --lib` with the focused selection below before
`python3 scripts/check-gui-goldens.py --update --jobs 4`. All five full captures passed.

| Draft | Tested revision | Focused tests and recorder run                                                                                                           | Full capture run                       |
| ----- | --------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| #4811 | `8d4d2516`      | Repeat uploads, batching and dismissal (9 passed), `cbbf01e6-1b9f-4b0a-abaa-118f57a8b686`                                                | `11daa890-0d00-4d80-a8d6-8bb8569fb097` |
| #4813 | `efbea50e`      | Startup focus, opt-out failure and retry (22 passed), `ea3307bc-256e-43d0-af4c-9b1424b343b6`                                             | `34e8943d-c738-4764-bedd-4e1c1de8c13e` |
| #4816 | `b124eb58`      | Sidebar warning names and muted hover help (6 passed), `c22516fe-4436-46a9-a21e-5474e0d75cf9`                                            | `41fb367f-32ad-4899-8440-1d224599c55e` |
| #4822 | `aab4f4b6`      | Upload recovery, bell failure and reminder frame (10 passed), `f6f58eda-de43-4a6a-b0a1-a952d6bc80e5`                                     | `1f0e30ae-ab99-4a46-b096-89ff02ac1ae9` |
| #4826 | `63323c1e`      | Reconnect, late bell replies, mark geometry, button fills, reduced motion and toolbar (9 passed), `8f711ec6-563b-4afd-8c83-490f08d9f224` | `db79906d-c215-493f-8f4b-46d95da7e861` |

Static checks passed with `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, and
`python -B scripts/check-test-sleeps.py` using the isolated interpreter. `python3 scripts/check-agents-md.py` passed
with a pre-existing archive citation warning. The screenshot catalog check
(`python3 -B scripts/test-gui-screenshot-batch.py`) passed 47 tests in each of four runs after new scene publication.
The run IDs are `1adbe765-fcea-4917-bc40-f13826b7ff07`, `1ddcfd2c-70fd-4ffb-be3b-a344b9433731`,
`dbbaf375-5ef4-4413-ab3c-49d11d68910b` and `ac2f3314-46f2-4a55-b8b8-c1350e1e5b05`.

Retained initial failures were a sidebar fixture visibility compile error (`6e19a31f-091b-4f2c-a173-f5afd6daa218`), an
upload fixture that had no upload (`b82ddb82-98b9-4028-80fd-7ed9c1022c22`), and a late bell fixture that left the panel
open (`10e486a7-325f-414e-b6e6-2665c2143bb6`). The final runs above covered the repaired scenarios. Evidence remains
applicable after adding published picture manifests and archive links. The docs-shot audit found zero affected current
crops or pointer targets. Validation covered the changed desktop behavior with the focused selections above. Broader
workspace tests were skipped, and native Mac relaunch coverage remains the separate gap described above.

All five drafts rebased onto main at `dfae22dc28c3`. Shared ledger and index conflicts kept both sets of entries. The
code, spec, test, changelog and screenshot patches match their reviewed versions across the rebase. Nearby changes on
main concerned other actions or documentation. Reused the runtime, Clippy, visual and catalog checks above and reran
`dprint check` and `python3 scripts/check-agents-md.py` against the rebased stack.

## Review gate

Each draft received independent Sol review and finished with no unresolved correctness findings. The reviewed changes
included which upload notices get promoted and fixes to the hover, bell and recovery fixtures. Opus inspected the
affected views in both appearances and found no remaining mismatch. The final reference runs were compared with the
approved captures before publication.

### Landing

#### What landed with it

Two plans landed this round: triage-tooling-nov24 first, which fixes maintainer tooling (test harnesses, screenshot
scripts, the drift runner and the release test) and touches none of this plan's code, then this one through
[#4811](https://github.com/scode/farhelm/pull/4811/changes),
[#4813](https://github.com/scode/farhelm/pull/4813/changes),
[#4816](https://github.com/scode/farhelm/pull/4816/changes), [#4822](https://github.com/scode/farhelm/pull/4822/changes)
and [#4826](https://github.com/scode/farhelm/pull/4826/changes). Since this plan's base (`dfae22dc`), main gained only
plans queue commits. Both plans add scenes to the app's screenshot tests; the landing's review checked by script that
every scene in the combined tree has a published light and dark reference picture, and the full comparison below matched
every one. Stacking this plan on the other conflicted only in the triage ledgers and the review queue's index, where
both plans had removed different findings and archived different decisions; I kept both sides at each of this plan's
commits, and a script check then confirmed the index matches the findings on disk and no decision is in both the ledger
and the archive.

#### Fixes made while landing

None to this plan. The landing fix #4833 named below belongs to triage-tooling-nov24.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. This round landed two
plans, triage-tooling-nov24 and triage-sessions-ui-nov24. Neither report's checks covered the two stacked together, so
the landing ran its checks on both plans stacked on main, with the conflict resolutions in the triage ledgers and the
review queue's index (`40fe32d9`). The only later change is the landing fix
[#4833](https://github.com/scode/farhelm/pull/4833/changes), which reformats one line of a test scene and removes a
blank line from one instruction paragraph; the formatters and the instruction check passed on the final tree
(`901f58bb`), which is what main now holds outside the plans queue.

- `cargo nextest run --locked -p farhelm-gui -p farhelm-desktop -p farhelm --lib --bins --test e2e` with
  `-E 'package(farhelm-gui) | package(farhelm-desktop) | test(/^real_agent_capture::drift::/)'`, on the test worker
  (recorder run `c3af473f-7286-4297-9ff6-25b444186cb7`): all 3593 selected tests passed, with no runtime skip. That is
  the desktop app's interface crate, the desktop app itself, and the drift tests that use stand-in agents.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and the desktop app's
  two clippy runs (`-p farhelm-desktop --all-targets`, with and without `--features agent`), on the test worker: passed.
- The desktop app's end-to-end smoke (`scripts/gui-e2e-smoke.py`, recorder run `659d1f71-c182-4ccd-ba94-df81cff63e54`:
  50 steps, one skipped, the paste step that headless Linux cannot run) and its Linux lifecycle run
  (`scripts/gui-lifecycle.py`, `38181353-2c50-412c-9e62-f4f4c6d1067d`), on the test worker: passed. Both plans changed
  what these drive: the script that drives the app, and the notices and Upgrade highlights shown when a window starts.
- `python3 scripts/check-gui-goldens.py`, the full comparison of the app's screenshot tests against their published
  reference pictures, locally (recorder run `af16a877-369e-409b-a4e3-e6c747a1ae82`): every scene matched, the new ones
  included.
- The changed scripts' own tests, locally, through the recorder: `scripts/test-run-harness-drift.py`
  (`ae3bb7ae-1b00-4552-8410-01eda68ccbb1`, 21 passed), `scripts/test-capture-agent-screens.py`
  (`d43cf8c2-d6c3-45a2-ad2b-ac98b584e370`, 13 passed), `scripts/test-worker/test-test-worker.py`
  (`673b2ef9-3173-4948-b02d-0ec625a2708b`, 18 passed) and the brick test's portable suite
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, `25f16a81-0bf6-42f3-92e8-4b7dd9a57f98`,
  324 passed).
- More of the changed scripts' own tests, locally, run directly rather than through the recorder, which was an oversight
  of mine, so they have no run id: `python3 -B scripts/test-docs-shots.py` (31 passed),
  `python3 -B scripts/test-check-gui-goldens.py` (21), `python3 -B scripts/test-gui-agent.py` (95),
  `python3 -B scripts/test-gui-screenshot-batch.py` (47), `python3 -B deflake/test-deflake.py` (20), and `deno check`
  and `deno test -A` on `scripts/boat/boat-tests.ts` (6) and `scripts/tensorlake/rust-tests.ts` (3).
- `shellcheck` and `py_compile` on the test worker harness's changed files, `bash -n` and `shellcheck` on
  `scripts/test-provision-centos.sh`, `python3 releasing/check-changelog.py --self-test` and `format`,
  `python3 scripts/check-agents-md.py` (no errors; one warning, about an old citation in the triage archive, already on
  main), `dprint check` and `python -B scripts/check-test-sleeps.py`, locally: passed.
- `cargo fmt --all -- --check`, locally: failed on the combined stack, on one line from triage-tooling-nov24, and passed
  after the landing fix #4833.

Reused, from triage-tooling-nov24's report: the CentOS provision and update check at `1c6765e6ec02`
(`4f766a39-8b03-4d47-a7e4-9d7ee18cf376`), which needs a container runtime the landing did not use, and the docs
screenshot capture at `48b1e92e53b2` (`2fa76b78-a99b-4743-abee-66a1c4dbbb1b`) whose pictures the executor inspected. The
landing's conflict resolutions and fix touched neither the CentOS script nor the docs screenshot pages.

Skipped: the rest of the workspace Rust suite, since no other crate changed; the live drift cases, which are opt-in and
spend real vendor turns; a live brick test, which needs a Mac VM; on a real Mac, the desktop app reopening itself after
an update and the macOS version of the session toolbar's screenshot test, which the tooling report already lists as
unverified; and CI, which runs only on demand.

#### What the report says that is no longer true

Nothing. The report's checks ran on this plan alone; the ones above ran on both plans together.

#### Things you should know that the report does not say

- **Where an interrupted upload is reported after a reconnect depends on what the open session shows first.** The report
  says a reconnect that restores the terminal can still report the interruption there, and that holds when the terminal
  comes straight back. When the session first settles on a card instead, such as the one shown while its host is
  unreachable, the interruption becomes a window notice at that point, so if the host then comes back the user sees the
  notice rather than the terminal's own line about the upload. Either way it is reported once, which is what the amended
  SPEC.md asks for.
- **The notice shown when Don't show again could not be saved names "upgrade highlights".** The dialog itself is titled
  "What's new in Farhelm" and the Settings switch reads "Show what's new after updates". The landing's review suggested
  naming the switch. I left the wording, since the dialog's own Don't show again help, its close button and the Settings
  help already say "upgrade highlights", and changing it would mean republishing that notice's reference pictures. Say
  so if you want the switch's name instead.
- **Small things the landing's review noticed and left:**
  - For a host that is being set up and also has a warning, a screen reader hears the warning last, after the
    instruction to open Hosts for details, where it is easy to miss.
  - A session title made only of spaces still counts as a title, so an upload notice for such a session names a blank
    session instead of leaving the session out.
  - The screenshot test for the recovery cards puts the upload notice directly above one card, a layout the app never
    draws.

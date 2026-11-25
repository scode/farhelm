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

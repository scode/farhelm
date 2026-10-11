# Trash and host settings dialogs

## What this was about

Tall dialogs crowded the window edges. Host settings could scroll its header and Close controls away, and the trash's
global Delete All sat below the whole archive list. During a slow trash cleanup, disabled controls gave no indication of
which hosts Farhelm was still waiting for.

The agreed four-part stack gives all twelve modal dialogs the same vertical margin, pins host settings' header and
footer, moves the trash total and global actions into a pinned header, and shows deletion progress per host without
changing the helm or supervisor.

## Things you should know

Only the middle of host settings scrolls; its dismiss control, Close button and save-as-you-go note stay visible. An
exceptionally long host destination or version is shortened visually in the heading so it cannot consume the whole
dialog. The complete destination remains in Connection and the dialog's accessible name.

The trash has no footer. Its title, total, Delete All and Close stay visible while the body scrolls, with the header
wrapping at narrow widths. Global confirmation is first in the body. Opening it brings Cancel and Delete Permanently
into view, even when the host names make the warning taller than the body; the full warning remains available by
scrolling up. Cancelling with Cancel, Escape or Close returns focus to Delete All.

Confirmed cleanup says how many of the originally involved hosts are still deleting. Each pending host shows its own
deleting indicator until it answers, including on failure. A successful empty host disappears from the list; existing
preserved-entry diagnostics and uncertain cleanup errors remain. The final reply clears progress, refreshes the list and
returns focus to Close. Close and Escape remain blocked during cleanup. This is per-host progress, as chosen, rather
than a per-checkout stream.

The required tooltip checks also exposed missing hover text on the shared checkout-folder Save and Retry buttons,
introduced before this plan. A supporting repair was made while building the stack. The neighbouring text-field and
post-clone Settings plans then landed with that repair; the final rebase keeps their wording and removes our duplicate.
The four completed TODO entries are removed; the separate single-checkout delete and neighbouring host-dialog redesign
entries remain.

## Open questions and possible follow-ups

None required to complete this plan. No product or protocol departure required a maintainer decision.

## The PRs

- [#1860](https://github.com/scode/farhelm/pull/1860/changes) — shared modal margins.
- [#1862](https://github.com/scode/farhelm/pull/1862/changes) — pinned host settings controls.
- [#1863](https://github.com/scode/farhelm/pull/1863/changes) — pinned trash header and reachable confirmation.
- [#1864](https://github.com/scode/farhelm/pull/1864/changes) — per-host deletion progress and completed TODO removal.

All four remain draft; none was readied or merged by this executor.

## Checks run, reused and skipped

- Run after the final rebase: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`,
  and `dx build --package farhelm-ui --platform web --release` passed. `dprint check` on SPEC, TODO and the four
  fragments, `python3 releasing/check-changelog.py format`, and the isolated `python -B scripts/check-test-sleeps.py`
  passed; the checker inspected 283 delays with zero missing reasons.
- The updated complete tooltip and text-field walk passed 4/4 after rebase on the combined build, run
  `e2a1e758-c111-4ac6-b55c-4ac4bea217a0`, using `npx playwright test tests/tooltip-coverage.spec.ts` through the
  recorder. It covers the new post-clone Settings field and upstream folder tooltip wording under this stack's margins.
- Modal geometry, templates and launcher Enter coverage passed 84/84 on Chromium and WebKit, run
  `2d61fd69-b3d8-48a6-a44d-01915b1f81f7`. Corrected template geometry separately passed 4/4, run
  `84ce4850-1590-4358-8a73-a50feef3caaf`.
- Host settings controls, including a 1024-character destination, passed 4/4, run
  `9835432d-1e33-48cf-9e49-ba4764517519`. Twenty broader host-settings and margin cases passed in run
  `6e2e6813-420c-4202-9050-8f5d2dd589d5`; its two tooltip failures were the pre-existing missing Save text, not a suite
  pass. Complete tooltip coverage passed 4/4 after repair, run `d104db1c-3c33-46c5-af58-1aa26b850c43`. Retry's
  refused-load branch was source-reviewed only.
- Trash header exposure, confirmation focus, real archive counts/cues, preservation behavior and margins passed 12/12,
  run `acb51011-8a62-4584-a8f7-125574a70612`. Corrected geometry separately passed 4/4, run
  `a1d75eb8-9707-46fc-9f28-7e637f79953e`. Real trash and pinned-header regressions passed another 8/8 with progress
  present, run `dd38444d-1543-40d0-8e98-cc7c5abd2c70`.
- Final held-reply deletion progress passed 8/8, run `d2fdc1b0-ab2e-4bf5-9db2-c13e5ef412b6`: success and failure, wide
  and narrow windows, on both engines. It proves both hosts pending, one host pending, finished results, disabled
  controls and Escape, completion focus, and a later one-host retry. The pending host's painted indicator and pinned
  controls are checked together before capture; narrow Chromium/WebKit and representative wide captures were inspected.
  Earlier progress coverage passed 8/8 before that capture addition, run `00772f06-2e71-4cfe-a207-88ec5247144d`.
- Every browser selection used the recorder with one worker, zero retries, pinned tmux, and both engines. Before/after
  screenshots were inspected at ordinary and narrow widths. Old-asset regression failures and same-session
  fixture/assertion mistakes are retained privately: command mode was initially omitted from the template fixture, and a
  strict body boundary rejected 0.125 pixels of scroll alignment. Both were corrected and rechecked. No latent flake was
  established.
- The final careful rebase read the landed text-field correction and post-clone Settings changes, their amended
  specifications and tests, and the queue landing notes through main `e6fc84cc`. Their new command field uses Settings'
  existing explicit-save layout; the updated tooltip walk was the concrete integration risk covered by the fresh
  combined-build run. The TODO conflict preserves both plans' completed-entry removals and this plan's four removals. No
  design conflict remained.
- Earlier modal, template, launcher, host-settings and trash checks cover the respective unit diffs above main
  `29f83fd9`; the final progress captures cover PR4 above its then-parent `9746afa9`, including the dirty
  implementation. Those contracts remain intact after the rebase, so their runtime evidence is reused. The rebased
  progress implementation and tests compare byte-for-byte with the tested unit diff. Combined-build checks passed. No
  full Rust or browser battery, desktop runtime, installer, provisioning or release checks are warranted: this plan
  changes no desktop-only path, backend, protocol or installation behavior, and the focused selections cover its
  geometry and lifecycle. No hosted CI or website deployment was requested.

## The review gate's outcome

Implementation and investigation stayed with the executor under no-workhorse. Each unit received the required fresh
native `gpt-6.1-sol` high source review, with the full test-authoring checklist for test changes. Reviews found and
resolved the long-heading footer risk and a center-only clipping oracle; a final unsupported integer-scroll comment was
qualified. The tooltip supporting repair received a scope review. PR4's final source followup accepted the pending-host
capture refinement with no findings. No source findings remain, and the executor's selected runtime and source checks
passed.

Fresh native `gpt-6.1-sol` medium readers accepted each commit/PR message. Required resume reconciliation and the final
report cold read use native readers on the executing session's inherited model. Reviewers did not independently run
tests or inspect screenshots; those are the executor's evidence. Actual native model attribution and usage counters were
unavailable. Private galaxy-brain session `babea5b4-d5fc-4824-9a71-faa36bdd96dd` retains the routing and review events;
the working log beside the checkouts records its exact location and private run paths.

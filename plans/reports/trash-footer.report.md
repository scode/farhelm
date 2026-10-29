## What this was about

Trash kept Close, Delete all and deletion progress in its header while other dialogs used a footer. The maintainer
revisited that exception and chose the shared layout, accepting the smaller archive viewport on phones. Trash now has
the shared heading with its can, total and close control, plus a pinned footer with progress, Close and Delete all. The
global deletion question sits directly above that footer; host-specific questions stay with their host.

## Things you should know

Deletion rules are unchanged: one checkout at a time per host, parallel hosts, the existing confirmation guards,
progress and listing recheck. Both close controls and Escape remain unavailable during deletion. Initial focus stays on
footer Close; keyboard traversal follows the visible header, archives, question and footer order.

Phone actions use the shared full-width stack. In the 360×300 stress fixture, the archive viewport measured 80px idle
and 36px with a long global question (48 archives on four hosts); the separate busy fixture measured 69px (three
archives on one host). Both engines agreed. The small remaining list is keyboard-scrollable. At the ordinary 390×520
capture size, the complete staged question and both answers are visible together. Very long questions keep their own
bounded scroll area, so their text can be reached without displacing the footer. From those ordinary phone images, the
list is about 300px idle, 274px busy and 180px with the question open; these are image measurements, unlike the exact
browser measurements above.

Visual review caught the first ordinary-phone layout hiding all warning text while retaining unnecessary list height.
The correction gives the question priority at ordinary heights, retains shrinking at short heights, aligns its content
with the footer, and dims the new header close while busy. The session-list documentation now says Delete all is in the
footer. The original TODO entry is removed and a changelog fragment is included.

## Open questions and possible follow-ups

The published Trash documentation screenshot still shows the old layout. Refreshing and publishing it remains a separate
request, as the plan required. No unresolved implementation decision remains.

At 390px while deleting, a row's disabled Delete button wraps and its top sliver appears at the list's lower scroll
edge. Everything remains reachable by scrolling. Visual review also noted the existing extra bottom gap after the last
host card. These were left unchanged to retain the shared inset and existing variable-height archive rows/card spacing;
a separate spacing refinement could revisit them.

## The PRs

- [#2787: Trash uses the shared header and footer](https://github.com/scode/farhelm/pull/2787/changes), draft at
  `81bf3af7c125dc1030ccbb50bba3450bc5c7c2a9`.

## Checks run, reused and skipped

Ran the selected browser command through the recorder with pinned tmux, three workers, zero retries and both engines:
`npx playwright test tests/trash-dialog.spec.ts tests/modal-layout.spec.ts tests/github-checkouts.spec.ts tests/popover-width-movement.spec.ts --grep trash`.
Run `c7c7be55-2e8e-4b70-9757-62b074718def` passed all 50 cases, 25 per engine, without skips or incomplete evidence. It
covers geometry, keyboard entry and rings, cancellation, busy explanations/progress, real archive cleanup and Trash
count effects on open menus.

The final web build passed. Focused staged captures through
`scripts/docs-screenshots.sh --only visual-review --no-build` passed in run `f2574be2-80a4-4995-985f-7884efc5b585`: all
16 final images, three tests passed and one unrelated approval test intentionally skipped. Before images reuse run
`775b7818-58cf-4095-9e36-ecee6b5af065` from main's unchanged product code. Captures cover idle multi-host Trash, global
confirmation, held deletion progress and Rename for comparison in Steel and Steel Light, wide and phone. Held requests
end in the existing staged refusal; no staged deletion succeeds.

Reused UI Rust coverage from run `9f817b7f-d166-4a56-8ffa-c52b1938bcfd`: 805 nextest cases passed, zero skips, complete
output and no runtime skip marker. Its UI Rust source is identical to the implementation in `c3fb7498`; subsequent
corrections changed CSS, browser checks and specification prose only. The same Rust source passed
`cargo clippy -p farhelm-ui --all-targets -- -D warnings` and `cargo fmt --all -- --check`. Changed Markdown formatting,
fragment format, instruction-link checks and the test-delay checker passed; the latter inspected 322 delays with zero
missing rationales. Instruction-link checks retained two pre-existing unrelated heading warnings.

Retained development failures: browser run `d0c8eca9-d06a-4b12-aba5-be7652b5212c` had 42 passes and eight failures from
a stale test variable and clipped short-window answers; narrow corrected runs passed before the clean 50-case run.
Capture run `b857d0f8-82e7-449f-b92c-0e5c566f5977` failed an incorrect whole-content bounding-box assertion; its
replacement checks the scrollport and complete answer edges, with successful recaptures. These are retained failures,
not passing evidence or newly established latent flakes.

Skipped the full workspace/browser batteries, installer/provisioning and native desktop execution because this changes
Trash presentation and preserves deletion logic; the focused cases exercise the affected real behavior on Chromium and
WebKit. Main advanced only with another plan's block and queue bookkeeping; its entire diff had no interaction, so no
rebase or additional runtime run was needed.

## The review gate's outcome

The exact fresh gpt-6.1-sol/high code reviewer found two test issues; both were corrected and rechecked, including the
final CSS and keyboard/capture assertions, with no remaining findings. Fresh scope reviews kept the corrections local
and rejected a zero-basis layout that would collapse natural short-content sizing. Commit and PR wording passed its cold
read.

The exact fresh Opus 5.5/high visual reviewer inspected all 32 before/after images individually; the delivery audit
confirmed 32 successful pixel deliveries and zero truncated text reads. The final review found no blocking issue and
confirmed the corrected warning, alignment, disabled close and shared frame in both palettes and sizes. Its remaining
low-severity scroll-edge and card-spacing suggestions were declined as described above; no required behavior was
omitted.

Two pre-existing visual suggestions were deliberately declined: unifying filled and neutral disabled buttons would
change the shared design, and painting a ring after pointer-opened confirmation would change the shared focus-visible
policy. Native keyboard traversal still produces the visible ring and passes on both engines.

### Landing

Landed as built. The checkout Trash dialog now uses the shared dialog layout you chose: its heading carries the can, the
total and a close control, and Close, Delete all and deletion progress sit in a pinned footer, with the global deletion
question directly above it. This reverses the pinned-header exception decided on your behalf for the Steel dialogs plan,
and removes that "Revisit" TODO entry. On phones the archive list is shorter.

- #2787: put Trash's actions below the archive list.

#### What landed since the stack was based, and whether it interacted

Only a TODO entry landed since the plan was based; nothing interacted. A fresh-context reviewer checked that no test or
screenshot spec outside the diff relies on the removed header, and that deletion behaves as before: one checkout at a
time per host, the same confirmations, Close, the new header close and Escape all blocked while deleting, and focus
starting on footer Close.

#### Fixes made at landing

None.

#### Things you should know

- The published Trash screenshot in the docs still shows the old header layout; refreshing it is a separate request, as
  the plan said. The screenshot spec for it has a comment that still puts the delete buttons at the top, and its callout
  may overlap the new footer; check both at the next screenshot refresh.
- SPEC.md's description of Trash's Tab order leaves out the global deletion question between the archives and the
  footer, which SPEC_impl.md includes; it is right while no question is open, just less precise. A one-phrase follow-up
  at most.
- The changelog draft does not mention the new close control in the heading.

#### Checks

This round landed only this plan. On the stack as delivered:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, the check that every test delay states its reason, `dprint check` across the
  repository, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the docs website
  build, the workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `e30ffa1a-b7c6-4e25-bab6-c70846ad9fd1`: all 805 passed;
  its desktop-build tests, run `45ef18e9-b8d0-4955-ba8c-d0eb690a3f73`: all 988 passed. The UI's JavaScript and
  stylesheet tests (`node --test` in `crates/farhelm-ui/js-tests`), run `a2e096b8-215b-4585-b860-a7d6a8e36d23`: 358
  passed.
- `npx playwright test` in `e2e/` with `tests/trash-dialog.spec.ts`, `tests/modal-layout.spec.ts`,
  `tests/github-checkouts.spec.ts` and `tests/popover-width-movement.spec.ts`, on Chromium and WebKit, three workers
  each with its own helm and supervisors, no retries, run `26524abb-8128-48fd-a1cf-a4d7b39d01f2`: all 134 passed.
- Reused: none. Skipped: the rest of the browser suite (only the Trash dialog changed, and these specs cover it and its
  callers), the rest of the Rust workspace (only the UI crate changed), the desktop smoke tests (no desktop-specific
  code changed) and the native Mac app (not runnable here).

The report's checks were the Trash cases of these specs; this landing ran the four spec files whole. Nothing in the
executor's report became untrue at landing.

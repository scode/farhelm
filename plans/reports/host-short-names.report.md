# Host short names and sidebar captions

## Since the last review

Both existing draft PRs changed; none was added or dropped. The naming API now falls back to the full display name when
a host has no usable short form. The rail now expands only from its narrow hover area or keyboard-visible focus, and its
widened names and background take no pointer events. The width value is declared with shared root geometry. The
specification and changelog describe that behavior, including the bottom bar retaining the full host name.

The original landing failures were reproduced in the two stylesheet checks before correction. The browser coverage now
also tests a mouse host choice releasing the list, crossing into a displayed name collapsing the rail, clicking Sort
through a keyboard-expanded rail, and scrolling from bare rail padding and gaps.

The second PR also repairs the visual-review test's navigation after an uninstall refusal: that helper opens Hosts, so
the test now returns through the Sessions control before opening Templates. It keeps the refusal and host-identity
assertions and uses the existing panel-readiness helper. It also waits for the selected template's Name field to receive
focus, then dismisses its help with an ordinary click in empty dialog space before the neutral editor picture. Hosts
settings now asserts all three named choices, including the newly landed automatic-update option, enabled and off.
Ordinary session-menu pictures await opening focus and hover the read-state item before capture, so help from a prior
keyboard interaction does not cover them. No product behavior or timeout changed.

## What this was about

Host icons alone gave little clue which machine each represented. The maintainer chose short captions under them, with
full names shown beside the icons when the rail expands. They explicitly rejected automatically storing aliases: an
existing alias remains the user's choice; otherwise the shared short-name rule uses `Local`, the first hostname label,
or a whole IP address. Session rows use that same short form for a known host and retain the supplied full name when the
current host list has no matching entry.

## Things you should know

The narrow rail remains 56px wide. The expanded names are display only: select a host through its avatar, rather than
its widened name. The display does not move the list, icons or footer, and it cannot intercept list controls. Mouse
focus alone does not leave it expanded. Keyboard-visible focus can keep the names shown while a pointer reaches a list
control beneath them.

The narrow hit area lives inside the original scrolling navigation, including its padding and gaps. This preserves
native scrolling without forwarding wheel events or adding application state. A source review caught an intermediate
outer-wrapper version that would have missed wheel gestures in those gaps; that version was corrected before browser
validation. Unknown host kinds without destinations retain their full name in captions and rows. The Hosts panel,
details, dialogs, tooltips, agent output and bottom bar retain full display names. Aliases, host resolution and wire
data are unchanged. Equal short names remain allowed.

## Open questions and possible follow-ups

No new maintainer decision is needed. The separate TODO asking the maintainer to revisit display-only widened names
remains on main, as recorded by the landing monitor; this round implements its decision and does not remove that item.

The earlier WebKit light-theme template-editor pictures show dark `Codex` text on a charcoal Agent type selector, making
that field hard to read. This is separate from host naming. Its cause, first appearance and persistence on current main
were not established; a current light-theme check is a possible follow-up.

## The PRs

- [#2810: shared short host names](https://github.com/scode/farhelm/pull/2810/changes) — existing foundation PR, amended
  with the empty-result fallback and its specification and unit test. Head `6862a3e18462e7c439e9814a256ab1bc9e3d2769`.
- [#2811: sidebar host labels](https://github.com/scode/farhelm/pull/2811/changes) — existing presentation PR, amended
  with the pointer, focus, scrolling, token and browser corrections, plus the visual-fixture repairs. Head
  `6e006267e5ee1fa27eb9ddb8b9f587d14af7967f`.

## Checks run, reused and skipped

Current source is rebased onto `7ca515bb`, including the terminal toolbar, menu-highlight and host-update work that
landed during execution. The entire incoming diff and actual resolution were audited. The specification keeps the new
automatic-update setting and pointer-focus menu behavior alongside this plan's naming rules. The TODO resolution removes
only this plan's two entries and keeps main's independent removals and rail reconsideration entry. Shared update-request
ownership, the update card/banner and their menu-movement handling remain intact. No design decision was needed.

Run this round:

- `node --test` in `crates/farhelm-ui/js-tests`: 372 passed, no failures or skips, run
  `4d8d8668-3153-485f-8b26-1cff3dfc5c2e`. The expanded-width token is explicitly shared geometry; palette-role checks
  still require their color, opacity and elevation roles independently.
- `cargo nextest run -p farhelm-proto -p farhelm-ui --lib -E 'test(host_name::tests::) | test(host_short_name)'`: five
  passed, 1009 selected out, run `6f148cbe-e91f-4f5e-8cf5-71bba5947f44`. Pinned nextest and tmux, four slots, zero
  retries; no runtime substrate skip. This covers the raw rule, aliases/local kind and the new full-name fallback.
- `cargo clippy -p farhelm-proto -p farhelm-ui --all-targets -- -D warnings`: passed, run
  `9e8842a8-64e5-40c1-abbc-3acf2fd864ed`, after the late rebase.
- `dx build --package farhelm-ui --platform web --release`: passed with the corrected inner hit area, run
  `7ec4d2f6-b8e9-420f-a449-43cec3853cf7`, rebuilt after the late rebase. Matching backend and fixture build
  `cargo build -p farhelm -p farhelm-fixtures` passed, run `fe1a16bd-2504-4a09-9e33-c76927483f87`.
- Combined-tree browser integration: 46 passed, 23 per engine, with no failures, skips, flaky or unstarted cases. The 28
  host-name/update/menu cases passed in `4f711416-09f9-4f26-b533-b3863ac3d984`; 16 rail/menu/alias/host-sort cases
  passed in `f2582fb8-a34e-425b-a8ba-b1d969109c99`; the two Hosts-panel cases passed in
  `08fb1a40-bd51-4c15-a032-136828046e5a`. A spacing typo in the selector omitted that last case from the second command;
  the separate command completed the intended coverage. Three workers, one owned stack each, pinned tmux, zero retries;
  all reports and output complete and all stacks cleaned up. The ten current naming/update screenshots were inspected.
  Some hover help obscures part of a picture, including most of the short long-name card picture; scrolling, pointer
  access and action reachability are supported by the functional assertions, not inferred from those stills.
- Current combined-tree ordinary dark-wide docs capture: Chromium passed staging and the ordinary visual case, run
  `5565c0c5-612b-4afb-a1d1-e03ba911e6eb`, with no failures, skips, flaky outcomes or global errors. The same default
  sequence reaches the new three-choice Hosts setting and the repaired menu, Sessions and template steps. WebKit passed
  the same two cases, run `eac7f7c7-76f8-4b70-afcf-8a3d09f0c105`, with no other outcomes or global errors. Generic
  recording retains source and complete output with normal cleanup; its private Playwright JSON is inspected directly
  because this mode does not supply validated browser counts. All 58 current Chromium images were inspected through five
  sheets, with 15 key originals enlarged; no blocking visual defect was found. The three Hosts choices, menu focus
  comparisons, template editor and full bottom-host names are legible and bounded. All 58 current WebKit images were
  likewise inspected through five sheets and 15 original-detail views, with no visual blocker. Together these 116
  pictures establish current dark-wide appearance; the earlier full sets supply the reused theme/width coverage
  described below. The old light-theme selector observation remains unresolved.
- Full `filters.spec.ts`, `sort.spec.ts`, `hosts-panel.spec.ts`, `host-short-names.spec.ts` and `sidebar.spec.ts`: 324
  passed, 162 per engine, zero failures, skips or flaky outcomes, run `2046f20f-b4d7-417c-91f9-8b06a82ecdef`. Three
  workers with one owned stack each, pinned tmux, zero retries. The report and output are complete, and all three stacks
  cleaned up. Collapsed and expanded naming screenshots from both engines were inspected; truncation, full names and
  destinations, fixed icon/footer positions and fallback labels look correct. The pre-existing New-button focus tooltip
  is visible in those captures.
- Full docs `visual-review.spec.ts` on the pre-rebase product: private configs inherit the maintained staging/fleet
  dependency and run Chromium and WebKit serially. Generic recording is used because strict browser recording rejects
  custom configs. Chromium passed its four visual cases and staging, with one intentionally skipped focused approval
  case, run `4b6a9a51-7075-4a0c-8923-b1e7aa0b0ed8`. Its 244 images were inspected across 21 contact sheets, with 22 key
  originals enlarged; no blocking visual defect was found. Those images cover the collapsed rail and full bottom-host
  display; the dedicated naming images cover hover expansion. WebKit passed the same four visual cases and staging, with
  one intentional focused approval skip, run `f59986dd-4724-422a-9cb9-9eb5113340bd`. All 244 WebKit images were
  inspected through 21 sheets, with 40 originals enlarged. No naming-layout blocker was found; the separate selector
  observation is described above. These full captures supply unchanged theme/width coverage; the current combined
  dark-wide captures above cover the newly landed Settings control, menus and changed fixture steps.
- `cargo fmt --all -- --check`, scoped `dprint check`, and `python3 releasing/check-changelog.py format`: passed. The
  pinned isolated source-delay checker inspected 327 delays; none lacks a rationale. No new delay was added.

Retained unsuccessful checks this round: the pre-correction token selection failed its two named checks in
`5314a623-9609-4597-af08-225ca63dbe87`; the first full JS run then failed one palette check because the new geometry
token also needed the shared-token inventory, run `acf9ae54-8cfb-43fb-a0ca-32c39bc03f61`. Both failures are retained;
later passes do not erase them. An initial web invocation used a wrong relative recorder path and stopped before a
recorder or build started; the absolute-path invocation corrected it. The intermediate outer-wrapper build passed but
was superseded by the corrected build above, not used as final browser evidence.

The first full Chromium visual attempt, `75589935-1af6-466c-8256-fe398c3a0490`, timed out in its dark-wide case while
trying to click the hidden Templates control with Hosts still open. The unchanged test on main had the same missing
navigation step. After the concrete cause was identified in its retained trace, the remaining run was interrupted: one
staging pass, one timeout, one interrupted case and three unstarted cases. Its evidence remains retained, including the
forced group cleanup. The focused corrected stage and dark-wide retry passed both cases, with normal cleanup, run
`3ec10784-63da-474f-bebe-95e19c6e838c`; that diagnostic does not replace the full visual runs.

The first full WebKit attempt, `d97f1f29-a616-4b9a-a40d-880766f05b2b`, failed the ordinary template-editor picture's
hidden-tooltip assertion: selecting a template had focused its Name field, whose help remains visible when the pointer
is merely parked elsewhere. It was stopped after identifying this separate fixture premise: one staging pass, one
failure, one interrupted case and three unstarted cases, with complete retained output and forced group cleanup. The
original test lacked explicit focus-tooltip dismissal at that point. The correction waits for Name's value and deferred
focus, then uses the existing hit-tested gap helper; the original hidden-tooltip assertion remains. The focused WebKit
staging and ordinary dark-wide retry passed, run `8ba02a31-85b9-484a-a402-6a66208e1cf7`, with normal cleanup. Full
WebKit passed as listed above; current combined-tree confirmation is reported separately.

The first combined Chromium capture, `c696261d-e6b8-4daf-8fab-27136cd4741a`, failed because the Hosts settings fixture
still expected two switches after main added the automatic-update choice. The correction checks exactly three named
choices, each enabled and unchecked, without changing a preference. The next attempt,
`61b37f8c-697d-41ae-9193-9dd1793ba859`, failed earlier at the ordinary Mark read menu picture because Rename help was
visible after preceding keyboard interactions. The correction waits for opening focus, then uses real pointer hover over
the read-state item, which has no tooltip, and observes the focus handoff. All four ordinary menu opens use this same
local preparation; intentional keyboard-focused pictures and the hidden-tooltip assertion remain. Both attempts ended
naturally with one staging pass and one failed case, complete retained output and normal cleanup. Neither is counted as
a pass or erased by later results.

Reused broad coverage: the 324-case browser pass and full visual runs above cover the unchanged naming behavior across
both engines, themes and widths before the late rebase. Chromium's full visual tree was `2faff1a5`; WebKit's was
`5c506338`, whose only additional change was the explicit Name-focus/gap preparation. The audited incoming work adds no
rail palette or width breakpoint. The fresh 46-case combined selection exercises its prompt geometry and menu focus; the
fresh ordinary dark-wide captures cover the new Hosts setting and current menu rendering. The earlier backend build
served only the pre-rebase browser checks; the combined backend and web bundle were rebuilt as listed above. The
unchanged PR1 message's original wording cold read remains applicable.

Skipped: another full Rust workspace/UI run, desktop, installer, provisioning and unrelated browser specs. This round
changes naming presentation and rail interaction; the focused Rust checks and required browser/visual selection cover
those risks. The earlier landing's shallow-pause and hung-save failures were already recorded known flakes, not fixes in
this plan. The three decision-named unrelated WebKit popup/feedback cases are not part of these five specs; no new
failure of them has triggered the conditional main comparison. No CI dispatch was needed; CI is manual only and would
not add the required browser coverage.

## Review gate

The required single fresh-context `gpt-6.1-sol` at high effort passed PR1. Its separate PR2 review found the scrolling
gap described above; the corrected delta and real-wheel regression then passed the same reviewer. A required own-model
scope reassessment found the inner narrow hit area to be the smallest coherent fix, with no extra mechanism or scope
expansion. A second scope reassessment confirmed that the visual fixture uses supported controls and existing helpers;
it caught the need to await Name's deferred focus before dismissing its help. Further bounded scope checks confirmed the
three current host choices and the local native-hover preparation for ordinary menus. The same required PR2 reviewer
passed all four visual-test corrections. Commit wording was cold-read independently; the revised PR2 text's claims held
up.

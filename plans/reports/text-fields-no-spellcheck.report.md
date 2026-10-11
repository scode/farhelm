# Text fields without browser correction

## What this was about

Templates and several other text fields still allowed spelling marks and automatic corrections, which can change
commands, paths and deliberately chosen names. The maintainer chose to turn correction off everywhere, including the
feedback message. The app now declares that policy on every text-entry field in its shared web and desktop UI.

## Things you should know

Eleven incomplete fields were corrected; an independent source inventory found 24 app text-entry sites and no missed
site. The feedback contact field keeps email autofill, the plan's explicit exception. The terminal's own input policy is
unchanged. Attributes stay inline with the fields, following the existing pattern rather than introducing a general
input component.

The browser regression check shares the existing dialog walk but uses a separate text-field oracle. It checks hidden and
disabled mounted fields as well as visible ones, includes inputs without an explicit type, and proves that removing each
of the four attributes from either an input or a textarea is detected. Each visited surface must expose its own expected
fields. The old launcher-only check was removed after its meaningful coverage moved into this walk. The completed TODO
entry is removed and a fixed changelog fragment is included.

The existing hover-help check exposed missing help on the checkout-folder Save button. Save and its load-error Retry
button now explain their actions. The separately delivered post-clone Settings PR #1858 fixes these same two controls
with equivalent wording; landing should keep one copy of that help. Its new command field already declares all four
opt-outs, according to its delivered report; that field is not part of this plan's tested main revision.

Browser preparation unintentionally pruned older versions from the shared browser cache. This did not change tracked
files or product state, but a checkout using an older Playwright version may need its browsers reinstalled. Later
validation used a separate matching browser image and did not touch that shared cache again.

## Open questions and possible follow-ups

None required to complete this plan. Both engines verified rendered opt-out attributes; this does not establish how
every OS spelling or correction service behaves in a native desktop window. A native reproduction that still corrects
text would need separate investigation before adding a broader webview setting.

## The PRs

- [#1859](https://github.com/scode/farhelm/pull/1859/changes) — turn off corrections in every app text field; draft head
  `0db1b1b57bee2eaaec33ae9e855cc514968251ea`, based on `546acba0`. The executor neither marked it ready nor merged it.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` passed, including after the Save/Retry help correction. These cover
  formatting, Rust linting and shared desktop component compilation.
- `cargo build` passed for the real helm and fixtures; the release web build
  `dx build --package farhelm-ui --platform web --release` passed and was rebuilt after the hover-help correction. Those
  are the artifacts used by the focused browser run.
- `dprint check TODO.md`, `python3 releasing/check-changelog.py format`, and the isolated
  `python -B scripts/check-test-sleeps.py` passed. The delay check found 283 delays and zero unannotated delays.
  Changelog format was rerun after adding curation context about the ancillary hover-help fix.
- Through `scripts/record-test-run.py --runner playwright --kind development --tmux required`, child
  `npx playwright test 'tooltip-coverage\.spec\.ts'`: four passed, two per engine, zero skips, failures, flaky or
  interrupted cases. Run `fb3db704-b3bc-43fe-b73d-cda968616bb0`, one worker and zero retries, Playwright 1.64.0 on its
  matching Ubuntu Noble browser image with pinned tmux 3.7c. It covers the complete dialog walk, text-attribute probes,
  all changed fields and retained hover assertions. The optional SSH fixture was unavailable; these local-host cases do
  not require it.
- Earlier retained runs are not passes: `0fa99b99-7ae7-436f-8ec6-b92a488dd1f7` failed before assertions because the
  build container lacked browser libraries; `e86032ad-6a94-45f2-9c51-880554c5d771` failed before assertions because the
  recorder removes browser-path overrides; `7a7c0d67-db48-4871-9a3d-6f68e09e6b4b` had two sidebar failures for the
  missing Save hover help and two session-view passes. The matching image, its default-cache alignment and the
  hover-help correction resolved the respective failures. Evidence remains private with the plan's working log; no
  latent flake was established.
- Builds and checks from the claim base were reused where code stayed identical. Runtime evidence covers the
  implementation on base `6b7deac3`; the final rebase to `546acba0` added only another plan's queue delivery/report.
  Earlier upstream changes were unrelated TODO/queue bookkeeping, release-test operation notes and a host-dialog
  redesign plan. Every landed diff was read; no product/spec contract changed and no new field reached main. The rebases
  preserve tested product/test code, so those results and builds cover the pushed head. The delay check is also reused
  because the subsequent edits add only button-help strings and curation context, with no delay or fixture changes.
- Broader template/feedback specs, the full Rust/browser suites, desktop runtime, installer/provisioning and release
  gates were skipped: this change does not alter field selection, save behavior or lifecycle, and focused browser
  rendering plus the compile checks cover its concrete risks. No hosted CI or deployment was triggered.

## The review gate's outcome

Implementation and investigation stayed with the executor under the no-workhorse rule. The required fresh native
`gpt-6.1-sol` high source reviewer found no actionable findings, independently surveyed every field and applied the full
test-authoring checklist. It verified the final command-tab setup correction and the ancillary Save/Retry help
additions. A separate documentation pass covered all touched files. Source review did not execute tests or verify native
OS correction behavior; runtime evidence above is the executor's.

The fresh native `gpt-6.1-sol` medium wording reader accepted the commit and PR text. Required resume reconciliation and
delivery-report cold reads use fresh native readers on the executing session's inherited model. Requested routes are
recorded; actual native model attribution and usage counters were unavailable. Private galaxy-brain records remain under
session `5c9810ab-dcb4-4a62-9a79-bfb239f2d810` in the configured state directory; the working log beside the checkouts
records its exact location.

### Landing

Landed on 2026-10-11 (UTC) as #1859. The rebase met no conflicts.

#### Review before merging

No findings. The review checked every text field in the UI: all 25 have browser corrections off, the feedback contact
field keeps email autofill as the plan decided, and the terminal's hidden input is correctly left alone. The new browser
sweep cannot pass vacuously: it requires a minimum number of fields on each surface it visits and catches each of the
four settings being removed. One note left as is: the changelog fragment's second paragraph describes how the fix was
found, which release curation will drop.

post-clone-in-settings, which landed on top, adds a post-clone command field; it has corrections off too. Both plans
added hover help to the checkout-folder field's Retry and Save buttons with different words; the landing kept
post-clone-in-settings' wording, which matches its own field. No test checks the exact text.

#### Checks

These two plans were landed together, text-fields-no-spellcheck first with post-clone-in-settings on top.

- Run now, on the two stacked, after the fixes listed in post-clone-in-settings' notes: `cargo fmt --all -- --check`,
  `dprint check`, the changelog lint, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop`; the supervisor,
  helm, UI, protocol and CLI unit tests with the GitHub-checkout end-to-end tests, through the recorder with pinned tmux
  3.7c, four slots and no retries (run `817f0fdc`, 2895 of 2895); the UI JavaScript tests (230 of 230); the docs website
  build; and on Chromium and WebKit with one worker and no retries the settings, tooltip-coverage, terminal,
  github-checkouts, github-checkout-composer, feedback, templates and quick-switcher specs (run `ccef1907`, 270 passed).
- Between that run and the merges, main changed only release-testing documentation.

Nothing in the report above was made untrue by the landing.

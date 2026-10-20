## What this was about

Three surfaces could hide controls or explanations beyond reachable scrolling: waiting approval cards in a very short
window, long host-outage explanations, and host setup, removal, uninstall or refusal dialogs. The maintainer chose to
keep controls and explanations reachable at every allowed window size. All three outcomes are implemented, in order,
within their separate 100-line gates: 73, 100 and 100 added/deleted code, test and documentation lines. Bookkeeping,
feedback removal and changelog fragments are excluded as agreed. No outcome was discarded.

## Things you should know

Approval cards keep their ordinary placement below session controls. When that leaves less than 44px, they temporarily
use the viewport so an answer remains reachable. This can cover session controls in a very short window. Both engines
reproduced the previous zero-height approval region at a 48px viewport; the final regression scrolls to and denies the
same live request.

Outage explanations now scroll from their beginning, and the text-only host card has a named keyboard stop. A fitting
card remains centered. When session notices leave less than 80px of terminal height, the card uses the viewport above
the sidebar bar. Review showed why a window-height cutoff was insufficient: a refused action or trust notice can use the
terminal's space even in a taller window. The fallback follows actual terminal space and its observer is released with
the session view.

Host action headings stay on one line while the full destination remains in the plan or body. Their bodies retain
readable space, and outer scrolling exposes the header and answers when these cannot fit together. Ordinary windows
retain pinned answers around the scrolling body. The fallback is scoped to host action dialogs. App settings, feedback,
welcome, SSH help and tunnel consent retain their existing scrolling behavior.

## Open questions and possible follow-ups

No decision is required for these outcomes. Browser checks cover Chromium and WebKit, including 48px approval/reconnect
and 160px host-dialog viewports. They do not establish native desktop runtime or actual host installation coverage; the
owned browser stack uses simulated provisioning actions. No live installation was changed.

## The PRs

- [#2406](https://github.com/scode/farhelm/pull/2406/changes): keep waiting approvals answerable in short windows.
- [#2411](https://github.com/scode/farhelm/pull/2411/changes): keep outage explanations reachable; based on #2406.
- [#2419](https://github.com/scode/farhelm/pull/2419/changes): keep host dialog content reachable; based on #2411.

## Checks run, reused and skipped

Browser commands ran from `e2e` through `python3 scripts/record-test-run.py` in Playwright development mode, with
Chromium and WebKit, one worker, zero retries and required pinned tmux. Final runs have complete source and result
reports, complete untruncated output and cleanup, and no skipped, flaky, interrupted or unstarted cases or global
errors. The owned local and SSH hosts actually connected.

- `npx playwright test approval-layout.spec.ts`: 26 passed in `43270a40-680a-4c4e-8385-2b98bcf8fffc`, covering ordinary
  placement, modal interaction and the new short-window answer. Before the fix, the first selected run
  `7d2e98f8-4d16-4ecc-9079-125dc1b70cfd` passed both engines at larger short sizes and did not reproduce the issue. The
  narrower boundary run `ae075500-613e-4284-a41d-0a5711c6c7a0` deliberately failed both engines at 48px because the
  region had zero height. Both observations are retained separately.
- `npx playwright test 'terminal-(multihost|reconnect)\.spec\.ts' -g 'stale-session-metadata-view|manual-reconnect-is-offered-during-the-first-rung'`:
  four passed in `97e9be5d-4c4f-4ebf-be51-cbce47cdd8eb`. A real rendered refusal band exhausted the terminal at 240px;
  the heading's paint/hit checks and native keyboard/wheel endpoints passed. After adding explicit fallback readiness to
  reconnect, its exact selected case passed both engines in `dd10b483-2c00-4ee6-9f40-f15f22d1018e`.
- `npx playwright test terminal-multihost.spec.ts -g 'host-remove-dialog-fits'`: two passed in
  `5bc66d4e-f3ac-415f-9f43-bf339a950884`. The 1024-character host heading stays bounded, its full destination is
  retained, both scrollports respond to native input, and every answer and dismiss control passes painted hit tests.
- `npx playwright test '(provisioning|terminal-multihost)\.spec\.ts' -g 'host-setup-layout|a refused Retry opens a host dialog|uninstall-remote-host'`:
  eight passed in `07eccd05-ecbe-4869-8bc0-08bd26d618e5`. Setup covers desktop and phone layouts, native inner-plan
  scrolling at 420px and outer-card keyboard scrolling at 160px. Refusal and uninstall cover outer wheel scrolling.
- The initial ten-case dialog run `d6c4a212-d616-45d5-ab5a-49d2a63f236c` had eight passes and two removal-test failures.
  Its assertion incorrectly demanded that an entire paragraph fit simultaneously inside a smaller scrollport. The
  corrected test proves the scrollport is painted and native Home/End reaches its flowing text. The failed run,
  screenshots and traces are retained; it is not used as a passing gate or recorded as a latent flake.
- Matching `cargo build` and Dioxus 0.7.10 release web builds passed. `cargo fmt --all -- --check`, changed Markdown
  dprint checks, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and the isolated
  `python -B scripts/check-test-sleeps.py` passed. The sleep checker inspected 304 delays with none missing a reason. An
  initial unqualified `dx` invocation resolved to another tool and failed before building; the actual Dioxus executable
  was then used successfully throughout.

The approval run covered base `88650419` plus its final patch; outage runs covered `2efbbb83` plus the outage patch;
final dialog runs covered `524c3b8b` plus the corrected dialog patch, with source fingerprint
`b1d95d6d9d526dc8078dd8f4faf6631115ace7702ca89fe7bf7b46672957721d`. Later commits and URL amendments changed only
execution bookkeeping. Earlier successful coverage remains applicable: subsequent CSS edits affect distinct dialog rules
and preserve the tested approval/outage behavior. The delay inventory is reused after corrections that added no delay
calls or source-discovery changes. Main through `2ea34288` changed only plan claims and delivery reports; its full diff
was read and introduced no product, specification or governing-instruction interaction requiring a rebase or new runtime
checks.

Broad Rust/Clippy, desktop runtime, installer and provisioning suites were skipped: shared UI markup and local geometry
changed, and compilation plus the focused browser selections cover those risks. CI is manual-only and was not
dispatched.

## The review gate's outcome

Each PR passed exactly one fresh-context gpt-6.1-sol review at high effort, with corrections checked by that same
reviewer. Outage review corrected the available-space condition and sidebar layering; the required separate scope
reassessment found no unnecessary complexity. Dialog review narrowed the fallback selectors and corrected the
whole-paragraph test oracle. No findings remain. Commit and PR wording passed independent cold reads. Documentation
passes are complete, each outcome ledger names its change, bookmark and PR, all feedback entries are removed, and all
three PRs remain open drafts.

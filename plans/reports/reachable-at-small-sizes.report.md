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

### Landing

Landed as #2406, #2411 and #2419, merged bottom-up. It was the sixth of sixteen plans in this landing round, after
launcher-retry-identity, launcher-composer-drafts, terminal-link-and-copy-fixes, transfer-leftover-cleanup and
trash-unknown-totals. Its content is what the report describes, rebased onto this round's main, with one SPEC.md
conflict resolution and one browser test fix described below.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 107's four plans, the five plans earlier in this round, and documentation and
plan bookkeeping. A fresh-context reviewer and I read them and the other plans of this round against this plan. Three
touch the same ground:

- host-entry-session-ids changed how sessions are identified and gave approval cards full-width rows that show which
  session asked, which session is targeted and which session a clone came from. Both it and this plan's first change
  edited the same SPEC.md paragraph about approval cards. The landing kept both: this plan's sentences about very short
  windows, then host-entry-session-ids' sentence about the identity rows. host-entry-session-ids added rows inside each
  card but did not change how the approval area is sized or placed, which is what this plan changes, and this plan's new
  browser checks still find the cards and answers they use.
- hosts-panel-menu-close closes an open host or session menu whenever the hosts panel changes height. The host dialogs
  this plan makes scrollable are overlays, so opening or closing them does not resize the hosts panel.
- terminal-link-and-copy-fixes, earlier in this round, refuses link clicks while something covers the terminal. Approval
  requests that take the full height of a very short window count as covering it, so a link click there is refused with
  the explanation that the link's destination details are covered, which is correct.

app-dialog-focus, later in this round, changes when host setup, uninstall and refusal dialogs may open, not how they lay
out. host-settings-focus changes focus in Host Settings, which does not use the whole-card scrolling this plan adds.
Both compose with this plan. The other conflicts were in the review feedback queue's index, where each plan removes its
own entries; all removals are kept.

#### Fixes made during the landing

- SPEC.md: the approval-card paragraph conflicted with host-entry-session-ids, as described above; both sides are kept
  (#2406).
- The new outage browser check in the two-host test file intercepted the Replace request for a stale session and
  answered it with a refusal, matching the request by the session's ID. After host-entry-session-ids, that ID is a full
  reference whose colon the UI encodes in the request address, so the match never fired, the real helm answered instead,
  and the test would have failed. The check now matches the encoded ID, the way host-entry-session-ids changed every
  similar check (#2411). The full browser run below, under Checks, ran with this fix and passed the test on both
  engines.

#### Possible follow-ups

- SPEC_impl.md says the approval region "uses the whole viewport" in a very short window. Only its height grows; its
  width stays tied to the main pane. SPEC.md's "the window's full height" is accurate. Possible follow-up: say "the
  viewport's full height" in SPEC_impl.md as well.

#### Checks

Every plan in this round was executed on a main from before round 107's session-identity change, so none had been tested
against it. That is why this round ran the whole Rust workspace and the full browser suite rather than a selection. All
of the following ran now, on all sixteen plans of this round stacked together on main, including the fixes made while
landing plans in this round:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and its `--self-test`,
  `python3 releasing/notes-review-page.py --self-test`, `scripts/check-desktop-assets.sh`, and the website build
  (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `40f2b98b-b160-4239-a653-44cd0bcdcf96`: 3948 passed.
  `cargo nextest run -p farhelm-ui --features desktop`, run `d288b68b-249f-4f21-9fe0-d4d6d1c4a738`: 717 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `f82f83f8-67c9-4e07-99ed-467c9830dc9f`: 317 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, one worker, no retries, run
  `81d76a96-912f-47ed-8e1f-57423edbeb32` (kept): 2183 passed and 14 skipped. Of the skips, 9 are the WebKit clipboard
  cases, because Playwright cannot grant WebKit clipboard permissions; 4 run only against a real, signed-in agent; and 1
  is a pixel capture that runs only on Chromium. One test failed: on WebKit, "model-only number key opens New with a
  temporary composition explanation" in `templates.spec.ts` timed out at its first step, waiting 60 seconds for the
  header's Templates button to settle before clicking it. No plan in this round changes that button or that test, and
  the file's other WebKit tests that open the same dialog passed.
  `npx playwright test --project=webkit-templates -g 'model-only number key opens New with a temporary composition explanation' --repeat-each=20`
  then passed all 20 (run `6c382916-77a1-4ecb-b4ee-a1dae8cac848`). The cause is probably machine load during the full
  run, not something this round broke, but that is not confirmed; on one observation it is not recorded as a flake.
- Skipped: the native Mac app (not runnable here) and the installer (unchanged).

The report was written on a main from before round 107's session-identity change, so its statement that no rebase was
needed and its focused checks no longer describe what landed; the combined runs above supersede them. Nothing else in
the executor's report became untrue at landing.

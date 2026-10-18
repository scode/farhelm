## What this was about

The open session did not show its host, and its identity row and separate tab row used space above the terminal. You
chose to put identity below the terminal and keep tabs and actions together above it.

The session's status, name, host, age, checkout mark, directory and command now appear in a slim bottom bar. Tabs share
the top row with the existing action buttons. The host uses the sidebar's icon, color and name, with stronger text to
make the active host easier to identify. Both bars use about 30px; the Mac top row keeps its existing 48px alignment.

## Things you should know

The host display reuses the sidebar's existing host information and shared rendering. It adds no host request. Directory
and command copying, their accessible descriptions and confirmation feedback remain available; the hidden-character
warning opens upward. Restart, Replace and Delete questions stay under their buttons, with Escape cancellation there.
All six actions remain visible at a 650px main pane; tabs scroll through the remaining space.

Conditional notices and tunnel chips stay above the terminal. Approval cards now follow the terminal area's top edge, so
moving the tabs does not move the cards over those notices. Interrupted-session recovery keeps the bottom bar inside the
window even when there is no terminal.

The recently landed terminal-link refusal message sits above approval cards. With the cards now beginning near the top
of the terminal, that explanation can cover the card's requester label until the pointer leaves the refused link. The
focused integration checks cover this combination's refusal and opening behavior.

Validation also reproduced a previously recorded confirmation-test timeout. Restart succeeded, then the fixture hung
while draining an intercepted status request. Its cleanup now waits for its own callbacks instead of a Playwright
internal promise that a failed fulfillment can leave unresolved. The action assertions and timeout budgets are
unchanged. The corrected confirmation spec passed all eight cases across Chromium and WebKit. The test is re-enabled in
flake sweeps, with the diagnosis and retained failure evidence appended to the flake log.

## Open questions and possible follow-ups

The docs text is updated on Work in a session and Stop, restart and resume. Their existing `header`, `tabs` and
`header-actions` screenshots and capture selectors, plus the README hero, still show the old layout. Refreshing those
assets belongs to the separate screenshot flows and was deliberately left for their next refresh.

WebKit covers the desktop engine family. The Mac app's own window and native tunnel operations were not exercised.

## The PRs

- [#2253 — move session details below the terminal](https://github.com/scode/farhelm/pull/2253/changes), draft head
  `cb54efbd2a6f18aa1d2b0ac952e385a2b034426a`.

## Checks run, reused and skipped

Browser file selections below used `npx playwright test` through the recorder, on Chromium and WebKit, with one worker,
zero retries and pinned tmux 3.7c. The listed names identify their `.spec.ts` files.

- Passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop` for the changed UI and shared host context. The compile check initially
  reported an unused reexport, which was removed before the final Clippy pass.
- Passed `cargo nextest run -p farhelm-ui --lib -E 'test(session_view::)'`: 24 passed, 496 selected out, four slots,
  zero retries, pinned nextest 0.9.148; recorder `b8e4bafa-3e1d-4f4c-baef-bbb3f80c67d1`. No runtime `SKIPPED` marker.
- Passed the complete `header`, `chrome`, `window-chrome`, `approval-layout`, `shell-scroll` and `sidebar` browser
  specs: 272 cases, 136 per engine, no skips or retries; recorder `f314a4df-c663-4126-8920-909a3fe0b58c`. This includes
  bottom-bar placement, host variants, narrow action visibility, Mac alignment, upward copy warnings and approval
  layout.
- The remaining browser run completed 718 cases: 717 passed and the original Chromium confirmation helper timed out.
  WebKit passed all 359 cases; Chromium passed 358 of 359. No skips, global errors or unstarted cases occurred. Files:
  `auth`, `buttons`, `destination-authority`, `feed`, `github-checkouts`, `listing-mutation-fence`, `m6-5-debts`,
  `readers`, `session-action-shortcuts`, `session-confirmations`, `sort`, `terminal-attachments`,
  `terminal-create-idempotency`, `terminal-flood`, `terminal-multihost`, `terminal-reconnect`, `terminal-replay-rename`,
  `terminal-restart`, `terminal-tabs`, `terminal`, `ui-plan-regressions` and `yolo-guard`. Recorder
  `0160f4ba-1804-4a5e-b84b-9863e598d988`, both engines, one worker, zero retries, pinned tmux.
- Original exact Chromium reproduction failed twice, recorder `30cee0cb-ab38-460c-be53-97aa84fe0446`. The fixture
  correction passed twice in 14.8 and 13.1 seconds, recorder `09790df0-e92c-4a27-b5c1-ef5ff84df6da`. A preceding
  selector mistake selected no tests, recorder `c5efca91-218e-416a-b7e0-502e227f76a1`. All failure evidence is retained.
  The complete corrected confirmation spec passed eight of eight, four per engine, no skips or retries; recorder
  `58382f01-25d5-437d-bda4-48b36639d22a`. These runs used the same application builds; only the test helper changed.
- Passed the website's `bun run build`, including internal links; changed-file `dprint check`;
  `python3 scripts/check-agents-md.py`; `python3 releasing/check-changelog.py format`; and the isolated-parser
  `python -B scripts/check-test-sleeps.py` (303 delays, none without rationale). Built the CLI, fixtures and web UI for
  browser execution. An initial `dx` invocation resolved to another tool and failed before building; the explicit Dioxus
  binary completed the required build.
- Passed the final rebased integration selection:
  `npx playwright test terminal-link-clicks.spec.ts terminal-links.spec.ts approval-layout.spec.ts`, 88 of 88 (44 per
  engine), no skips, retries, global errors or incomplete cases; recorder `daebe5e4-624b-40d4-8bab-47873d97b834` at
  `cb54efbd`. This covers the newly landed link-refusal checks with the moved approval region.
- Reused: the UI unit, desktop compile, Clippy and website results came from the reviewed working tree on `50457cf7`;
  later Rust edits only corrected comments, local names and an unused reexport. The original browser runs cover the
  implementation pushed as `c322616b`. The careful rebase onto `3b9b88f4` preserved that diff exactly. Main's
  observed-YOLO latch and acceptance clarifications do not change session layout, host rendering or these fixture
  agents. Main's terminal-link refusal does interact with the approval-card position; the final integration selection
  above passed. The web bundle was rebuilt after rebasing. Subsequent local changes concern the tested confirmation
  helper and its flake bookkeeping.
- Skipped: full workspace, installer, provisioning, release and native desktop runtime suites; this change concerns
  session layout and host rendering, covered by the selected UI units, desktop compile check and changed browser specs.
  CI is manual-only and browser CI is disabled; no CI run was dispatched. No live installation was changed.

## The review gate's outcome

The single required fresh-context gpt-6.1-sol reviewer at high effort found two layout defects: a classic tab scrollbar
could make the top row taller, and the interrupted-session card could push the bottom bar below the window. Both were
fixed and the reviewer confirmed them resolved. The same reviewer accepted the callback-drain correction with no new
finding. Review included the complete test-authoring checklist. The commit and PR wording passed their separate cold
read. The reviewer verified the integrated helper matches the tested candidate and accepted its flake bookkeeping.

### Landing

Landed on its own in round 89, as #2253, with two fixes made during the landing and one conflict resolved (below).

#### What landed since the PR was based, and whether it interacted

The executor had already rebased onto main after the refusal to open a terminal link whose hover an approval card covers
(#2254, the report's "terminal-link refusal") landed and re-ran its browser tests against it. While this landing was
being checked, five more changes reached main: a batch of recorded review findings (#2258, documentation only) and four
test fixes (#2259 to #2262). One of them, #2259, independently fixed the same confirmation-test hang this PR also fixed,
in the same test file, with its own flake-log entry, and removed the same TODO.md and sweep-exclusion lines. The rest
merged without conflict, and none of the others uses a selector this PR removed.

The landing monitor read the PR against what main holds, and a fresh-context reviewer did the same independently.
Besides the problems fixed below, neither found an interaction:

- That link refusal measures the approval cards where they are when the link is clicked, so it follows them to their new
  place. In the steady state the cards still begin just below the terminal's top edge, as they did before, so the
  refusal explanation overlaps the top of a card exactly as it did before this PR; the report reads as though that were
  new.
- The delete shortcut still finds the Delete button, Rename still works from the session list and its shortcut, and
  Escape for the top row's questions moved with them. Naming after quick launch, the quick-launch keys, the menu focus
  fix, the one-way YOLO shield and the sidebar's host and permission marks are unaffected; the bar's host label is the
  session list's own.
- The tunnel strip now sits directly above the terminal, as SPEC.md and the docs say.

The changelog entry is a new "changed" entry; nothing it describes has shipped.

#### Fixes made during the landing

- **The bar showed the full home directory.** SPEC.md, the docs page and the plan's recorded decision ("home shown as
  `~`") all say the directory in the bar abbreviates the home directory to `~`, but the bar showed the full path; the
  old header never abbreviated it either, so the plan's "as today" was mistaken. The bar now uses the session list's
  rule, so a directory reads the same in both places. The hover, the screen-reader description and the copied value keep
  the full path. One exception keeps the full path visible: when the folded-away part of the path contains a hidden or
  direction-changing character, because the copy warning says such characters are shown in the bar. A unit test covers
  both cases.
- **The quick-launch pill hid the new bar.** While a launch sequence is pending, its pill sits over the bottom of the
  terminal; with the bar now at the bottom of the same pane, it covered the session name, host and age. It now sits just
  above the bar when a session is open, and a browser test checks that it stays inside the terminal area and clear of
  the bar.
- **Conflict with #2259.** The confirmation test file takes main's fix together with this PR's two updated selectors,
  and the flake log keeps main's entry for that hang; this PR's own entry, which described its now-superseded version of
  the fix, was dropped.

A fresh-context reviewer checked the two fixes and the new test, including the test-writing checklist, and its one
correction (the hidden-character exception) is included.

#### Things to keep in mind

- The docs screenshots still show the old layout, as the report says. Two of their capture scripts will fail on the next
  screenshot refresh, not just look stale: the "header" shot on the Work in a session page and the "header actions" shot
  on the Stop, restart and resume page both wait for the old header element, which no longer exists. The screenshot
  refresh is the place to rewrite them.
- A few texts still describe the old layout: the hidden-characters warning says the characters are "shown above", though
  the bar is now below its warning; some style-sheet comments; and the manual Mac checklist's mention of the session
  header.
- In a short window with tall notices, an approval card can now cover part of the bar, and in the Mac app the tunnel
  error notice covers the bar's right end, where the command is shown. Neither was checked on a Mac.
- The sidebar variant of the flaky Replace test
  (`plain Replace keeps a newer same-source key after old sidebar YOLO cancellation`) fails the same way as the header
  variant already on record, on main as well as with this PR (Checks, below). It has no flake-log entry, Deflake entry
  or sweep exclusion yet, so the next deflake sweep will catch it again. Suggested follow-up: widen the header variant's
  existing entries to cover both variants.
- The codex-trust-prompts plan's open PR (#2255) now has a small textual conflict with this change in the session view
  (both edit the same element; keep both), and no functional collision. Separately, that PR does not remove its own
  TODO.md entry.

#### Checks

- Run now, on the PR with the landing fixes:
  - `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
    `cargo check -p farhelm-ui --features desktop`, and the test-sleep check (303 delays, none without a reason).
  - `cargo nextest run -p farhelm-ui --lib -E 'test(session_view::) | test(list::row::)'` through the test recorder,
    four slots, no retries: 44 passed, run `35b14271-9dd0-4bca-b14f-02b76df6ca80`.
  - `npx playwright test quick-launch.spec.ts` through the recorder with pinned tmux, one worker, no retries: 64 passed
    in Chromium and WebKit, run `8f8f8b59-e4ce-401d-a107-73039f83cd8b`.
  - Before the review's correction, the same Rust checks and `npx playwright test` on `quick-launch.spec.ts`,
    `header.spec.ts`, `replace.spec.ts`, `restart-with.spec.ts`, `session-view-claims.spec.ts`,
    `tooltip-coverage.spec.ts`, `modal-layout.spec.ts` and `terminal-scroll-freeze.spec.ts`, which include the session
    view specs the executor had not run: 235 passed and one WebKit test failed, run
    `280a0932-1db0-40e7-9c32-eca8eaebba8d`. The correction only changes the bar's directory text for paths under a home
    directory, which these tests never use.
  - After resolving the conflict with #2259,
    `npx playwright test session-confirmations.spec.ts terminal-reconnect.spec.ts` on both engines: 60 passed, run
    `dc754fb1-cca3-4da9-845b-5c75d5f959e5`.
- The one failure, `plain Replace keeps a newer same-source key after old sidebar YOLO cancellation`, timed out waiting
  for the source row's menu in WebKit. Run alone six times on each, it failed three of six times with this PR (run
  `6328360c-ab06-4f40-884c-86b979f3a1a9`) and two of six on main without it (run
  `18a485cb-d9da-40a4-9152-6e419dc34b65`). It is the sidebar twin of the open flake-log entry and Deflake TODO entry for
  the header variant of the same test, with the same error; this PR did not cause it.
- Reused: the executor's other runs listed in the report, at the revisions it gives. The landing fixes touch only the
  bar's directory text and the pill's position, which those runs do not exercise.
- Skipped: everything the report skipped, for the reasons it gives.

Two things in the executor's report no longer hold. The confirmation-test fix and flake-log entry that landed are
#2259's, not this PR's. The overlap between the link-refusal explanation and the approval cards is not new (above).
Nothing else in it became untrue at landing.

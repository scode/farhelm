## What this was about

This carries out twelve triaged problems in Farhelm's browser tests. Some tests could pass while the behavior they
claimed to guard was broken; failed setup or assertions could also leave shared policy, sessions, a browser context or
held requests behind. The maintainer chose contained test fixes, with harder cases discarded or returned for a decision
according to each recorded gate. Nothing changes for someone running Farhelm.

The tests now observe pending terminal retries themselves, close a sibling while another close is held, deliver a plan
actually made for the previous host destination, and check that a refused Stop did not kill the agent when its host
returned. Progress reads receive a complete published value, and unreadable process information cannot certify a shell's
death. The changed fixtures arm cleanup before fallible setup and attempt independent releases even after a restore
fails, preserving the test's original error.

## Things you should know

The first development run exposed an incorrect locator in the new sibling-close test: the confirmation prompt is below
the terminal strip, rather than inside the tab's control. The corrected exact Chromium case passed. A separate
controlled failure after the first close reached its hold preserved the original error and completed cleanup without a
hook timeout. The temporary failure was removed. These were same-session test corrections; no product defect or latent
flake was established.

The initial combined run was interrupted after WebKit reached the same known locator error. It needed the recorder's
forced cleanup and has no complete terminal report, so its earlier progress is not counted as passing evidence. The
failure and its available artifacts remain retained privately.

All twelve outcomes were implemented within their agreed scope. None needed a product testing hook, tripped a complexity
gate or went back for a decision. The twelve feedback files and index entries were removed, and only their Execution
fields changed in the triage ledger.

The source review found that network receipt could precede the UI acting on a late plan. The test covers two ways an
unsubmitted Update becomes invalid: the host's destination changes, or another Update starts on that host. Once the
delayed reply arrives, the browser must neither revive the cancelled Update nor submit it. The test watches for ten 50
ms samples, counting every submission request throughout and checking that no local Update remains waiting. After a
destination change, details stay hidden. When another Update starts, its running progress panel stays visible while the
global details setting stays off. This is a bounded observation window, not an application-completion signal. The
reviewer also identified pre-existing cleanup errors that could hide the original failure in the two changed
terminal-tab tests; these now report cleanup trouble alongside the original error and still fail on cleanup-only errors.

The first run of those corrections mistakenly required hidden details in the foreign-run leg as well. Both engines
rejected that assertion because the foreign update's progress panel is legitimate. That run is retained. The
phase-specific correction passed narrowly on both engines; no product failure was found.

## Open questions and possible follow-ups

None required. The optional product mutation that deliberately leaves a terminal mount retry running was not performed.
The test establishes a positive retry witness before removal and observes ten retry periods afterwards. The source
reviewer accepted that evidence: a surviving retry would continue increasing the count even after its tab's element
disappeared. That is a source-based assessment, not an executed mutation check. The controlled route-failure stimulus
above adds direct failure-path evidence without a product change.

## PRs

[PR #1930 — browser assertions and cleanup](https://github.com/scode/farhelm/pull/1930/changes), draft, one review unit
for all twelve outcomes.

## Checks run, reused and skipped

- Built the CLI and browser application with `cargo build` and the pinned Dioxus 0.7.10 web build. Existing Playwright
  1.64.0 dependencies and Chromium/WebKit binaries were available. Runtime runs used pinned tmux 3.7c, one browser
  worker and zero retries.
- Final recorded browser selection: `npx playwright test <six-spec-selection> -g <changed-test-titles>` through the
  strict recorder, run `aa6ca542-1fb8-4abd-9c63-52851d25e80c`. All 28 cases passed, 14 each on Chromium and WebKit, with
  zero failures, skips, retries, interruptions, unstarted cases or global errors. The complete serial killed-supervisor
  group ran against the real SSH fleet. Reports were complete, recorder policy passed and cleanup finished normally.
- After review corrections: `npx playwright test <two-spec-selection> -g <three-changed-test-titles>`, run
  `f0500d16-5dc5-48b8-adea-4de155f3d059`, had four terminal-tab passes and two provisioning failures caused by the
  incorrect foreign-panel assertion above. After correcting only that assertion, the exact provisioning case passed on
  both engines in run `00b88d34-a935-4abf-8bdd-13e141a10f8c`, with complete reports, zero skips or global errors and
  normal cleanup. Reused the four unchanged tab passes from the failed run and 22 unaffected passes from `aa6ca542`; the
  failed run remains a failure. An intermediate receipt-plus-window variant also passed two cases in
  `c2c6f010-6fd1-4893-9bf8-d230086e8af5`, but final provisioning evidence is the later corrected run.
- Exact sibling-close Chromium run `3814c323-bdfd-4986-942e-7d4ef14942e9` passed. Controlled assertion-failure run
  `fc15b92f-5d40-4c06-a8f6-16baab4cdf64` failed as intended with the original stimulus error, no teardown timeout and
  normal stack cleanup. Initial run `2c71ff26-cb66-400f-a686-153ca842a6a4` remains interrupted, incomplete failed
  evidence; none of its progress replaces final validation.
- The actual cleanup helper's two private failure-path checks passed in recorded run
  `a6834aac-e756-4dfe-b9cc-c3863f264ccd`: later releases still run, cleanup failures are retained together, and the
  original assertion error survives. Every changed file received a separate documentation pass.
- The isolated `python -B scripts/check-test-sleeps.py` passed finally with 291 delays and zero unannotated. The later
  foreign-panel correction changes no delay. Targeted Markdown `dprint check` passed. TypeScript specs are outside this
  repository's dprint plugin coverage; its attempted TypeScript format invocation refused without changing files.
  `git diff --check` passed.
- Reused the builds and unaffected runtime evidence after careful rebases through main at `1d6b9eac`: intervening
  changes were queue records, disjoint triage bookkeeping, new plans, host/tooling specification limits and Rust-only
  integration tests. They changed no executable dependency or shared browser fixture. Rebase preserved our six-spec
  patch; the later source corrections received narrow validation. Index conflicts preserved both plans' disjoint
  removals. The final rebase onto `3d9fab36` adds the landed supervisor/helm unit-test fixes and test-state cleanup
  changes. The six browser specs remain unchanged. The cleanup changes affect the startup sweep of abandoned test runs;
  they do not alter this run's owned live stack or its test assertions, and the landing report records focused cleanup
  coverage. The shipped CLI and browser build inputs remain unchanged, so no new browser run or rebuild was warranted.
  Index conflicts preserved both sets of removals; the index matches all 44 remaining feedback files exactly once.
- Full browser suite, Rust runtime tests, doctests, desktop, installer and JavaScript harness runs were skipped: only
  these browser specs and review bookkeeping changed, and the focused selection covers their affected behavior. WebKit
  supplies engine-family coverage, not a native macOS application test.

Private evidence and failed-run disposition are recorded in `farhelm-plan-browser-oracle-and-fixture-fixes-log.md`
beside the checkouts.

## Review gate outcome

The required single fresh gpt-6.1-sol high source review passed after the contained late-plan observation and
terminal-tab cleanup corrections. Its final follow-up checked the foreign-run panel correction against product source
and found no remaining concrete findings; it acknowledges that the preceding follow-up missed that fixture premise.
Source review did not certify runtime execution. The separate gpt-6.1-sol medium wording cold read accepted the commit
and PR wording. Implementation and validation stayed with the executor; no workhorse was delegated.

Private galaxy-brain evidence uses session `29405ad6-d9d8-4919-8d82-c1a7afa9b9f2`, referenced in the working log. Native
tools exposed no actual model attribution, reasoning effort or token usage; those remain evidence gaps. Some
activation/local-decision events were recorded after implementation began and describe recording time, not reconstructed
start times.

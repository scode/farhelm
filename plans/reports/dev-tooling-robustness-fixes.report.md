## What this was about

Nine triaged findings concerned development and release checks that could report the wrong result, retain inconsistent
evidence, or consume unbounded resources. The maintainer chose small fixes and agreed that anything larger should be
discarded. All nine fixes fit that limit; no finding was discarded. Their feedback files and index entries are removed,
and their execution records point to one draft PR. Nothing changes for someone running Farhelm.

Deflake is the maintainer's test-hunt tool: it runs test groups, repeats failures and tells a separate agent whether to
record an intermittent failure or fix a repeatable one. Its evaluation gives a fresh agent two deliberately broken tests
in a disposable checkout. A successful evaluation shows that the agent can understand the reports, record the
intermittent case, fix the obvious wrong assertion and finish without polling or breaking the commit rules. It checks
the driver and instructions together; the focused unit tests separately cover the new inconclusive-runner verdict.

Deflake now calls a repeatedly failing individual test deterministic only when three rerun reports prove that the
selected test ran and failed. A runner failure, including nextest being unable to start the test process, leaves the
verdict unclassified; a passing rerun still establishes a flake. Release smoke checks require the launched helm to
remain alive and the response to contain the Farhelm UI bootstrap asset. Docs-preview readiness requires the listener to
match the preview's background lock and process age. A different server cannot qualify solely by returning HTML or
running from the same checkout.

The release brick recorder validates and hashes the same retained bytes, including repeated continuity receipts. Guest
SSH addresses reject IPv6 scope suffixes. README hero staging refuses duplicate session titles before their title-based
lookup can conflate sessions. Test-discovery tmux version probes have a three-second deadline and kill their child on
timeout. Recorder diagnostics retain at most one writer blocked in a kernel write instead of accumulating a worker per
call.

## Things you should know

Deflake reads only the first and last 2 MiB of a console log for classification, preserving the initial evidence
announcement and final failure excerpt. Console capture remains uncapped on disk. Capping the capture would have
required a new copying worker and was outside the agreed small fix; the decision expressly allowed bounded
classification reads instead.

A diagnostic blocked inside a kernel write can remain alive beyond its caller's deadline. Its single process-wide slot
remains occupied, and later diagnostics are dropped. Nonblocking backpressure honors the deadline. This keeps descriptor
flags unchanged and prevents accumulating blocked writers, but it cannot make a blocking kernel write interruptible.

The source reviewer identified nextest's startup-error XML as an important competing mechanism: a named test is not
proof that it executed. The correction preserves broad extraction during the initial sweep and excludes startup errors
only when deciding whether a rerun proves a test failure. Its exact XML regression case passed, and the reviewer
accepted the correction.

The portable brick tests exercise fake Tart fixtures. They do not establish a real Mac release brick-test pass. The
preview identity checks ran against a controlled extracted predicate; no live preview was started or changed. README
hero validation used the real Playwright TypeScript loader against the checked-in scenario and a duplicate-title
fixture; it did not stage a live fleet.

The release smoke refusals received source review, shell syntax checks and shellcheck, with the asset marker checked
against a retained built page. No runtime experiment made the launched helm die or supplied an unrelated listener's
HTML. The focused supervisor adoption test exercised ordinary version discovery and the existing refusal of an old tmux
server. It did not plant a hanging version probe; the new timeout and kill-on-drop behavior received source and
compiler/lint review only. These are limits of the evidence, not claimed runtime passes for those failure paths.

The first required deflake evaluation failed during preparation because the launch selected an unrelated executable
named `dx`. No tests ran in that attempt. Its failure evidence remains retained as run
`6ce270c5-cbd2-4928-b6c0-74da4d2ff629`; the retry selects the installed Dioxus CLI through its launch environment. The
subject also exposed an instruction ambiguity: a background completion notification could contain only an exit code,
while the event text remained in the completed command's output. A reviewed clarification permits one read of that
completed output and explicitly allows fetching the evaluation's local origin. Waiting still forbids polling, log
inspection and status calls; budgets, events, acknowledgements and GitHub restrictions are unchanged.

The second evaluation reached the expected events and final exit, but did not pass the procedure: its report abbreviated
an event and it ran an extra test despite the evaluation's formatting-only exception. Run
`533f27d0-c127-4ddc-8e82-586d9295c75e` remains retained as failed evidence. It also exposed incorrect local-origin
default-branch references. The setup now explicitly points both references at main; the prompt makes the disposable
commit exceptions and unabridged event requirement explicit. Source reassessment accepted these corrections as necessary
and small, and review of the applied patch passed. They add no driver mechanism or protocol change.

## Open questions and possible follow-ups

None required to ship. On-disk deflake capture remains unbounded under the recorded decision. The one blocked diagnostic
slot can suppress later diagnostic messages until that write returns; this is the bounded behavior chosen instead of
adding another output subsystem.

## PRs

- [#1935 — tighten development and release checks](https://github.com/scode/farhelm/pull/1935/changes), draft; one PR
  for all nine outcomes, never marked ready or merged by this executor.

## Checks run, reused and skipped

Runtime commands used the test-run recorder. The focused Rust run used pinned nextest 0.9.148, pinned tmux 3.7c, four
slots and zero retries.

- Run: `python3 deflake/test-deflake.py` through the recorder, final run `751bb568-51b0-4ecc-b352-141846754ab5`: 18
  passed, including the exact startup-error report. Earlier run `1ff881cd-0440-48b4-af69-b4fca1307a44` also passed
  before that correction; the final run supersedes its classification coverage.
- Run: the two focused diagnostic-writer tests through the recorder, run `e9649269-2ef7-4e10-990d-d3e43f564c7a`: both
  passed. They establish an actually full owned pipe and release the blocked worker before reusing its descriptor.
- Run: the portable brick/network Python harness through the recorder, run `20e292ba-6047-449f-b903-f796d3ac3980`: 68
  passed. The changed evidence test replaces bytes after the read and verifies one-read identity and the exact stored
  digest; network cases cover rejected scope suffixes and accepted plain IPv4/IPv6.
- Run: controlled preview identity cases, run `243ef2b0-50ad-4aea-acaf-c3912e6dfbb5`, and checked-in hero validation
  plus duplicate-title refusal, run `9737ae67-dde5-4c13-ad91-a1cfe40d2bc9`: passed.
- Run: `cargo nextest run -p farhelm-supervisor --lib -E <exact-below-floor-adoption-test>` through the recorder, run
  `759349c5-e4ed-4e6e-9109-25956d2436e0`: one substantive pass, no runtime skip. The 1101 skipped tests were outside the
  selection.
- Run: supervisor all-targets Clippy, Rust formatting, preview syntax and shellcheck, and syntax/shellcheck of the
  actual extracted release smoke block: passed. Initial Rust formatting and the shebangless block's shellcheck
  invocation were corrected before their successful checks.
- Run: isolated `python -B scripts/check-test-sleeps.py`: 289 delays, zero unannotated after the latest rebase. Targeted
  dprint passed after final bookkeeping formatting. A private bookkeeping audit verifies nine Execution-only ledger
  changes, the exact nine removals and a feedback index matching every remaining file.
- Run: the required fresh Sonnet medium evaluation passed cleanly in driver run `76809c08-2f80-43d1-b7da-38986877c941`.
  It received exactly the planted proto flake (three passing reruns), the planted JS assertion failure (three exit-1
  reruns), and sweep completion listing one flake. The subject made separate local record and assertion-fix commits,
  reproduced every event without abbreviation, followed the disposable-commit gate exceptions and reached final wait
  exit 4. Five waits included one exit 3. The sweep target was absent afterwards; both locks were free and owned
  disposable state was removed while holding them. All three attempts' reports, run records and streams remain retained;
  only the third is a clean pass.
- Reused across the careful rebase onto `a0084743`: all runtime evidence above. Intervening changes were specifications,
  new plans, reports and disjoint bookkeeping; no executable dependency changed. The specifications retain the
  security/data boundaries and require serialized tooling runs per checkout. The index conflict preserved both plans'
  removals. No new runtime test was warranted for that mechanical resolution.
- Reused across the next careful rebase onto `1d6b9eac`: all focused runtime evidence above. The landed changes
  strengthened separate CLI integration tests, Linux selection, persistence witnesses and shutdown cleanup. They change
  no dependency of these tooling fixes or the evaluation's proto/JS fixtures. A source comparison confirmed the affected
  executable files were unchanged; the sleep inventory was checked again. The bookkeeping audit caught an old-base
  working-copy mistake before any push, and the index was rebuilt against the correct base while preserving both plans'
  removals.
- Reused across the third careful rebase onto `3d9fab36`: all focused runtime evidence above. The landed changes
  strengthen disjoint helm/supervisor unit-test fixtures and retain stale test-state directories when a live tmux kill
  is unconfirmed. The adoption fixture uses a held live directory and unchanged owned-server guard; its version
  discovery and assertions are unchanged. All affected tooling source and instructions compare identically with the
  evaluated parent. The feedback index conflict preserved both sets of removals, and the audit passed against the new
  base.
- Reused after rebasing onto `66dc1a19`: all focused checks above. The complete intervening diff contains only plan
  claims, deliveries and reports, with no executable, instruction or ledger change. The rebased patch is unchanged, and
  the bookkeeping audit still passes. No runtime repetition or sleep-check refresh was warranted.
- Reused after the final rebase onto `8218f6cb`: the complete 232-line intervening diff again contains only queue states
  and reports. The patch compares unchanged, including the evaluated driver and instructions. Adding the PR URL changes
  only nine Execution fields; formatting and the exact bookkeeping audit passed. No runtime test was repeated.
- Skipped: broad Rust and browser suites, desktop, provisioning, standalone website and release builds. The change is
  limited to these maintainer checks and helpers; focused execution, static checks and the required deflake eval cover
  their affected contracts. The required eval includes its own web-build preparation, small proto selection, feed
  browser spec on Chromium and WebKit, JavaScript harness and installer harness. Those prescribed checks are separate
  from the broad suites skipped here. Existing UI bootstrap naming was established from the Cargo target, default Dioxus
  configuration and a retained built index, so no release build was needed to discover it.

Private evidence and decisions are recorded in `farhelm-plan-dev-tooling-robustness-fixes-log.md` beside the checkouts.
Private review-routing evidence uses session `4718212d-9196-4328-a2d9-32c66cea9cba`, also referenced there. Native tools
expose neither independently confirmed model attribution nor token counters. Some follow-up launch evidence was recorded
late; its gap is retained without fabricated timing or usage.

## Review gate outcome

The required fresh-context gpt-6.1-sol high source reviewer received the complete goal, nine authoritative ledger
entries and the full test-authoring checklist. Its only blocking finding was nextest startup-error classification,
corrected and accepted in a follow-up. It also accepted both necessary, small evaluation setup/instruction corrections
and passed their applied patches. A fresh gpt-6.1-sol medium wording cold read accepted the commit and PR wording.
Implementation and focused validation stayed with the executor. The required deflake evaluation uses a fresh Sonnet
medium subject as prescribed by its procedure; its result is recorded above. The required independent report reader
accepted this account after the tool's purpose and static-only coverage limits were clarified, and found no public
repository hygiene violation.

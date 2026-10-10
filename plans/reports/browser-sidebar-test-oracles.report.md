## What this was about

Ten browser-test gaps could let broken behavior pass or leave sessions, hosts, a held request, or a supervisor behind
after a failed test. You chose to fix them together, while leaving any outcome that needed substantially more work for
triage.

## Things you should know

All ten outcomes are implemented. No product behavior or specification changed, and none tripped the complexity gate.
Their feedback entries were removed and their execution records point to the same draft PR.

The title-retry check now changes only the session name while keeping the invalid folder, so a folder edit cannot hide
reuse of the old creation key. Notification checks distinguish the intended session from another open session. The
refusal check for launching without future confirmations keeps the real helm build stamp, so it exercises normal error
recovery rather than a build-mismatch state.

The manual-unread check waits until unread is visible before measuring a later session-list reply. A harmless rename
then proves both the sidebar and the independently refreshed open-session view have rendered later data before the test
checks that no automatic read-mark undid the manual choice. The remote-folder Browse check accepts only log bytes
appended after its own action, with bounded reads that continue beyond the first chunk.

Failed-test cleanup now releases the held host request before fallible session cleanup, rejects refused session
deletion, finds and removes a newly added host even when failure came before its ID was captured, and preserves an
already-serving supervisor after setup fails before the intended kill. The harness's intentional teardown-failure child
records successful body completion only after its final assertion.

Execution found a mistake in the newly added unread check: waiting for the manual write's unused response body can hang
in Chromium because the UI drops that body after successful headers. The exact test reproduced it. The corrected test
requires successful status and rendered unread state; its later listing and detail reads still require completion and
rendering. Both engines passed the correction. This was a test mistake found and fixed in this run, not an established
product regression.

## Open questions and possible follow-ups

None. No outcome was deferred. The unread fault control verifies that the test notices a non-null read-mark request; it
does not mutate the Rust automatic-mark effect itself. Independent source review checked the two readers and their
effect boundary. No stronger claim about arbitrary future scheduling is made.

## PRs

- [#1821 — strengthen sidebar browser checks and cleanup](https://github.com/scode/farhelm/pull/1821/changes), one draft
  PR for all ten outcomes.

## Checks run, reused and skipped

Runtime commands used `python3 scripts/record-test-run.py`, pinned tmux 3.7c where required, Playwright 1.64, one
browser worker and zero retries. The application and release web assets were built before product checks. The remote
scenarios used a real SSH-connected second supervisor, rather than skipping their substrate.

The focused product selection exercised fourteen affected scenarios in each engine: four notification cases, sidebar
width cutoff, remote Browse, manual unread, title-only retry, confirmation refusal, held-host clone, add-host blank
fields, and three killed-supervisor cases. Run `3c10bfbd-da06-418c-b217-b1590044cef0` executed all 28 cases: 27 passed
and Chromium manual unread timed out at the unsupported body wait. Exact unmodified Chromium repetition
`e9927304-86d9-49f4-9ec8-8372135a364d` reproduced that timeout. Both failed records are retained privately.

After correction, manual unread passed in both engines in `dc23567f-3c37-441f-b13b-5e903bec0a75` (2/2). After every
temporary edit was restored byte-for-byte, manual unread and remote Browse passed in both engines in
`8454ee0e-b90f-4829-9182-c4012985abc8` (4/4). The other initial product passes are reused because their exact source and
product build were restored unchanged; the earlier 28-case command is not described as a passing run.

The full harness contracts have 27 Chromium passes from `52df8c3b-58d4-45e7-96e8-18b26dff721e` and 27 WebKit passes from
`d7eb9f19-658e-46ae-99f0-8e7b0b37da35`. The first command failed overall because WebKit could not find a required
library in the disposable sandbox. After fixing only that sandbox's linker configuration, the exact WebKit socket
contract passed in `440fed1c-eb9c-4098-9d56-ae6b56b90304`, then the full WebKit harness passed. Chromium's coverage
remains applicable. The restored teardown-child contract also passed in both engines in
`fda8b460-9710-4823-9361-4b1a10376256` (2/2).

Temporary fixture faults and forced failure paths were run separately on Chromium. These are deliberate negative
controls, not product failures:

| Control and observed result                                                                             | Recorder run                           |
| ------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| A child body assertion fails; the parent rejects it as proof of teardown-only failure.                  | `3fe55d97-4dbc-4e86-aced-a9e45001f4bb` |
| Reusing the creation key fails the distinct-key assertion.                                              | `3fe96a36-c023-40c7-89ac-8f21974652fd` |
| A mark redirected to another real session fails the addressed-session assertion.                        | `d8a77439-5b85-4be5-827e-ba051257efa4` |
| An injected automatic read-mark fails the zero-mark assertion.                                          | `374de15b-92a2-4308-91bf-691190ad7654` |
| An older Browse receipt cannot substitute for this action's suppressed new receipt.                     | `4d3997e9-43c4-4c62-8664-71b92bc20f76` |
| An additional 150 KiB before the new receipt still passes the advancing bounded search.                 | `170ce72a-96b6-4162-9655-23d07ab92bcf` |
| Failure before normal host-request release still drains the route and leaves the owned sessions absent. | `eb085635-93ea-4bbb-8ed4-383e5a280c5e` |
| Refused deletion fails cleanup; a separate real deletion removes the fixture.                           | `304266f2-3306-4e65-b40f-48138c24583b` |
| Failure after host registration but before ID capture still removes the unique destination.             | `2b281cc7-fb61-482d-b205-51cf0c873494` |
| Setup failure before killing the supervisor preserves the live service and its owned handle.            | `1c0e254b-c172-4c12-bc11-1f930f8dd3f0` |
| Removing the refusal's build stamp fails the no-mismatch assertion.                                     | `38df912f-ede2-4ba2-889d-383f3761255f` |

The private driver initially expected the child's marker name in its diagnostic; inspection confirmed the actual
intended rejection, so that control needed no runtime retry. All fault records and their dispositions are retained. No
product or generated assets were mutated, so the original unmodified builds remain applicable. Generic recordings retain
command and lifecycle evidence but do not provide validated engine-count totals. The portable run summarizer found all
19 retained records with complete discovery and was archived privately.

The isolated source-only delay checker passed with 282 delays and zero missing rationales. Formatting of the execution
ledger and feedback index passed. The repository's dprint configuration has no TypeScript formatter; its earlier
TypeScript invocation found no files and is not counted as a formatting pass. A separate documentation pass covered all
seven changed TypeScript files and the runtime correction.

Every main diff from the tested base `054c3798` through `196dc805` was read. Queue/report records, Settings TODO
entries, unattended-execution instructions and a new checkout-folder/UI plan changed no executable code, specification
or fixture. The clean rebase therefore preserves each reused command's coverage. The PR URL edits change completion
records only and passed formatting.

The full browser suite, unrelated cases in the changed specs, Rust runtime tests and Clippy were skipped: the affected
test scenarios, shared stub callers and cleanup hooks were exercised directly, while product code stayed unchanged.
Broader execution would not cover a remaining identified interaction. No native desktop-shell or macOS execution is
claimed.

## Review gate

The prescribed fresh gpt-6.1-sol source reviewer at high effort received all ten contracts and the full test-authoring
checklist. Its findings about independent detail rendering, an advancing log search and the exact clear payload were
independently checked and fixed; its final source check passed. A fresh scope reassessment reviewed the subsequent
write-body correction, confirmed that removing it preserved the required proof, and found no unnecessary mechanism or
gate trip. Runtime observations and final bookkeeping were assessed separately by the executor.

The commit/PR wording cold read requested gpt-6.1-sol at medium effort and passed. The independent resume check passed.
Implementation and runtime investigation stayed local under no-workhorse mode. Native tools did not expose actual model
identities or usage counters; these are requested reviewer routes. Private routing evidence is retained under session
UUID `babea5b4-d5fc-4824-9a71-faa36bdd96dd` and `farhelm-plan-browser-sidebar-test-oracles-log.md` beside the checkouts.
Missing earlier launch-event metadata is recorded as an evidence gap.

### Landing

Landed on 2026-10-10 (UTC) as #1821. The rebase met only the review queue index.

#### A fix made while landing

The sidebar test "composer menu-closed Tab order follows the displayed launch groups" was failing on main on both
engines, independently of this plan (run `4bb205ad` on plain main): earlier today the launcher gained the "folder" and
"managed checkout" destination buttons (#1786), two Tab stops before the host picker, and the test's expected order was
never updated. The landing added the two buttons to that order, in #1821 before it merged; the test passed in the run
below.

#### Things to know

Left as they are: one test's timeout comment still adds up a wait that was removed; one assertion in the notifications
spec repeats what the exact checks after it already prove; and in two specs a cleanup failure in `finally` now replaces
the body's own error. A plan still in flight (checkout-folder-and-ui-fixes) adds a launcher control and may move the Tab
order again.

#### Checks

These six plans were landed together, stacked in landing order on main after gui-text-safety, ui-interaction-fixes and
helm-cli-fixes merged; browser-terminal-test-oracles landed after them in the same round.

- Run now, on the six stacked: `cargo fmt --all -- --check`, `dprint check`, the changelog lint, the plans-queue tests,
  the test-recorder tests, the installer tests (517 checks), `sh -n scripts/install.sh`,
  `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the supervisor, helm,
  UI, protocol and CLI unit tests with the agent-relay, upload, boot-outcome, hook-identity, restart-with-resume and
  session-lifecycle end-to-end tests, through the recorder with pinned tmux 3.7c, four slots and no retries (run
  `8bc6001f`, 3064 of 3064); the UI JavaScript tests (230 of 230); and on Chromium and WebKit with one worker and no
  retries the notifications, sidebar-resize, sidebar, terminal-attachments, terminal-create-idempotency,
  terminal-multihost, yolo-guard, templates and quick-switcher specs (run `77ec2061`, 472 passed). `shellcheck` reports
  the same warnings for `scripts/build-private-tmux.sh` and `scripts/desktop-smoke.sh` on main as with these changes.
- These plans were claimed while an earlier round was still landing, which delayed their merge; the landing instructions
  were changed the same day (#1829) so that a round's plans are fixed when it starts.

Nothing in the report above was made untrue by the landing.

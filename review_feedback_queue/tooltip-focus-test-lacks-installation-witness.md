# Tooltip focus tests lack an installation witness

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Tooltip focus assertions can run before the tooltip listener is installed.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The fixture's availability observation does not witness listener installation across the actual Dioxus/script lifecycle.
Source checks narrowed the conditional ordering; no browser trace established its frequency or the concrete
false-pass/failure. An installation/readiness oracle at the intended boundary would settle that premise.

Source locations: `e2e/tests/tooltip.spec.ts:525-536`. Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Separate discovery pass. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: a healthy tooltip
implementation can fail the keyboard-focus regression if its asynchronously fetched script installs after the test's
only focus event. The test establishes row readiness, not listener readiness, then waits for an event effect that cannot
be replayed. Source: loadRow 51-85 waits for a synthetic row/status age; parkPointer 231-240 moves to a target without
tooltip and accepts a hidden/absent tooltip. The test focuses Settings exactly once at 533 before visibility/text
assertions 535-536. lib.rs 1553-1557 renders TOOLTIP_JS with AppBody without suspending content. Local Cargo.lock pins
dioxus-document 0.7.10 at 1320-1323; its elements/script.rs 92-118 calls create_script and returns empty; dioxus-web
0.7.10 src/document.rs 138-145 queues insertion, and 167-185 creates/appends the script without a load barrier. Exact
upstream v0.7.10 resolves to 57d6794ad60b949e5bd8aa282f6f8c3dc97a365e; packages/cli/src/build/web.rs 283-311 initializes
WASM with an unawaited .then and 517-534 inserts an async module loader. Thus page.goto's document load is not an
application-script readiness oracle. Tooltip install initializes target=null and registers listeners, without inspecting
activeElement or replaying prior focus: assets/tooltip.js 210-253,284-355,370-526; focusin at 474-493 is what schedules
this help. If row rendering and the focus event precede script completion, installation leaves no target or timer to
satisfy the test. Suggested fix: await the existing installed-hook/API readiness before deliberate focus/pointer stimuli
in this suite; preserve explicit late-install tests if any are added separately. Bucket other; private test reliability
only. Confidence: confirmed conditional source mechanism, actual occurrence unmeasured. Missing observation: a
standard-run ordering trace where the row and gear focus precede tooltip installation, with no subsequent input event;
source permits that ordering but cannot measure its occurrence/frequency. A retained failure with asset/focus
timestamps, or authorized delayed-asset reproduction, would settle it. Actual checks: followed row and pointer helpers,
test action, app Script producer, exact-version framework script insertion, immutable upstream loader and full tooltip
install body; checked the potentially-refuting page.goto load barrier and found the WASM .then path does not supply it.
No tests, builds, experiments or browser/native actions ran. Coverage searches across SPEC.md, SPEC_impl.md, empty
Planned, BUGS.md, queue index/files, TRIAGE_OUTCOMES.md and plans/queue found no exact coverage; SPEC_impl.md 1185-1217
specifies the help behavior, not this fixture race. No filter's rare-trigger premise established. proposed_drop none;
possible_cover none exact. Provenance: g053 separate discovery after parent accepted synthesis f1. Inherited: exclusive
base da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c has the same helpers and focus sequence at tooltip.spec.ts
42-86,226-241,525-539. Do not attribute introduction to casing patch b91fffda992b0dd586b0a90564b20bac89482db2. External
primary source:
https://github.com/DioxusLabs/dioxus/blob/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/packages/cli/src/build/web.rs#L283

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g053/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

# Uninstall test restoration can precede its owned run

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The uninstall progress test can restore its backend and registry while its accepted uninstall is still active.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Connected can satisfy the observation before later deletion, and a final forced Delete can refuse while the run is busy.
Source investigation establishes the missing settlement boundary; the adverse execution order and resulting
private-stack contamination were not observed. No live installation was touched.

Source locations: `e2e/tests/terminal-multihost.spec.ts:4676-4679`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. TLDR: if the inline-uninstall test fails after confirmation
while its run remains active, its cleanup can return with a host that the run will subsequently forget; the file's final
forced replacement can instead fail because that host is still busy. This is private browser-test fixture contamination,
not a product installation or user-work-loss claim. Source basis: the test introduces 1500 ms per-action delay at
4629-4636, confirms at 4665, and restores probe configuration plus restoreFleetRow at 4676-4679 without retaining or
settling the accepted provisioning run. restoreFleetRow at 688-716 accepts a matching existing row and waits only for
connected. The test backend reads action behavior and sleeps per action at
crates/farhelm-helm/src/provisioning/e2e.rs:197-226. The accepted server task owns the provisioning lock and survives
the caller at provisioning/service.rs:1794-1825, then ForgetHost deletes that row at 2508-2522 via 1845-1862. afterAll's
replacement at terminal-multihost.spec.ts:5141-5145 attempts DELETE, and farhelm-helm/src/hosts.rs:1155-1157,1189-1203
immediately refuses a busy host. e2e/playwright.config.ts:58-73,98-107 and e2e/stack-slots.ts:4-12 establish long-lived
stacks reused by later workers/projects. Actual checks: followed test setup/action/finally, restore helper, final hook,
accepted-run ownership, injected delay, final deletion, busy-removal refusal and stack reuse; no runtime, tests or
experiments. Missing premise: an actual post-confirm assertion/interaction failure aligned with an unfinished accepted
run and the subsequent cleanup scheduling. No specific latent flake or deterministic ordinary passing-run failure is
established. A retained failed run or an authorized fault-injection reproduction tracing run ID, finally return,
afterAll DELETE and host deletion would settle occurrence and downstream impact; prohibited source-only runtime work
cannot supply that observation. Source establishes the conditional ordering gap, not frequency. Suggested bucket other.
Confidence likely consequence with that timing premise unverified; cleanup omission confirmed. Coverage searches across
SPEC.md, SPEC_impl.md, empty TODO Planned, plans/queue, BUGS.md, queue/index and TRIAGE_OUTCOMES found no exact
acceptance or repair. Related settings-write and held-reply queue items have different actions and cleanup mechanisms;
no exact cover. proposed_drop none; possible_cover none exact. Provenance: g053 later synthesis after parent accepted
initial no-finding pass; inherited unchanged cleanup exists already at exclusive base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c:e2e/tests/terminal-multihost.spec.ts:4675-4678. Do not attribute introduction
to b91fffda992b0dd586b0a90564b20bac89482db2; its assigned changes only recase the assertions in this test.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g053/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

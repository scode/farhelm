# Retired container isolation tests were deliberately removed

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Retired container isolation tests were deliberately removed

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g137/c1`, `g138/c1`.

### g137/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc

## TLDR

The removed isolation tests raise a potentially highest security concern if old isolation-dependent execution remains
reachable. The specification supports retiring the old positive execution paths and their tests; it does not waive
legacy refusal or confirmed Delete. The replacement fixture proves decoding, refusal and absent-resource deletion, not
removal of a real running container. No concrete product defect is established by the assigned test changes.

## Details

Original concern preserved: the assigned patch removes real Podman account, wrapper, isolation, tabs,
download-confinement and retry-cleanup tests. If the old outside-supervisor execution routes still admitted work, losing
those checks could leave containment failures undetected, potentially exposing host files or credentials. No shipped
reachable route with that consequence has been established here, and rarity would not lower that impact if such a route
were established.

Exact narrow acceptance: SPEC.md, Retired containerized sessions (4816-4823), states: "The earlier experimental model,
whose supervisor stayed outside the container and bind-mounted a host checkout, is retired. Existing sessions support
Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and Replace with are refused. Delete retains
the legacy container-removal path and confirms absence. Missing Podman counts as nothing left to remove." SPEC_impl.md,
Rootless Podman sessions (8647-8656), states: "Remove creation helpers, account selection, launch fields and switches,
hooks-off branches, special tab/attach and file-confinement paths once nothing can create or open these sessions. Normal
container environments require no supervisor-side Podman implementation." The condition "once nothing can create or open
these sessions" remains material. This supports removal of retired positive-path tests only; it is not a blanket
security or cleanup exception.

Source checks from the initial pass: containerized_sessions.rs:1-227, including the context beyond the assigned split,
seeds image_file JSON and asserts the old launch decodes and survives a supervisor restart (:45-105).
farhelm-proto/src/launch.rs:630-668 preserves the retired selection with empty image for image_file-only data;
preservation does not grant launch authority. service/core.rs:9273-9281 gates direct and recovery creates before
allocation. service/teardown.rs:533-577 derives legacy removal from stored launch, acquires the permanent creator lock,
revokes the ticket and requires successful removal before later teardown. container.rs:126-191 retains that lock and
requires Podman's exists status 1, or accepts missing Podman as explicitly permitted above. Its tests at :222-294
separately assert missing-engine cleanup, permanent-inode waiter serialization and rejection of live/error exists
statuses. These source checks established no concrete deletion-contract violation.

Synthesis across batches: git diff --stat from the reviewed revision to 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 showed
no changes to the retained decoder (launch.rs), legacy cleanup implementation (container.rs), replacement
containerized_sessions.rs or shared teststate owner (lib.rs). The later legacy_session fixture still retains its TestDir
through restart and rechecks the persisted launch. The actual shared TestDir Drop at
farhelm-teststate/src/lib.rs:794-818 ends terminal hosts before directory removal, or retains the directory if host end
is unconfirmed. That owner protects the fixture's host/database lifetime; it is not evidence that Podman removed an old
running container. The fixture creates an ordinary host session running true before rewriting its row and never creates
a Podman container. The e2e Delete assertion therefore cannot distinguish successful removal of a running container from
an implementation that only handles absence. The focused fake-engine tests establish control-result semantics, not
real-engine removal coverage. Caller reports g095 independently confirms lazy missing-engine deletion acceptance and
g117/g135 confirm active-launch guards precede filesystem access; those are attributed interface confirmations, not
executions or a parent-accepted drop.

Suggested bucket: other for the established test-coverage consideration. Could-be-highest: yes, preserving the original
concern if a reachable old isolation-dependent execution route is established. Confidence: confirmed specification text
and source analysis for narrow retirement/fixture claims; no runtime observation and no established security route.
Proposed_drop: only the obsolete positive-path test-removal concern, under the exact conditional retirement rule quoted
above. Legacy refusal and confirmed Delete remain required. Independent potentially-highest check remains the caller's;
no parent drop accepted. Possible_cover: none; coverage sources were not consulted. Material caveats: no certification
of the new disposable-environment model, all legacy refusal routes, or real running-container cleanup; the absent
fixture and shared TestDir cannot supply those premises.

Returned proposal (claims-087, event 7): **covered**, suggested bucket **other**.

The specification accepts retiring the old positive execution paths and their tests. This proposed disposition covers
that narrow concern only. Legacy refusal and confirmed Delete remain required; the separate lost Delete oracle is
retained as g137/f1.

Original stronger consequence: Removing isolation/account/wrapper/tab/download-confinement tests could conceal host-file
or credential exposure if retired outside-supervisor execution remains reachable. The original security concern remains
conditional; its rarity would not reduce impact.

Unresolved premise or limitation: A shipped reachable retired execution route that exposes host files or credentials has
not been established. This bounded check does not certify every route or the new environment model.

Proposed basis: Narrow specification acceptance of obsolete positive-path tests, conditional on execution retirement;
not acceptance of missing refusal or Delete tests. Parent must independently check the potentially highest drop.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g137 record](records/review-through-00de0b6c/groups/g137.json). Record ID: `c1`.

### g138/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc

## TLDR

The removed tests exercised container launches that this revision deliberately stops supporting; their removal is not
itself a regression.

## Details

Assigned patch 00322-001 removes the real Podman terminal-host launch/Stop/Delete test and adds refusal tests for
persisted retired sessions. The fixture at containerized_sessions.rs:43-105 creates an ordinary session, replaces its
stored launch with legacy image_file configuration, restarts the supervisor, and asserts the stored old launch was
loaded. The new refusal tests check the retired error, not merely any error. SPEC.md:4816-4823 at this revision says
exactly: ‘The earlier experimental model, whose supervisor stayed outside the container and bind-mounted a host
checkout, is retired. Existing sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone,
Replace and Replace with are refused. Delete retains the legacy container-removal path and confirms absence. Missing
Podman counts as nothing left to remove.’ This explicitly accepts removing support for the old launch route. Suggested
bucket: other for the considered coverage concern; confidence: confirmed by source and specification, no execution.
proposed_drop: specification acceptance quoted above. Could-be-highest: no established shipped security or loss-of-work
route from removing unsupported-launch coverage; deletion's retained cleanup contract still needs separate source
inspection. possible_cover: none; coverage sources not searched.\n\nFollow-up source check: the empty-container E2E
fixture alone cannot distinguish an omitted engine call, but container.rs:271-293 has a fake-engine matrix for exists
exit statuses 0, 1 and 125 and asserts success only for confirmed absence (1), retaining the initialization fence
otherwise. service/teardown.rs:533-577 derives the cleanup coordinates from the stored launch and awaits revoke/remove
with propagated errors before subsequent teardown and row removal. This settles the separate cleanup-contract check
named above. These are source observations, not executed tests.

Discovery-neighbour correction: supplied g137/f1 establishes a distinct retained-Delete test-oracle gap. My earlier
follow-up that the fake-engine exit-status matrix settles the deletion coverage concern was too broad: its code=1
fixture is already absent, ignores whether rm was invoked, and current production correctness does not replace an
existing-object removal assertion. Preserve c1 proposed-drop reasoning only for retired positive launch/Stop coverage,
not retained Delete removal coverage. The exact removal-test concern is already recorded by g137/f1 in supplied
interface context, so do not duplicate it under a new ID here. possible_cover for the retained Delete coverage concern:
g137/f1, supplied neighbour record, not a prohibited coverage search. No parent drop accepted. The proposed drop for
unsupported positive-launch coverage does not erase this independent test-only issue; original consequences remain in
prior events.

Returned proposal (claims-087, event 10): **covered**, suggested bucket **other**.

Same narrow retirement acceptance as g137/c1. The history's claim that the fake-engine matrix settled Delete coverage
was expressly corrected; it must not dispose of g137/f1.

Original stronger consequence: The record initially considered removing launch/Stop/Delete tests together and later
claimed focused tests settled cleanup. Its final correction preserves the distinct lost Delete oracle as g137/f1; the
alias g137/c1 also preserves the conditional host-file/credential-exposure concern.

Unresolved premise or limitation: No demonstrated shipped security or loss-of-work route from positive-path test
retirement. All refusal paths and real-engine cleanup are not certified here.

Proposed basis: Accept only obsolete positive-launch/Stop test retirement. Do not treat the earlier broad cleanup
conclusion as current or as a parent-accepted drop.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g138 record](records/review-through-00de0b6c/groups/g138.json). Record ID: `c1`.

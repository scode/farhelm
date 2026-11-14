# Ordinary Add retains source exclusion during post-Running SSH readiness

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An ordinary new Tensorlake host whose sandbox has reached Running can keep other launches and
snapshot publication for the same recipe waiting while its SSH gateway becomes ready. This is extra serialization across
transport readiness, not established permanent deadlock or data loss.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g025/f4`.

### g025/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: An ordinary new Tensorlake host whose sandbox has
reached Running can keep other launches and snapshot publication for the same recipe waiting while its SSH gateway
becomes ready. This is extra serialization across transport readiness, not established permanent deadlock or data loss.
Details: the assigned create_recorded extraction now awaits backend.wait_tensorlake_ssh after observing Running and
dropping its guard argument (hosts.rs:471-497). Ordinary Add's create_host_sandbox passes None for that argument
(provisioning/tensorlake_add.rs:1599-1616), while its real source_guard remains in the caller: acquired at 1352/1360,
held through the helper await at 1378-1380, released only at 1385. At the immediate parent revision, the outer guard was
dropped at tensorlake_add.rs:1069 BEFORE the separate SSH readiness await at 1074; this is an introduced
extraction-order regression, not a pre-range lock policy. Session launch transfers its real guard into
PreparedSandbox/create and therefore releases it at Running as intended. The readiness backend only runs bounded/paced
transport checks (provisioning/tensorlake_backend.rs:58-89); it takes no recipe source lock, so there is no
source-established recursive-lock cycle. Readiness's nominal minute retry budget can be extended by its last bounded
shell attempt/cleanup; no exact wall-clock duration or occurrence is claimed. Competing source selection takes
lock_snapshot_use (tensorlake/hosts.rs:227); publication also needs it as supplied g045/l1 documents.

Contract: SPEC_impl.md Tensorlake prepared snapshots: 'One per-recipe async mutex excludes source selection/create
through the first observed Running state from snapshot publication and deletion. Installation and supervisor attachment
run outside that exclusion.' Transfer the caller's source guard into create_recorded or keep ordinary Add's readiness
await after the real outer guard is dropped. Full assigned range check: only 222bf6253bcd168db04bcbb1df20180c88306998
later touches provisioning/tensorlake_add.rs; no later tensorlake/hosts.rs change is listed. This record is source
analysis, not executed behavior.

Documentation check is separate: hosts.rs:360-363 says the passed guard ends at Running before the readiness helper; the
implementation does drop THAT parameter at 484. The added comment accurately describes the provider/session transfer and
parameter lifetime but does not account for ordinary Add's still-owned outer guard. The caller's retained 'Running is
the live-process exclusion boundary' comment at 1382-1385 is now false as an effective boundary for Add. No distinct new
documentation defect requiring a separate fix from this lock-transfer issue is asserted; correcting ordinary Add's
ownership repairs the discrepancy. Production's lack of deadlock does not dispose of bounded delay or incorrect
effective-boundary explanation.

Suggested bucket: other for unnecessary launch/build delay and possible longer billed-builder lifetime;
could-be-highest: no for this established route. Preserve original f4 possible-highest ownership suspicion: accidental
adoption/later deletion of an unrelated sandbox remains source-refuted by saved-ID/pending ownership checks; that
original consequence is not silently erased or used to inflate this delay finding. Confidence: confirmed source/contract
regression; no runtime tests or reproductions. proposed_drop: false for the bounded exclusion regression.
possible_cover: confirmed duplicate of supplied g045/l1, same underlying outer-guard/helper-order defect and fix; parent
owns deduplication. This is not a newly discovered independent issue in discovery, since the neighbour record was read
before the source check. Revision reason: earlier candidate only refuted ownership/data-loss/deadlock routes and
improperly left the independently valid bounded-delay caveat unreported. Lead f4 is closed by this finding; no
unresolved premise remains for the lock ordering.

Returned proposal (claims-013, event 19): **retain-verified**, suggested bucket **other**.

Ordinary Add holds its actual source guard in the caller while the extracted helper now waits for SSH after Running. The
helper drops only its None argument, extending exclusion past the specified Running boundary.

Original stronger consequence: The initial candidate considered deleted-source use, unrelated sandbox adoption and later
deletion, or recreation without admitted source; unrelated-resource loss would be highest. Those ownership suspicions
were proposed source-refuted. The initial caveat about post-Running delay was then investigated and promoted to this
retained finding; preserve both dispositions.

Unresolved premise or limitation: No runtime incidence, exact elapsed delay or measured billing increase. The latest
source delay conclusion is established. The proposed g045/l1 alias still needs the parent's comparison of that named
record.

Proposed basis: Retain the lock-transfer regression and its misleading effective-boundary comment together. Send the
original unrelated-resource-loss refutation/lowering to the independent drop check. Parent may consolidate g045/l1 only
after confirming exact cause and fix.

Existing parent disposition (event 19): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-verified-other-source-mutex-delay-with-original-ownership-refutation.
Full013ordinaryAddcallerretainsrealguardaroundcreate_host_sandboxpassingNoneintoextractedcreate_recorded,
whichdropsNoneatRunningthenawaitsSSH; parentpreviouslydroppedrealguardatRunningbeforessh.
NewpostRunningpeerlaunch/buildexclusionpastcontractestablished, boundedattemptsnotrecursivepermanentdeadlock; no
runtimeelapsed/billingclaim. Retainothermutexboundary+falseeffectiverationale, compare045l1samecause/fixbeforealias.
Earlierdeletedsource/unrelatedsandboxadoption/recreationlossroutesnarrowlyrefutedbyadmission/savedID/pending/source-before-send
checks; preserveoriginalhighestpossibilitiesforfreshgate, notsilentlowering.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g025 record](records/review-through-00de0b6c/groups/g025.json). Record ID: `f4`.

# The intermediate sealed-listing change has no helm seal sender yet

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The intermediate sealed-listing change has no helm seal sender yet

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g026/f3`.

### g026/f3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 25 (candidate, open):

Reviewed commit: 47f33bbdbd5ff4a6e437e97a800f9083be43096e

## TLDR
Considered whether ignoring a supervisor's sealed flag can strand a deleted environment. In this intermediate change the helm cannot yet request a seal, so that deletion-recovery route is not established.

## Details
The patch forwards SessionListing.sealed through drain_sessions (:870) and explicitly ignores it in HostActor refresh (:4836-4840), explaining that no helm request seals a supervisor yet. A pinned git grep for DeleteSessionOrSeal across crates/farhelm-helm finds only the SessionListing field documentation in client.rs:665. This supports the comment: no shipped helm sender at this revision. SPEC_impl.md Environment Delete and the seal describes the intended later sender and finisher; this patch adds the carried fact while neither path is active. No source premise establishes a user deletion that seals through this helm but is then ignored. Suggested bucket for a stranded disposable resource would be high/material UX and continued billing. Could-be-highest: no established loss-of-work/security route; an unsolicited seal field from an untrusted ordinary supervisor is not authority to destroy it. Confidence: confirmed source-only absence of a sender at this revision. proposed_drop: intermediate protocol plumbing with no active seal-producing route; retain this record for parent check against adjacent group integration. possible_cover: none. No execution.

Synthesis turn 3 checked the later integration narrowly at inclusive endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, without adding mandatory review scope. The sender now exists at client.rs:3844-3869 and is called by sessions/sandbox.rs:223 after session-owned/machine-deleting binding and provisioning-lock checks (:167-195). Sealed replies synchronously finish destruction (:257-262). Recovery now consumes listing.sealed at manager.rs:5117-5132, gated by seal_is_actionable (:2699-2707: session-owned AND removal_deletes_machine); ordinary hosts only get a problem. That handler is attributed by pinned blame to 222bf6253bcd168db04bcbb1df20180c88306998. sessions/sandbox.rs:339-361 supplies the pending-seal finisher, calling finish_seal, whose locked path records outcome and invokes hosts::finish_sealed_environment (:281-304); lib.rs:2521 starts it in production. This removes the earlier uncertainty about whether final integration consumes the flag. The original suspected consequence, stranded environment/billing after an ignored seal, is not established for the reviewed intermediate 47f33bb revision (no own sender) or the checked endpoint (sender plus consumer/finisher). This is not a general audit of later deletion implementation, mixed-version releases or all finisher failure paths. proposed_drop remains source refutation/phase plumbing, pending parent disposition; no parent acceptance assumed. No runtime observation.

Reopened synthesis turn 5, named g024/l5: independently compared both assigned manager patches with the stopped-actor branch. The stopped local Arc/wait scope remains byte-equivalent in shape from 56659839785691f76da3af5d8f51b511cff4ba8b:3845-3892 to endpoint:4054-4101; neither sealed forwarding/destructuring nor removal of two fixture fields changes it. The later manager sealed handler upgrades within its own if-let (:5121), outside the stopped loop. Therefore this candidate’s narrow no-sender/then-consumer conclusion is not a claim that manager ownership is otherwise correct and does not refute or repair g024/l5. The direct-drop resource-lifetime issue stays with its original owner; explicit graceful shutdown still cancels actors. No previous consequence, proposed-drop basis or parent disposition changed.

Returned proposal (claims-014, event 11): **refuted**, suggested bucket **high**.

The intermediate listing-plumbing revision has no helm seal sender. At the inclusive endpoint, the sender is paired with both synchronous finishing and listing-driven recovery, so the exact ignored-seal theory is not established.

Original stronger consequence: A deleted environment could remain stranded and continue billing if the helm ignores its supervisor's seal. High fits that material UX/billing consequence; no user-work loss or security route was established.

Unresolved premise or limitation: No shipped self-produced seal ignored by this helm was shown at the intermediate revision; endpoint consumption is explicit. Failure routes beyond flag consumption were not assigned here.

Proposed basis: Intermediate protocol plumbing has no own producer, and the checked completed integration supplies the consumer and finisher.

Existing parent disposition (event 11): excluded. Parent explicitly accepted named refuted proposal from claims-014 after inspecting it. The intermediate listing-plumbing revision has no helm seal sender. At the inclusive endpoint, the sender is paired with both synchronous finishing and listing-driven recovery, so the exact ignored-seal theory is not established. Intermediate protocol plumbing has no own producer, and the checked completed integration supplies the consumer and finisher. Checked source/results: ["47f33bbdbd5ff4a6e437e97a800f9083be43096e:manager.rs:4833-4839 deliberately ignores sealed before sender integration. Pinned git grep DeleteSessionOrSeal across farhelm-helm returns only client.rs:665 documentation, no request sender.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:sessions/sandbox.rs:167-195 admits only session-owned machine-deleting hosts under lifecycle/provisioning exclusion; :223 sends delete_session_or_seal; :257-262 synchronously finishes a Sealed reply.","At the endpoint manager.rs:2699-2707 defines actionable seals and :5117-5132 consumes the listing flag, noting an actionable seal or retaining an ordinary-host problem. sessions/sandbox.rs:281-304 records finish outcomes and invokes destruction; :339-361 drives pending seals. lib.rs:2521 starts that finisher in production.","This is not a claim about all finisher failures, mixed-version operation or provider deletion correctness. No runtime incident was reproduced."] Current coverage/results: {"SPEC.md":"Sandbox sessions:1514-1538 requires sealing plus eventual environment destruction/retry, rather than accepting ignored seals.","SPEC_impl.md":"Environment Delete and the seal:6106-6159 describes the same sender/consumer/finisher. The checked integration implements the relevant flag consumption.","TODO.md Planned":"Empty.","plans/queue":"environment-agent-operations.md:509-531 describes the intended seal and recovery integration; source now implements it, so no unlanded-plan cover is asserted.","BUGS.md":"Unrelated SSH transfer-output hang only.","INDEX.md and queue files":"Searched seal/stranded environment and named source mechanisms; no exact ignored-flag item found.","TRIAGE_OUTCOMES.md":"No matching active decision found. Archive not read.","FILTER.md":"No filter invoked; billing/resource stranding was not discarded as merely rare."} Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "A deleted environment could remain stranded and continue billing if the helm ignores its supervisor's seal. High fits that material UX/billing consequence; no user-work loss or security route was established."; "No shipped self-produced seal ignored by this helm was shown at the intermediate revision; endpoint consumption is explicit. Failure routes beyond flag consumption were not assigned here.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f3`.


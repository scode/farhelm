# Replacement SIGCONT has a spec-accepted short PID-reuse window

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: If the old server exits and its pid is reused immediately after SIGTERM, the following SIGCONT could resume an unrelated stopped process. The original consequence remains; no wrong-target execution was observed. Suggested bucket: other for the source-supported SIGCONT possibility. Could-be-highest: uncertain; security or work-loss consequences would require a harmful unrelated stopped process and immediate PID reuse, neither established in a shipped scenario. Confidence: confirmed source sequence with a hypothetical reuse premise, explicitly accepted by specification, not reproduced. Lead resolution: hosts/process.rs:416–452 documents the SIGTERM/SIGCONT/SIGKILL sequence and explicitly states that SIGCONT follows SIGTERM without its own check. git diff of this file between the reviewed commit and its parent is empty; the sequence is not introduced by the assigned deletion. terminate_process_within at 490–498 sends SIGTERM and SIGCONT back-to-back with no intervening await or unbounded work; SIGKILL at 502–507 revalidates after its await. proposed_drop: exact SPEC_impl.md:5105–5118 acceptance, under Runtime state, says: "Code that signals or adopts a process by its number may lose the race in which that process exits and the kernel hands the number to an unrelated process within a short, bounded window: between reading a number and checking it, between reaping a child and signalling its group, while a watchdog fires just as its child finishes, or during a polling loop of at most a few seconds." It further says: "What is not accepted is carrying a bare number across unbounded

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g162/l4`.

### g162/l4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: If the old server exits and its pid is reused immediately after SIGTERM, the following SIGCONT could resume an unrelated stopped process. The original consequence remains; no wrong-target execution was observed. Suggested bucket: other for the source-supported SIGCONT possibility. Could-be-highest: uncertain; security or work-loss consequences would require a harmful unrelated stopped process and immediate PID reuse, neither established in a shipped scenario. Confidence: confirmed source sequence with a hypothetical reuse premise, explicitly accepted by specification, not reproduced. Lead resolution: hosts/process.rs:416–452 documents the SIGTERM/SIGCONT/SIGKILL sequence and explicitly states that SIGCONT follows SIGTERM without its own check. git diff of this file between the reviewed commit and its parent is empty; the sequence is not introduced by the assigned deletion. terminate_process_within at 490–498 sends SIGTERM and SIGCONT back-to-back with no intervening await or unbounded work; SIGKILL at 502–507 revalidates after its await. proposed_drop: exact SPEC_impl.md:5105–5118 acceptance, under Runtime state, says: "Code that signals or adopts a process by its number may lose the race in which that process exits and the kernel hands the number to an unrelated process within a short, bounded window: between reading a number and checking it, between reaping a child and signalling its group, while a watchdog fires just as its child finishes, or during a polling loop of at most a few seconds." It further says: "What is not accepted is carrying a bare number across unbounded waits or asynchronous work, or storing it for later." (line wraps normalized only). The back-to-back continuation signal fits the short accepted window; the source's own termination docstring cites this exact specification acceptance. possible_cover: none beyond that specification rule. Caveats: retained as a finding for independent checking because the possible impact boundary is uncertain; no drop has been accepted by the parent. This source check resolves l4's missing contract/change premise and leaves no open lead. It does not decide g128's guarded-Delete or g111's private-spare residuals and does not weaken f1's process/history-loss consequence. No tests or reproductions performed.

Returned proposal (claims-103, event 9): **covered**, suggested bucket **other**.

The back-to-back SIGTERM/SIGCONT sequence has the stated conditional wrong-process continuation window. The implementation specification expressly accepts this bounded process-number reuse race. No runtime occurrence or harmful unrelated-process consequence was established.

Original stronger consequence: An unrelated stopped process could be resumed after immediate reuse of the old server's PID; possible security or user-work harm was left uncertain, not disproved.

Unresolved premise or limitation: Actual immediate PID reuse and a stopped replacement process whose resumption causes security or work-loss harm were not established.

Proposed basis: Exact bounded-window specification acceptance, limited to the back-to-back continuation signal. This is a proposed coverage disposition, not approval of the drop.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g162 record](records/review-through-00de0b6c/groups/g162.json). Record ID: `l4`.


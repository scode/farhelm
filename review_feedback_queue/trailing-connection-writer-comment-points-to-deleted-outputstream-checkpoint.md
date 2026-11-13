# Connection writer diagnostics still point to the deleted stream checkpoint

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: A maintainer tracing terminal-output loss is told to consult a first diagnostic checkpoint in a stream implementation this commit deletes.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g170/l4`, `g171/f5`.

### g170/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: A maintainer tracing terminal-output loss is told to consult a first diagnostic checkpoint in a stream implementation this commit deletes. Details: connection.rs:985–987 says 'Output-path checkpoint; see the first one in `OutputStream::next_output`. Data frames only: control replies are not what that path loses.' The assigned deletion removes the OutputStream type/ownership and the whole stream.rs file, including next_output in the adjacent batch. At the immediate parent, stream.rs:1206 defines next_output and :1320–1342 contains the expressly named first debug checkpoint, explains its downstream writer-checkpoint map and logs pane-output decode lengths. Thus the comment used to point to a real diagnostic map. The excluded range base 6e423e30068b62c6cfa276c6d720ee35bb70387a also has the same stream map at :1320–1342 and the referring writer comment at connection.rs:1035–1037. Whole-path git log over the pinned range shows only this deletion changes stream.rs, establishing introduction of the missing target within the reviewed range, rather than assuming parent equivalence establishes pre-range behavior. At the reviewed commit an allowed-source git grep over crates, docs and SPEC files finds this as the only remaining reference to the deleted stream/cutover API set; there is no replacement alias or executable target. Later-range diff of connection.rs through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes other comments/types but not this pointer; direct tip read confirms the same comment still at :985–987. No later repair. Correct grid production behavior does not refute a stale diagnostic instruction. Governing contract: root documentation instructions require comments to give accurate surrounding context and lifecycle constraints; SPEC_impl.md Terminal host/Grid streams describes the actual retained output path, and Terminal substrate: private tmux server explicitly says the driver is gone. Suggested fix: remove the retired cross-reference or replace it with an accurate map to the host/grid diagnostics; do not restore retired stream code. Suggested bucket: other/documentation correctness. could-be-highest: no established security or loss-of-work route from this pointer. Confidence: confirmed source/history analysis, not execution or a generated-doc check. proposed_drop: false. possible_cover: none in the completely read discovery-neighbours.json; no forbidden coverage sources searched. Caveats: runtime data-frame logging still executes, and no malfunction or missing mandatory diagnostic event is alleged. The residual reference was noted incidentally in c2, but this pass separately validated its historical target, introduced documentation defect and lack of later repair; c2's runtime-dependency refutation remains distinct. Scope: caller context checked solely for the implications of assigned OutputStream/file deletion. Lead l4 closed as a finding; no missing premise remains.

Returned proposal (claims-107, event 9): **retain-verified**, suggested bucket **other**.

The retained writer comment directs maintainers to a deleted first checkpoint. Historical source establishes that the reference was valid at the range base and lost its target in-range. The pointer still exists at the reviewed tip and recording-time main.

Original stronger consequence: Maintainers tracing terminal-output loss are sent to a deleted checkpoint and explanation. Earlier records expressly disclaimed a runtime loss, leak or logging malfunction.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record one other-priority source-comment finding with both g170/l4 and g171/f5 provenance. Preserve historical validity, in-range deletion and endpoint persistence.

Existing parent disposition (event 9): verified. Parent explicitly accepted named retain-verified proposal from claims-107 after inspecting it. The retained writer comment directs maintainers to a deleted first checkpoint. Historical source establishes that the reference was valid at the range base and lost its target in-range. The pointer still exists at the reviewed tip and recording-time main. Record one other-priority source-comment finding with both g170/l4 and g171/f5 provenance. Preserve historical validity, in-range deletion and endpoint persistence. Checked source/results: ["6e423e3:service/connection.rs:1035-1037 points to OutputStream::next_output; tmux/stream.rs:1320-1342 contains the first checkpoint, diagnostic hop map and no-payload rationale. Thus the reference was valid at the exclusive base.","1ed482d deletes tmux/stream.rs. The scoped range history has only this stream-file change and only this matching reference/target change; 1ed482d..00de0b6 connection.rs diff leaves the pointer untouched.","00de0b6:service/connection.rs:985-993 still contains the reference and working data-frame length debug event; source grep finds no executable OutputStream target. Current connection.rs:986 retains it too.","No runtime logging failure, disclosure, transport failure or generated-document failure is alleged or reproduced. Source comments are within scope even though ordinary workflow/docs files are excluded as independent targets."] Current coverage/results: {"SPEC.md":"Searched terminal/output/diagnostic terms; no accepted stale diagnostic-map behavior.","SPEC_impl.md":"Terminal host, emulator and Grid streams describe the retained path; the old-server section says only startup retirement remains. None covers a false source-navigation pointer.","TODO.md Planned":"Empty.","plans/queue":"INDEX has no unlanded plans. tmux-drop-from-docs criterion 3/5 broadly required comment cleanup, but REPORTS.md:32 says it already landed. The still-present pointer is not covered by that completed plan.","BUGS.md":"No related accepted issue.","queue INDEX and matching files":"Full INDEX read and searches for OutputStream, stream.rs, deleted checkpoint and output-trace terminology found no exact existing item. g171/f5 is the exact named alias; unrelated stale-comment items concern other targets.","TRIAGE_OUTCOMES.md":"No exact active diagnostic-pointer decision; archive unread.","FILTER.md":"Rare-imprecise-diagnostic filter does not fully match: this persistent false documentation pointer exists whenever followed, with no required rare runtime trigger. Retain."} Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Maintainers tracing terminal-output loss are sent to a deleted checkpoint and explanation. Earlier records expressly disclaimed a runtime loss, leak or logging malfunction."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g170 record](records/review-through-00de0b6c/groups/g170.json). Record ID: `l4`.

### g171/f5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR
A maintainer following the terminal-output diagnostic trail is directed to a checkpoint and explanation that this commit deletes. The remaining logging still works; the new defect is a misleading diagnostic documentation pointer.

## Details
The assigned deletion removes stream.rs:1206 onward, including next_output and its first-checkpoint explanation at parent-side :1320 onward (assigned patch 00957-001.patch:116-135). The retained writer comment in service/connection.rs:985-987 still says: "Output-path checkpoint; see the first one in `OutputStream::next_output`. Data frames only: control replies are not what that path loses." At the reviewed revision there is no OutputStream or next_output implementation to follow; the viewer uses the grid path. The deleted explanation identified the hop-by-hop diagnostic chain and its no-payload logging policy, so this is an operational source-navigation/documentation defect rather than evidence that the logger itself leaks content or loses frames. Correctness of the surviving logger does not refute its false pointer. Fix direction: describe the retained frame-writer trace in terms of the actual host/grid output path or remove the defunct cross-reference while preserving any accurate tracing rationale.

Suggested bucket: other/documentation correctness. Could-be-highest: no established security or loss-of-work route. Confidence: confirmed pinned source and assigned deletion; whole-range attribution and endpoint persistence now checked. proposed_drop: false. possible_cover: refreshed interface record g170/l4 now independently identifies this exact writer checkpoint pointer, including valid base/parent target and persistence at tip; preserve both provenance IDs for parent consolidation. The prior snapshot had no exact duplicate; g008/f2 remains a distinct stale Terminal architecture pointer in SPEC_impl.md. This is reconciliation only, not a new discovery or an accepted drop. No prohibited coverage source read. Material caveat: no runtime logging failure, credential leak or production transport defect is alleged. Source-only analysis; no tests or reproductions. Whole-range check: the exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a and immediate parent both contain this writer pointer at connection.rs:1036 and the actual next_output first-checkpoint explanation at stream.rs:1206,1320-1335. The pointer was valid at the range base; deleting its target in 1ed482d introduces the mismatch. A scoped git log -G identifies only 1ed482d changing matching references in the range, and the pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains this exact writer pointer at connection.rs:986. No later repair or open source premise remains.

Returned proposal (claims-107, event 16): **duplicate**, suggested bucket **other**.

Exact same writer comment, deleted target, introduction and diagnostic consequence as g170/l4. Retain one verified finding with both provenance IDs; this is not a drop of the defect.

Original stronger consequence: A maintainer following terminal-output diagnostics is directed to a deleted checkpoint/explanation. Every history revision explicitly preserves that narrow documentation consequence and disclaims runtime loss/leak/malfunction.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact named-claim deduplication into the g170/l4 recording, retaining both original histories/provenance and all caveats.

Existing parent disposition (event 16): duplicate. Full107 exact same retained connection writer comment pointing toOutputStream::next_output, validbase target deleted1ed and still dangling at tip. Canonical170l4event9 retained. Preserve data-length debugevent stillworks, no logging failure/disclosure/transport defect inferred; both original owners retained as provenance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g171 record](records/review-through-00de0b6c/groups/g171.json). Record ID: `f5`.


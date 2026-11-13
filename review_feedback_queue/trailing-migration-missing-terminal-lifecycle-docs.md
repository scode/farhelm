# Migration commit has stale lifecycle docs, corrected by later removal

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: this intermediate commit teaches that old-terminal absence reflects a terminal that failed to survive restart, and that Interrupted identifies boot classification, despite intentional retirement producing both absence and Interrupted.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g117/c3`.

### g117/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR: this intermediate commit teaches that old-terminal absence reflects a terminal that failed to survive restart, and that Interrupted identifies boot classification, despite intentional retirement producing both absence and Interrupted. Details: terminals.rs:817-833 versus core.rs:6965-6969 (old rows unconditionally resolve to no terminal) and :7097-7114 (migration interrupts rows last launching, running or stopping). SPEC.md:3003-3016 and SPEC_impl.md:2979-2983 require this deliberate retirement. The wrong explanation can change a maintainer's interpretation of terminal presence probing and the causes of Interrupted. Suggested bucket: other/documentation correctness. Confidence: confirmed source/contract analysis; no execution performed. could-be-highest: no. proposed_drop: false for the reviewed historical commit, with parent retaining disposition; the initial absent-user-harm exclusion is withdrawn. possible_cover: direct source history proves correction in 1ed482dde220452e424973faf38a5e1318986333, the next selected commit: git log -S 'An entry marked' identifies that commit, and its terminals.rs diff replaces the obsolete helper documentation with explicit boot-or-update Interrupted causes, terminal-host unavailability, and old-kind startup retirement; it also replaces the nearby restart-gap resolver comment. Endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:terminals.rs:150-184 retains the corrected explanation. No coverage/filter sources searched. Material caveat: no remaining endpoint defect established, and no runtime malfunction established at either revision; this is the concrete intermediate documentation misinformation required by historical review. Original consequence and correction are both preserved so the parent can decide final disposition.

Returned proposal (claims-076, event 9): **needs-parent**, suggested bucket **other**.

The intermediate documentation defect is verified, and its correction within the review range is verified. It is neither source-refuted at the reviewed commit nor a remaining endpoint defect. Parent must choose the already-corrected historical disposition; do not revive the withdrawn absent-user-harm rationale.

Original stronger consequence: Maintainers could infer old terminal presence is still probed and Interrupted means reboot. No security, work loss or incorrect runtime behavior was established. Initial proposed exclusion solely for no user-facing harm was withdrawn.

Unresolved premise or limitation: No endpoint defect remains. The available disposition vocabulary has no 'fixed within range'; needs-parent preserves that distinction.

Proposed basis: Verified historical defect, corrected by 1ed482dd before the endpoint; retain both facts in the parent's completion record.

Existing parent disposition (event 9): rejected. Parent read full claims076. dbfc6975's new old-terminal retirement leaves service/terminals806-850 teaching only failed-survival/boot interruption despite same-boot deliberate retirement. Real historical maintenance-documentation defect;1ed482 corrects151-182 and path remains corrected through00de. Preserve withdrawn no-user-harm dismissal, actual defect and exact repair; no residual queue item, not refuted or out-of-range.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g117 record](records/review-through-00de0b6c/groups/g117.json). Record ID: `c3`.


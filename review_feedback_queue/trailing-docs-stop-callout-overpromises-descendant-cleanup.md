# Exclusive range base already contains the broad Stop claim and conflicting cleanup caveat

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The intermediate capture source overpromises that Stop ends everything the agent started,
although detached jobs may remain. Whole-range attribution now checked: at the exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a, e2e/docs-shots/stop-restart-resume.spec.ts:53-56 already contains exactly
'Stop: ends the agent and everything it started. The session and its terminal stay.' The same base's SPEC.md:2038
already says jobs a user-run tmux moves outside its groups may survive Stop, Restart, Replace, Delete or tab close, and
:2048 says exactly 'a detached process that hides or overwrites its environment survives stop and delete there.'
Therefore both the misleading generated prose and the governing descendant-cleanup exception predate the entire reviewed
range, not merely the immediate reviewed parent. Bounded git log of exclusive base through
2c5a1d5a8ebe7c0d0889ac5db40b272d75ae02cb for the assigned capture file returns only that reviewed commit. Its change
adds tab closure to the already-broad row-menu claim, plus a new header callout that does not promise all descendants
end. Later assigned commit 53d618e4a033749efc9f9f10adef168643737b99 corrects the row-menu prose and moves its
annotation; that repair is separate from the attribution proof and would not erase an introduced intermediate defect.
Suggested bucket: other, generated documentation/capture wording. Confidence: confirmed by exclusive-base, bounded
history, reviewed and later source; no runtime or visual execution. proposed_drop: exact pre-range source proof for both
sides of the contradiction, not scope excl

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g148/c1`.

### g148/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (candidate, open):

Reviewed commit: 2c5a1d5a8ebe7c0d0889ac5db40b272d75ae02cb. TLDR: The intermediate capture source overpromises that Stop
ends everything the agent started, although detached jobs may remain. Whole-range attribution now checked: at the
exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a, e2e/docs-shots/stop-restart-resume.spec.ts:53-56 already
contains exactly 'Stop: ends the agent and everything it started. The session and its terminal stay.' The same base's
SPEC.md:2038 already says jobs a user-run tmux moves outside its groups may survive Stop, Restart, Replace, Delete or
tab close, and :2048 says exactly 'a detached process that hides or overwrites its environment survives stop and delete
there.' Therefore both the misleading generated prose and the governing descendant-cleanup exception predate the entire
reviewed range, not merely the immediate reviewed parent. Bounded git log of exclusive base through
2c5a1d5a8ebe7c0d0889ac5db40b272d75ae02cb for the assigned capture file returns only that reviewed commit. Its change
adds tab closure to the already-broad row-menu claim, plus a new header callout that does not promise all descendants
end. Later assigned commit 53d618e4a033749efc9f9f10adef168643737b99 corrects the row-menu prose and moves its
annotation; that repair is separate from the attribution proof and would not erase an introduced intermediate defect.
Suggested bucket: other, generated documentation/capture wording. Confidence: confirmed by exclusive-base, bounded
history, reviewed and later source; no runtime or visual execution. proposed_drop: exact pre-range source proof for both
sides of the contradiction, not scope exclusion or correctness of production cleanup. Original inaccurate-prose
consequence remains intact. Could-be-highest: no established security or loss-of-work route from this Stop description.
possible_cover: none searched. Caveat: parent has not accepted a drop; this event supplies the missing whole-range
premise for independent judgment. This revises c1's earlier immediate-parent-only proposed-drop basis without deleting
its history.

Returned proposal (claims-095, event 6): **out-of-range**, suggested bucket **other**.

The intermediate screenshot source does overpromise descendant cleanup, but both the broad wording and its conflicting
cleanup exception exist at the exclusive range base. The reviewed change adds tab closure, not the overpromise. The
later repair is corroboration, not the attribution basis.

Original stronger consequence: The intermediate generated screenshot tells readers Stop ends everything the agent
started although detached jobs may remain. Earlier records first relied on the immediate parent and later repair; the
latest record supplies whole-range base proof. That inaccurate-prose consequence is preserved.

Unresolved premise or limitation: None for pre-range attribution. A security or loss-of-work consequence caused by
relying on this screenshot has not been shown.

Proposed basis: Exclude only this pre-range documentation defect, using the exact exclusive-base contradiction. Do not
use acceptable production cleanup, ordinary-docs scope exclusion or subsequent repair as a substitute. Flagged
conservatively for independent judgment because dismissal concerns a process-termination guarantee, although only
documentation correctness is established.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g148 record](records/review-through-00de0b6c/groups/g148.json). Record ID: `c1`.

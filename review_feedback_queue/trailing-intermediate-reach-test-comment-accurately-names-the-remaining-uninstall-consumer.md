# Intermediate reach-test comment accurately names the remaining uninstall consumer

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Intermediate reach-test comment accurately names the remaining uninstall consumer

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g045/c9`.

### g045/c9

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 27 (candidate, open):

# The revised executable-path explanation is valid at its recorded revision

Reviewed commit: 17a3fe858c5511a24b7403af15dd41c93bf69fd7.

## TLDR

Checked whether the changed test comment falsely says uninstall runs the accepted host tmux after its unit pinning was removed. At this intermediate revision uninstall still uses exactly that accepted executable, so the new explanation is supported. No documentation defect is reported.

## Details

The assigned patch replaces the unit-pinning rationale with 'The EXECUTABLE, not its directory: uninstall runs it to end the private tmux server.' The fixture establishes an absolute /usr/bin/tmux meeting the acceptance floor and asserts that exact host_tmux value (provisioning.rs:7937-7947). plan_uninstall passes reach.host_tmux directly to StopSupervisor, falling back to lib/tmux only when it is None (provisioning/plan.rs:1151-1159). SystemBackend::stop still accepts tmux_program and tmux_socket, explaining direct private-server cleanup after the service stop (provisioning/backend.rs:3259-3273,3307-3318). Thus removal of unit pinning does not make this distinct uninstall rationale stale at 17a3. Later 500 removes that separate cleanup dependency; its later contract is not evidence against an intermediate statement that was true when committed. Suggested bucket: other for the considered documentation issue. Could-be-highest: no, this test explanation does not introduce a security or work-loss instruction. Confidence: confirmed source refutation, no execution. proposed_drop: direct surviving consumer supports the comment at the reviewed revision. possible_cover: none. Caveats: this check does not endorse later uninstall cleanup changes or revise c5/l2; those have separate recorded consequences and drop bases. No tests, builds, reproductions or live actions performed.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `c9`.


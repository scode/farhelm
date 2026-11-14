# Old tmux terminals lose restart history (specified behavior)

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Restart of a session whose old tmux terminal was ended starts without its old terminal history.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g110/f1`.

### g110/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Restart of a session whose old tmux terminal was ended
starts without its old terminal history. Details: assigned patch removes TmuxSeedFault and the
tmux_seed_gate/tmux_seed_fault hooks and narrows RelaunchPlan.seed to host history (core.rs:4007–4016); this retires the
path that copied history from an old tmux pane. Suggested bucket: highest if loss of retained terminal output is treated
as loss of user-owned work; could-be-highest: yes. Confidence: confirmed from source and explicit specification, not
execution. proposed_drop: SPEC.md, Lifecycle operations, lines 1945–1947 explicitly says: "When the terminal no longer
exists (the process that kept it ended, after a reboot, or an old terminal this release ended, see
[Terminal experience](#terminal-experience)), Restart starts with an empty one." SPEC_impl.md, Terminal substrate:
private tmux server, says "This release ends them at startup (below); every terminal runs in a terminal host" and "The
running supervisor runs no tmux at all, and its driver of the old terminals is gone". This candidate concerns the
intentional removal of history migration only; it does not claim an independent defect in old terminals or startup
killing. possible_cover: none searched. Caveat: requires independent drop check because the consequence could be
highest. Synthesis, turn 2: parent reports independently logged g099/f1 and g053/f1 with the same accepted old-terminal
history/termination consequence; these are possible duplicates, not an accepted drop. Preserve the original
retained-output loss claim and could-be-highest treatment. The proposed specification basis is limited to intentional
retirement/history loss. It does not cover generic survival guidance (parent names g004/g071) or an omitted failure
warning (parent names g033); those distinct issues are not resolved by this record. No peer logs or prohibited coverage
sources were searched, and no parent or independent drop verdict has been accepted.

Returned proposal (claims-071, event 6): **covered**, suggested bucket **highest**.

Loss of old-terminal scrollback on Restart is an explicit product decision. Preserve the potential user-owned-output
consequence and require independent agreement before omitting it.

Original stronger consequence: Restart loses retained old tmux output, potentially user-owned work. Acceptance concerns
intentional retirement/history loss only, not generic survival guidance or an omitted warning.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Propose specification coverage limited to intentional old-terminal/history retirement; parent must
independently check it and reconcile the history-supplied aliases rather than assuming their drops.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g110 record](records/review-through-00de0b6c/groups/g110.json). Record ID: `f1`.

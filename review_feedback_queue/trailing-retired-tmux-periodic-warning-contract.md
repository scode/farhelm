# Old-terminal warning tests are obsolete under the migration contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Old-terminal warning tests are obsolete under the migration contract

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g131/c2`, `g132/c2`.

### g131/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 2 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333

## TLDR
Deleting periodic old-terminal warning tests agrees with the release's intentional removal of old terminals.

## Details
Considered whether deleting the three old-terminal notification lifecycle tests leaves users unaware of running old agents or resolves warnings too early. SPEC.md:2799-2809 (Status; exact passage located in the full section read) explicitly says: "That release is this one, so no warning is recorded any more, and every supervisor start marks any unresolved one resolved." It also specifies a new notice for each still-live old agent ended at startup. This replaces, rather than preserves, the deleted tests' contracts of one periodic warning while old agent/tab panes remain live. core.rs:6144-6151 starts the old-server termination step for the owning supervisor; :7077-7091 invokes old_terminals::record_ended after committed conversion, even with no converted rows; core/old_terminals.rs:517-546 documents and implements recording the new notice and resolving the old warning kind. No periodic old-terminal warning is now expected.
Suggested bucket: other (not reported). Confidence: confirmed by source and specification, no execution. proposed_drop: true; explicit specification replacement quoted above, corroborated by startup callers. Could-be-highest: no established security or loss-of-work consequence from deleting these notification tests; the intentional old-process termination is specified behavior outside this test-deletion concern. possible_cover: none; coverage sources not consulted. Caveat: this checks the assigned notification-test removal and replacement's caller contract, not an independent audit of all old-terminal termination mechanics.

Returned proposal (claims-083, event 2): **covered**, suggested bucket **other**.

The removed tests require periodic warnings for terminals the release intentionally retires. The replacement startup contract explicitly resolves those warnings and records a different persistent notice. This does not adjudicate whether ending old processes is safe in every failure case.

Original stronger consequence: Deleting the old notification lifecycle tests might leave users unaware of still-running old agents or resolve warnings too early. No security or work-loss route from the test deletion itself was established. The separate intentional startup termination policy and its mechanics were outside this candidate's audit.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification replacement, corroborated by startup callers. Conservatively register the proposed omission because its surrounding migration ends user processes; agreement must stay confined to obsolete periodic-warning tests. g132/c2 overlaps this warning question but also considers death/reap work, so it is not an exact alias.

Existing parent disposition (event 2): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g131 record](records/review-through-00de0b6c/groups/g131.json). Record ID: `c2`.

### g132/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Synthesis confirms that deleting the ticker's old-terminal-warning and tmux observation work does not omit a continuing responsibility for supported terminals. Original considered consequence retained: legacy sessions could lose needed status/death/tab-reap updates or old-terminal warnings could remain unresolved after the periodic reconciliation is removed. Checks/results: service/terminals.rs:122-141 returns HostTerminal and its :175-176 identifies old tmux rows as ended by startup. service/core.rs:6142-6184 calls old-terminal termination before reload; :7080-7090 invokes old_terminals::record_ended for writable reloads even when the converted/ended list is empty. service/core/old_terminals.rs:525-546 records the new OldTerminalEnded notices separately and then resolves all OldTerminal warnings unconditionally with respect to ended-list membership; resolution failure is logged, and :522-524 documents retry at a later start. A complete supervisor-src reference search finds no remaining OldTerminal warning producer or reconcile_old_terminal_notifications call, only the startup resolver, enum compatibility and documentation. Exact contract: SPEC.md:2807 says 'no warning is recorded any more, and every supervisor start marks any unresolved one resolved'; :2811-2813 separately says nothing resolves the new notice. SPEC_impl.md:2883-2891 states 'This release ends them at startup (below); every terminal runs in a terminal host' and 'The running supervisor runs no tmux at all, and its driver of the old terminals is gone'. Those contracts and callers support retiring the ticker responsibilities while preserving the startup-only distinction between resolved warnings and persistent new notices. Suggested bucket: other for the considered missing periodic reconciliation claim. Confidence: confirmed source/contract refutation; no execution. proposed_drop: the claimed continuing ticker responsibility is refuted by the new terminal type and the cited startup resolver/contract. possible_cover: none. Could-be-highest: no for this considered claim; no route from this ticker deletion to killing user work is asserted. Caveats: does not judge or propose dropping a separate finding about startup termination's destructive policy, accepted cleanup failure residuals or database-failure behavior; those are distinct from whether the removed periodic warning/reap code is still required.

Returned proposal (claims-083, event 8): **covered**, suggested bucket **other**.

The new terminal model removes the continuing periodic tmux responsibilities. Warning resolution is performed during writable startup reload even when no old agent rows were converted; new termination notices remain separate and unresolved. No supported legacy runtime is left needing pane-map sampling or tmux-tab reap.

Original stronger consequence: Legacy sessions might lose needed status/death/tab-reap updates, or old warnings might remain unresolved after periodic reconciliation is removed. No route from this ticker deletion to killing user work was asserted. Separate destructive startup policy, cleanup residuals and database-failure behavior are expressly outside this proposed omission.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification retirement plus source refutation of the missing startup resolver. Conservatively register the proposed omission because the broader migration ends user processes; the check must not swallow a separate startup termination finding.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g132 record](records/review-through-00de0b6c/groups/g132.json). Record ID: `c2`.


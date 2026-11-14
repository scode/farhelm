# Old-terminal process wiring gap is historical after later source retirement

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deletion removes a concrete real-process regression oracle for old terminals while the reviewed
intermediate revision still serves them. This remains a confirmed test-only finding, not an observed product failure.
Original claim preserved: adoption and byte-stream serving, migration state and notification resolution,
last-private-server shutdown, and unusable-tmux lifecycle protection lose their combined shipped-process check; retained
tmux-driver and ticker tests cover mechanisms but not that process wiring. Governing references: SPEC.md at
647f0227:3013-3038 and SPEC_impl.md:2294-2296 retain old compatibility; removed tests are
an_old_session_after_the_switch_to_hosts_is_reported_as_one_to_move,
the_private_tmux_server_ends_with_the_last_old_terminal and old_terminals_without_a_usable_tmux_are_reported_not_alive
in host_processes.rs. Repair at the reviewed revision: seed a real earlier-build server and corresponding persisted
session, then start the bare supervisor and assert its listing/stream/lifecycle behavior, without restoring production
creation of new tmux terminals. Later retirement checked separately: git history first adds
service/core/old_terminals.rs in dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. At pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, SPEC.md:2998-3010 explicitly changes the contract: 'A supervisor that finds
that tmux server when it starts ends it, and with it everything that still ran in an old terminal, without needing a
tmux program.' SPEC.md:2804-2813 replaces the migration warning with an ended-terminal notice. Complete SPEC_impl.md
sectio

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g151/l1`.

### g151/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: deletion removes a concrete real-process regression
oracle for old terminals while the reviewed intermediate revision still serves them. This remains a confirmed test-only
finding, not an observed product failure. Original claim preserved: adoption and byte-stream serving, migration state
and notification resolution, last-private-server shutdown, and unusable-tmux lifecycle protection lose their combined
shipped-process check; retained tmux-driver and ticker tests cover mechanisms but not that process wiring. Governing
references: SPEC.md at 647f0227:3013-3038 and SPEC_impl.md:2294-2296 retain old compatibility; removed tests are
an_old_session_after_the_switch_to_hosts_is_reported_as_one_to_move,
the_private_tmux_server_ends_with_the_last_old_terminal and old_terminals_without_a_usable_tmux_are_reported_not_alive
in host_processes.rs. Repair at the reviewed revision: seed a real earlier-build server and corresponding persisted
session, then start the bare supervisor and assert its listing/stream/lifecycle behavior, without restoring production
creation of new tmux terminals. Later retirement checked separately: git history first adds
service/core/old_terminals.rs in dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. At pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db, SPEC.md:2998-3010 explicitly changes the contract: 'A supervisor that finds
that tmux server when it starts ends it, and with it everything that still ran in an old terminal, without needing a
tmux program.' SPEC.md:2804-2813 replaces the migration warning with an ended-terminal notice. Complete SPEC_impl.md
section 'Ending the old private tmux server', lines 2780-2823, describes the replacement. Tip core.rs:6141-6184 calls
end_private_server before reload; :6674-6678 gives every Tmux row no terminal; :6807-6821 converts live old rows to
TerminalEnded. Thus the deleted steady-state adoption/serving/warning oracles have no remaining subject at tip.
Suggested bucket: other (test-only gap at the intermediate revision); confidence: confirmed by source, no execution.
proposed_drop: false for the historical finding; its former no-product-failure drop basis is withdrawn. Current-tip
applicability is retired, so do not restore obsolete compatibility tests as a forward fix merely because this historical
gap existed. possible_cover: parent identifies g152/l1 for the broader deleted migration suite; consolidate overlapping
historical gaps if useful, retaining these exact lost process oracles. g156's sink coverage topic was not independently
reviewed here. Could-be-highest: no; no shipped data-loss/security defect is established. Caveats: no claim that
compatibility broke in execution, no claim of equivalent lower-layer coverage, and no judgment here of the replacement
retirement implementation's correctness.

Returned proposal (claims-097, event 8): **needs-parent**, suggested bucket **other**.

The intermediate process-level coverage loss is established. Its subject was retired before the review tip, so there is
no demonstrated current repair to make. Preserve the historical finding and ask the parent to decide whether that
history warrants a queue item; do not restore obsolete compatibility tests.

Original stronger consequence: Lost combined real-process checks for adoption/serving, warning resolution,
last-private-server shutdown, and protection against deleting or duplicating an unreachable old agent. No runtime
breakage was established. The earlier lower-layer-coverage drop was withdrawn and remains withdrawn; lower-layer
correctness is not equivalent process coverage.

Unresolved premise or limitation: None for the historical test-only consequence. Whether to record a fully retired
intermediate gap is a parent recording decision.

Proposed basis: Historical claim verified; current applicability retired by an explicit later contract and source
change. This consultation does not authorize dropping the history. Related g152/l1 adds actual
migration/history-transition oracles and can be consolidated as an extension, but is not an exact alias.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g151 record](records/review-through-00de0b6c/groups/g151.json). Record ID: `l1`.

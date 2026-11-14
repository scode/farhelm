# upgrade intentionally ends work still in old tmux terminals

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: This remains a known deliberate retirement consequence, with a proposed specification-based drop
awaiting independent judgment. Source route: core.rs:6142-6151 ends the private server before reload;
old_terminals.rs:176-279 verifies the peer user/process and socket command line before termination, and uses the
snapshot only after verified signalling. hosts/process.rs:181-184 checks the exact -S socket pair; 476-575 resolves the
socket, rechecks pinned identity and command line before SIGTERM/SIGKILL and fails on mismatch/unreadability.
Replacement lifecycle checks this turn: core.rs:6634-6645 separately adopts host-backed tabs; 6675-6679 treats old-kind
terminal addresses as absent; old_terminals.rs:290-408 sweeps converted agents as AgentOnly and old tabs by tab id,
skipping scope-discovered tabs with host launch records; core.rs:7045-7090 sweeps before transition commit and records
ended notices afterward. Later Delete is not made best effort by this acceptance: teardown.rs:310-420 retains
WholeSession marker sweeping without a terminal and enumerates historical tab/launch scopes, and 531-561 propagates
failed process sweep or unconfirmed host shutdown before row removal. No concrete distinct wrong-target or
later-unconfirmed-Delete route was established from this assigned deletion. Suggested bucket: highest (loss of
user-owned processes/work). Could-be-highest: yes. Confidence: confirmed source and contract, no execution.
proposed_drop: SPEC.md Terminal experience, 3003-3017 explicitly says "This release ends them: the first time a host's
supervisor of this release start

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g165/f1`.

### g165/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original consequence preserved: Starting this release ends
agents and tab programs still running in the old private tmux server, losing those running processes and the old
terminal contents. Provenance: assigned deletion packet 00951-005.patch:372-495 removes the adoption regression
requiring the old server to remain running because it holds live sessions; original test is tmux.rs:6849 at this
commit's parent. TLDR: This remains a known deliberate retirement consequence, with a proposed specification-based drop
awaiting independent judgment. Source route: core.rs:6142-6151 ends the private server before reload;
old_terminals.rs:176-279 verifies the peer user/process and socket command line before termination, and uses the
snapshot only after verified signalling. hosts/process.rs:181-184 checks the exact -S socket pair; 476-575 resolves the
socket, rechecks pinned identity and command line before SIGTERM/SIGKILL and fails on mismatch/unreadability.
Replacement lifecycle checks this turn: core.rs:6634-6645 separately adopts host-backed tabs; 6675-6679 treats old-kind
terminal addresses as absent; old_terminals.rs:290-408 sweeps converted agents as AgentOnly and old tabs by tab id,
skipping scope-discovered tabs with host launch records; core.rs:7045-7090 sweeps before transition commit and records
ended notices afterward. Later Delete is not made best effort by this acceptance: teardown.rs:310-420 retains
WholeSession marker sweeping without a terminal and enumerates historical tab/launch scopes, and 531-561 propagates
failed process sweep or unconfirmed host shutdown before row removal. No concrete distinct wrong-target or
later-unconfirmed-Delete route was established from this assigned deletion. Suggested bucket: highest (loss of
user-owned processes/work). Could-be-highest: yes. Confidence: confirmed source and contract, no execution.
proposed_drop: SPEC.md Terminal experience, 3003-3017 explicitly says "This release ends them: the first time a host's
supervisor of this release starts, it ends that tmux server, and with it everything that still ran in an old terminal,
without needing a tmux program." It preserves an ended agent's outcome but says "its terminal's last screen, which lived
in the server, is gone; old tabs are closed." The known failed-termination residual is also explicitly accepted there:
"A failure to end the server or to stop what ran there is logged and never stops the supervisor from starting; those
sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still
running." SPEC_impl.md Terminal substrate: private tmux server specifies the same boundaries and budget. possible_cover:
none beyond these exact specification acceptances; no forbidden coverage sources searched. Caveats: the known
duplicate-after-Restart residual is preserved here as related provenance, not claimed as a new discovery. This
acceptance does not excuse misleading update guidance, missing warnings, wrong-target termination or unconfirmed Delete;
no concrete separate route for those arose from the assigned test deletion. No proposed drop has been accepted;
independent highest-bucket checking remains required.

Returned proposal (claims-104, event 9): **covered**, suggested bucket **highest**.

The loss of old terminal programs and screen contents is real by source and explicitly accepted by both specifications.
This recommendation covers that deliberate migration and its stated failed-termination residual only; independent drop
approval remains required.

Original stronger consequence: Starting the release ends agents and tab programs in the old private tmux server, losing
running processes and old terminal contents. A failed termination can also leave an old agent running beside one
subsequently restarted. Both consequences remain preserved as highest-impact provenance.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance, pending independent drop check. It does not accept missing update
warnings, wrong-target termination or unconfirmed later Delete.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g165 record](records/review-through-00de0b6c/groups/g165.json). Record ID: `f1`.

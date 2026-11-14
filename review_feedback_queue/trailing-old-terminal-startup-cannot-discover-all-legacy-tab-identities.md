# Old tab daemons can survive when their tab id is undiscoverable

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: detached work from an old shell tab can remain running after the upgrade until its session is
deleted.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g149/c2`.

### g149/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. TLDR: detached work from an old shell tab can remain running
after the upgrade until its session is deleted. Details: sweep builds its session set only from converted live old-agent
rows and tab-marked snapshot panes; old_tabs obtains other ids from those sessions' scope names and preserves scopes
with terminal-host tab records (launch.rs:890-906 establishes that those records survive until a host is confirmed
ended). It deliberately omits an already-ended old agent's session when its old server cannot be read; unscoped tabs
also have no recoverable tab id. Suggested bucket: high for materially persisting unwanted old processes;
could-be-highest: yes, if surviving work later overwrites user files, but that additional loss route is not established
here. Confidence: confirmed source behavior, no execution. proposed_drop: explicit specification acceptance only; no
parent drop has been accepted, and an independent check is required before any drop. Exact SPEC_impl.md:2989-2994: 'A
whole session is never swept, since its host terminals carry the same session marker; what cannot be attributed to an
old tab is left for the session's Delete, which sweeps the whole session. Once the server is gone, an old tab is found
only through the scope units of a session whose agent this start converted, so a scoped tab of any other session (its
agent already ended, or in a terminal host), an unscoped tab, and one whose shell's environment macOS hides wait for
that Delete.' SPEC.md:3011-3013 states the same limitation. possible_cover: none known. Strongest original consequence,
persisting old work with possible later file loss, is preserved; rarity is not a lowering basis. This accepts startup
discovery limitations only; it does not establish that a subsequent Delete may falsely report the surviving work gone.
Synthesis revision changes kind to finding because a could-be-highest drop resting only on specification acceptance
needs independent review. Later-revision check: pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
SPEC_impl.md:2810-2814 retains exactly the startup tab-discovery residual and leaves it for Delete. The complete diff of
old_terminals.rs since its introduction changes only the documentation link. Original source :321-328 and :369-402 was
rechecked: the session set and host-tab record exclusion match that narrow acceptance. No later repair or specification
broadening to permit false success on a later Delete was found in this scope. Discovery-turn interface synthesis:
supplied g007/c4 completes a separate highest authority route for an ended host agent with an old live tab when startup
gets EAGAIN/no pane inventory and no converted old-agent row: startup performs no tab sweep, current host-only
listing/guard sees zero tabs, and later WholeSession cleanup can end undisclosed tab work. g128/l1 separately
establishes that route for an old agent surviving the bounded sweep. They concern consent after startup, not only
whether this residual was documented; the instruction leaving survivors for Delete does not itself settle a NothingAlive
guard. Both remain owned by their original groups and are not new g149 discoveries. g097/l4 is also distinct: a stale
snapshot PID read as a new tab owner enters old_tabs direct list, which does not run the host-record exclusion reserved
for scope-derived candidates. Preserving live host records in that latter branch does not refute that highest
wrong-target route or settle its proposed short-window PID-race acceptance. No parent drop/lowering is accepted. g111/l1
and g144/c3 are retained separately as moved-subtree work loss and test owner/drop-order cleanup respectively, not
deduplicated into c2. These crosschecks preserve the original residual-process consequence and exact proposed-drop text
above.

Returned proposal (claims-096, event 16): **covered**, suggested bucket **high**.

The specifications explicitly leave unattributable old tab processes for session Delete. This covers startup discovery
limitations only and requires an independent check before dropping the possible work-loss concern.

Original stronger consequence: Unwanted old-tab work can persist after upgrade and could later overwrite user-owned
files. Do not replace that conditional stronger concern with a rarity dismissal.

Unresolved premise or limitation: A surviving process subsequently overwriting user files is not established. The
established residual process can persist until Delete; the startup limitation is explicitly accepted.

Proposed basis: Propose exact startup-discovery coverage, subject to independent drop check. Do not deduplicate separate
later Delete-consent or stale-PID wrong-target claims mentioned in this history; they have different defects and are
outside this named batch.

Existing parent disposition (event 16): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g149 record](records/review-through-00de0b6c/groups/g149.json). Record ID: `c2`.

# Accepted survivor residual does not establish destructive overreach

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An old private tmux server can remain running after uninstall if it already outlived its stopped
supervisor; this is incomplete survivor cleanup, not evidence that uninstall acts destructively on a target the user did
not confirm. Original consequence preserved: surviving old terminals/processes can keep using resources and become
unreachable through Farhelm after the host/install is removed. Suggested bucket absent acceptance: high for incomplete
uninstall; no established security or loss-of-work route. Confidence: confirmed source behavior and explicit
specification acceptance, source-only. Exact acceptance at SPEC.md:214-217 (line wrapping joined): 'Uninstall has no
tmux step of its own: stopping the supervisor's service ends whatever still runs under it, an earlier release's private
tmux server included, but one that outlived a supervisor stopped before uninstall (on a host never updated to a release
that ends it at startup, say) keeps running, which the maintainer accepted on 2026-11-11.' proposed_drop: only this
exact survivor residual is proposed for exclusion on that acceptance; no parent drop is accepted or assumed, and this is
not permission to discard a wrong-target or excessive-destructive-action finding. Synthesis checks: backend.rs stop diff
removes the frozen socket/client kill-server sequence rather than redirecting it; reviewed stop at :3260-3360 still
skips systemctl stop for inactive/failed units and explicitly confirms terminal hosts stopped before later directory
removal. plan.rs:941-1132 freezes the named service, binary, state directory and install di

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g006/f2`.

### g006/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (candidate, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: An old private tmux server can remain running after
uninstall if it already outlived its stopped supervisor; this is incomplete survivor cleanup, not evidence that
uninstall acts destructively on a target the user did not confirm. Original consequence preserved: surviving old
terminals/processes can keep using resources and become unreachable through Farhelm after the host/install is removed.
Suggested bucket absent acceptance: high for incomplete uninstall; no established security or loss-of-work route.
Confidence: confirmed source behavior and explicit specification acceptance, source-only. Exact acceptance at
SPEC.md:214-217 (line wrapping joined): 'Uninstall has no tmux step of its own: stopping the supervisor's service ends
whatever still runs under it, an earlier release's private tmux server included, but one that outlived a supervisor
stopped before uninstall (on a host never updated to a release that ends it at startup, say) keeps running, which the
maintainer accepted on 2026-11-11.' proposed_drop: only this exact survivor residual is proposed for exclusion on that
acceptance; no parent drop is accepted or assumed, and this is not permission to discard a wrong-target or
excessive-destructive-action finding. Synthesis checks: backend.rs stop diff removes the frozen socket/client
kill-server sequence rather than redirecting it; reviewed stop at :3260-3360 still skips systemctl stop for
inactive/failed units and explicitly confirms terminal hosts stopped before later directory removal. plan.rs:941-1132
freezes the named service, binary, state directory and install directory; it refuses binary/state containment
violations, and its Linux action order puts StopSupervisor before RemoveDirectory. service.rs:1710-1948 checks connected
sessions and probed identity or, offline, absence plus gone unit and inactive service, and :2435-2454 requires unchanged
registration and an identical replanned uninstall before execution. plan.rs:443-451 discloses stopping the
supervisor/terminal hosts and removing the named install directory including anything else in it. backend.rs:3420-3475
refuses a directory that became a symlink or contains a mount before rm-rf. These checks establish no additional
wrong-target or unconfirmed destructive route for f2; they do not assert exhaustive implementation coverage.
Could-be-highest: no route established for this survivor-only consequence; unrelated or newly established destructive
harm must be assessed separately and is not covered by the quoted acceptance. possible_cover: none; no prohibited
coverage search. No reproduction or runtime observation. Revision preserves all earlier consequences and the proposed
drop, narrowing that proposal to the accepted residual without treating acceptance as a lower severity for hypothetical
harm.

Returned proposal (claims-003, event 7): **covered**, suggested bucket **high**.

The exact surviving-server residual is explicitly accepted in both specifications. It is incomplete cleanup and
unreachable retained terminal access, not a demonstrated wrong-target deletion or excessive destructive action. High
describes the consequence absent its acceptance; acceptance is a recording basis, not a severity downgrade.

Original stronger consequence: Old terminals/processes may keep consuming resources and become unreachable through
Farhelm after the host/install is removed. The earlier high-if-unaccepted consequence remains preserved; no claim that
the surviving process itself was killed or work lost.

Unresolved premise or limitation: No missing premise for the accepted survivor behavior. No established security or
work-loss route from leaving these same-account processes alive; unrelated destructive behavior is excluded from this
basis.

Proposed basis: Exact specification acceptance for survivor-only cleanup. Conservative independent check because
user-owned processes and their access are involved; agreement must not cover hypothetical wrong-target removal or other
uninstall defects.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered. Full003offlineuninstallinactive/failedservicestopskipwithnoownprivatetmuxcleanup
leavesoldserver/resourcesaliveandFarhelmaccessgone. ExactSPEC210-219/SPEC_impl7813-7832explicitlyaccept
stopped-supervisor survivor includingstartupendingfailurethenstop. Preserve highifnotaccepted/inaccessibleworkcontext,
noimplicitseveritydowngrade orwrongtarget/excessdestructionacceptance. Freshcheckexactsurvivoronlyexception; no
runtime/ownershipbroadauditcertification.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g006 record](records/review-through-00de0b6c/groups/g006.json). Record ID: `f2`.

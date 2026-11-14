# Uninstall no longer ends an already-detached legacy tmux server

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: uninstall can leave an old Farhelm tmux server and its processes running when the supervisor was
already stopped before uninstall; the host entry and installed binary can then go while those processes remain. Source:
service.rs:2834-2841 drops tmux_program/tmux_socket from the stop dispatch; backend.rs:3303-3340 stops active service
and terminal hosts only, and skips the service stop for inactive/failed units. Exact accepted residual in
SPEC.md:211-218: 'Uninstall has no tmux step of its own: stopping the supervisor's service ends whatever still runs
under it, an earlier release's private tmux server included, but one that outlived a supervisor stopped before uninstall
(on a host never updated to a release that ends it at startup, say) keeps running, which the maintainer accepted on
2026-11-11.' SPEC_impl.md Provisioning explicitly also includes startup cleanup failure followed by supervisor stop.
Suggested bucket: highest pending independent determination, preserving possible process/work consequences; earlier
events' other classification is not an accepted lowering. Could-be-highest: yes. Confidence: confirmed orphan-survival
route from source; additional work-loss/security consequence not demonstrated. proposed_drop: exact specification
acceptance above, proposed only, not accepted. possible_cover: SPEC.md/SPEC_impl.md exact residual; caller named related
g048/c1, g048/l1, g047/c1, g047/c3, g012/c1, not yet read this turn. Caveats: no runtime execution; no claim that Add is
permitted to mutate an answering supervisor; that separate question must be source-checked rather

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g046/f1`.

### g046/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d. TLDR: uninstall can leave an old Farhelm tmux server and its
processes running when the supervisor was already stopped before uninstall; the host entry and installed binary can then
go while those processes remain. Source: service.rs:2834-2841 drops tmux_program/tmux_socket from the stop dispatch;
backend.rs:3303-3340 stops active service and terminal hosts only, and skips the service stop for inactive/failed units.
Exact accepted residual in SPEC.md:211-218: 'Uninstall has no tmux step of its own: stopping the supervisor's service
ends whatever still runs under it, an earlier release's private tmux server included, but one that outlived a supervisor
stopped before uninstall (on a host never updated to a release that ends it at startup, say) keeps running, which the
maintainer accepted on 2026-11-11.' SPEC_impl.md Provisioning explicitly also includes startup cleanup failure followed
by supervisor stop. Suggested bucket: highest pending independent determination, preserving possible process/work
consequences; earlier events' other classification is not an accepted lowering. Could-be-highest: yes. Confidence:
confirmed orphan-survival route from source; additional work-loss/security consequence not demonstrated. proposed_drop:
exact specification acceptance above, proposed only, not accepted. possible_cover: SPEC.md/SPEC_impl.md exact residual;
caller named related g048/c1, g048/l1, g047/c1, g047/c3, g012/c1, not yet read this turn. Caveats: no runtime execution;
no claim that Add is permitted to mutate an answering supervisor; that separate question must be source-checked rather
than inferred from this uninstall residual. Parent explicitly accepted no drop or lowering. This revision preserves the
original consequence and acceptance text for independent checks. Synthesis: read only the caller-named related IDs in
g048, g047 and g012. g048/c1 and g047/c1 and g012/c1 match the exact accepted orphan residual; g048/l1 instead concerns
Add over an answering supervisor, independently source-refuted in this group as c3. The latest g047/c3 is an open lead
about broader fixture readiness and failure coverage, so its earlier proposed refutation is not adopted as settled. This
group independently checked its own retained live-terminal-host test in c4. Re-read service.rs:2631-2868 and :2940-2969:
only RemoveBundledTmux converts its backend failure into a warning; StopSupervisor errors still stop execution through
fail_action before RemoveDirectory/ForgetHost, retain Failed progress and leave view retrievable for the existing host.
No new violation beyond this accepted residual established. Exact accepted text and possible highest consequence above
remain unchanged; no drop or lowering accepted.

Returned proposal (claims-029, event 10): **covered**, suggested bucket **other**.

Source establishes the legacy orphan-survival residual, including removal of the registered host and binary while those
processes remain. Both specifications explicitly accept exactly that residual. This is only a proposed coverage
disposition, requiring independent drop and consequence review.

Original stronger consequence: Latest record deliberately keeps highest pending independent determination because
cleanup retirement concerns processes/work; earlier other classifications were explicitly not accepted lowerings. Legacy
processes can remain after host and binary removal. Preserve all of that for the independent check.

Unresolved premise or limitation: Additional loss of user work or security harm outside accepted process survival has
not been established. No Add-over-answering-supervisor route follows from this acceptance.

Proposed basis: Exact specification acceptance for this residual only. Other is the bucket suggested by the established
leftover-process consequence, not an accepted lowering of the preserved possible highest concern.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-legacy-private-server-survival-after-offline-uninstall. Full029
stopinactiveunit/terminalhostcleanupdoesnotreachdetachedoldserver; ordinarystopfailurestillreturnsbeforedelete,
onlybundledtmuxremovebest-effort. Sourcelegacyprocessesmayremainafterhost/binaryremoval,
exactproduct/implexplicitlyacceptpreviouslystoppedsupervisor/failedstartupendresidual.
Highestpossibilitypreserveduntilfreshgate, proposedotheronlyestablishedleftoverimpact.
NotordinaryAddoveransweringsupervisoracceptance, no blanketupgradefilter orobservedcgroupclaim.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g046 record](records/review-through-00de0b6c/groups/g046.json). Record ID: `f1`.

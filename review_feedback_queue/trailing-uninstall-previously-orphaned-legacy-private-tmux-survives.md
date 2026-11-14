# Deleted retry test concerns only the accepted previously orphaned tmux server

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deleted retry test concerns only the accepted previously orphaned tmux server

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g047/c1`, `g048/c1`.

### g047/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (candidate, open):

# Deleted retry test targets the expressly accepted orphan-server residual

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

Uninstall may leave an earlier private tmux server running when its supervisor had already been stopped and its unit
forgotten. The deleted regression test required uninstall to end that exact residual, which the reviewed specification
expressly accepts.

## Details

Revision narrows the proposed drop after reading the caller-supplied g048 c1 and l1 records. Assigned patch
01267-001.patch:828-1043 deletes wait_private_tmux_gone and
uninstall_retry_ends_tmux_after_systemd_forgets_the_inactive_unit. The deleted test first stops its process-only
service, removes its unit, reloads systemd and establishes that the service is inactive and its tmux server still
answers before invoking stop again. Its tested precondition is therefore the previously stopped-supervisor residual, not
ordinary Add over an answering supervisor, nor stop of a currently active unit. SPEC.md, Topology, exact acceptance:
"Uninstall has no tmux step of its own: stopping the supervisor's service ends whatever still runs under it, an earlier
release's private tmux server included, but one that outlived a supervisor stopped before uninstall (on a host never
updated to a release that ends it at startup, say) keeps running, which the maintainer accepted on 2026-11-11."
SPEC_impl.md, Provisioning, accepts the same residual, including an earlier startup ending that failed followed by a
supervisor stop.

Cross-batch assumption checked: service.rs:2238-2311 re-probes original/planned Add coordinates at confirmation, adopts
a compatible answering supervisor via UseAsIs, also adopts a skewed answering supervisor, and executes only on Absent.
This agrees with g048/l1 and is a separate source refutation of ordinary Add removing tmux beneath an answering old
supervisor. The orphan-uninstall acceptance must not be extended to justify hypothetical Add disruption or loss of work;
this record makes no such proposal.

Suggested bucket: other (original residual process-cleanup and deleted-test concern). Confidence: confirmed by source
and contract, not execution. proposed_drop: exact specification acceptance, limited to deletion of the regression
assertion requiring cleanup of an already orphaned private server after its supervisor was previously stopped.
possible_cover: same underlying residual as caller-supplied g048/c1; no queue coverage search performed.
Could-be-highest: no established security or loss-of-work route; leaving this server alive is an omitted cleanup, not
termination of user work. Material caveat: active-unit termination, terminal-host cleanup, Add adoption and update
safety retain their separate contracts; this proposed drop covers none of their potential violations. Parent has
accepted no drop.

Returned proposal (claims-030, event 8): **covered**, suggested bucket **other**.

The deleted retry test demanded termination of exactly the already orphaned private tmux server the specification
accepts surviving. That acceptance does not cover active-unit or terminal-host teardown, or Add over an answering
supervisor.

Original stronger consequence: Uninstall can leave an old private tmux server and its jobs running after its supervisor
was stopped earlier; deleting its regression test raised a lifecycle-coverage concern. Earlier records did not establish
destruction of work or security harm.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact specification acceptance, limited to the deleted orphan-server assertion. Conservatively register
the proposed dismissal because the source concerns user-owned processes; this consultation does not authorize a drop.

Existing parent disposition (event 8): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-retired-positive-orphan-server-retry-oracle. Parent read full030
andactualSPEC178-220 orphanacceptance. Deletedtestexplicitlystops/removesprocessonlyunit,
provesoldprivateserversurvives, thenrequiresstopstepkillit; exactacceptedresidualnolongersupportedtestrequirement.
Notretirementmandatoryactiveunit/terminalhostterminationnorordinaryAddoveransweringserver.
Same048c1/046f1residualwithseparatelostoracleprovenance; preserveallhistories/possibleworkframingfreshgate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g047 record](records/review-through-00de0b6c/groups/g047.json). Record ID: `c1`.

### g048/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (candidate, open):

# Uninstall leaves a detached old private tmux server

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

Uninstalling a host whose earlier supervisor was already stopped can leave its old private tmux server running after the
installation directory is removed. That residual is distinct from terminal hosts, whose termination remains mandatory
and whose failure retains the installation and listed host.

## Details

Original consequence preserved: removal of socket-addressed tmux cleanup can leave the earlier private server and its
jobs alive after uninstall. StopSupervisor no longer freezes a tmux executable or socket (plan.rs:284-300).
SPEC.md:214-217 explicitly accepts: "Uninstall has no tmux step of its own: stopping the supervisor's service ends
whatever still runs under it, an earlier release's private tmux server included, but one that outlived a supervisor
stopped before uninstall (on a host never updated to a release that ends it at startup, say) keeps running, which the
maintainer accepted on 2026-11-11." SPEC_impl.md, Provisioning: "Uninstall has no tmux step: a private tmux server that
outlived a supervisor stopped before uninstall (a host never updated to a release that ends it at startup, or one whose
startup ending failed and whose supervisor was later stopped) keeps running. The maintainer accepted that residual on
2026-11-11 when the step was removed."

Synthesis checks: plan.rs:1098-1124 retains disable, unit removal, daemon reload, StopSupervisor with frozen
Farhelm/state paths, lib removal, ForgetHost. backend.rs:3304-3321 stops the unit when active and invokes the frozen
installed Farhelm's internal stop-terminal-hosts whenever hosts exists; only exit 0 proceeds. At :3322-3339 non-socket
leftovers, inability to remove the hosts directory, and lookup errors fail before lib removal. service.rs:2660-2685
stops at the first error, and :2713-2745 retains Failed status, failing step, diagnostic, retry remedy, and listed host.
:2834-2862 forwards exactly the frozen stop coordinates and only forgets the host after prior actions succeed. Offline
retry at :1809-1822 requires positive supervisor absence and uses recorded paths; :1898-1925 additionally requires the
service file absent and service inactive, while a connected supervisor must belong to the service. :1972-1982 allows the
post-lib-removal retry to proceed without resolving a gone binary.

Cross-input checked: g047/c1 concerns deletion of the orphan-tmux regression test and matches this accepted residual;
g012/c1 records the same precise contract. g047/c3's retained fixture teardown is independently supported by
provisioning.rs:12028-12109 and :12113-12229: nonce-unit cleanup, explicit end_fixture_hosts, kernel-listener checks,
and state retention when termination cannot be confirmed remain. This is not a reason to treat terminal-host termination
as optional.

Suggested bucket: other (residual process cleanup). Could-be-highest: no established security or loss-of-work
consequence; surviving the server preserves rather than terminates its jobs. Confidence: confirmed from source and
contract, not execution. proposed_drop: exact specification acceptance quoted above, supported by retained mandatory
terminal-host teardown. Parent has accepted no drop; candidate remains for independent checking. possible_cover:
cross-input g047/c1 and g012/c1 describe this same residual; no coverage metadata or prohibited queue sources searched.
Caveats: no runtime cleanup was observed; this assesses removal consequences rather than unrelated retired
implementation defects. All earlier candidate history and potential harms remain in the append-only log.

Retained endpoint and failure-reporting checks completed: lib.rs:836-843 still registers POST /api/hosts/{id}/uninstall
and GET /api/hosts/{id}/provisioning. provisioning/http.rs:270-297 plans without a body, confirms through
start_uninstall with a body, returns the accepted run identity, and reads the retained provisioning view.
service.rs:2001-2017 consumes one host-bound Uninstall plan; :2435-2452 re-plans under the run lock and refuses a
changed plan before execution. The view at :2953-2969 still returns failed progress while the host row exists.
ForgetHost only deletes progress after successful earlier teardown/removal (service.rs:2149-2168,2179-2184). Thus a stop
failure leaves the frozen executable available, its host listed, and the failure diagnostic retrievable; success removes
that progress deliberately. The retained real test at provisioning.rs:13324-13332 captures and proves the ended
terminal-host socket is live before uninstall, and :13438-13446 checks that exact socket no longer listens afterwards.
These are source assertions inspected, not executed observations.

Returned proposal (claims-030, event 7): **covered**, suggested bucket **other**.

The surviving already orphaned legacy server is source-supported and expressly accepted. Terminal-host stop remains
mandatory and fails before library removal; that distinct contract is not waived.

Original stronger consequence: Uninstall can leave an earlier private tmux server and its jobs alive after removing the
installation directory. No security or loss-of-work route was established; continued jobs are the residual, not their
destruction. Earlier caveats about terminal-host termination remain.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact orphan-server specification acceptance. Conservatively register because user-owned processes are
involved; this does not authorize dropping or lowering any separate terminal-host, active-unit or Add claim.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-previously-orphaned-private-server-survival. Full030
frozenstopcoords/firstfailurehaltsbeforelibForgetHost; explicitterminalhostsstopandnonsocketlookupremovalerrorsfail.
Oldprivateserverhasnocoordinate/stopstepandsurvivespreviouslystoppedsupervisor,
exactcurrentproduct/implacceptincludingfailedstartupendlaterstop. Same047deletedtest/046findingresidual,
keepaspectsnotblanketcleanupwaiver. Continuedjobsnotdestructionandnohigherestablishedharmbutgateconservative.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g048 record](records/review-through-00de0b6c/groups/g048.json). Record ID: `c1`.

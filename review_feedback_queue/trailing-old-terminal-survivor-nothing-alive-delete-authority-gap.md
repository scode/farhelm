# Hidden old-tab and old-agent Deletes share the no-live authority gap

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Hidden old-tab and old-agent Deletes share the no-live authority gap

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g007/c4`.

### g007/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333; introducing retirement within range: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Pending-input synthesis before discovery. Original TLDR preserved: a supported mixed session with an ended host agent and still-live old tab can show no tabs after startup fails to reach the old server; its direct NothingAlive Delete later kills that hidden tab and can lose work in it. Source route retained: predecessor SPEC_impl.md:2956-2958 supports host-agent/old-tab coexistence; reviewed old_terminals.rs:179-189 returns no pane inventory on a documented connection failure while leaving the server running, :296-298 skips an empty converted/pane sweep, and :321-328 does not derive a tab session from a host-agent row. UI status.rs:409-428, list/shared.rs:37-70 and list/view.rs:3201-3232 dispatch direct NothingAlive Delete; api.rs:4251-4256 and helm sessions.rs:3631-3634 preserve that guard; handlers.rs:1253-1279 checks it before teardown. core.rs:13307-13376 checks current/recorded hosts only; teardown.rs:402-418 conservatively requires the sweep for missing historical inventory, :431-452 enumerates scopes, and :531-540 performs WholeSession. This is a live undisclosed tab, not a detached descendant of an agent that ended. Prior missed-cleanup concern remains refuted and prior other bucket applied only to that completeness evidence; it does not dispose of confirmation authority. Supplied neighbour g128/l1 corroborates the same guard-to-WholeSession mismatch with a different producer: a pure legacy row upgraded to tmux kind, no .host records or host tabs, startup budget actually leaves its marked agent running, Interrupted status, NothingAlive guard passes, later successful sweep kills it. Its source/metadata/forwarding premises are complete in the supplied record; no fresh-sweep duplicate-agent premise is required. These are the same underlying no-live Delete authority gap with distinct retained-process producers, suitable for caller consolidation while preserving g007/c4 and g128/l1 provenance and both consequences. g032/l5's separate false Interrupted Restart question has a different answer/AgentOnly path and different remedy; do not merge that merely because it shares startup survivors. Suggested bucket: highest for loss of undisclosed live work; could-be-highest: yes. Confidence: conditional source-confirmed control flow, no occurrence observed. Exact proposed_drop/competing contracts retained, no drop accepted: SPEC.md:2048 says 'a destructive confirmation authorizes only what the prompt the user answered said would happen'; :2054-2056 requires refusing NothingAlive if 'the agent or any terminal tab is running'. SPEC.md:3014-3016 accepts startup failure and says 'those sessions are treated as ended all the same, accepting that a Restart could then start a second agent beside one still running'. SPEC.md:3012-3013 says 'what cannot be attributed to an old tab is left for the session's Delete, which stops everything of the session'. SPEC_impl.md:2920-2921 says 'A sweep that runs out of time leaves the rest for the session's Restart or Delete: the conversion commits regardless.' Independent judgment must decide whether these deferred-cleanup/ended-state clauses also waive the explicit guard; successful cleanup alone is not authority or a refutation. possible_cover: supplied g128/l1, same root/guard mismatch, not new discovery. No forbidden coverage sources, reproduction or runtime tests; no open lead remains for this reconciliation. Original highest consequence and all historical proposed-drop bases remain preserved under this ID.

Returned proposal (claims-003, event 13): **filtered**, suggested bucket **highest**.

The conditional guard-to-WholeSession mismatch is source-established, distinct from the earlier refuted missing-cleanup concern. It nevertheless appears wholly inside the current older-release-plus-rare-condition filter: supported mixed old/new terminal state survives an upgrade and a rare startup connection failure leaves an old live tab hidden. Do not call successful cleanup a consent refutation.

Original stronger consequence: Original concern: missing old-tab discovery might leave live processes modifying work; conservative teardown refuted missing cleanup. History then established a separate highest route: direct no-live Delete kills undisclosed live old tab/editor work. Preserve the earlier other bucket's limited completeness scope, every proposed-drop caveat and the final highest authority consequence.

Unresolved premise or limitation: No missing source premise for the conditional old-tab route; actual EAGAIN/live workload was not reproduced. Independent judgment must approve the filter's complete boundary. If instead dropping on specification acceptance, independently settle the no-live-consent versus deferred-cleanup ambiguity rather than assuming the latter wins.

Proposed basis: Proposed filter-only exclusion for old-release carried state plus rare startup connection/cleanup fault, subject to independent highest-drop agreement. Without agreement, retain verified highest and preserve the specification ambiguity.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: filtered-upgrade-plus-rare. Full003realmixedhostagentended+liveoldtabproducer supportedbeforeupgrade, rarestartupEAGAIN/livelistenrefusal losesattributedpaneinventory, NothingAlivehost-onlyguardpasses, conservativeWholeSessionthenkillsundisclosedoldtab/editor. Highestconsentdefectsourceconfirmed, notmissingcleanupandnotrefutedbycorrectsweep. Wholetriggerolderstateplusrarestartupfault; affected-sessionworklossfilterexplicitlyincludes, nosecurity/corruption/bricking/normalupdateinabilityclaimed. Freshcheckfullfilterand128l1alias. IfusingSPECacceptanceinsteadmustsettlenoliveconsentconflict; parentproposesfilteronly, nomisleadingwarningacceptanceinferred.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g007 record](records/review-through-00de0b6c/groups/g007.json). Record ID: `c4`.


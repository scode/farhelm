# supplied duplicate: interrupted Restart confirmation understates cleanup

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: A Restart question opened while an old-terminal agent was working can later say 'nothing left to stop' after the update marks it interrupted, yet the confirmed Restart can terminate surviving agent work. This is separate from f1's question of two agents running concurrently: the fresh sweep that narrowed f1 establishes this cleanup-authority concern instead. Potentially highest consequence: surviving user-owned processes/work can be stopped beyond the displayed destructive consequence. No runtime occurrence or work loss observed. Source/contract checks at the assigned revision: status.rs:608-615 explicitly reasons that Interrupted leaves nothing behind, and :634-635 changes the consequence to 'Interrupted by a host reboot or a Farhelm update — nothing left to stop; restart it:'. session_view.rs:199-200 and :2255-2264 opens a question only from Running, while :2043 reads the still-open ConfirmSlot independently of current status. :2594-2595 renders the consequence for the current shown status, :2008 derives false stop consent for Interrupted through status.rs:650-651, and :2612-2621 takes the rendered opening and sends that captured false consent. The generic button tooltip at :2600 still names stopping the agent, so it conflicts with rather than repairs the substantive 'nothing left' consequence sentence. The route requires a question left mounted through update/reconnect and a later Interrupted detail render; navigation/unmount cancels that premise. Actual cleanup: core.rs:12845-12891 treats a terminal-less converted old row as no live pane; :12906 calls refuse_unconsented

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g005/f4`.

### g005/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed assigned commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Interface provenance: caller-supplied g032/l5, independently source-checked here; this is reconciliation of an existing neighbour concern, not a new independent discovery.

TLDR: A Restart question opened while an old-terminal agent was working can later say 'nothing left to stop' after the update marks it interrupted, yet the confirmed Restart can terminate surviving agent work. This is separate from f1's question of two agents running concurrently: the fresh sweep that narrowed f1 establishes this cleanup-authority concern instead. Potentially highest consequence: surviving user-owned processes/work can be stopped beyond the displayed destructive consequence. No runtime occurrence or work loss observed.

Source/contract checks at the assigned revision: status.rs:608-615 explicitly reasons that Interrupted leaves nothing behind, and :634-635 changes the consequence to 'Interrupted by a host reboot or a Farhelm update — nothing left to stop; restart it:'. session_view.rs:199-200 and :2255-2264 opens a question only from Running, while :2043 reads the still-open ConfirmSlot independently of current status. :2594-2595 renders the consequence for the current shown status, :2008 derives false stop consent for Interrupted through status.rs:650-651, and :2612-2621 takes the rendered opening and sends that captured false consent. The generic button tooltip at :2600 still names stopping the agent, so it conflicts with rather than repairs the substantive 'nothing left' consequence sentence. The route requires a question left mounted through update/reconnect and a later Interrupted detail render; navigation/unmount cancels that premise.

Actual cleanup: core.rs:12845-12891 treats a terminal-less converted old row as no live pane; :12906 calls refuse_unconsented_stop with alive=false. That helper at :3784-3797 refuses only alive && !stop_if_running && Running, so this consent does not block leftover reaping. core.rs:12985-13010 performs the fresh recorded-scope AgentOnly sweep before relaunch. Retirement uses Warn and a shared fifteen-second budget (old_terminals.rs:290-355), then core.rs:7337-7382 converts the row regardless of failure. An ordinary budget exhaustion can therefore leave identifiable old agent work which this later sweep stops. No missing-marker or second-agent premise is required.

Exact acceptance boundary preserved: SPEC_impl.md:2994-2995 says 'A sweep that runs out of time leaves the rest for the session's Restart or Delete: the conversion commits regardless.' :2998-3000 accepts startup continuation and possible duplicate agents. These accept the residual and later cleanup, not an explicit 'nothing left to stop' confirmation. The separate controlling contract is SPEC_impl.md:1071 onward, Answers bind to what was drawn, and SPEC.md Lifecycle operations: a destructive confirmation authorizes only the consequence shown at the click. The assigned SPEC.md:3529-3537 intentionally extends Interrupted-card explanations to updates; it supplies no waiver of confirmation truthfulness. Suggested correction is accurate consequence wording, preserving accepted cleanup rather than removing the fresh sweep.

Suggested bucket: highest for stopping undisclosed surviving user work, conditional on the stated supported timing; rarity is not a lowering basis. Could-be-highest: true. Confidence: confirmed conditional source/control route and contract mismatch; no execution. proposed_drop: false. possible_cover: exact supplied g032/l5 duplicate, retained for parent deduplication with both provenances. Previous f4 lead is closed to this source-confirmed interface finding. No parent drop/lowering accepted. No reproduction, test, build or forbidden coverage search. Read all supplied discovery-neighbours.json lines 1..1111 before the new source search; no other neighbour issue is newly claimed here.

Returned proposal (claims-003, event 11): **filtered**, suggested bucket **highest**.

The conditional warning/cleanup mismatch is established by source. A retained Restart question can change to Interrupted and promise nothing remains to stop, while the fresh AgentOnly cleanup stops a startup survivor. The current older-release-plus-rare-condition filter appears to cover the entire route. This is a proposed exclusion, not an accepted drop or a claim that the warning is correct.

Original stronger consequence: Stopping surviving user-owned agent work beyond the displayed nothing-left consequence; highest retained. Initial supplied lead was independently checked by the primary reviewer. No duplicate-agent or marker-invisibility premise is required; no loss was reproduced.

Unresolved premise or limitation: No missing source premise for the conditional control route. Runtime occurrence, survivor workload and frequency were not measured. Independent check must agree the startup fault and retained-question sequence fall wholly within the upgrade filter.

Proposed basis: Apply only the older-release-plus-another-rare-condition filter, after independent highest-drop agreement. If its boundary is disputed, retain verified at highest; best-effort startup acceptance alone does not settle false confirmation wording.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: filtered-upgrade-plus-rare. Parentreadfull003. RetainedRestartquestioncanredrawInterrupted/nothingleftwhilestartupWarncleanupfailedandfreshAgentOnlysweepnowkills oldsurvivor beyonddisplayedstopconsent. Sourceconditionalharmhighest; freshsuccessfulcleanupSUPPLIESharm, notrefutation. Wholetriggerrequiresolderrunningstateacrossupgrade plusstartupcleanupfault/timeout andretainedquestion; filterexpresslycoversaffected-sessionworklosswhenadditionalrarecondition,nosecurity/database/brickingnormalupdatefailureclaimed. Freshcheckcompletefilterboundary andsame032l5alias; preservefalsewarningandno-runtimeobservation. Plainretirementacceptancedoesnotacceptwrongconfirmation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g005 record](records/review-through-00de0b6c/groups/g005.json). Record ID: `f4`.


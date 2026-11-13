# Fresh Restart refutes ordinary duplicate-agent inference, not misleading stop consent

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The fresh Restart sweep continues to refute a simple inference that any failed startup cleanup launches a duplicate. It does not refute a different consequence: that same later sweep may stop surviving work after a confirmation says nothing remains to stop. Original f1 possible highest consequence remains concurrent agents modifying and losing user work. Its latest lowering to other on established documentation/acceptance evidence remains provisional, not a parent-accepted lowering; no ordinary duplicate-agent route beyond the fresh selectors is established. Source checks retained: core.rs:12985-13010 invokes the prior scope and AgentOnly with Refuse; sweep.rs:252-260 covers current and legacy agent markers; :1448-1562 refuses unconfirmed recorded scopes and retains portable scanning; :1111-1219 repeatedly enumerates/escalates/confirms. Marker-invisible unscoped survivors are separately accepted in SPEC.md:2088-2099. Exact proposed_drop basis remains SPEC_impl.md:2998-3000: 'A failure anywhere is logged and never fatal (the maintainer chose to start anyway and treat old sessions as ended, accepting that a Restart could then start a second agent beside one still running)'. No drop accepted. Supplied exact neighbour g032/l5 now establishes the distinct conditional consent route: Running question remains open across a render changed to Interrupted; status.rs:634-635 says 'Interrupted by a host reboot or a Farhelm update — nothing left to stop; restart it:', restart_prompt_stops becomes false, and the answer uses that rendered false consent. The fresh AgentOnly sweep can then k

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g007/f1`.

### g007/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4. Pending-input synthesis before discovery. TLDR: The fresh Restart sweep continues to refute a simple inference that any failed startup cleanup launches a duplicate. It does not refute a different consequence: that same later sweep may stop surviving work after a confirmation says nothing remains to stop. Original f1 possible highest consequence remains concurrent agents modifying and losing user work. Its latest lowering to other on established documentation/acceptance evidence remains provisional, not a parent-accepted lowering; no ordinary duplicate-agent route beyond the fresh selectors is established. Source checks retained: core.rs:12985-13010 invokes the prior scope and AgentOnly with Refuse; sweep.rs:252-260 covers current and legacy agent markers; :1448-1562 refuses unconfirmed recorded scopes and retains portable scanning; :1111-1219 repeatedly enumerates/escalates/confirms. Marker-invisible unscoped survivors are separately accepted in SPEC.md:2088-2099. Exact proposed_drop basis remains SPEC_impl.md:2998-3000: 'A failure anywhere is logged and never fatal (the maintainer chose to start anyway and treat old sessions as ended, accepting that a Restart could then start a second agent beside one still running)'. No drop accepted. Supplied exact neighbour g032/l5 now establishes the distinct conditional consent route: Running question remains open across a render changed to Interrupted; status.rs:634-635 says 'Interrupted by a host reboot or a Farhelm update — nothing left to stop; restart it:', restart_prompt_stops becomes false, and the answer uses that rendered false consent. The fresh AgentOnly sweep can then kill a budget-surviving agent/descendant before relaunch. This does not require the agent to evade that sweep or a duplicate to be launched. That neighbour retains highest for stopped user work beyond the warning and owns the GUI source finding; do not duplicate it as a new g007 discovery. Exact competing contracts remain SPEC_impl.md:1071 onward, 'Answers bind to what was drawn', and :2994-2995, 'A sweep that runs out of time leaves the rest for the session’s Restart or Delete: the conversion commits regardless.' The latter authorizes deferred cleanup, not necessarily inaccurate confirmation wording. This is a separate consequence and correction surface from c4's hidden-old-tab NothingAlive Delete; neither is source-refuted by the fresh Restart cleanup. possible_cover: g005/f1 and g109/c1 cover the duplicate fallback as supplied in discovery-neighbours.json; g032/l5 covers the distinct Restart confirmation. No forbidden coverage search. Confidence: source-confirmed fresh-cleanup analysis; neighbour-supplied source-conditional consent route, not runtime observation. Could-be-highest: original duplicate/work-loss hypothesis preserved; distinct confirmation-related work-loss claim retained by g032/l5. No new ID, no runtime test/reproduction, and no open readily checkable premise for this bounded reconciliation.

Returned proposal (claims-003, event 12): **covered**, suggested bucket **other**.

Best-effort startup and the possible duplicate-agent residual are expressly accepted. Source also defeats the simple inference that a startup cleanup failure alone causes two agents: Restart performs another refusing sweep. Original potential work loss and provisional lowering must remain in the independent register.

Original stronger consequence: Two agents may modify the same work and lose user-owned data. The history first suggested potentially highest, then provisionally lowered to other after finding fresh Restart cleanup. That lowering and proposed acceptance-based drop remain unapproved; separate false-consent harm is retained by g005/f4 and separate Delete authority by g007/c4.

Unresolved premise or limitation: A supported ordinary live agent must survive all fresh Restart marker/root/scope selectors and run alongside the new agent before the original highest duplicate/work-loss route is established. No runtime installed-agent premise supplied that process shape.

Proposed basis: Exact acceptance of startup continuation and potential duplicate agents, narrowly scoped; independent check must also judge the historical consequence lowering. Fresh cleanup does not authorize dismissing false confirmation or hidden-live-tab harm.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered-same-root-g005/f1. Full003samebest-effortstartup/possibleduplicateagentresidual as005f1. ExactSPECacceptsbothcontinuedstartup/endedclassificationandsecondagentbesidesurvivor; freshRestartRefusesweepmeansstartupfailurealonedoesnotprovecoexistingagents. Preserveoriginalpossibleworkcorruption/highestandprovisionalotherlowering forfreshgate, notapplyparentlowering. Portabledetachedunidentifiablesurvivorlimitationdistinct andnoblanketownershipwaiver. SeparatefalseRestartwarning005f4andNothingAliveDelete007c4stayunrefuted.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g007 record](records/review-through-00de0b6c/groups/g007.json). Record ID: `f1`.


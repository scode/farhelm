# Normal removal parks or cancels the listing before clearing its seal

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Investigated whether an in-flight listing can recreate a seal entry after successful host removal. The ordinary removal ordering excludes the proposed late-publication route, so no new bookkeeping defect is established. Original possible consequence retained: an invisible Pending/Failed entry after its host was forgotten, not extra destruction or changed resource authority.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g027/l9`.

### g027/l9

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 25 (candidate, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998. TLDR: Investigated whether an in-flight listing can recreate a seal entry after successful host removal. The ordinary removal ordering excludes the proposed late-publication route, so no new bookkeeping defect is established. Original possible consequence retained: an invisible Pending/Failed entry after its host was forgotten, not extra destruction or changed resource authority.

Details: refresh_once :5121-5123 does check actionable and note the seal before acquiring its cache lock, but the entire refresh future is inside serve's nudge select :4825-4831. Every machine-deleting forget_host_locked :1513-1517 first awaits hold_connection. That method :1663-1688 sets a fresh hold/incarnation, withdraws the client, nudges and waits for the matching parked acknowledgement. Outer actor run :3868-3879 acknowledges parked only after leaving serve, so the refresh future (including its synchronous seal note) has already completed or been dropped before the acknowledged hold returns. A nudge landing just after refresh completion is checked again at :4850-4861 before serve continues. Removal then destroys, serializes the row/cache deletion, stops its actor (provisioning/service.rs:2147-2159), closes tunnels and only afterwards clears the seal (hosts.rs:1570-1575). With no actor present, seal_is_actionable :2700-2704 cannot admit a new note. remove_host_for, remove_host_checked and finish_sealed_environment all converge on this same held removal; foreign ordinary hosts never get actionable seals. Thus the initial separate actionability/note calls are not by themselves evidence of publication after successful ordinary forgetting.

Checked exceptional boundary rather than assuming an unbounded hold: hold_connection :1682-1693 may continue after HOLD_PARK_TIMEOUT. The examined note path itself contains no await between its completed drain and actionability/note; the status/park route supplies cancellation, and no source-produced awaited operation was found that leaves an old refresh able to resume this seal note after a matching parked acknowledgement. A hypothetical thread stopped inside this synchronous segment beyond the park deadline would add an unestablished scheduling premise, not the concrete ordinary interleaving initially considered. No such incidence or blocking mechanism is claimed. The existing documented bounded shared-SSH release occurs outside this refresh note segment. No need to retain an open lead absent a supported contradictory ordering.

Contract: previously read SPEC_impl Environment Delete and the seal requires held destruction, retained seal failure and forgetting only after resource proof. The checked ownership/hold sequencing, rather than specification acceptance of stale records, refutes this particular candidate. Suggested bucket other if the suspected bookkeeping leak existed. Could-be-highest:false, no security or user-work-loss route alleged. Confidence: confirmed source refutation of the ordinary late-listing route, no execution. proposed_drop: source ordering/cancellation refutation above, not impact minimization or specification acceptance. possible_cover:none; known local l4 and canonical g028/l3 concern actual failure/outcome policy, not this candidate. Existing local l3 resource-continuity analysis is not used as a substitute for these cancellation checks. Revision: same-ID l9 lead closed to candidate after hold, serve, acknowledgement, all removal caller and clear-order checks; original suspected consequence preserved. No tests/builds/reproductions, prohibited coverage searches, nested agents, checkout writes or live actions.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `l9`.


# A delayed Delete result can leave a destroyed environment described as staying

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: A delayed Delete result can leave a destroyed environment described as staying

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g038/l2`.

### g038/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

## TLDR

Deleting the final two sessions can leave a stays/count notice indefinitely for a sandbox or container this same window
already knows was destroyed. The session rows can be gone while the success band still says the environment stays, until
dismissal or an unrelated host read.

## Details

The count is published without checking already accepted host evidence: publish_environment_delete (:590-607) installs
the kept text/tag; its only reconcile_hosts caller is accepted host-read completion (:2473-2476).
kept_band_after_hosts_read(None, read) does nothing, so an earlier successful absent/deleting read does not prevent
later publication.

One-GUI shipped sequence: confirm Session-scope Delete for A and its remaining peer B while A is pending. Each has its
own row claim; view.rs:1364-1408 explicitly permits unrelated sessions during requests. The helm serializes environment
Deletes using environment_delete_turn and provisioning (:176-265 in sessions/sandbox.rs). A receives Deleted {
remaining: 1 }, forgets A and returns the count at :250-257, releasing its function-local locks before client
consumption. Its HTTP body delivery can be delayed independently. B then deletes the last session and destroys the
resource at :259-263. The B completion and successful fleet-triggered hosts read can finish before As body reaches the
page. That read now proves host absence and reconciles the still-empty tag. A then publishes the old count via
view.rs:3005-3010, ignoring the current absent host. No deletion failure or multiple GUI is required.

Final source checks establish the response boundary: sessions.rs:3651-3654 awaits delete_environment_session, then
constructs the Axum JSON response, after the callee local lock/turn lifetimes. api/sandbox_sessions.rs:224-244 awaits
response headers and then response.text before returning the Session result; the count cannot reach the UI before that
separate body await finishes. No ordering binds that consumption to B or the separate WebSocket/hosts requests.

Persistence is bounded by repair triggers, not a guaranteed timer: A refreshes request_listing only, through
delete_refresh and :3120-3132. request_listing/request_hosts are distinct (:2510-2513); feed notices read both
(:2567-2574), but all server-side events from A and B may be consumed before delayed A publication. A already forgot its
server row before B, so receiving its response creates no new server event. No session-list commit or render calls
reconcile_hosts. Remaining automatic hosts reads use fallback_polls_now (:2580-2606), disabled by healthy feed
(feed.rs:233-234). Enumerated additional request_hosts consumers are explicit host mutation (:2628-2633) and launch
completion (:4890-4894), neither implicit in this session Delete. Notice markup (:6110-6128) is outside the session-list
match, so it survives an empty list. HostsRead::lookup (:1612-1627 in hosts.rs) retains successful absence as Absent; a
failed/pending lookup is not required for this route. An unrelated later host read, dismissal or remount removes it, but
an idle healthy page need do none.

Contract: SPEC.md Sandbox sessions (:1512-1525) requires a kept-resource result and accepts only a brief
concurrent-delete discrepancy followed by truth. The added absent/deleting retirement helper and test (:6700-6785)
expressly aim to remove a count once a current hosts read proves the resource going/gone. This ordering resurrects it
after that evidence, rather than waiting for evidence. Container sessions inherits the contract. Reconcile the new
publication with current accepted HostsRead or request hosts after publishing; preserve any checkout warning when
removing the count.

Attribution/novelty: count/tag publication is introduced by this assigned commit; the complete view.rs diff to inclusive
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db leaves it unchanged. Distinct from own l1 Restart retention, g036/l2 and
g088/l3 header-failure handoff, and g027/l4/g028/l3 sealed-failure bookkeeping. Suggested bucket: other, unchanged; only
contradictory lingering diagnostic is established. Confidence: confirmed source-permitted response/read interleaving,
never observed execution; incidence unmeasured. proposed_drop: false. possible_cover: none in supplied exact neighbour
records, no prohibited coverage search. Could-be-highest: no established security or work-loss route. Caveats: B
separately authorizes real destruction, and this bug never preserves or destroys extra resource state. Client-paced bulk
requests on one host do not produce this exact trigger; independent single-row/header Deletes do. The old response must
arrive after final host evidence, with no subsequent host read. No remaining missing code premise. No tests, builds,
runtime reproductions or live-install action.

Returned proposal (claims-023, event 15): **retain-verified**, suggested bucket **other**.

Source permits a delayed non-final Delete body to publish a kept count after a later Delete destroyed the resource and
the accepted hosts read already proved absence. No subsequent hosts read is guaranteed on an idle healthy feed. The
consequence is a stale result band, not unauthorized destruction.

Original stronger consequence: Earlier wording called the stale stays/count notice permanent. Latest wording correctly
bounds it by dismissal, remount or an unrelated subsequent host read. The original and latest records both classify only
a contradictory diagnostic as other, with no work-loss route. Initial lead included B's destruction failure; the
established route also permits successful destruction.

Unresolved premise or limitation: No missing source premise for the ordering. Actual runtime occurrence is unmeasured.
No route to extra destruction, security impact or loss of user work is shown; B's destruction is separately consented.

Proposed basis: Retain the established publication/read ordering defect as other, preserving source-only confidence and
the filter ambiguity. No exact existing cover or same-root alias was found among the named claims or searched queue
items.

Existing parent disposition (event 15): verified. Parent explicitly accepted named retain-verified proposal from
claims-023 after inspecting it. Source permits a delayed non-final Delete body to publish a kept count after a later
Delete destroyed the resource and the accepted hosts read already proved absence. No subsequent hosts read is guaranteed
on an idle healthy feed. The consequence is a stale result band, not unauthorized destruction. Retain the established
publication/read ordering defect as other, preserving source-only confidence and the filter ambiguity. No exact existing
cover or same-root alias was found among the named claims or searched queue items. Checked source/results: ["At
2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa, list/view.rs:479-507 retires only an existing tag; :590-607 publishes a new
tag without HostsRead. The only reconcile_hosts call is accepted host-read completion at :2476. :3005-3010
unconditionally publishes a successful old count.","At the same revision, list/view.rs:1364-1408 uses per-session claims
and explicitly permits unrelated sessions. Thus separate A/B Session Deletes can coexist; this is not the same-host
paced bulk path.","At the same revision, helm sessions/sandbox.rs:176-265 serializes each host's Delete with local
turn/provisioning guards. A's Deleted { remaining: 1 } return releases those guards before the client consumes HTTP
output. sessions.rs:3651-3654 constructs the response after awaiting the callee. UI api/sandbox_sessions.rs:224-244
separately awaits headers and text, permitting B and host reads to finish before A's body is consumed.","At the same
revision, view.rs:2910 and :3120-3132 refresh only request_listing; :2510-2513 has distinct list/hosts readers.
:2567-2574 reads both on fleet events, which can all settle before A's body. :2580-2606 gates future host polls on
fallback_polls_now; feed.rs:233-234 disables them while healthy. Historical git grep enumerated remaining request_hosts
consumers as mount, host mutation and launch completion, not implicit post-Delete refresh.","At the same revision,
hosts.rs:1612-1627 represents a successful missing host as Absent. view.rs:6110-6128 renders the band before the listing
match, including an empty list, and exposes Dismiss. The full view.rs diff through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db leaves this code unchanged.","This establishes a source-permitted ordering
only. No runtime reproduction, incidence measurement, real resource destruction or live-install action occurred."]
Current coverage/results: ["Current SPEC.md:1519-1529 accepts only a brief concurrent-Delete discrepancy followed by
truth; it does not clearly accept republishing a count after already accepted absence. Container sessions at :4831-4848
inherits this rule. SPEC_impl.md:6106-6159 describes the same seal protocol and no stale-band exception.","TODO.md:25-26
Planned is empty. Searches of plans/queue found environment-agent-operations.md:493-565's implemented design, including
a brief stays message followed by truth, but no pending work ordering publication against current host evidence. Other
environment plans share the subsystem, not this trigger/remedy.","BUGS.md is unrelated. INDEX.md and queue searches for
kept/stays counts, late environment Delete results, KeptNotice and publish_environment_delete found no same-root alias.
bulk-sandbox-delete-drops-eligible-peer.md is admission/coalescing; frozen-sandbox-retains-header-lifecycle-answers.md
and sandbox-lifecycle-questions-lack-opening-guards.md concern different answer owners. Active TRIAGE_OUTCOMES.md has no
matching result-count decision; archive not read.","FILTER.md ambiguity: rare response/read timing is established and
the result band could count as a misleading diagnostic. Its display branch requires a display that is briefly wrong;
here it can remain indefinitely on the idle page and an explicit session refresh does not retire it. Ordinary Dismiss,
remount or a later hosts read repairs it, so there is no durable corruption or restart-only repair. Recommend retention
unless the parent establishes that the independent diagnostic branch fully covers this success-result band. Rarity alone
is not the basis for omission."] Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Earlier wording called the stale stays/count notice permanent. Latest wording correctly bounds it by
dismissal, remount or an unrelated subsequent host read. The original and latest records both classify only a
contradictory diagnostic as other, with no work-loss route. Initial lead included B's destruction failure; the
established route also permits successful destruction."; "No missing source premise for the ordering. Actual runtime
occurrence is unmeasured. No route to extra destruction, security impact or loss of user work is shown; B's destruction
is separately consented.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g038 record](records/review-through-00de0b6c/groups/g038.json). Record ID: `l2`.

# Supplied timing-comment defect is source-confirmed and already owned by g020/l7

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Independently confirmed the supplied internal timing-documentation error: the helm's local
still-launching reply is normally earlier, but is not guaranteed to beat the supervisor relay's deadline. This
corroborates g020/l7 event23 and creates no new reported duplicate finding.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g018/f5`.

### g018/f5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 15 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR: Independently confirmed the supplied internal
timing-documentation error: the helm's local still-launching reply is normally earlier, but is not guaranteed to beat
the supervisor relay's deadline. This corroborates g020/l7 event23 and creates no new reported duplicate finding.
Details: agent_requests.rs 4550–4554 asserts 'answering by the end of the allowance after the Allow reaches the agent
before its relay gives up'. The actual local timeout at 4555–4557 is created after approved returns (4493–4515), the
optional clone-owner route check (4519–4528), and the awaited owned start_session_environment registration (4541–4549).
Registration awaits recipe/provider state at provisioning/tensorlake_add.rs 357–386 and add_confirmations.lock_owned at
388 before returning its result receiver. In contrast relay_agent_request at service/agent_relay.rs 646–651 adds
ordinary + approval + environment allowances, and HelmLink::upcall begins the single answer timer on queue acceptance at
317–326. Pre-card resolution, queue transit and post-Allow pre-timeout work consume that already-running relay
allowance; no shared deadline or remaining-budget subtraction is passed to the local timeout. Even apart from transport
time for the final reply, if those phases use more than the ordinary allowance plus unused approval allowance, the relay
expires before the local timeout. This establishes the mismatch without a runtime delay claim. Exact governing text
SPEC_impl.md 9393–9396: 'The relay's extra 30 seconds is an allowance for the work the helm does before and after the
wait, not a guaranteed ordering' and slow resolution can time out at the relay with unknown outcome. Environment wording
at 6695–6697 specifically says the agent 'normally hears that before its relay gives up'. Suggested correction: qualify
the internal comment as normal delivery with enough remaining relay budget; retain distinct clock origins, owned
continuation and honest unknown outcome. Suggested bucket: other (internal documentation only). Confidence: confirmed
source/caller/contract analysis. proposed_drop: no semantic drop or lowering; no new local reported finding because the
exact supplied g020/l7 event23 already owns this comment, consequence, revision and location. Parent owns
deduplication/disposition. possible_cover: exact g020/l7 event23, independently corroborated here; g005/f2 event7 is
distinct, concerning receipt-relative product span versus fresh post-Allow allowance, and is not treated as a duplicate.
Could-be-highest: no; no security, work-loss, production timeout or cancellation defect alleged. Caveats: fast phases
and early approval leave ample slack; runtime incidence unmeasured. f4's agent-guide exception remains independently
refuted and is not a basis for rejecting this documentation defect. No tests, build or reproduction.
Attribution/endpoint source check remains to confirm the new comment's patch origin and unchanged in-range status;
timing premises themselves are settled. Synthesis completion: the 190d844e parent-to-commit diff explicitly adds the
entire caller comment and local timeout at agent_requests.rs 4550–4557. Later hunk headers over these named paths show
changes only at unrelated agent Delete/test spans and provisioning update admission; service/agent_relay.rs has no later
diff. The endpoint caller source at 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db 4551–4557 retains the same comment and
fresh local timeout, so no later in-range repair. The first broad later diff was clipped; only its targeted hunk headers
and exact endpoint timing span are relied on, not unread unrelated tests. approvals.rs 485–538 confirms the approval
deadline is created when its card is inserted after resolution, independently of the relay clock. Read the full
b106c0e49646ecb9a17e1a06b17a70da0ece73e0 SPEC.md 4289–4341 and SPEC_impl.md 6658–6702 sections: g005/f2 correctly
concerns the separate effective-span description, not the newly implemented caller comment. Our corroboration does not
expand that record into a production timer failure or merge the two documentation causes. All readily checkable premises
for the supplied caller-comment mismatch are resolved. No open local lead; f5 remains a candidate solely to record
existing g020/l7 ownership, with no accepted parent drop or new duplicate finding.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g018 record](records/review-through-00de0b6c/groups/g018.json). Record ID: `f5`.

# Generic lost-reply remedy does not prescribe an unsupported environment retry key

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Generic lost-reply remedy does not prescribe an unsupported environment retry key

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g018/f10`.

### g018/f10

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 30 (candidate, open):

Reviewed commit: 190d844e58c96cf197aa838d0b0eea8062f48435. TLDR considered: the new environment-creating verb might
inherit a generic CLI/relay unknown-outcome remedy telling it to retry with an idempotency key, which environment
launches refuse. Actual checks refute that specific guidance concern. agent_client.rs:490-507 retains
request.is_mutating and the expected reply kind before dispatch; 557-564 sends a mismatched successful shape through
lost_reply; 623-630 appends only AGENT_MUTATION_UNKNOWN_REMEDY. The exact shared constant at
farhelm-proto/src/lib.rs:2595-2596 says to check the sessions current state before retrying, since the change may
already have taken effect; it says nothing about supplying a key or retrying immediately. is_mutating explicitly
includes CreateEnvironment at lib.rs:2240-2242, may_wait_for_user includes it at 2279-2281, and the assigned
ReplyKind::of_verb branch at agent_client.rs:681-683 expects Created for create/environment/clone. The
no-competing-CLI-timeout rule remains at agent_client.rs:193-199, leaving the supervisor the authoritative deadline
source. The product environment contract at SPEC.md:4336-4341 retains unknown outcome while the owned task carries on
and refuses keys; the existing manual exception (already source-checked under f4) separately describes environment
launch recovery. No stale ordinary-key instruction is introduced by the assigned reply-shape addition or this shared
error remedy. Suggested bucket if the suspected misleading remedy existed: other; Could-be-highest:false, no established
security or user-work-loss route. Confidence: confirmed source refutation, no runtime execution. proposed_drop: exact
key-free constant and caller classification above; this is a non-report candidate, no parent drop or lowering presumed.
possible_cover:none; f4 is earlier guide-specific analysis and is not reopened or changed. No unresolved premise or open
lead under f10. No tests, builds, reproductions, live actions or prohibited coverage reads.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g018 record](records/review-through-00de0b6c/groups/g018.json). Record ID: `f10`.

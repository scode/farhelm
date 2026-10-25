# Host edit documentation overstates pre-approval refusal

Reviewed commit: 02df2958e6ff2266a81b9f53715cefbf66bfbe76

## TLDR

A rename colliding with another host's name can ask for approval and then fail, although its docstring says refused
aliases get no card. The source check is syntax-only before approval; no authorization defect is claimed.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-helm/src/agent_requests.rs:2982`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Discovery pass 1, reviewed commit 02df2958e6ff2266a81b9f53715cefbf66bfbe76. TLDR: a host rename to a name another host
already uses can ask for approval and then be refused, contrary to the new code documentation's claim that refused
aliases never get a card. This is a documentation-only finding; no product-behavior fix is required by this claim. Exact
original anchor: agent_requests.rs:2982-2984. Before approval, lines 3004-3010 call only validate_alias
(store.rs:1591-1610), which has no registry access and checks trimming, controls and length. After approval at
agent_requests.rs:3034-3046, the identity writer reaches hosts.rs:1004-1015 and store.update_alias. The latter compares
effective display names and returns AliasTaken at store.rs:6660-6661 before writing. A duplicate-name edit therefore
reaches the card even when the conflicting name already exists. Narrow the docstring's guarantee to malformed alias
values; keep the post-approval store collision check. Suggested bucket: other. Confidence: confirmed static ordering;
the broad wording is inaccurate. Caveat: the preceding sentence is about canonicalization and could be read as intending
syntax refusals only, which is why this is limited to correcting that wording. No security, false success or
user-work-loss route is claimed. possible_cover: no exact coverage found. Reviewed SPEC.md:2793-2797 and pinned-tip
SPEC.md:2812-2818 allow the shared route's refusals on Allow; they support the behavior, not the docstring's broader
guarantee. Pinned SPEC_impl.md:3619-3631 and 5699-5720 distinguish syntax, collisions and protected writes without
promising pre-card collision refusal. TODO Planned is empty; BUGS, queue INDEX and ledger searches found no matching
pre-card-refusal documentation issue. The ledger's clearing-local-alias-restores-colliding-name.md addresses a collision
guard, not when an agent card is shown. FILTER's rare-diagnostic predicate is not established: an already-used alias is
ordinary CLI input and needs no rare condition, and this is a code-contract claim rather than a runtime diagnostic.
proposed_drop: none. The same sentence remains in pinned-tip agent_requests.rs:3034-3036. No reproduction attempted.

Provenance: `g293/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.

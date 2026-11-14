# Rollback phrase references a policy that already retains allocated checkouts

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: The implementation specification tells maintainers that an old-terminal create retry is rolled
back when refused, but an earlier attempt that already allocated its checkout deliberately keeps a visible error session
and its ownership records. This is inaccurate failure-lifecycle documentation; the inspected implementation preserves
the checkout correctly.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g008/f4`.

### g008/f4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. In-range introduction:
647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513.

TLDR: The implementation specification tells maintainers that an old-terminal create retry is rolled back when refused,
but an earlier attempt that already allocated its checkout deliberately keeps a visible error session and its ownership
records. This is inaccurate failure-lifecycle documentation; the inspected implementation preserves the checkout
correctly.

Details: Assigned patch lines 280-287 rewrites/reflows the Creating agents in hosts paragraph. At the reviewed source
SPEC_impl.md:2258-2259 it says a row the takeover claims is refused before anything starts and "rolled back like a
launch whose spec never landed", with its refusal settled against the intent key, without the allocated-checkout
distinction. core.rs:14700-14763 implements two different outcomes: no checkout or a still-Planned checkout can roll
back; an allocated or unreadable checkout calls retain_create_refusal and keeps the session and checkout records. The
source contract explicitly states why: deleting the last ownership record would orphan the checkout. Ordinary
post-allocation failure handling at core.rs:11104-11112 likewise explicitly calls its retained-refusal path "not a
rollback". Product contract SPEC.md:1757 says exactly: "Once allocation has occurred, failures retain the session and
its checkout association for inspection and Delete." SPEC_impl.md:3807-3809 says: "A post-allocation failure atomically
retains an error session and membership instead of losing the only deletion handle." Those correct governing statements
make the blanket retry-rollback wording inconsistent; production correctness does not refute the distinct documentation
defect. Clarify the retry paragraph with the same pre-allocation/allocated distinction as the helper.

Attribution checked across the pinned range: the exclusive base has no retried-tmux/takeover rollback assertion in
SPEC_impl.md. git log -S identifies 647f0227 as its introduction (then SPEC_impl.md:2315-2319); that revision's
core.rs:15854-15925 already implements and documents the allocated-checkout retention branch. Thus predating 1ed482d
does not put the defect outside the review range; 1ed482d retains it in this assigned rewrite. The concern is not a new
runtime regression at either revision.

Suggested bucket: other (documentation contract accuracy). Confidence: confirmed by source, governing contracts and
full-range attribution; no runtime behavior observed. Could-be-highest: no established security or loss-of-work route
from this prose; the current runtime explicitly retains user work and its deletion handle. proposed_drop: false.
possible_cover: none found in the supplied interface context; no prohibited coverage sources searched. Caveats: ordinary
non-checkout retry rollback is correct; the defect is the omitted allocated-checkout exception, not the refusal itself,
the existence of a terminal-host replacement, or the accepted retirement consequence. This revision closes f4's
rollback-terminology lead under its original ID.

Endpoint check: inclusive pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:SPEC_impl.md:2180-2185 retains the same
unqualified refusal/rollback wording; no later repair was found. Exact numbered current ordinary-create contrast is
core.rs:11102-11107 (post-mkdir failure uses retained-refusal, explicitly not a rollback). The earlier broad line
references include this passage and do not identify a separate defect. Introduction search used the shorter stable
subject A retried keyed create whose interrupted because line wrapping changes the longer rollback phrase.

Completed governing-context check: read the complete Managed checkouts subsection (SPEC.md:1570-1883) and complete Owned
checkout admission and lifetime subsection (SPEC_impl.md:3776-4102). They retain the allocated-session rule and
explicitly distinguish retained error sessions from no-surviving-row refusals; no rule was found making allocated
old-terminal retries an exception. This is a bounded documentation finding, with no additional runtime defect inferred.

Turn-5 reconciliation with caller-supplied g006/f3 latest event: revise my earlier confirmed-mismatch judgment. The
exact sentence says rolled back LIKE A LAUNCH WHOSE SPEC NEVER LANDED, not that every row or checkout is deleted. Read
as a policy reference, that comparison imports the post-allocation exception explicitly specified by the complete Owned
checkout admission subsection and SPEC.md Managed checkouts. Directly checked 647f0227 core.rs:11827-11863: the
referenced FailedStart::Spec branch itself distinguishes ordinary rollback from retain_create_refusal after allocation.
The old-terminal helper at :15866-15924 makes exactly the same distinction. This matters independently of production
correctness: the prose cross-reference and its governing allocation rule describe the same qualified policy. My earlier
use of core.rs:11102-11107 saying not a rollback does not establish a stronger claim of unconditional row deletion in
this paragraph; it distinguishes a branch within that same policy. The stronger contradiction I previously asserted is
therefore not established. Original possible reader consequence remains: an isolated literal reading of rolled back
could lead a maintainer to expect no retained session; a clarity improvement could name the exception locally. That is
not enough to report a separate correctness defect when the sentence expressly points at an already qualified failure
policy.

Current suggested bucket if the ambiguity were retained: other. Could-be-highest: no established or suspected
security/work-loss route; preserve the prior consequence without manufacturing runtime damage. Current confidence:
source/contract refutation of the claimed policy mismatch, not execution-based. Current proposed_drop: exact comparison
wording plus SPEC.md:1757, Once allocation has occurred, failures retain the session and its checkout association for
inspection and Delete, and SPEC_impl.md:3807-3809, A post-allocation failure atomically retains an error session and
membership instead of losing the only deletion handle. These explicit qualifications apply to the referenced
spec-not-landed policy as the directly read branch confirms. This is a proposed exclusion only; no parent drop is
accepted or inferred. possible_cover: g006/f3 is the exact same wording concern with this same interpretation, supplied
by the caller and read directly; not a second defect. Original finding rationale, source evidence,
allocation-preservation motivation, range attribution and strongest consequence remain above in this append-only record.
No source premise remains open for this bounded documentation interpretation.

Returned proposal (claims-004, event 13): **refuted**, suggested bucket **other**.

The sentence compares refusal with an existing qualified policy; it does not say every allocated checkout or row is
removed. The original categorical contradiction is not established. The withdrawn stronger interpretation remains
recorded here.

Original stronger consequence: Earlier events called the paragraph inaccurate failure-lifecycle documentation that could
make maintainers expect no retained session. Runtime preservation was always acknowledged; no actual deletion or
orphaning was claimed. Latest event withdraws the categorical mismatch, preserving a possible isolated-reader clarity
concern.

Unresolved premise or limitation: The claim of an unconditional removal policy would require text overriding the
referenced failure policy's explicit post-allocation exception; none was identified.

Proposed basis: Propose the latest source/contract refutation, preserving the withdrawal. The original ambiguity touches
the only deletion handle for user-owned checkout work, so obtain an independent drop judgment. History mentions external
g006/f3 as an alias, but that record was not assigned or read here and no cross-batch duplicate decision is inferred.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: source-refuted-same-root-g006/f3. Full004samequalifiedrollbackcomparisonas006f3,
in-range647authorshipnotprebase. 'Likeaspecneverlanded' referencespolicywith
explicitpostallocationretainerrorrow/membership;
actualFailedStart::Specandoldretrybothpreserveallocated/unreadableorigins. No overridingunconditionaldeletiontext.
Preserveearliercategoricaldocumentationfindingwithdrawal/isolatedreaderclarity/ownershiphandleharmforfreshgate,
notrefutedmerelybyproductioncorrectness.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g008 record](records/review-through-00de0b6c/groups/g008.json). Record ID: `f4`.

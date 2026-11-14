# Sandbox lifecycle questions omit opening and claim ownership

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Sandbox lifecycle questions lack the required opening-token and operation-claim ownership. Same-dialog Cancel and Submit
before redraw is product-accepted when its destructive guards hold; a stronger obsolete-opening loss route remains
unverified.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/sandbox_sessions.rs:187-211,248-266`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

Controls stores an Option<Operation> and a separate held operation guard instead of using ConfirmSlot
(sandbox_sessions.rs:187-213). Cancel/Escape clear them without an opening token (:248-263); Confirm unconditionally
executes (:264-266). A later Cancel can also release the local guard after execution has begun. SPEC_impl.md:1042-1071
requires rendered opening identity and task-owned claim transfer; ops.rs:317-349 explains the first-answer-wins
requirement.

The source-supported same-dialog Cancel-then-Confirm Stop route remains in the history, but SPEC.md:4856-4859 explicitly
permits that burst when the matching broad warning and destructive preconditions still hold. Fresh independent
consultations agreed narrowly that this consequence alone is product-accepted and the retained implementation defect is
other. The earlier orphan-Delete ledger precedent also includes a separate cross-opening route and does not withdraw
that product text.

Stronger obsolete-opening substitution remains unverified after investigation: matching Dioxus source resolves current
mounted listeners, and no complete shipped cross-opening/element-ID sequence was established. Do not claim it was
refuted globally or borrow the separate no-question guard violation. Use a rendered-token ConfirmSlot and transfer its
guard into the task. g051-L2, g056-stale, g057-controls-confirmation and g058-s2 preserve the same history and
qualification. Suggested bucket other, confirmed implementation-contract omission.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g054/g054-control-answer-binding`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g026/f1`.

### g026/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: 1d24c6af0b6fdbbec854623ff53f90281b8271ab

## TLDR

An old Stop and suspend answer could consume a later lifecycle question's live claim. Changing the selected session does
not establish a wrong-session route because the selected-session view is keyed by session ID. This concern is not
reported against this group's patch because the confirmation mechanism predates the review range.

## Details

Source checks: lib.rs:2268-2270 keys SessionView by session.id. sandbox_sessions.rs:261-284 refreshes execute through
use_callback and takes live held/stop_guards; :327-359 uses Signal<Option<Operation>>, unconditional Cancel/Escape
clearing, and execute.call(operation), without an opening token. list/view.rs:3070-3075 forwards a Stop action to the
ordinary Stop path. SPEC_impl.md:1068-1086 says "A handler for something the user answered or confirmed acts on what the
render that drew the control showed, never on state it reads when it runs" and "Every question, confirmation and
destructive prompt goes through ConfirmSlot". ops.rs:245-249 explicitly describes queued cancel/reopen/old-answer
bursts. The potential consequence remains stopping processes under a cancelled earlier answer while a later Suspend
question is open; could-be-highest: yes, if this route ends user-owned work. No cross-session route established.
Pinned-source git blame attributes question opening/clearing and answer dispatch to 952183b99 and live claim transfer to
bd2fdb7ff. Both git merge-base --is-ancestor checks against the review range's exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a exited 0, confirming pre-range ancestry. The assigned 1d24c6af change adds
launch-target exclusion at lines 264-272 and disabled markup, which does not introduce the token omission. Suggested
bucket: highest only if reported elsewhere with the destructive route established; this is a scope exclusion here.
Confidence: confirmed pre-existing source structure; no execution. proposed_drop: outside assigned patch/range, pending
parent independent check for could-be-highest. possible_cover: none; no queue or coverage sources searched. Mandatory
patch packets remain unread because the caller manifest is invalid; attribution is from pinned-source blame, not a
claimed patch audit. Revision adds ancestry-check results and dispatch-path source confirmation.

Coverage repair revision: all mandatory patch packets were delivered through the corrected manifest in initial-review
continuation turn 2. The sandbox_sessions.rs patch at 1d24c6af0b6fdbbec854623ff53f90281b8271ab confirms it changes
launch-target exclusion and page-lock ownership, while retaining the pre-existing question signal, live held claim and
answer dispatch unchanged. The prior mandatory-unread caveat is superseded; scope exclusion and other caveats remain.

Synthesis turn 3: independently read the exclusive-base sandbox_sessions.rs, not only blame. At
6e423e30068b62c6cfa276c6d720ee35bb70387a:190-191 the question and held claim are unversioned signals; :197-207 clears
the current question and consumes the live held claim and live session guards; :268-286 Cancel/Escape clear those
signals and Confirm calls execute(operation), with no opening identity. Thus the old-question/new-question mechanism
itself exists at the exclusive base. At 086d4ff96efb13bdb51accaa1ce0a0a6f7d5ca5b:229-233,262,275,288 a new stop_guards
signal freezes guard values when start runs and reads that live signal at execute. The assigned patch changes neither
prompt opening identity nor which held claim an old answer consumes. Parent supplied g023/f4 is a DIFFERENT boundary: an
ended/no-tabs render is refreshed before the start callback runs, which then captures ended=false. The exclusive base
already has that refreshed callback route (:220-247 and :300-303) and computes guards even later at execute (:197-207).
The 086d4ff patch addresses refresh DURING the awaited activity read, not refresh BEFORE the start callback; neither
addresses reopening tokens. These source checks support the pre-range attribution independently, but no parent drop is
presumed accepted. g025/f3 opposite-direction behavior was not supplied as source and is not treated as a refutation or
duplicate. The keyed-session check refutes only the cross-selected-session theory; it says nothing about an older answer
consuming a newer opening for the SAME session. The potential process-loss consequence and could-be-highest flag remain.
No execution. proposed_drop remains pre-range scope, pending independent parent disposition.

Returned proposal (claims-014, event 10): **out-of-range**, suggested bucket **other**.

The missing opening token and live held-claim consumption are already present at the exclusive base, and the existing
queue item preserves the same contract omission plus the unresolved stronger cross-opening consequence. The assigned
launch-exclusion change does not introduce this mechanism.

Original stronger consequence: An old Stop and suspend answer could consume a later lifecycle question's live claim and
stop processes under cancelled consent. Initial cross-selected-session termination was narrowed by the keyed-view check.
The corrected mandatory-patch-packet caveat is superseded, while this possible process-loss route remains unresolved.

Unresolved premise or limitation: The full shipped event/element-identity ordering that makes an old affirmative answer
consume a later opening and terminate user-owned work remains unestablished. This is retained uncertainty, not a global
refutation.

Proposed basis: Pre-range mechanism and exact existing queue retention. Bucket other applies only to the established
contract omission; independent agreement is still required for scope exclusion or treating the stronger possible
process-loss claim outside highest.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-existing-queue-cover-with-original-stronger-uncertainty. Full014 exact queue
sandbox-lifecycle-questions-lack-opening-guards retains same missing opening token/current-held claim and investigated
obsolete-opening uncertainty. Same-dialog burst acceptance only when destructive guards hold; keyed session identity
refutes cross-selected-session reuse only. Proposed no new item for exact already retained root, not generic
safety/source refutation; out-of-range attribution alone not needed for cover. Preserve possible original process-loss
chain/missing native element-order premise and new Podman scope if actual late answer bears on it. Fresh gate checks
full exact queue content and narrow coverage, not subsecond rarity.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f1`.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g028/l1`.

### g028/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (candidate, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa. Original TLDR considered: a delayed answer to Suspend/Stop
and suspend might act on a newer prompt or session and potentially stop a different running process. Could-be-highest:
yes for a process-loss route beyond the warning; that original different-session consequence has not been established
here and is not replaced by a claim of safety. Suggested bucket of hypothetical effect: highest if that route is shown,
high for wrong lifecycle direction without work loss. Confidence: confirmed pre-existing context, not an in-scope
finding; no claim that the underlying confirmation behavior is safe. Source validation: the assigned patch adds Delete
helpers at sandbox_sessions.rs:92-205 and Delete tests at 503-695, leaving Controls and its question/execute handlers
untouched. More strongly, exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has Controls.question as
Signal<Option<Operation>> at line 190; execute is a use_callback at 197-218 using execute_session from the current
render and deriving Stop guards from that session at 203-206; start is a use_callback at 220 and the un-tokened question
confirm invokes execute.call(operation) at 284-286. Thus both missing question identity and refreshed callback state
existed before the reviewed range. At the assigned UI revision, Controls at 338-464 retains the plain question and
callback structure, with stop_guards captured at click handling; this is distinct from new
DeleteScope/EnvironmentDelete, whose Delete answer handlers are token-bound. Parent crossinput says g023/f4
independently established a pre-handler refreshed Stop-guard route and proved it at the exclusive base; that report is
corroborating context, not an accepted parent drop or a newly adopted local finding. g025 direction and g026
old-question claims remain separate causes/provenance. proposed_drop: outside assigned review scope, with exact charter
text: 'Review only your assigned functional group, including each selected commit’s patch, not merely an endpoint diff.'
This record does not purport to dispose of the other reviewers' records or of the underlying pre-existing behavior.
possible_cover: none. SPEC_impl.md GUI: Dioxus, Answers bind to what was drawn remains the applicable contract; no spec
acceptance is claimed. This revision replaces the earlier immediate-parent-only age check with explicit exclusive-base
source evidence and preserves the original suspected consequences. No execution or reproduction performed.

Returned proposal (claims-015, event 9): **out-of-range**, suggested bucket **other**.

The missing prompt identity and refreshed callback structure already exist at the exclusive base. The named Delete patch
does not introduce them. An existing queue item preserves the same implementation omission and stronger unverified
obsolete-opening consequence.

Original stronger consequence: Delayed Suspend/Stop and suspend could answer a newer prompt or stop a different running
process, potentially highest. Latest history explicitly preserves that hypothetical consequence while proposing range
exclusion.

Unresolved premise or limitation: A complete shipped delayed-answer sequence changing the prompt/session target beyond
the displayed warning remains unestablished. The base proves age of the mechanism, not safety of every possible
sequence.

Proposed basis: Propose out-of-range for this named record's pre-existing mechanism, with existing-queue same-root
context. Independent check must preserve the stronger hypothesis and must not dispose of other reviewers'
direction/old-question/guard records.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-existing-opening-token-queue-cover. Full015 pre-base unversioned question/refreshed
callback and unchanged Delete-patch Controls, exact existing sandbox-lifecycle-questions-lack-opening-guards retains
same omission and stronger investigated obsolete-opening uncertainty. Prefer exact current queue coverage over broad
source-age exclusion; pending025/023 Podman-domain attribution may supply further scope. Same-dialog burst acceptance
cannot cover obsolete opening; no complete harmful physical element sequence shown but not refuted by mere missing
reproduction. Fresh gate exact scope/queue, separateguard/direction findings unaffected.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g028 record](records/review-through-00de0b6c/groups/g028.json). Record ID: `l1`.

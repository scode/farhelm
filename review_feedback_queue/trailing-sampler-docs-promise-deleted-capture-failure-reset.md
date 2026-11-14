# Sampler field documentation still promises the deleted capture-failure reset

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The sampler's current lifecycle documentation tells future maintainers that failed reads
clear prompt evidence and reset the activity baseline, but this change deletes the only operation that did that. This is
a documentation correctness defect; no current runtime regression is claimed.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g132/c3`.

### g132/c3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: The sampler's current lifecycle documentation tells
future maintainers that failed reads clear prompt evidence and reset the activity baseline, but this change deletes the
only operation that did that. This is a documentation correctness defect; no current runtime regression is claimed.
Details: ticker.rs:373-375 promises workspace_trust_prompt is 'cleared on failed capture', :411-416 describes reading
falling back after 'a failed capture withdrew what the last screen showed', and :422-431 declares transition_ready
'False for a new or reloaded run and after a failed capture' and links that reset to suppressing activity/reordering
after recovery. The assigned patch deletes ActivitySample::forget_tail and forget_tail_hinted, which previously
fulfilled those statements. At the reviewed revision a complete src search for transition_ready finds only its
declaration (:432), read (:609), and assignment to true (:614); Default at :364-365 and new/reloaded/replacement
constructors establish its initial false state. The retained sampler skips unavailable/replaying hosts (:1327-1337), and
infallible host screen reads have no failed-capture reset event. These comments are present-tense field contracts and
explanatory baseline guarantees, not explicitly historical context. They should describe the actual retained lifecycle
or explicitly label the retired tmux behavior as history. Original considered consequence retained: a future maintainer
can rely on a nonexistent failure-recovery reset when reasoning about prompt hints and activity ordering. The earlier
rationale that runtime harm had not been shown was insufficient to dismiss inaccurate documentation. Suggested bucket:
other. Confidence: confirmed by source, including all transition_ready references; no execution. proposed_drop: false;
parent owns coverage/filter judgment. possible_cover: none known, coverage sources not searched. Could-be-highest: no;
this is an inaccurate internal lifecycle description, with no established security or loss-of-work route. Caveat: this
does not assert the removed reset is needed by the host-only implementation, nor that host encoding/unavailability
currently fabricates activity; c1 separately refutes that regression claim. Additional synthesis check:
ticker.rs:265-268 also documents Observation.prior as None on the first sample after a failed capture, and :379-380 says
a later failed capture can withdraw a recognized waiting claim. Those present-tense statements depend on the same
deleted reset, so this is one connected documentation finding, not separate issues. The retained constructors at
:512-541 create fresh baseline evidence for new/reloaded/replacement runs; none is a capture-failure recovery path.

Returned proposal (claims-083, event 7): **retain-verified**, suggested bucket **other**.

The present-tense sampler contracts still describe resetting screen evidence and transition readiness on failed
captures, but the patch deletes the operation implementing that lifecycle. Infallible host-only sampling does not supply
an equivalent event. This is a documentation correctness defect, even though no runtime regression is established.

Original stronger consequence: Future maintainers can rely on a nonexistent failure-recovery reset when reasoning about
prompt hints and activity ordering. The first record set this aside for lack of runtime harm, then explicitly reopened
and verified the documentation contract. No security or user-work-loss route was claimed; g132/c1 separately refutes a
new production host-read regression. New/reloaded/replacement initialization is not a failed-capture recovery event.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the inaccurate current lifecycle declarations with other priority. Keep the matching SPEC_impl
wording and the already-landed cleanup plan visible as context, without treating either as cover for incorrect source
documentation.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-083 after inspecting it. The present-tense sampler contracts still describe resetting screen evidence and
transition readiness on failed captures, but the patch deletes the operation implementing that lifecycle. Infallible
host-only sampling does not supply an equivalent event. This is a documentation correctness defect, even though no
runtime regression is established. Retain the inaccurate current lifecycle declarations with other priority. Keep the
matching SPEC_impl wording and the already-landed cleanup plan visible as context, without treating either as cover for
incorrect source documentation. Checked source/results: ["At 1ed482dde220452e424973faf38a5e1318986333, ticker.rs:265-268
says the first sample after failed capture has no prior; :373-380 promises trust clearing and waiting-claim withdrawal;
:411-431 describes fallback/reset after failure. These are current contracts, not labelled historical
explanation.","Parent ticker.rs:743-750 implements forget_tail clearing trust, comparison and transition_ready. The
reviewed revision removes that method and forget_tail_hinted. A complete reviewed-src transition_ready search returns
only declaration :432, read :609 and assignment true :614. Fresh/reloaded/replacement constructors (:512-541) establish
initial state rather than failure recovery.","Reviewed ticker.rs:1327-1337 skips unavailable/replaying hosts;
hosts/terminal.rs:2229-2247 and :2275-2284 expose infallible emulator/view reads. No retained failed-capture reset was
found. Diff through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes unrelated ticker comments and leaves these
contracts intact; current ticker.rs:267,372-424 still contains them.","No runtime defect, fabricated activity during
encoding, or need to restore the old reset is asserted. The earlier no-runtime-harm dismissal was withdrawn in the named
history and is not a valid source refutation."] Current coverage/results: ["Both specs searched. Current
SPEC_impl.md:3702-3704 also says the trust bool resets after failed capture; :4234-4235 and :4251-4254 repeat
capture-failure baseline language. These describe the same obsolete lifecycle, not explicit acceptance that source
comments may falsely promise it. Preserve this contract ambiguity for triage; do not silently turn it into a runtime
finding.","TODO.md Planned empty. BUGS.md and active TRIAGE_OUTCOMES.md have no matching sampler-contract
disposition.","plans/queue/tmux-drop-from-docs.md:68-75 and :88-96 broadly require stale comments cleaned, but
plans/REPORTS.md:32 records the plan landed. It is neither pending coverage nor evidence these particular comments were
repaired.","INDEX.md plus matching queue text searched. transition-accessor-comment.md concerns only an obsolete
accessor spelling while preserving its contract, not this false recovery lifecycle. No exact existing item or useful
same-root extension found.","FILTER.md checked. A permanently misleading internal lifecycle contract has no rare runtime
trigger or wholly self-correcting diagnostic consequence, so no full filter match."] Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "Future maintainers can rely on a nonexistent
failure-recovery reset when reasoning about prompt hints and activity ordering. The first record set this aside for lack
of runtime harm, then explicitly reopened and verified the documentation contract. No security or user-work-loss route
was claimed; g132/c1 separately refutes a new production host-read regression. New/reloaded/replacement initialization
is not a failed-capture recovery event."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g132 record](records/review-through-00de0b6c/groups/g132.json). Record ID: `c3`.

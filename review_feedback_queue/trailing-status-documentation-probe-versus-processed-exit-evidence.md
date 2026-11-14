# Status documentation conflates process liveness with processed exit evidence

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Rewritten status and test documentation teaches the wrong lifecycle boundary: it calls a
live reading proof that the host's child has not exited and says observation can see death before the exit record/code
arrives. The implementation deliberately keeps status live after process death until the exit record is processed. This
misleads maintainers about when status and launch classification may treat a terminal as dead; no runtime
misclassification is established. Suggested bucket: other (newly inaccurate lifecycle documentation). Could-be-highest:
no demonstrated security or user-work-loss route from the prose. Confidence: confirmed source/caller/range analysis, no
execution. proposed_drop: false. possible_cover: none matching in supplied neighbour records; no prohibited coverage
search.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g130/l3`.

### g130/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Rewritten status and test documentation teaches the
wrong lifecycle boundary: it calls a live reading proof that the host's child has not exited and says observation can
see death before the exit record/code arrives. The implementation deliberately keeps status live after process death
until the exit record is processed. This misleads maintainers about when status and launch classification may treat a
terminal as dead; no runtime misclassification is established. Suggested bucket: other (newly inaccurate lifecycle
documentation). Could-be-highest: no demonstrated security or user-work-loss route from the prose. Confidence: confirmed
source/caller/range analysis, no execution. proposed_drop: false. possible_cover: none matching in supplied neighbour
records; no prohibited coverage search. Details: production docstring status.rs:179–181 newly describes liveness as "the
host's child has not exited — a fact". The rewritten enrichment-test docstring :1053–1057 says "a host terminal's probe
says dead before its exit record arrives" and therefore "the observation that first sees the death can have no code
while the next one does". These are two manifestations of the same conflation. The only production caller of observation
is observe_entry at :704, which obtains live from terminal_evidence at :580–604; :124–127 calls host.evidence, not
probe. hosts/terminal.rs:835–850 explicitly keeps Running evidence dead:false until the complete ChildExit is processed,
then publishes dead:true and maps the code together. probe at :818–827 can return Dead before that, but is not this
observer's input. Signal death yields no code (:853–859, :886–889), rather than a temporarily unread normal-exit code.
The synthetic test values at :1060–1067 still usefully check monotonic enrichment of a previously stored unknown code;
they do not demonstrate the claimed within-host observation sequence. The processed-exit contract is explicit in
hosts/terminal.rs:835–837 and SPEC_impl.md:2703–2704, :2760–2762. Correct production behavior does not refute the
documentation defect. Attribution: exclusive-base 6e423e30068b62c6cfa276c6d720ee35bb70387a and the immediate parent
explain tmux pane_dead before pane_dead_status (base status.rs:1303–1306, parent :1298–1301), rather than falsely
describing host observation. The assigned patch also replaces the earlier live-tmux-pane fact with the incorrect
child-not-exited claim. The only later in-range status.rs commit is f560058da19e02716e6f943883a9508b447aff4e, whose diff
only removes terminal-kind reply fields/helper and leaves both explanations. Pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the test explanation at :1033–1037. Final source check supplies a valid
enrichment reason without excusing the misleading prose: service/sweep.rs:2035–2050 can record StopCompleted with
unknown code immediately after its process sweep and explicitly relies on a later tick to enrich it;
handlers.rs:997–1026 likewise distinguishes process-probe death from final-state/exit-record arrival. Thus keep the
useful synthetic enrichment assertions, but explain a previously incomplete stored outcome (for example Stop's earlier
recording), rather than inventing an early-dead evidence reading by this observer. Original consequence preserved:
misleading future lifecycle/classification reasoning, not a demonstrated user-visible failure. Lead l3 is resolved; no
missing source premise remains. This revision adds the concrete valid producer of stored unknown-code outcomes and
completes discovery checking.

Returned proposal (claims-082, event 11): **retain-verified**, suggested bucket **other**.

New production and test prose attributes the observer's liveness/enrichment to process-probe timing, while its caller
consumes evidence that stays live until the exit record and code are processed together.

Original stronger consequence: Misleading future lifecycle/classification reasoning, with no demonstrated runtime
misclassification, security harm, or user-owned-work loss. The test remains useful for enriching previously stored
unknown codes; do not delete it or claim such outcomes unreachable.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain introduced lifecycle documentation defect at both locations. Correct production ordering does not
refute it. No FILTER.md boundary covers the entire consequence.

Existing parent disposition (event 11): verified. Parent explicitly accepted named retain-verified proposal from
claims-082 after inspecting it. New production and test prose attributes the observer's liveness/enrichment to
process-probe timing, while its caller consumes evidence that stays live until the exit record and code are processed
together. Retain introduced lifecycle documentation defect at both locations. Correct production ordering does not
refute it. No FILTER.md boundary covers the entire consequence. Checked source/results:
["1ed482dde220452e424973faf38a5e1318986333:service/status.rs:179-181 says the child has not exited; :1053-1057 claims
the observer first sees host death without code because the probe precedes the exit record. The patch replaces
tmux-specific explanations with these assertions.","Same revision status.rs:124-127 terminal_evidence calls
host.evidence; observe_entry at :580-604 obtains that evidence and :704 calls observation. hosts/terminal.rs:818-850
shows probe can report Dead while Running evidence remains dead:false; Exited evidence sets dead and maps the complete
ChildExit together. Signals legitimately have no code (:853-889).","Same revision service/sweep.rs:2035-2050 can record
StopCompleted with an unknown code immediately after its sweep and explicitly relies on later enrichment. This provides
a valid reason for the synthetic assertions without validating their rewritten timing
explanation.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the false liveness prose at :179-181 and test
explanation at :1033-1037; the only intervening status diff removes reply fields/helper."] Current coverage/results:
["SPEC.md:2651-2684 Status and SPEC_impl.md:2134-2146/2662-2666 processed-exit boundaries were read. They support the
evidence distinction; none accepts misleading lifecycle documentation.","TODO Planned empty; plans queue index empty;
BUGS unrelated. Queue INDEX and active ledger searches for probe death/processed exit/enrichment/liveness documentation
found no exact item. stop-annotation-exit-witness.md was read: it concerns a particular test's premature assertion, not
this explanatory contract. transition-accessor-comment.md is a stale accessor name with an otherwise accurate contract,
also distinct."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Misleading
future lifecycle/classification reasoning, with no demonstrated runtime misclassification, security harm, or
user-owned-work loss. The test remains useful for enriching previously stored unknown codes; do not delete it or claim
such outcomes unreachable."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g130 record](records/review-through-00de0b6c/groups/g130.json). Record ID: `l3`.

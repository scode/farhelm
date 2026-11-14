# Earlier codec-removal refutation does not refute the known inherited-history excerpt route

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Earlier codec-removal refutation does not refute the known inherited-history excerpt route

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g167/c4`.

### g167/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333; excluded range base compared:
6e423e30068b62c6cfa276c6d720ee35bb70387a. Original TLDR considered remains: deleting pane-mode/capture parsing might
make follow-up prompts use stale paste modes (newlines submitted as keys), or quote another launch's terminal output in
a durable launch error. Original proposed-drop basis was correct host-only gates, processed exit and a blank new screen.
Synthesis correction: these refute a dependency on deleted tmux parsing, but do not refute contamination after inherited
history is promoted into that screen. Supplied neighbour g100/c1 establishes that distinct known route, overlapping
g013/f2's later in-range specification change; no new discovery is claimed. Own source checks now confirm
grid/resize.rs:25-28 grows screen height through grow_lines and :57-71 promotes existing history;
snapshot/v1/mod.rs:242-268 seeds primary history above a blank screen; launch_artifacts.rs:59-76 quotes current visible
screen after processed exit. The history-promotion mechanism invalidates a blanket assertion that visible-screen
extraction always excludes previous-run content. Base launch_artifacts.rs:52-94 already contains the same host-only
quote and blank-screen rationale, and grid/resize.rs has no base-to-reviewed diff. Do not infer that the bug predates
the entire range merely because it predates this assigned deletion: g100/c1 and g013/f2 retain the known in-range
documentation/contract attribution for parent reconciliation. This group's codec deletion supplies no additional changed
responsibility or independent route. Confidence: source-confirmed correction of the earlier refutation, with the
complete viewer/publication route supplied by g100's exact latest record; no execution. Suggested bucket for
inherited-content diagnostic: other; could-be-highest: no established work-loss/security route for that diagnostic.
Preserve the earlier potential-highest prompt hypothesis (unsafe submission could damage work if its missing route were
established); source checks and direct retained tests still refute that hypothesis as a consequence of this codec
deletion. proposed_drop: false for the known inherited-history diagnostic; earlier blanket source refutation withdrawn.
Attribution-only proposed drop of a new defect in this codec deletion remains a proposal, not an accepted disposition of
the known broader issue. Prompt-specific source-refutation proposal remains unchanged: core.rs:11913-12010 gates the
current entry's host on liveness/caught_up/paste mode, helper 2192-2233 awaits paste Written before submit, and
host_terminals.rs:541-588,1484-1535 directly asserts refusal and exact delivery. Those tests do not claim to test
inherited-screen provenance. possible_cover: supplied g100/c1 and g013/f2 are known overlapping records; no prohibited
sources searched. No readily checkable premise remains open for this reconciliation. No runtime tests or reproductions.

Returned proposal (claims-106, event 13): **retain-verified**, suggested bucket **other**.

Retain the corrected inherited-history diagnostic/specification concern for parent reconciliation. The earlier blanket
excerpt refutation is withdrawn: a host's visible screen can contain seeded prior-run history after height growth. The
runtime mechanism predates the range, but this range adds the false blanket specification claim. The prompt-mode
deletion hypothesis remains separately source-refuted.

Original stronger consequence: Deleting pane-mode/capture parsing might make follow-up prompts use stale paste modes,
submitting newlines as keys and potentially damaging work if an execution route existed, or quote another launch's
output in a durable launch error. Earlier blank-new-screen refutation of the excerpt consequence was explicitly
withdrawn; the diagnostic consequence is other, with no established security/work-loss route.

Unresolved premise or limitation: No missing premise for the source contradiction. Parent must reconcile with supplied
neighboring g100/c1 and g013/f2, whose logs are outside this batch and were not read. No established
unintended-command/work-loss route for the prompt-mode hypothesis.

Proposed basis: Keep the inherited-content/specification claim; reconcile rather than write a second item if g013/f2
already owns it. Independently check the separate proposed refutation of the potential-highest prompt route. Do not call
the whole issue out-of-range merely because its runtime mechanism exists at the base.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g167 record](records/review-through-00de0b6c/groups/g167.json). Record ID: `c4`.

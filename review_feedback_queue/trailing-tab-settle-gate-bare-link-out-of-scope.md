# TabSettleGate retains an unresolved documentation link through the pinned tip

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Developers reading the supervisor's test-seam documentation lose the link to the
tab-start timing bound. The unresolved reference survives the remaining reviewed commits. No effect on running sessions
is established.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g105/l2`.

### g105/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. Synthesis also checked through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

TLDR: Developers reading the supervisor's test-seam documentation lose the link to the tab-start timing bound. The
unresolved reference survives the remaining reviewed commits. No effect on running sessions is established.

Details: The original patch removes TAB_LAUNCH_SETTLE from the super::terminals import at core.rs:78-82, but the public
TabSettleGate docstring still links to [`TAB_LAUNCH_SETTLE`] at :748. Source search finds no declaration or import of
that name elsewhere in core.rs. The constant is defined at service/terminals.rs:47; host_tabs.rs:126 imports it only
into that child module. TabSettleGate remains publicly re-exported by service/mod.rs:133-140. The name is therefore
absent from the docstring's defining lexical scope. Qualify the reference as super::terminals::TAB_LAUNCH_SETTLE, or
render it as ordinary code text to avoid linking a public docstring to a private constant.

Later-change check: git log over the three related paths identifies dbfc69751215dbf15e1d31d5bed73c3754c4ebc4,
1ed482dde220452e424973faf38a5e1318986333, f560058da19e02716e6f943883a9508b447aff4e and
08399d092d20815ced6619c26df2391944ae2b4a. The last changes nearby historical wording and renames the gate's underlying
alias, but leaves the bare link unchanged. A source search at the pinned tip finds the same lone TAB_LAUNCH_SETTLE
reference at core.rs:662, followed by the public TabSettleGate alias at :671. No later qualification repair was found.

Suggested bucket: other; consequence is a broken developer-documentation reference. Confidence: confirmed source-level
name-resolution analysis, not observed generated-page behavior. Rustdoc was not executed; no warning, build failure or
warnings-as-errors consequence is claimed. proposed_drop: false. Could-be-highest: false; no security or user-work
route. possible_cover: none. Material caveat: the target is crate-private, so ordinary code text may be more suitable
than a qualified public-doc link. The original suspected consequence is preserved. Lead l2 remains resolved to a
finding.

Returned proposal (claims-068, event 7): **retain-verified**, suggested bucket **other**.

The public test-seam documentation retains a bare intra-doc link after the timing constant's import was removed. The
source-level unresolved name is established; generated-page behavior and build diagnostics were not observed.

Original stronger consequence: A broken link in generated developer documentation; earlier investigation did not assert
a runtime failure. Keep the private-target and no-rustdoc-execution caveats.

Unresolved premise or limitation: The exact rustdoc warning or rendered link appearance remains unobserved. No runtime
consequence or ordinary-build failure is established.

Proposed basis: Retain source-confirmed unresolved documentation reference as other, without upgrading it to an observed
tool failure.

Existing parent disposition (event 7): verified. Parent explicitly accepted named retain-verified proposal from
claims-068 after inspecting it. The public test-seam documentation retains a bare intra-doc link after the timing
constant's import was removed. The source-level unresolved name is established; generated-page behavior and build
diagnostics were not observed. Retain source-confirmed unresolved documentation reference as other, without upgrading it
to an observed tool failure. Checked source/results: ["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:core.rs:78-82 no longer
imports TAB_LAUNCH_SETTLE; its sole remaining occurrence is the bare link at 748. TabSettleGate remains public at 757
and publicly re-exported by service/mod.rs:133-140.","Same revision service/terminals.rs:47 defines the crate-private
constant. A child module's own import cannot put it in core's lexical scope. Qualifying the path or using plain code
text would address the unresolved reference, with private-target visibility considered separately.","Pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:core.rs:662 retains the sole bare reference and 671 the public alias. No
rustdoc, build, CI-failure or warnings-as-errors observation is claimed."] Current coverage/results: ["SPEC.md and
SPEC_impl.md searched for tab settle/test-seam/documentation references; no explicit acceptance of this unresolved
link.","TODO.md Planned empty; BUGS.md unrelated; plans/queue searched for exact gate/constant/link. tmux-drop-from-docs
discusses comment and dangling-reference audits but already landed per REPORTS.md:32; no unlanded exact fix
coverage.","INDEX.md, queue text and active TRIAGE_OUTCOMES.md searched for TabSettleGate/TAB_LAUNCH_SETTLE/rustdoc
link. No matching open item or decision. Archive not read.","FILTER.md does not cover this persistent
developer-documentation reference defect merely because its impact is small. No rare runtime trigger is required."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "A broken link in generated
developer documentation; earlier investigation did not assert a runtime failure. Keep the private-target and
no-rustdoc-execution caveats."; "The exact rustdoc warning or rendered link appearance remains unobserved. No runtime
consequence or ordinary-build failure is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g105 record](records/review-through-00de0b6c/groups/g105.json). Record ID: `l2`.

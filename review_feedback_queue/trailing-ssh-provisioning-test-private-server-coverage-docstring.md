# SSH integration test still claims private-tmux teardown coverage

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: SSH integration test still claims private-tmux teardown coverage

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g047/c2`.

### g047/c2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 5 (finding, open):

# SSH test promises private-server coverage it no longer supplies

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

A maintainer reading the SSH provisioning test is told it verifies uninstall ends the private tmux server, but the
changed fixture no longer starts or observes that server. This overstates what a passing test proves.

## Details

The retained test docstring at provisioning.rs:13481-13484 says UNINSTALL ends its terminal hosts 'and the private
server'. The assigned change removes debug_tmux, the tmux payload, the tmux socket, and wait_private_tmux_gone from
real_provisioning_case (assigned patch 01267-001.patch:1046-1118). The fixture creates terminal-host sessions and checks
their own sockets instead. Its terminal-host coverage does not establish private-tmux server absence. SPEC.md and
SPEC_impl.md, Provisioning, distinguish terminal-host termination from the accepted residual legacy server that outlived
a previously stopped supervisor. Update the test documentation to describe the resources and assertions it actually
exercises.

Suggested bucket: other (incorrect maintainer test documentation). Confidence: confirmed from source, not execution.
proposed_drop: false; the prior proposal based on lacking product effect or the charter's emphasis is retracted.
possible_cover: none; parent owns queue filters and non-spec coverage. Could-be-highest: no established security or
loss-of-work consequence. Caveat: this is a documentation correctness finding, not evidence that product uninstall leaks
a currently owned terminal host or private server. Revision promotes c2 from candidate and preserves its original
stale-prose consequence.

Returned proposal (claims-030, event 5): **retain-verified**, suggested bucket **other**.

The retained SSH test documentation promises private-server teardown coverage although its changed fixture creates and
observes terminal hosts only. Correct supported product teardown does not repair this documentation defect. The earlier
proposed drop was expressly retracted.

Original stronger consequence: Stale test prose tells maintainers a pass proves private-tmux teardown. First record
proposed dropping it for lack of product effect; line 5 retracts that proposal and retains the same documentation
consequence. No stronger security or work-loss route was claimed.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record as other: source-confirmed overstatement of test coverage, with no demonstrated product failure.

Existing parent disposition (event 5): verified. Parent explicitly accepted named retain-verified proposal from
claims-030 after inspecting it. The retained SSH test documentation promises private-server teardown coverage although
its changed fixture creates and observes terminal hosts only. Correct supported product teardown does not repair this
documentation defect. The earlier proposed drop was expressly retracted. Record as other: source-confirmed overstatement
of test coverage, with no demonstrated product failure. Checked source/results:
["500548b9253b07a26fd03119679184b69544e31d:crates/farhelm-helm/src/provisioning.rs:13481-13483 says UNINSTALL ends
terminal hosts 'and the private server'.","Same revision provisioning.rs:13194-13208,13316-13332,13438-13446 creates
terminal-host sessions and checks terminal-host listeners; the deleted private-tmux fixture/test at parent revision
:13270-13446 no longer supplies that observation.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:provisioning.rs:13481-13483
still has the claim; recording-time main also retains it. The change makes an existing description stale within the
review range. No test run or product teardown failure is alleged."] Current coverage/results: {"SPEC.md and
SPEC_impl.md":"Topology and Provisioning distinguish mandatory terminal-host teardown from accepted legacy orphan-tmux
survival; neither accepts inaccurate test documentation.","TODO.md Planned":"Empty, lines
25-27.","plans/queue":"tmux-drop-from-docs criterion 5 promised a comment audit, but plans/REPORTS.md:32 records that
plan already landed; it cannot cover this surviving omission as unlanded work.","BUGS.md":"Only the distinct
provisioning pipe-holder hang; no match.","INDEX.md and matching queue files":"Full index plus searches for the exact
test, real_provisioning_case, private server and tmux found no same-root item. Other test-documentation records concern
different tests and promises.","TRIAGE_OUTCOMES.md":"No active decision for this SSH-test documentation consequence;
archive not read.","FILTER.md":"No whole boundary applies: this is persistent test documentation, not a rare transient
diagnostic."} Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Stale test
prose tells maintainers a pass proves private-tmux teardown. First record proposed dropping it for lack of product
effect; line 5 retracts that proposal and retains the same documentation consequence. No stronger security or work-loss
route was claimed."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g047 record](records/review-through-00de0b6c/groups/g047.json). Record ID: `c2`.

# UI phase-wide regression guards omit the new host phases

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: host UI tests claim exhaustive phase coverage and a guard against a new phase borrowing
another's label, but omit Stopped and later Deleting. New-state mapping regressions can pass that claimed guard. Source:
every_phase:6995-7050 promises Every phase with unique sentinels, yet never constructs Stopped; the assigned 350a4b33
patch updates its TensorlakeChanging.starting field while adding Stopped production matches but not a Stopped fixture.
every_phase_keeps_the_helms_wire_token:7170-7201 says 'Exhaustive over the whole taxonomy rather than a sample' and
calls a new phase borrowing another's word 'invisible unless every phase is listed'. It derives labels from that
incomplete helper, compares only the old 11 tokens and tests uniqueness only among them. Changing phase_label's new
Stopped literal to an old token would leave this guard satisfied; no execution/mutation was performed. Rail
classification at 7076-7099, outage cause/remedy at 8023-8053, adoption, connection and update-category tests reuse the
same helper, often zipping an old 11-case expected table. Later 222bf625 adds Deleting production display/cause/label
branches without adding that variant to the helper or tables. Current production labels are correct; that does not
refute this independent assurance/documentation defect. Checks: all Stopped and Deleting references in this UI file were
inspected by scoped source search and occur only in production branches, not test fixtures. The adjacent attention-list
tests contain no Stopped construction at 350a4b33 either (a reason string 'stopped' belongs to Retired), bu

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g031/f8`.

### g031/f8

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 24 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462; omission widened by assigned
222bf6253bcd168db04bcbb1df20180c88306998 and persists through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: host UI
tests claim exhaustive phase coverage and a guard against a new phase borrowing another's label, but omit Stopped and
later Deleting. New-state mapping regressions can pass that claimed guard. Source: every_phase:6995-7050 promises Every
phase with unique sentinels, yet never constructs Stopped; the assigned 350a4b33 patch updates its
TensorlakeChanging.starting field while adding Stopped production matches but not a Stopped fixture.
every_phase_keeps_the_helms_wire_token:7170-7201 says 'Exhaustive over the whole taxonomy rather than a sample' and
calls a new phase borrowing another's word 'invisible unless every phase is listed'. It derives labels from that
incomplete helper, compares only the old 11 tokens and tests uniqueness only among them. Changing phase_label's new
Stopped literal to an old token would leave this guard satisfied; no execution/mutation was performed. Rail
classification at 7076-7099, outage cause/remedy at 8023-8053, adoption, connection and update-category tests reuse the
same helper, often zipping an old 11-case expected table. Later 222bf625 adds Deleting production display/cause/label
branches without adding that variant to the helper or tables. Current production labels are correct; that does not
refute this independent assurance/documentation defect. Checks: all Stopped and Deleting references in this UI file were
inspected by scoped source search and occur only in production branches, not test fixtures. The adjacent attention-list
tests contain no Stopped construction at 350a4b33 either (a reason string 'stopped' belongs to Retired), but no separate
demand for attention-list coverage is made here. At pinned tip every_phase remains the old list at 7091-7150 and its
exhaustive-token test remains at 7263-7298; there are still no test constructions of either new variant in hosts.rs.
Whole-range attribution: the exclusive base already had the exhaustive promise and fixture but its HostPhase enum had
neither Stopped nor Deleting; the new range variants widen that promised test domain. No later in-range matrix repair
found. Contract: supplied test-authoring review requires an observable distinguishing competing mechanisms; the test
explicitly names new-variant label collision as its purpose. Root test docstring rules require the stated
specification/why to match the actual fixture. Fix: include new states and meaningful substates in the shared taxonomy
fixture and the corresponding explicit expectations, preserving independent display/evidence oracles. Suggested bucket
other (maintainer regression assurance and inaccurate exhaustive claim). Confidence confirmed source/test/contract
mismatch, source-only. proposed_drop false. possible_cover related neighbour g024/l2 concerns backend HostStateView
phase/serializer vocabulary in another crate; it cannot establish UI mappings/rail/outage behavior, so preserve distinct
source and owner even if one remediation updates both. Could-be-highest false; no runtime security or work-loss
consequence asserted. All readily checkable premises settled; no open lead under f8. Original omitted-new-phase
consequence retained; the separately enumerated every_phase_detail expectation table was already incomplete for older
variants and is not separately attributed as a new defect here.

Returned proposal (claims-018, event 24): **retain-verified**, suggested bucket **other**.

The test advertises exhaustive protection against new phase-label collisions but its fixture omits the two new variants.
Correct current production labels do not supply that missing regression oracle.

Original stronger consequence: New-state UI mapping regressions can bypass tests described as exhaustive. No present
runtime label, security or user-work-loss consequence was claimed. Older incompleteness in the separate detail
expectation table is not newly attributed here.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain UI fixture/table coverage as its own root. The supplied history's backend serializer neighbour
does not establish an alias or coverage of UI rail/outage mappings.

Existing parent disposition (event 24): verified. Parent explicitly accepted named retain-verified proposal from
claims-018 after inspecting it. The test advertises exhaustive protection against new phase-label collisions but its
fixture omits the two new variants. Correct current production labels do not supply that missing regression oracle.
Retain UI fixture/table coverage as its own root. The supplied history's backend serializer neighbour does not establish
an alias or coverage of UI rail/outage mappings. Checked source/results: ["350a4b33b6d4476f4a78e43cea124bc2b828f462:UI
hosts.rs:6995-7050 every_phase contains the old 11 cases and no Stopped; :7170-7201 explicitly claims exhaustive
taxonomy coverage, computes labels from that fixture and compares only those 11 tokens.","350a4b33:hosts.rs:7076-7099
zips the same fixture with an old rail table. Exclusive-base-to-350a4b33 diff updates TensorlakeChanging.starting in the
fixture without adding Stopped.","Tip 00de0b6c hosts.rs source search finds Stopped and Deleting only before tests;
every_phase remains at :7091 and the exhaustive-token guard at :7271. Both new production phase labels exist at
:375-377. Source only; no mutation test or runtime test was run."] Current coverage/results: ["Both specs require the
new phase distinctions; no acceptance of incomplete phase-oracle claims was found. This is a test
assurance/source-docstring defect, not a current runtime label defect.","TODO.md Planned and plans/queue/INDEX.md are
empty. BUGS.md is unrelated. INDEX.md, queue content and active TRIAGE_OUTCOMES.md searches for every_phase, exhaustive
host taxonomy, Stopped/Deleting fixture omissions found no exact item.","FILTER.md has no boundary for this ordinary
test-oracle mismatch."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved:
"New-state UI mapping regressions can bypass tests described as exhaustive. No present runtime label, security or
user-work-loss consequence was claimed. Older incompleteness in the separate detail expectation table is not newly
attributed here."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g031 record](records/review-through-00de0b6c/groups/g031.json). Record ID: `f8`.

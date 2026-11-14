# shell harness docs still describe the deleted conformance battery

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the retained test helper's explanation directs maintainers to a conformance battery that
the same change deletes, hiding its actual remaining purpose: selecting bash for the tab-close/SIGTERM test.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g155/c6`.

### g155/c6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; checked through pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the retained test helper's explanation directs maintainers to a
conformance battery that the same change deletes, hiding its actual remaining purpose: selecting bash for the
tab-close/SIGTERM test. Details: terminal_tabs.rs:28-32 says 'Used to give the agent terminal a plain shell so the
conformance tests can drive BOTH terminals identically' and follows with the former dead-at-open rationale. The assigned
patch removes terminal_conformance_holds_for_the_agent_and_for_a_tab and its harness_with_shell_tmux wrapper. A pinned
call search after the change finds harness_with_shell only at its definition (:33) and
closing_a_tab_returns_well_inside_the_sigterm_grace (:490); that caller pins /bin/bash and checks its SIGTERM-ignore
premise (:483-527), not agent/tab conformance. Whole-range attribution: at excluded base
6e423e30068b62c6cfa276c6d720ee35bb70387a, the same prose is present and the described conformance battery still exists
at :1937 (using the tmux sibling wrapper), so merely observing that the prose predates 647f0227 would miss the new
dangling explanation caused by this in-range deletion. At tip, the unchanged paragraph remains at :27-31, the sole
helper caller remains the close-latency test at :489, and targeted searches find neither deleted conformance nor old
dead-at-open function names. The host_tabs dead-open replacement still has its own seams; that does not restore the
deleted conformance battery referred to by this paragraph. Update the helper explanation to its surviving
shell-selection/close-latency responsibility, retaining useful seam context without claiming the removed caller exists.
Suggested bucket: other, test documentation correctness. Confidence: confirmed by base/parent/reviewed/tip source and
exact caller searches; no execution. proposed_drop: false. possible_cover: none in the supplied neighbour records.
Could-be-highest: no established security or loss-of-work route. This is a bounded documentation defect, distinct from
c5's lost request oracle and from any production correctness claim. No parent drop accepted. All readily
source-checkable attribution and persistence premises resolved; c6 is no longer an open lead.

Returned proposal (claims-099, event 15): **retain-verified**, suggested bucket **other**.

The deletion makes the retained shell helper's explanation dangle, and it remains wrong at tip. This is test
documentation, independently of runtime correctness. c8 is related stale prose on a different helper and could share a
correction, but is not exact coverage.

Original stronger consequence: The initial lead also questioned dead-at-open caller prose. The host dead-open
replacement has its own seams, but it does not restore the deleted conformance battery. Established impact is misleading
maintainer guidance, not product failure or work loss.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Record persistent in-range dangling helper rationale, with its sole surviving bash/Close caller.

Existing parent disposition (event 15): verified. Parent explicitly accepted named retain-verified proposal from
claims-099 after inspecting it. The deletion makes the retained shell helper's explanation dangle, and it remains wrong
at tip. This is test documentation, independently of runtime correctness. c8 is related stale prose on a different
helper and could share a correction, but is not exact coverage. Record persistent in-range dangling helper rationale,
with its sole surviving bash/Close caller. Checked source/results:
["647f0227:crates/farhelm/tests/e2e/terminal_tabs.rs:25-39 describes conformance callers; after deletion the actual call
is the /bin/bash Close/SIGTERM test at :490.","6e423e30:terminal_tabs.rs:28-58 has the same prose and its tmux sibling;
grep locates terminal_conformance_holds_for_the_agent_and_for_a_tab at :1937 using that sibling at :1938. Thus the
wording is old but its described battery disappears in range.","00de0b6c:terminal_tabs.rs:24-39 retains the conformance
rationale, while grep finds only its definition at :32 and the bash caller at :489. The close test's rationale at
:469-489 is SIGTERM/hangup latency, not agent/tab conformance."] Current coverage/results: {"SPEC.md /
SPEC_impl.md":"Session view, Terminal host/test-footing and Testing sections supply launch/test context but no
acceptance of inaccurate helper documentation.","TODO.md Planned":"Empty.","plans/queue":"No unlanded index entries. The
retained tmux-drop-from-docs comment/final-audit criteria are historical context, not an unlanded fix; the prose remains
at tip.","BUGS.md":"No match.","INDEX.md / matching queue files":"No shell-helper/conformance-caller documentation item.
Read conformance-test-never-admits-management-candidates.md: it concerns saturated management permits, not this deleted
battery.","TRIAGE_OUTCOMES.md":"The matching conformance heading at :1351 records discard of the management-permit test
issue; scope does not match. No helper-doc decision found.","FILTER.md":"Stale developer explanation has no qualifying
rare/transient diagnostic trigger."} Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "The initial lead also questioned dead-at-open caller prose. The host dead-open replacement has its own
seams, but it does not restore the deleted conformance battery. Established impact is misleading maintainer guidance,
not product failure or work loss."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g155 record](records/review-through-00de0b6c/groups/g155.json). Record ID: `c6`.

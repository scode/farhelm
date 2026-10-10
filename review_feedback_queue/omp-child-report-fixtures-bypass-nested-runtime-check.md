# OMP child-report fixtures bypass the nested-runtime check

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The child-report test rejects its fixture before checking nested-runtime ownership.

## Details

F157 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:20822` — OMP child-report fixtures bypass the
nested-runtime check

The competing reporter is a sibling process tree rather than a descendant anchored to the pane. It fails the earlier
pane-anchor check, so breaking the later nested-runtime rejection still leaves the integration test green. Have the pane
runtime spawn the competitor, establish that ancestry reaches the pane, and verify rejection at the runtime-ownership
decision.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:20822–20906: spawn_sibling starts a separate direct child while
  preserving the first runtime's pane anchor.
- crates/farhelm-supervisor/src/service/core.rs:21673–21700,21738–21770,21802–21818: three child-report scenarios use
  that sibling fixture.
- crates/farhelm-supervisor/src/service/report_files.rs:311–325 and crates/farhelm-supervisor/src/procs.rs:478–493:
  missing pane ancestry refuses before report_conversation.
- crates/farhelm-supervisor/src/service/core/vendor/omp.rs:194–225: the nested-runtime corridor proof occurs later.
- crates/farhelm-supervisor/src/procs.rs:3007–3019: a separate unit test directly covers nested-runtime corridor
  refusal.
- crates/farhelm-supervisor/src/service/core.rs:20756 makes the first runtime the pane; line 20822 creates a second
  separately launched chain while leaving that pane unchanged.
- crates/farhelm-supervisor/src/service/core.rs:20876 spawns each runtime directly from the test process; line 21051
  sends its report through admit_hook_report.
- crates/farhelm-supervisor/src/service/report_files.rs:311 requires ancestry anchored at the pane before calling
  report_conversation.
- crates/farhelm-supervisor/src/service/core/vendor/omp.rs:208 performs OMP corridor classification only after that
  earlier anchor.
- crates/farhelm-supervisor/src/service/core.rs:21673, :21738 and :21802 use the sibling fixture in the three claimed
  parent/child tests.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/omp-bun-pane-proof.md:20–40 and TRIAGE_OUTCOMES.md:7107–7130 concern the production
  unreadable-Bun-pane case, not this sibling fixture. TRIAGE_OUTCOMES.md:5870–5889 concerns the earlier production
  safeguard.
- The open omp-bun-pane-proof item concerns production classification of a particular anchored Bun case; it does not
  repair these integration-test premises. The non-first-class-harness filter does not clearly cover this test-oracle
  defect.

Caveats:

- The existing fixtures do test refusal of unrelated sibling ancestry.
- No additional production ownership vulnerability is established.
- The fixtures correctly cover unrelated sibling-ancestry refusal.
- The nested-runtime helper itself has unit coverage.
- No new production ownership vulnerability is established.
- The three tests share the single editable spawn_sibling fixture region.
- Does not establish that production currently accepts ordinary nested reports.
- The three tests share this faulty fixture; they remain one input finding.
- Does not establish a general production nested-report acceptance bug.
- The three tests share one faulty fixture root; retain all three affected call sites, but no separate finding is
  required per caller.
- No mutation or runtime test.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_03:p1:F4`,
`gap_supervisor_state_sec_04:p1:F1`.

- `gap_supervisor_state_cor_03:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_04:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

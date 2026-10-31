# Alias propagation regression reloads before its live check

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The alias regression reloads the page before checking the row name, so it misses the promised live update behavior.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Reloading bypasses the stale mounted-row state the test should distinguish. The current production compared-row signal
is correct; the older stale-memo rationale is inaccurate context, not evidence of a current production omission.

Source locations: `e2e/tests/sidebar.spec.ts:8234-8246,8279-8282`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit 08af8c0173c559ad95675c4160c6371f93c7bc1e; unchanged test text and behavior checked at inclusive tip
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: the alias regression can pass without proving that a host-name change
reaches an existing session row in the open app. Historical and pinned sidebar.spec.ts:8234-8246 say the session-row
label rides a separate /api/sessions signal and then reload before checking Build Box; 8279-8282 reload again before
checking the cleared alias's derived short name. That rationale is obsolete: historical SPEC_impl.md:323-328 and pinned
SPEC_impl.md:324-329 require the existing host read's Host::short_name() to enter compared RowState so alias/destination
changes redraw memoized rows; current list/view.rs:4749-4750 and list/shared.rs:112,169-172 implement it. The producer
field check found host_views uses one manager snapshot for destination, alias and full name (helm hosts.rs:422-435), and
double_option preserves omitted-vs-null aliases in UI lib.rs:697-747. The naming test limitation is therefore not a
missing input field or known production failure. Actual checks: read the full remote alias test
(sidebar.spec.ts:8060-8283), current naming test file in the initial pass, provisional-host test at 3497-3561 (which
observes unknown-to-known naming, not an existing known host's Some-name-to-Some-name update), and registry-change test
filters.spec.ts:116-205 (which checks rail labels/selection, not changed session-row names). The alias test's shared
mocked alias variable updates both host and session replies; its reload discards existing row component state. It does
not distinguish correct live projection from an implementation that leaves an already-known row's name stale until
remount. This is a confirmed source-level regression-evidence gap and stale test rationale, not a run mutant or observed
browser failure. Suggested correction: establish an existing session row before the mutation, hold its session payload
unchanged while the host alias/short name changes, and observe the row update before reload; preserve the test's
legitimate distinction between client rendering and real backend alias derivation. Suggested bucket: other. Could be
highest: no established security or user-work-loss consequence; this finding concerns test evidence only. Confidence:
confirmed source limitation; runtime behavior not measured. proposed_drop: none. possible_cover: none found after
searches of SPEC.md, SPEC_impl.md, TODO.md (Planned empty), BUGS.md, plans/queue, review_feedback_queue/INDEX.md and
queue files, and TRIAGE_OUTCOMES.md for alias propagation, reload, memoization, host short names and this test. The
plan's static naming assertions do not explicitly accept the obsolete signal rationale or this uncovered live contract;
operational-artifact dispositions do not cover executable product regression tests. No claim that all tests elsewhere
were exhaustively audited.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g106/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

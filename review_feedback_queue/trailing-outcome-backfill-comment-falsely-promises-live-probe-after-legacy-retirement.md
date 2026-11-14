# Outcome-backfill comment promises live reconciliation that legacy rows no longer receive

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The migration explanation tells maintainers that old sessions get checked against their
live terminals, but this release deliberately skips that check and classifies the old rows through terminal retirement.
This is inaccurate migration documentation, with no demonstrated runtime defect.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g150/l1`.

### g150/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: The migration explanation tells maintainers that old
sessions get checked against their live terminals, but this release deliberately skips that check and classifies the old
rows through terminal retirement. This is inaccurate migration documentation, with no demonstrated runtime defect.
Details: The assigned rewrite at store.rs:1832-1834 says: 'the very first reload after this migration reconciles each
row against its live terminal anyway.' This is the version-1 outcome backfill (1824-1853), where every existing row gets
outcome_state=launching. The same ordered migration ladder later adds terminal_kind with DEFAULT tmux at :2493-2508, so
those rows are old-kind rows, not host rows eligible for live adoption. Current reload at service/core.rs:6675-6679
unconditionally maps Tmux to None and explicitly never looks up its address; :6805-6822 sends no-sentinel live-outcome
old rows through Transition::TerminalEnded and continues before the ordinary terminal-evidence reconciliation. Thus the
asserted live-terminal reconciliation is not the reason those legacy outcomes can be safely defaulted in this release.
SPEC_impl.md:2903-2909 accurately says the reload takes every old-kind row as having no terminal, without looking its
recorded pane id up, and converts still-live outcomes unless the sentinel establishes Error. Production behavior
satisfies that intended retirement contract; that does not make this distinct explanatory promise accurate. Fix
direction: retain the conservative historical launching default while explaining that current reload applies
old-terminal retirement, rather than claiming a live probe is still performed. Range check: the base revision
6e423e30068b62c6cfa276c6d720ee35bb70387a:store.rs:1795-1797 had the historical live-tmux-probe explanation. The assigned
commit expressly rewrites that line to the current live-terminal claim. The complete store.rs history within the pinned
range was inspected, and the full diff from the reviewed revision to tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
changes only two unrelated comments, so this statement is not later repaired. The earlier in-range retirement commit
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 already supplies the runtime change; direct attribution checks follow before
delivery. Suggested bucket: other (internal documentation correctness). Confidence: confirmed source/contract mismatch,
no execution. proposed_drop: false; retirement acceptance describes intended behavior, not acceptance of a misleading
migration explanation. could_be_highest: false; no security or user-work-loss route asserted from this comment.
possible_cover: no matching record in supplied neighbour findings; prohibited coverage sources not searched. Caveats:
the schema/backfill code itself is unchanged by the assigned patch, and no change to the migration default or retirement
policy is requested. This revision closes l1's migration-kind premise and retains the original consequence of a
misleading migration invariant/rationale. Attribution checks completed: at exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a, core.rs:7182-7225 really resolves Tmux rows through agent_pane_from_states or
pane_evidence from the live pane snapshot, matching the old comment. The dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 patch
directly deletes those lookup branches and adds TerminalKind::Tmux => None; its source :6433-6438 ends the old server
and disables the driver before reload and :6965-6969 skips old lookup. Therefore the explanatory mismatch arises inside
the reviewed range with retirement, then the assigned 1ed482dd hunk rewrites the same rationale without correcting it.
It is not an excluded pre-range defect and is not newly caused solely by the 1ed482dd runtime deletion. Exact
specification coordinates corrected/confirmed: SPEC_impl.md:2904-2908 contains the old-kind no-lookup and conversion
rule. All readily checkable premises for this documentation finding are now settled; no open lead remains. The complete
post-reviewed-commit store diff confirms it survives the pinned tip.

Returned proposal (claims-096, event 8): **retain-verified**, suggested bucket **other**.

The migration rationale promises live-terminal reconciliation for rows that the current reload deliberately never
probes. Correct retirement behavior does not make that source comment accurate.

Original stronger consequence: Misleading migration invariant/rationale for maintainers; no runtime defect,
migration-default change, or security/data-loss route was claimed.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as other source-documentation correctness. Explain conservative historical backfill plus current
terminal retirement; do not change the default or retirement behavior.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from
claims-096 after inspecting it. The migration rationale promises live-terminal reconciliation for rows that the current
reload deliberately never probes. Correct retirement behavior does not make that source comment accurate. Retain as
other source-documentation correctness. Explain conservative historical backfill plus current terminal retirement; do
not change the default or retirement behavior. Checked source/results:
["1ed482dde220452e424973faf38a5e1318986333:crates/farhelm-supervisor/src/store.rs:1824-1853 and tip same lines backfill
all v1 outcomes to launching and claim the first reload reconciles against each live
terminal.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:store.rs:2493-2508 gives all pre-32 rows terminal_kind DEFAULT
tmux, so this backfill's rows are legacy-kind rows.","1ed482dde220452e424973faf38a5e1318986333:service/core.rs:6675-6679
maps Tmux to None without lookup; 6805-6822 applies TerminalEnded and continues before host reconciliation.","Exclusive
base 6e423e30068b62c6cfa276c6d720ee35bb70387a:store.rs:1795-1797 described live tmux probes and
service/core.rs:7182-7225 actually performed them. In-range retirement dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 removed
those lookups; the assigned 1ed482dd rewrite preserves the misleading claim. Its later tip diff changes only two
unrelated comments."] Current coverage/results: [{"source":"SPEC.md and SPEC_impl.md","result":"SPEC_impl.md:2798-2804
accurately specifies legacy no-lookup conversion; SPEC.md:2998-3011 specifies retirement. Neither accepts false
migration rationale. tmux-drop-from-docs plan's broad comment sweep is already landed per plans/REPORTS.md:32, so it is
not unlanded coverage."},{"source":"TODO.md:25-26","result":"Planned bucket is empty. The phase-3 removal idea at
225-232 is in Mid term, not Planned, and does not cover these findings."},{"source":"plans/queue","result":"Searched
named behavior and symbols. No matching unlanded corrective plan. Relevant tmux-end-old-terminals, tmux-drop-from-docs
and environment-agent-operations plans are already landed per plans/REPORTS.md:28-32, despite retained queue
files."},{"source":"BUGS.md","result":"Read fully; sole accepted ssh-output-pipe hang is
unrelated."},{"source":"review_feedback_queue/INDEX.md and matching files","result":"Searched actual mechanisms, symbols
and consequences. No exact existing item or useful same-root extension found.
tab-reap-deadline-excludes-listing-request.md was read; its test polling helper is distinct from production startup
cleanup."},{"source":"TRIAGE_OUTCOMES.md","result":"Searched actual mechanisms and consequences; no matching active
decision. Archive not read."},{"source":"review_feedback_queue/FILTER.md","result":"Read fully. No whole-boundary filter
match used."}] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Misleading
migration invariant/rationale for maintainers; no runtime defect, migration-default change, or security/data-loss route
was claimed."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g150 record](records/review-through-00de0b6c/groups/g150.json). Record ID: `l1`.

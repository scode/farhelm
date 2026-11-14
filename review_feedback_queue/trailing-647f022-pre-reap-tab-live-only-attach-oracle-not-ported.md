# Dead-but-unreaped tab refusal gap remains applicable to host tabs at tip

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: deleting a_tab_whose_shell_exited_vanishes_from_listings_and_still_closes removes the
regression oracle that an exited tab is hidden and unattachable as NotFound before its terminal has actually been
reaped, while close can still destroy it. Confirmed test-only gap, not an observed product failure. The original concern
about tab cleanup remains narrowed to this interactive-versus-close boundary: host_tabs.rs:475-519 still covers
failed-open process cleanup, and :531-566 covers eventual exit-driven destruction, but its conjunctive
host/socket/record disappearance wait cannot establish pre-reap attach refusal. At 647f0227 terminals.rs:899-912 applies
the dead-tab gate to host tabs, :983-993 applies it to tmux tabs; handlers.rs:1780-1785 propagates that resolver's
refusal before attachment. The retained host_tabs.rs:1783-1833 unit test checks pre-reap listing/reap selection only,
never attach or the live-only resolver. Targeted searches over the pinned host-tab e2e/unit sources and tip
supervisor/Rust e2e/browser tests found no corresponding pre-reap tab attach assertion; direct resolver call searches
find only production wrappers/callers, no resolver test. Later source retirement does eliminate old tmux tabs, but tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains this contract for host tabs: complete SPEC_impl.md 'Tabs in hosts'
bullet :2234-2269 says a shell-exited tab 'is hidden at once'; terminals.rs:105-149 and :223-248 keeps LiveOnly versus
DeadIncluded and exact NotFound behavior. Tip handlers.rs:1667-1679 routes Attach through LiveOnly before takeover;
core.rs:13985-

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g151/c3`.

### g151/c3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: deleting
a_tab_whose_shell_exited_vanishes_from_listings_and_still_closes removes the regression oracle that an exited tab is
hidden and unattachable as NotFound before its terminal has actually been reaped, while close can still destroy it.
Confirmed test-only gap, not an observed product failure. The original concern about tab cleanup remains narrowed to
this interactive-versus-close boundary: host_tabs.rs:475-519 still covers failed-open process cleanup, and :531-566
covers eventual exit-driven destruction, but its conjunctive host/socket/record disappearance wait cannot establish
pre-reap attach refusal. At 647f0227 terminals.rs:899-912 applies the dead-tab gate to host tabs, :983-993 applies it to
tmux tabs; handlers.rs:1780-1785 propagates that resolver's refusal before attachment. The retained
host_tabs.rs:1783-1833 unit test checks pre-reap listing/reap selection only, never attach or the live-only resolver.
Targeted searches over the pinned host-tab e2e/unit sources and tip supervisor/Rust e2e/browser tests found no
corresponding pre-reap tab attach assertion; direct resolver call searches find only production wrappers/callers, no
resolver test. Later source retirement does eliminate old tmux tabs, but tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db
retains this contract for host tabs: complete SPEC_impl.md 'Tabs in hosts' bullet :2234-2269 says a shell-exited tab 'is
hidden at once'; terminals.rs:105-149 and :223-248 keeps LiveOnly versus DeadIncluded and exact NotFound behavior. Tip
handlers.rs:1667-1679 routes Attach through LiveOnly before takeover; core.rs:13985-14011 routes user Close and reap
through DeadIncluded. These actual callers confirm the missing distinction survives retirement. Repair: port the deleted
test to a real host tab on an in-process supervisor without automatic reap; independently wait for processed shell exit,
assert the owned host/socket/record remain, check listing absence and exact attach refusal, then manually close and
verify host destruction. Suggested bucket: other (maintainer test coverage). Confidence: confirmed source analysis.
proposed_drop: false for this still-required oracle; prior broad drop withdrawn. Separately, BeforeMarking is obsolete
new-tmux-window creation, adequately distinguished from the missing boundary and not proposed as a fix to restore.
Could-be-highest: no established product security/data-loss route. possible_cover: none known for this specific oracle;
parent-reported g152/l1 overlaps g151/l1's old-session migration coverage, not this surviving host-tab contract.
Caveats: existing runtime source implements the intended behavior; no reproduction, test execution or observed failure
is claimed. Search absence is scoped to named pinned sources and call searches, not a proof about all possible
indirectly equivalent assertions.

Returned proposal (claims-097, event 10): **retain-verified**, suggested bucket **other**.

The deletion removes a concrete pre-reap refusal/close oracle. Eventual destruction and dead-tab listing tests do not
protect the interactive-versus-close boundary, which remains in host terminals. This is test coverage, not evidence of
incorrect current attachment behavior.

Original stronger consequence: The first candidate combined failed-open process cleanup and dead-tab handling.
Failed-open cleanup remains directly tested, and BeforeMarking belongs to removed tmux-window creation. The initial
broad drop was withdrawn for pre-reap refusal; preserve that withdrawal and the narrower surviving oracle gap. No actual
leftover-process or privacy defect was established.

Unresolved premise or limitation: No missing premise for the narrow missing-oracle finding. Attribution must remain a
lost tmux test whose backend-independent contract was not ported, rather than a formerly protected host-code branch.

Proposed basis: Retain the surviving pre-reap behavioral-oracle gap as other priority. Keep the obsolete BeforeMarking
portion separate and do not restore it.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from
claims-097 after inspecting it. The deletion removes a concrete pre-reap refusal/close oracle. Eventual destruction and
dead-tab listing tests do not protect the interactive-versus-close boundary, which remains in host terminals. This is
test coverage, not evidence of incorrect current attachment behavior. Retain the surviving pre-reap behavioral-oracle
gap as other priority. Keep the obsolete BeforeMarking portion separate and do not restore it. Checked source/results:
["647f022 parent tab_lifecycle_edges.rs:173-250 establishes shell exit, hidden listing, exact NotFound attachment
refusal, and continued physical pane existence before manual close. The deleted fixture is tmux-backed.","At 647f022,
host_tabs.rs:475-519 checks failed-open cleanup; :531-566 waits for listing absence AND host/socket/record destruction.
core/host_tabs.rs:1783-1833 checks listing and reap selection without calling the live-only resolver. Neither
distinguishes pre-reap refusal.","At 647f022, terminals.rs:906-912 and :983-993 implement separate host and tmux
live-only gates. The deleted test directly exercised the tmux gate, not the host branch: the supported current claim is
an unported behavioral oracle, not that the old test would detect a host-branch mutation.","At tip 00de0b6,
terminals.rs:105-149,223-248 still separates LiveOnly from DeadIncluded; handlers.rs:1674-1679 uses the former for
Attach; core.rs:14003-14011 uses the latter for Close/reap. Targeted tip searches for dead/exited/unreaped attach
assertions and resolve_terminal callers found no equivalent tab test. Search absence is bounded evidence, not a proof
against every indirectly equivalent test."] Current coverage/results: ["Current SPEC_impl.md Tabs in hosts (:2234-2269)
says exited tabs are hidden at once and separately describes close/reap. This preserves the behavioral distinction; it
does not accept missing regression protection. SPEC.md terminal-tab rules were searched alongside it.","TODO Planned is
empty. Searches of plans/queue, BUGS.md, active ledger and queue INDEX/matching queue terms found no exact pre-reap
Attach/Close-oracle coverage. Zombie-agent and tab-reap-deadline items have different triggers and assertions.","The
landed tmux-end-old-terminals plan criterion 5 requires tests whose subject survives to run on terminal hosts; it is not
an unlanded fix. FILTER.md does not cover ordinary missing test sensitivity."] Exact cited cover: [] Original stronger
consequence and missing premise remain preserved: "The first candidate combined failed-open process cleanup and dead-tab
handling. Failed-open cleanup remains directly tested, and BeforeMarking belongs to removed tmux-window creation. The
initial broad drop was withdrawn for pre-reap refusal; preserve that withdrawal and the narrower surviving oracle gap.
No actual leftover-process or privacy defect was established."; "No missing premise for the narrow missing-oracle
finding. Attribution must remain a lost tmux test whose backend-independent contract was not ported, rather than a
formerly protected host-code branch.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g151 record](records/review-through-00de0b6c/groups/g151.json). Record ID: `c3`.

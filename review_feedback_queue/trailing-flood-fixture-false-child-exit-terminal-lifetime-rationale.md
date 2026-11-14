# Flood fixture docs incorrectly say program exit ends its terminal

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the rewritten fixture rationale teaches future test authors that a program must remain
alive to keep a host-backed terminal's history. Farhelm explicitly keeps exited programs' terminals and their contents,
so this is a false lifecycle explanation in a shared test fixture. This is documentation correctness, not a failing
flood test or a claim of current terminal-history loss.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g144/l6`.

### g144/l6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513; persists through inclusive tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: the rewritten fixture rationale teaches future test authors that a
program must remain alive to keep a host-backed terminal's history. Farhelm explicitly keeps exited programs' terminals
and their contents, so this is a false lifecycle explanation in a shared test fixture. This is documentation
correctness, not a failing flood test or a claim of current terminal-history loss. Details: the assigned rewrite at
fake_agent.rs:1899-1901 states 'Tests need a definite end to assert a tail against, and the process must not exit, which
would end the terminal whose history is under test.' Whole-range attribution checked: at exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a:1949-1952 the rationale referred specifically to tmux's dead-pane placeholder
changing content; the assigned 647f0227 patch moves the shared explanation onto flood_gated, removes that
substrate-specific explanation and newly asserts that exit ends the terminal. That newly generalized host-lifecycle
claim is what this finding reports; no independent defect in retired fixture code is alleged. Exact contradicting
reviewed contract, SPEC.md:2907-2909: 'A stopped or exited session's terminal stays viewable while its host is up, since
the terminal outlives the process. Viewable means what the terminal itself holds.' Actual source agrees:
terminal-host/src/host.rs:455-487 reaps the child, drains available output and journals the exit without setting host
shutdown; the event loop at347 continues processing its client. Supervisor hosts/terminal.rs:3811-3824 flushes open
synchronized output, publishes, and sets Liveness::Exited without clearing emulator history or ending the host,
explicitly allowing later descendant output. The grid flood caller launches the retained FloodGated variant
(grid-flood.spec.ts:20), checks final retained screen/history, and only deletes the session in finally; no extra
fixture-specific destruction occurs when flood_gated returns (fake_agent.rs:1930-1932). The current fixture actually
waits, so those current runtime assertions do not test the claimed contrary lifecycle and do not refute this
documentation defect. Persistence: pinned-tip fake_agent.rs:1824-1826 retains the same claim; full path history after
introduction contains only f560058d,303a1264,08399d09, and the tip check confirms none repairs it. Suggested fix: retain
the observable idle-wait contract but remove the incorrect claim that a host terminal ends or loses its history when the
program exits; document an actual test-specific reason only if known. Suggested bucket: other (misleading
fixture/lifecycle documentation). Confidence: confirmed source/contract contradiction; no runtime behavior observed,
tests/builds/reproductions not run. proposed_drop: false. possible_cover: supplied neighbour records have no equivalent
flood-exit rationale finding; prohibited coverage sources not searched. Could-be-highest: no established or suspected
security, user-work-loss or catastrophic tooling route from this explanation. Material caveats: this does not request
removing the fixture's idle wait, does not certify all final-screen changes on exit, and does not revive the old tmux
explanation. All readily checkable premises resolved; no open lead remains for l6.

Returned proposal (claims-092, event 14): **retain-verified**, suggested bucket **other**.

The rewritten fixture comment falsely makes keeping terminal history depend on its program remaining alive. The
specified and implemented terminal lifetime is longer than the program's. Current passing flood assertions would not
refute this documentation defect.

Original stronger consequence: Future test authors could infer that fixture exit destroys retained terminal history. The
record never alleged current history loss or a failing flood test.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain an other-priority source-comment finding. Remove the false lifecycle reason; retain the wait
unless an actual test-specific reason for changing it is established. Do not revive the retired tmux explanation.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-092 after inspecting it. The rewritten fixture comment falsely makes keeping terminal history depend on its
program remaining alive. The specified and implemented terminal lifetime is longer than the program's. Current passing
flood assertions would not refute this documentation defect. Retain an other-priority source-comment finding. Remove the
false lifecycle reason; retain the wait unless an actual test-specific reason for changing it is established. Do not
revive the retired tmux explanation. Checked source/results:
["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:fake_agent.rs:1899-1901 says process exit would end the terminal whose
history is tested. Exclusive base :1949-1952 instead described a tmux-specific dead-pane placeholder, so the generalized
host-lifetime claim is introduced in range.","Same revision SPEC.md:2907-2911 explicitly says stopped/exited terminals
remain viewable because the terminal outlives the process. terminal-host/src/host.rs:454-486 records child exit and
finishes its drain without ending the host.","Same revision grid-flood.spec.ts:155-214 launches the host-backed fixture
and deletes its session only in finally. fake_agent.rs:1930-1932 returns normally after input; no fixture-specific
terminal destruction is present. Inclusive tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:fake_agent.rs:1824-1826 retains
the false rationale. No runtime test or reproduction."] Current coverage/results: ["Current SPEC.md:2888-2900 repeats
the contrary terminal-outlives-process contract; SPEC_impl.md Terminal host/Testing supplies no exception for this
fixture.","TODO.md Planned empty; BUGS.md and active ledger have no matching lifecycle-rationale claim. INDEX.md and
stopped-reader-output-witness.md/host-flood-stall-window-claim.md concern overlap and timeout evidence, not the false
child-exit explanation; no extension or exact alias found.","tmux-drop-from-docs.md:68-75,88-102 requires comment
rationales in current terms but has already landed (plans/REPORTS.md:32). No unlanded plan supplies exact coverage.
FILTER.md's diagnostic filter does not cover an ordinary false lifecycle explanation."] Exact cited cover: [] Original
stronger consequence and missing premise remain preserved: "Future test authors could infer that fixture exit destroys
retained terminal history. The record never alleged current history loss or a failing flood test."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g144 record](records/review-through-00de0b6c/groups/g144.json). Record ID: `l6`.

# Still-supported old-terminal recovery loses integration regression protection

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: existing old-terminal sessions are still supported at this revision, but their recovery from
input-client failure, connection loss, a killed sink and supervisor death loses its distinguishing integration checks.
This is a test-only coverage finding. No production failure was reproduced or inferred from execution. Revision of l1
after completed synthesis: the earlier proposed drop based on absence of a production defect is withdrawn; a test-only
defect does not require an already-demonstrated runtime failure. Intermediate source/contract: assigned
00802-001.patch:1-1000 removes input-client-failure/queued-output reattachment, connection-loss/queued-output
reattachment, held natural-detach/takeover arbitration, killed-sink healing while both terminals answer, concurrent
successful first-attach client counts, and real-supervisor-SIGKILL/stale-client cleanup scenarios. Patch lines 1001-1299
also retire their /proc diagnostic support and part of its test. SPEC_impl.md at 647f0227:2956-2962 says old tabs stay
in tmux until closed and explicitly says, 'This path changes only for bug fixes.' The removed tests therefore still have
a supported subject in that commit. The concrete lost assertions include replacement attach replying only after old
input/output clients are gone; both attached terminals still answering after a real sink is killed; exactly five control
clients for two attached terminals and one shared sink; and a fresh supervisor reaping two deliberately stale attached
clients with queued output while the underlying session remains attachable. This record asserts the lost

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g156/l1`.

### g156/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: existing old-terminal sessions are still supported at
this revision, but their recovery from input-client failure, connection loss, a killed sink and supervisor death loses
its distinguishing integration checks. This is a test-only coverage finding. No production failure was reproduced or
inferred from execution. Revision of l1 after completed synthesis: the earlier proposed drop based on absence of a
production defect is withdrawn; a test-only defect does not require an already-demonstrated runtime failure.

Intermediate source/contract: assigned 00802-001.patch:1-1000 removes input-client-failure/queued-output reattachment,
connection-loss/queued-output reattachment, held natural-detach/takeover arbitration, killed-sink healing while both
terminals answer, concurrent successful first-attach client counts, and real-supervisor-SIGKILL/stale-client cleanup
scenarios. Patch lines 1001-1299 also retire their /proc diagnostic support and part of its test. SPEC_impl.md at
647f0227:2956-2962 says old tabs stay in tmux until closed and explicitly says, 'This path changes only for bug fixes.'
The removed tests therefore still have a supported subject in that commit. The concrete lost assertions include
replacement attach replying only after old input/output clients are gone; both attached terminals still answering after
a real sink is killed; exactly five control clients for two attached terminals and one shared sink; and a fresh
supervisor reaping two deliberately stale attached clients with queued output while the underlying session remains
attachable. This record asserts the lost regression protection, not that the implementation violates those assertions.

Retained tests were checked rather than assumed equivalent. tmux/sink.rs:396-466 only establishes steady-state
filtered-pane readability with and without a sink. service/terminals.rs:4068-4110 checks endless retries through a
failing injected opener; 4172-4248 checks orderly process reaping and EOF-before-replacement with cat/sh stand-ins;
4690-4799 kills a cat-backed sink and checks retry timing, but subsequent opens fail. None proves successful real-tmux
sink replacement leaves both attached terminals working under load. terminals.rs:3696-3798 gates two missing-session
reservations and requires both opens to fail, so does not replace counting clients after two successful real
attachments. The earlier registry/barrier seam checks at 3430-3690 protect lower-level ownership states, not
queued-output transport failure through a live supervisor. tmux.rs:9377-9425 invokes reap_stale_control_clients after
finding no server, requiring zero and no command invocation; it cannot distinguish omitting the positive stale-client
reap. Symbol/caller searches found only that test call and the production startup call at core.rs:6447. Unchanged
production identity arbitration, acknowledged no-output shutdown, barriers and reaping are not tests for their caller
integration.

Later retirement is established, with its limits. The immediately following recorded commit
dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 changes the policy: SPEC_impl.md:2956-2967 says 'This release ends them at
startup' and 'The running supervisor runs no tmux at all.' Its core.rs:6426-6438 actually calls
OldTerminals::end_private_server before reload and then unconditionally tmux.no_server_after_startup();
core.rs:6965-6969 returns no terminal for every tmux-kind row. Thus the old-client/sink recovery subject is retired
there even when the server-ending step fails; the supervisor no longer serves it.
1ed482dde220452e424973faf38a5e1318986333 then removes the driver/sink implementation, and at pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db the searched sink/reap symbols are absent. This makes the old-terminal portion
historical by tip, not an ongoing missing test for supported old-terminal recovery. It does not refute the gap at
647f0227.

The shared natural-detach arbitration survives the retirement and receives replacement protection later:
f560058da19e02716e6f943883a9508b447aff4e introduces takeover_reason_wins_over_a_held_natural_detach in
core/tests/host_terminals.rs (identified by git log -S). At tip, lines 3939-4047 hold a host input failure behind a
Delete mark, take over, release arbitration, assert exactly one TakenOver notice, and verify winner output. It is absent
by that name at 647f0227 and 1ed482d. Do not treat generic arbitration as an ongoing uncovered contract at tip.

Suggested bucket: other, because the established harm is lost regression protection in maintainer tests.
Could-be-highest: no established catastrophic tooling, shipped security or user-work-loss consequence from this
deletion; the original failure scenarios could threaten user sessions, but this finding establishes test loss only.
Confidence: confirmed by source and contract analysis; no tests, builds or reproductions run. Proposed_drop: false.
Parent decides whether subsequent retirement warrants exclusion; no drop is accepted by this record. Possible_cover:
parent reports g152/l1 for migration integration protection lost at the same intermediate revision, and g151 for related
lower-layer/process coverage questions. These may share a retirement rationale, but this group's
input/connection/sink/crash recovery assertions are distinct from migration assertions; no coverage metadata or queued
reports were read. Suggested remedy at the intermediate revision: retain or rehost the distinguishing
real-tmux/supervisor regression cases until the support-removal commit, or keep that historical gap explicit in the
review disposition. No unresolved source premise remains for this bounded coverage conclusion.

Returned proposal (claims-099, event 8): **retain-verified**, suggested bucket **other**.

Real old-terminal transport/recovery integration tests are removed before that runtime stops being supported. Retain the
bounded intermediate coverage loss. The earlier proposed drop based solely on absence of a demonstrated product bug was
withdrawn. c5 concerns other deleted pending-cleanup request tests, not an exact alias.

Original stronger consequence: The original lead asked about safety/reattachment coverage while old terminals still run.
The initial intentional-retirement/no-product-defect proposed drop was expressly withdrawn. Retained
registry/barrier/readability/retry tests refute total coverage loss only. Later retirement and the later host
arbitration test limit current relevance; migration-test and lower-layer concerns reported from g152/g151 are related
but not proven aliases in this batch.

Unresolved premise or limitation: None for deletion before retirement. No live runtime defect, frequency or catastrophic
test-tooling consequence is established.

Proposed basis: Record intermediate loss of distinguishing recovery regression tests as other. Preserve surviving unit
coverage and later subject retirement; do not infer a shipped loss-of-work bug.

Existing parent disposition (event 8): rejected. Full099:647 removes six real old-terminal
inputfailure/connectionloss/takeover/sinkkill/concurrentfirstattach/supervisorcrash recovery scenarios while
oldsinksserve. Stand-in retry/reap/backoff and registry/barrier tests do not replace successful real recovery/attachment
witnesses. Preserve initial no-productfailure dismissal expresslywithdrawn. Laterdbfc/1ed retirement/removal
andtipheldnaturaldetachhosttest endexactlegacysubject; no currentoldfixturefix demanded, no inferredruntimeworkloss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g156 record](records/review-through-00de0b6c/groups/g156.json). Record ID: `l1`.

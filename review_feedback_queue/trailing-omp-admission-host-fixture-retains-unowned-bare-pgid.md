# Host-backed fixture cleanup can SIGKILL a recycled, unrelated process group

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: supervisor tests can kill unrelated same-account work if the fixture's entire old
group/session ends and its number is reused while the test still retains its cleanup guard. The earlier finding's
strongest consequence is retained; leader reaping alone is not sufficient evidence of reuse. Source sequence: (1)
claude_report_survives_the_former_claim_deadline invokes spawn_claude_runtime (core.rs:21518); that starts sh -c 'sh
internal hook --vendor claude & ...; wait' (22847-22856). The host child calls setsid before exec
(terminal-host/src/spawn.rs:555-580), so its PID P is also its PGID and SID. The terminal host itself has separately
called setsid (spawn.rs:143-146), and therefore is not a member retaining the child's group/session. (2) start_in_host
records OwnedRuntime { child: None, group: P } (core.rs:22806-22814), then awaits store.session at 22815-22820 and the
session-map lock at 22825-22828 with no guard-lifetime or child-lifetime bound. A permitted interleaving delays this
test task after guard registration while the separate host and its child processes continue. (3) The internal reporter
script is only '#!/bin/sh sleep 25 ' (22725-22731). On its natural completion the inner shell waits/reaps its sleeper
then exits; the outer shell's explicit wait reaps the reporter then exits. If a shell optimizes its final external
command, that removes a process rather than adding an unbounded descendant. This normal-completion route leaves no
orphaned sleeper or un-reaped reporter prerequisite. The host promptly reaps the outer child via waitpid(WNOHANG)
(terminal-host/src/hos

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g114/f1`.

### g114/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 9 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Synthesis revision: distinguishes leader reaping from actual
empty-group/session lifetime and supplies a complete conditional interleaving. TLDR: supervisor tests can kill unrelated
same-account work if the fixture's entire old group/session ends and its number is reused while the test still retains
its cleanup guard. The earlier finding's strongest consequence is retained; leader reaping alone is not sufficient
evidence of reuse. Source sequence: (1) claude_report_survives_the_former_claim_deadline invokes spawn_claude_runtime
(core.rs:21518); that starts sh -c 'sh internal hook --vendor claude & ...; wait' (22847-22856). The host child calls
setsid before exec (terminal-host/src/spawn.rs:555-580), so its PID P is also its PGID and SID. The terminal host itself
has separately called setsid (spawn.rs:143-146), and therefore is not a member retaining the child's group/session. (2)
start_in_host records OwnedRuntime { child: None, group: P } (core.rs:22806-22814), then awaits store.session at
22815-22820 and the session-map lock at 22825-22828 with no guard-lifetime or child-lifetime bound. A permitted
interleaving delays this test task after guard registration while the separate host and its child processes continue.
(3) The internal reporter script is only '#!/bin/sh sleep 25 ' (22725-22731). On its natural completion the inner shell
waits/reaps its sleeper then exits; the outer shell's explicit wait reaps the reporter then exits. If a shell optimizes
its final external command, that removes a process rather than adding an unbounded descendant. This normal-completion
route leaves no orphaned sleeper or un-reaped reporter prerequisite. The host promptly reaps the outer child via
waitpid(WNOHANG) (terminal-host/src/host.rs:347,454-475). Now all members of P's group and session have ended and been
reaped. A still-live or unreaped descendant would prevent this step: do not infer free P merely because the leader was
reaped. The busy-claim test's explicit reporter kill at core.rs:21447-21454 by itself is NOT the proof that P is free; a
surviving sleep/group member must first go. (4) After the group/session is truly empty and the leader has been reaped, a
later fork may allocate P to an unrelated same-account child; that child may become group leader with setpgid(0,0) or
setsid. Linux fork documentation explicitly excludes an existing PGID or SID from new PIDs, while setpgid/setsid allow
the new PID to name its own group: https://man7.org/linux/man-pages/man2/fork.2.html (DESCRIPTION),
https://man7.org/linux/man-pages/man2/setpgid.2.html (DESCRIPTION and NOTES),
https://man7.org/linux/man-pages/man2/setsid.2.html (DESCRIPTION). This establishes why live descendants prevent reuse,
and why an entirely ended/reaped group has no such identity protection. (5) The test task resumes. Even if
spawn_claude_runtime now fails its reporter-alive/parent premise (core.rs:22863-22871), unwinding drops OmpAdmission.
Drop calls kill_owner (23241-23254), which drains the retained None guard (23048-23052). OwnedRuntime::teardown
unconditionally calls kill_process_group(P) (22545-22551), issuing kill(-P,SIGKILL) without a start-identity or
group-ownership check (22582-22587). That signal targets the unrelated replacement group, not the retired fixture.
Contract: test-authoring requires peer liveness before relying on it during teardown; root machine-sharing rules forbid
touching unrelated processes; SPEC_impl.md:2123-2126 ties a kill root to the pinned child's continuing identity.
Suggested fix: retain the host/pinned runtime identity and make group cleanup conditional on proven ownership; preserve
descendant cleanup without treating a bare retired PGID as a durable handle. Suggested bucket: highest proposed, because
unrelated user-owned running processes can be killed; harm is not confined to test tooling. Could-be-highest: yes.
Confidence: confirmed source-level unsafe identity lifetime and source-supported conditional interleaving; not an
observed event. Exact occurrence premise not established by repository source: a sufficiently delayed guard/task and
enough concurrent process creation for P actually to be reallocated as another same-account PGID before that guard is
dropped. These are scheduling/allocation conditions, not behavior this test forces. Available source establishes that
nothing prevents them; it cannot establish that they occurred, their probability, or the host's numeric allocation
sequence. Process-lifecycle/scheduler evidence from a real affected run would establish occurrence, but none was
requested or collected and no reproduction was performed. proposed_drop: false. possible_cover: none. Material caveats:
test-only defect, no production session cleanup bug claimed; a live descendant retaining the old group makes the signal
hit the owned group and does not demonstrate this failure. Direct Some(child) still reserves its unreaped leader until
an actual wait/try_wait releases it. External checks were read-only primary Linux interface documentation, not
execution.

Returned proposal (claims-073, event 9): **retain-verified**, suggested bucket **highest**.

The unsafe identity lifetime and a complete conditional route are established by source: a host-backed fixture can keep
a numeric group after its whole former group/session has ended, then signal a replacement group. Test-only location does
not confine the potential victims to tooling. Preserve the occurrence and allocation caveats.

Original stronger consequence: Supervisor test teardown can SIGKILL unrelated same-account user-owned running work after
complete old group/session termination and numeric reuse. The earlier suggestion that leader reaping alone proves reuse
is explicitly rejected. Direct Some(child) retains its protection only until an actual wait/try_wait releases the
leader.

Unresolved premise or limitation: Occurrence remains unobserved: enough delay and concurrent process creation must
actually reuse the fully released number as another same-account group before guard drop. Source establishes that no
identity reservation or verification prevents the conditional signal; it does not establish allocation sequence or
probability.

Proposed basis: Retain highest for the established conditional wrong-process signal route, with test-only scope and
unobserved scheduling/allocation conditions explicit. Any later dismissal or lowering needs the independent check.

Existing parent disposition (event 9): verified. Parent explicitly accepted named retain-verified proposal from
claims-073 after inspecting it. The unsafe identity lifetime and a complete conditional route are established by source:
a host-backed fixture can keep a numeric group after its whole former group/session has ended, then signal a replacement
group. Test-only location does not confine the potential victims to tooling. Preserve the occurrence and allocation
caveats. Retain highest for the established conditional wrong-process signal route, with test-only scope and unobserved
scheduling/allocation conditions explicit. Any later dismissal or lowering needs the independent check. Checked
source/results: ["At 1ed482dde220452e424973faf38a5e1318986333, core.rs:22806-22828 stores only group=pid with child=None
before asynchronous waits. :22539-22551 still unconditionally signals the group, then waits only for Some(child);
:22582-22587 issues kill(-group,SIGKILL). :23048-23052 and :23241-23254 invoke that teardown on normal cleanup, panic
and cancellation.","core.rs:22725-22731 writes a sleep-25 reporter. :22847-22871 starts a shell that explicitly waits
for its reporter. A permitted delay after registration lets that whole finite chain complete, without assuming that
leader reaping alone releases a group with surviving descendants.","At that revision, terminal-host/src/spawn.rs:143-146
and :555-580 put host and child in separate sessions. terminal-host/src/host.rs:347,454-475 reaps the child; :761-766
explicitly acknowledges numeric reuse after reaping. Nothing in the None cleanup guard retains process identity.","Linux
interface documentation checked read-only: fork excludes existing PGID/SID numbers from a new PID
(https://man7.org/linux/man-pages/man2/fork.2.html, DESCRIPTION); setsid makes the new PID its own PGID/SID
(https://man7.org/linux/man-pages/man2/setsid.2.html, DESCRIPTION); kill with a negative group number targets that
current group and same-account targets are signalable (https://man7.org/linux/man-pages/man2/kill.2.html, DESCRIPTION).
Thus all old group/session members must first be gone/reaped; subsequent reuse is possible, not shown to have
occurred.","Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db core.rs:22471-22531 still has the same bare-group
cleanup. No runtime test, reproduction, observed unrelated kill or frequency estimate."] Current coverage/results:
["SPEC.md lifecycle ownership searches found no acceptance of killing unrelated work. SPEC_impl.md Terminal host
:2047-2051 permits a kill root only while the child is alive and matches its pinned identity; .agents/test-authoring.md
requires peer liveness before relying on it during teardown.","TODO.md Planned empty. Exact fixture, owner, group,
identity and reuse searches in plans/queue found no unlanded fix; tmux plans are landed per plans/REPORTS.md:29-32.
BUGS.md concerns a provisioning SSH pipe and is unrelated.","INDEX.md and active TRIAGE_OUTCOMES.md found no exact
cover. owner-death-child-precedes-pid-publication.md and TRIAGE_OUTCOMES.md:1267-1283 concern an unregistered
spare-builder sleeper, not a stale registered PGID signalling unrelated work. That item's mention of a separate
historical same-run PID-reuse acceptance does not establish coverage of this new fixture.","FILTER.md's host-process
verification-to-signal handoff filter explicitly excludes storing a process number for later use and other waits. This
fixture does exactly that, and it does not satisfy the older-upgrade or session-status filters. No full filter match."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Supervisor test teardown can
SIGKILL unrelated same-account user-owned running work after complete old group/session termination and numeric reuse.
The earlier suggestion that leader reaping alone proves reuse is explicitly rejected. Direct Some(child) retains its
protection only until an actual wait/try_wait releases the leader."; "Occurrence remains unobserved: enough delay and
concurrent process creation must actually reuse the fully released number as another same-account group before guard
drop. Source establishes that no identity reservation or verification prevents the conditional signal; it does not
establish allocation sequence or probability.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g114 record](records/review-through-00de0b6c/groups/g114.json). Record ID: `f1`.

# Settle surviving host starters before removing browser test state

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

After a supervisor crashes during a terminal launch, browser-test cleanup can report success and delete its state while
a terminal host finishes starting. That leaves an owned test process running without its socket or retained run
directory. This concerns maintainer tooling, not an established loss of user sessions.

## Details

Source coordinates from the original records: `e2e/start-stack.sh:418`; `scripts/desktop-smoke.sh:578`. References below
use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The ordinary/both-host browser stack introduced in e16ac4b2758a41fae246c1c15d4c0a5f1872d1ee relies on cleanup at
start-stack.sh:418-441: creators_gone checks only helm, supervisor and replacement pids (:355-368); stop_terminal_hosts
then snapshots current socket entries; success permits rm -rf. hosts/stop.rs:358-370,399-461 performs a one-time
directory listing and main.rs:1998-2015 returns success for all discovered hosts stopped. It does not settle launch
first stages from launch records.

A dead supervisor is not sufficient after abrupt death. SPEC_impl.md:2195-2197 explicitly states 'A supervisor that dies
after the record leaves a first stage that can still start the host on its own'. host_launch.rs:517-559 writes the
record and gives that stage its spec. hosts/spawn.rs:350-359 uses kill_on_drop, which protects ordinary task
cancellation, not abrupt process death; no parent-death hook exists in the searched host startup paths.
terminal-host/src/spawn.rs:85-123 lets the surviving first stage fork and wait for a second stage. The second stage
detaches with setsid (:143-146), binds (:173), starts its child and reports readiness to that still-live first stage
(:90-93,196-202). A valid source interleaving is: supervisor dies after spec delivery; cleanup's socket listing finishes
before this second stage binds; the second stage binds and reports ready while cleanup handles previously listed hosts;
cleanup removes the entire state tree. The new host was absent from the listing and no later scan runs. It remains alive
even if its child exits (terminal-host/SPEC.md, Child exit and end of output:146-149; host.rs:454-486 records exit
without ending the host). A detached replacement supervisor and its children are outside the initial Playwright stack
process group; the orphan-watcher path also signals only the script and named service pids, so group shutdown is not a
universal fence here.

Ordinary SIGTERM is distinct and source-settled: run drops its owned Tokio runtime before exiting (core.rs:18765-18797),
cancellation drops the kill_on_drop child, and an unreported second stage abandons its child when the first stage is
gone. terminal-multihost.spec.ts:648-667 deliberately uses SIGTERM. Q2 is therefore closed for orderly shutdown but has
this crash-path defect. Q3's all_stopped contract remains valid for discovered sockets; this finding is about the
missing creator census before discovery, not a false per-host success status. The underlying cleanup assumption also
existed in the earlier opt-in host run; the assigned change makes hosts the ordinary run and enables them on the remote
too.

Fix direction: before treating the socket census as final, fence/settle surviving first-stage launch records with
identity checks (the supervisor's retry-settlement code already has this concept), or conservatively retain state when
such creators may remain. Merely repeating a scan without fencing creators does not close the race. A source-guided
reproduction would hold a host launch before its bind, abruptly end only its supervisor, begin cleanup, then allow the
stage to bind between census and removal; no such experiment was run in this review.

Suggested bucket: other, because the established harm is leaked maintainer fixture processes and lost cleanup evidence.
Confidence: confirmed source ordering and permitted crash interleaving, not reproduced. proposed_drop: false.
possible_cover: none; distinct from g001/l4=g003/l2 sentinel observation-window gap. Histories and the ordinary-stop
refutation remain preserved.

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02. Consolidation revision after reading latest peer g001/l10 and
g003/l3 and parent focus. Preserve the original abrupt-death consequence, full source route and SPEC_impl.md:2195-2197
crash-surviving starter citation: cleanup can miss a late-bound owned host and delete its socket/state. This shares the
pending-creator/final-scan root with g001/l10 and g003/l3; possible_cover is those peer IDs, not a new queue-coverage
assertion. Correct the earlier overstatement that ordinary SIGTERM was fully source-refuted: kill_on_drop requests the
first-stage kill but does not wait for its completion; runtime exit is not itself a cross-process descriptor-close
barrier. Peers inspected pinned Tokio 1.53.2 process/mod.rs:1105-1128,1240-1252 and process/unix/mod.rs:169-172. Thus
the ordinary-cancellation variant remains unverified after investigation with exact missing premise: can a late binder
report readiness successfully before the kill-requested first stage's pipe reader closes, after supervisor exit is
observed but before state removal? Failed readiness self-abandons and pre-scan bind is discovered, narrowing the window;
neither observation proves a general orderly fence. The abrupt-death variant does not need this uncertain kill-delivery
premise because the first stage survives the supervisor without a destructor, explicitly allowed by the spec. Confidence
confirmed for the source-established abrupt interleaving; ordinary variant investigated-unverified, not promoted.
Suggested bucket remains other for owned tooling process/evidence leakage; no original consequence lowered or erased;
proposed_drop false. No runtime experiment or new duplicate cleanup finding. All original records remain.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g002/g002-l4`,
`g001/l10`, `g003/l3`. The retained append-only histories preserve all earlier claims and corrections. Reviewed source
references name their own revisions; the inclusive reviewed tip is the hash above. This review used source inspection
only, with no product runtime experiments or live-install changes.

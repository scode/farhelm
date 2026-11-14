# Automatic-tab cleanup contract omits failed-close retries

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Automatic-tab cleanup contract omits failed-close retries

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g016/l3`.

### g016/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: f96dd4363d97dc7e07dea5b8947c96be6bf912e8

## TLDR

The rewritten implementation contract says automatic tab cleanup proceeds only when the runtime reports that the shell
exited. It also retries previously authorized closes whose host could not be confirmed ended, even when the aborted
runtime still reports running or unavailable. Following the new absolute condition would suppress that recovery and
could leave failed tab closes unfinished. This is a documentation contradiction; no current runtime cleanup failure is
claimed.

## Details

Assigned changed text at SPEC_impl.md:4826–4827 says 'Automatic tab cleanup goes ahead only while the tab’s runtime says
its shell exited, checked under its lifecycle claim before signalling'. The parent hunk instead described rechecking a
tmux window; this commit introduces the absolute host-runtime-exit statement. git log -S over the full exclusive
6e423e30068b62c6cfa276c6d720ee35bb70387a through inclusive 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db range attributes
that new phrase to this commit. The runtime files are unchanged by this documentation commit. Its only later in-range
SPEC_impl.md diff changes unrelated setup wording, not this assertion.

At the reviewed revision, service/core/host_tabs.rs:146–156 explains that a failed host end aborts the runtime, leaving
its old status possibly running forever, so automatic reaping must retry it. :193–199 explicitly checks
end_failed/retry_at before finished(); :312–332 marks and schedules those retries. service/ticker.rs:1651–1662 consumes
that due list and calls try_close_tab. service/core.rs:13923–13935 takes the lifecycle claim and passes
only_if_dead=true through close_tab_claimed; :13989–14012 resolves the same tab and invokes its host close. No caller
adds a fresh shell-exit requirement. Back in host_tabs.rs:929–932, end_failed bypasses the unavailable-runtime refusal,
and :963–969 checks reap_due again under the claim. The failure path at :1025–1028 marks the tab for precisely this
later retry. These checks establish an automatic cleanup path without the specified runtime-exit observation.
Independently, finished at :181–185 also admits a gone host, which need not have delivered a shell-exit record.

The complete Tabs in hosts contract at SPEC_impl.md:2262–2264 correctly describes the exception: 'A close that cannot
confirm the end keeps the tab and its record and marks it, and since its stopped runtime announces nothing, the reap
closes it again until one does'. That accurate rule and the working runtime do not reconcile the later absolute
only-while-exited condition. Qualify :4826 to cover ordinary first-time dead-tab reap plus continuation of a previously
authorized failed close. The retry is finishing that earlier close; this finding does not allege it obtains new
authority to stop unrelated live work.

Revision closes l3 from lead to finding after checking the scheduler, both destructive-boundary checks, failure marking,
full governing host-tab subsection and in-range attribution. Suggested bucket: other, documentation correctness.
Confidence: confirmed by source/contract, not execution. proposed_drop: false. possible_cover: none among supplied
interface records; no coverage-source search. Could-be-highest: no established shipped security or loss-of-work route;
the maintenance consequence is hypothetical and not severity evidence. No runtime tests or reproductions run.

Returned proposal (claims-007, event 8): **retain-verified**, suggested bucket **other**.

The absolute shell-exit condition omits the intentional retry of a previously authorized close whose host could not be
confirmed ended. Source and another specification paragraph explicitly require that recovery. This is a documentation
contradiction, not evidence of current unauthorized signalling or failed cleanup.

Original stronger consequence: Following the absolute condition in a future change could suppress recovery and leave
failed closes unfinished. The original lead and final finding both keep that maintenance risk separate from current
runtime behavior and recognize the earlier close's authority.

Unresolved premise or limitation: No missing source premise for the documentation mismatch. A future implementation
suppressing recovery is hypothetical; no current unfinished close or user-work loss was demonstrated.

Proposed basis: Retain as a distinct documentation defect, other priority; clarify the first ordinary reap versus
continuation of an already authorized failed close.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from
claims-007 after inspecting it. The absolute shell-exit condition omits the intentional retry of a previously authorized
close whose host could not be confirmed ended. Source and another specification paragraph explicitly require that
recovery. This is a documentation contradiction, not evidence of current unauthorized signalling or failed cleanup.
Retain as a distinct documentation defect, other priority; clarify the first ordinary reap versus continuation of an
already authorized failed close. Checked source/results:
["f96dd4363d97dc7e07dea5b8947c96be6bf912e8:SPEC_impl.md:4826-4827 introduces the absolute runtime-shell-exited
condition. git log -S over the full assigned range names f96dd436 as its introduction; the runtime files are unchanged
by this documentation commit.","Pinned crates/farhelm-supervisor/src/service/core/host_tabs.rs:146-164 explains the
stopped-runtime stale-status problem. :193-199 admits end_failed/retry_at independently of finished(); :312-332 marks
and selects those retries. :181-185 also considers absent hosts finished without needing a shell-exit
record.","service/ticker.rs:1651-1662 submits the due list. service/core.rs:13923-13935 claims the session and passes
only_if_dead=true; :13989-14012 resolves the tab and calls host close without adding a shell-exit gate.
host_tabs.rs:929-932 permits the failed-close continuation even with an unavailable runtime; :963-969 rechecks reap_due
under the claim, and :1025-1028 marks a failed host end for that continuation.","Pinned SPEC_impl.md:2234-2269 is the
complete Tabs in hosts contract; :2262-2264 explicitly describes retrying that failed close. A retry finishes earlier
authorized work, rather than gaining authority over unrelated live work. No runtime reproduction ran."] Current
coverage/results: ["Current SPEC_impl.md:2262-2264 and :4826-4829 retain the competing rules. The accurate retry
paragraph establishes the contradiction; it does not explicitly accept the later false absolute condition.
SPEC.md:2106-2115 and :2150-2169 describes visible failure/retry and ordinary automatic tab removal, not an exception
authorizing inaccurate documentation.","TODO.md Planned and plans/queue/INDEX.md are empty; tmux cleanup plans have
landed. BUGS.md's SSH-helper issue is unrelated.","Searched INDEX.md, queue files and active TRIAGE_OUTCOMES.md for
automatic cleanup, failed closes, tab retry and reap. tab-reap-deadline-excludes-listing-request.md and
tab-retry-test-measures-observation-time.md concern test observation boundaries, not this contract.
restart-unwind-omits-tree-sweep-failure.md concerns agent Restart failure disposition, not failed tab-close retries. No
exact cover or useful extension found; no FILTER.md boundary covers the persistent contradiction."] Exact cited cover:
[] Original stronger consequence and missing premise remain preserved: "Following the absolute condition in a future
change could suppress recovery and leave failed closes unfinished. The original lead and final finding both keep that
maintenance risk separate from current runtime behavior and recognize the earlier close's authority."; "No missing
source premise for the documentation mismatch. A future implementation suppressing recovery is hypothetical; no current
unfinished close or user-work loss was demonstrated.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g016 record](records/review-through-00de0b6c/groups/g016.json). Record ID: `l3`.

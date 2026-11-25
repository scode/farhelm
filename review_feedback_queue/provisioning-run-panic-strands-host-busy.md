# A panicking provisioning run strands its host busy forever

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

If a setup/update/uninstall run ever panics mid-flight, that host is left marked busy with a run showing "Running"
forever; no further Setup/Update/Uninstall can start on it until the user Removes the host or restarts Farhelm.

## Details

**other.** Source inspection; commissioner confirmed the spawn and cleanup paths. Bucketed by what is established: the
missing cleanup on panic. A panic reachable from host input or ordinary use is NOT demonstrated (the unverified premise,
kept from the reviewer). A demonstrated reachable panic would make this a denial of host management → high. It is not
highest — no user data/credentials/processes are lost, and Remove/restart recovers. FILTER.md rare-glitch filter does
not match: the consequence persists until user action (not self-correcting, retry is refused while busy).

`crates/farhelm-helm/src/provisioning/service.rs:2589-2604` (`launch_claimed_run`) spawns the run as a bare
`tokio::spawn` whose body is `revalidate_and_execute`, with no `catch_unwind`. The comment at 2596-2597 states the run
hands the provisioning lock back "as it publishes its end (`Self::end_run`), not when this task ends". The busy marker
and Running progress view are cleared only by `end_run` (2792-2806), `fail_action` (3401), `finish_preflight_failure`
(3290), `finish_use_as_is` (3217), the route-error withdrawal (2569-2578), or `forget_host_memory` (2697, reached by
Remove/uninstall). A panic unwinds past all of these: the `ProvisionLockGuard` drops (lock frees) but `memory.busy` and
`memory.runs[host]` stay set. No production reaper notices a dead run task — `memory.tasks` is consulted only by
`forget_host_memory` and the test-only `abort_run`; `is_finished()` is used for connection actors and tests, never to
reconcile provisioning busy. The only `catch_unwind` in provisioning is the Tensorlake session-launch path
(`tensorlake_add.rs:495-526`), which also has its own `busy.remove` fallbacks — an asymmetry showing ordinary runs were
assumed panic-free rather than guarded.

Verify: arm a panic inside `revalidate_and_execute` and observe `busy` retains the host and `runs()` still lists it
Running after the task ends. Fix shape: wrap the run body in `AssertUnwindSafe(...).catch_unwind()` and publish a Failed
run + clear busy on panic (mirroring the Tensorlake launch guard), or add a reaper for finished/panicked task handles.

Reviewer metadata (area 5 chunk D, provisioning execution/plans): suggested bucket: other; confidence: confirmed for the
missing guard, stranding trigger unverified; proposed_drop: none; possible_cover: none known. Commissioner validation:
spawn/cleanup asymmetry confirmed at the cited lines; the queue's one provisioning-panic mention
(trailing-mac-reboot-shell-discards-command-status-tag) is confined to a maintainer test helper, not a production run
path, so it does not establish the trigger. Coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue)
— no match.

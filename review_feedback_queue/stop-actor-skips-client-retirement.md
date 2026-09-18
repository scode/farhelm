# Removed host's connection lingers instead of tearing down

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After a host is stopped or removed, its connection can stay open and its in-flight operations keep hanging instead of
failing promptly — a removed machine lingers as live transport rather than tearing down.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible. Coordinator verified.

`retire_withdrawn`'s contract (`crates/farhelm-helm/src/manager.rs:1245-1259`) states "every site that removes or
replaces a row's `client` calls this with what it took out, because withdrawing a client and tearing its connection down
have to be ONE event" — dropping the last `Arc` does not reliably close the connection (an agent answer being assembled
owns a writer-channel clone that keeps the writer parked and the transport open, and may then write a fleet answer onto
a row serving a different machine). Four publish sites comply, but two removal sites do not: `stop_actor`
(manager.rs:2554-2576) removes the handle and aborts the tasks without taking the client out of the removed status for
retirement (the Retired-supervisor path that might have retired it is cancelled with the abort), and `sync_registry`'s
removed-row retain arm (manager.rs:1399-1414) aborts without retiring.

Teardown then waits for the last `Arc<SupervisorClient>` drop (the Drop backstop, client.rs:825): while any captured
clone survives (in-flight session operation, terminal relay), the removed host's transport (ssh child/socket) stays
open, pending requests are not failed promptly with `SentUnanswered`, and in-flight agent answers are not aborted.
Bounded: a re-added destination gets a fresh never-recycled `HostId`, so misattribution is impossible — the host lingers
as live transport and un-failed operations instead of tearing down at withdrawal. (`shutdown` shares the shape but is
process-terminal, so moot.) Possible: needs a surviving clone at removal; impact is lingering, not misrouting.

Suggested fix: in `stop_actor` and the retain arm, take the client from the removed handle's status (`send_modify` +
`take`, as the reconfigure path does at :1483-1503) and pass it to `retire_withdrawn` — both call sites hold only sync
locks and `retire()` is sync.

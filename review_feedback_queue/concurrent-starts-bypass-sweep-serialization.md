# Concurrent starts bypass sweep serialization

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Concurrent sweep starts could publish the losing run as current.

## Details

F305 — **possible** — `deflake/bin/deflake:1318` — Concurrent starts bypass sweep serialization

The daemon lock prevents two live sweeps, but publication and success reporting happen before successful lock
acquisition is acknowledged. Overlapping starts can publish a losing run and leave its dead process number behind, so
status and stop address it instead of the live run; later reuse has a separate wrong-target risk. The overlap was not
executed. Coordinate publication with confirmed daemon ownership and roll back losing-run state.

## Evidence and triage context

- deflake/bin/deflake:1301-1308 acquires and releases the preliminary lock before publication and spawn.
- deflake/bin/deflake:185-196 gives overlapping starts from the same checkout the same current-pointer path.
- deflake/bin/deflake:1310-1318 creates a run and publishes its pointer before the child acquires the daemon lock.
- deflake/bin/deflake:1325-1337 spawns, stores the PID, prints started, and returns zero without a successful-lock
  handshake.
- deflake/bin/deflake:1344-1347 refuses the losing daemon before Daemon.main and its orderly PID-file cleanup. This
  prevents concurrent sweeps but does not undo publication.
- deflake/bin/deflake:1286-1294 and :1456-1463 make stop follow the published losing run and signal its stored PID.
- review_feedback_queue/FILTER.md:24-42 does not cover durable wrong state, success reported for failure, or
  wrong-target destructive actions. SPEC_impl.md:2201-2202 excludes stored-for-later PIDs.
- The overlap was not executed. No matching Planned item, queue coverage, or triage acceptance was identified.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_sec:p1:C2`.

- `test_infrastructure_04_sec:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

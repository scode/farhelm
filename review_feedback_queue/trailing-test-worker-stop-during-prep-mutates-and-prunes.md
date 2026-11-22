# Stopping a test-worker run during preparation still syncs source, prunes evidence, and publishes an empty finished directory

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

Stopping a test-worker run during its preparation phase after worker admission still syncs the cancelled run's files
into the remote checkout, evicts an older retained run, and marks an empty evidence directory as finished.

## Details

**other.** Source inspection. Main reference `scripts/test-worker/remote/with-lock.sh:134-153`.

`scripts/test-worker/remote/with-lock.sh` only checks `[ ! -e "$stage/stop" ] || return 143` once, at line 79 inside
`admit_worker` (before auto-bootstrap at line 88, before `sweep_stale_stages` at line 135, and before source
installation at lines 139-142). When `scripts/test-worker/remote/stop.sh:18-22` records `$stage/stop` during
auto-bootstrap or source installation (before `run-workload.py` touches `$stage/running`), `stop.sh` reports
`stop requested for $id during preparation` and exits 0, but `with-lock.sh` never re-checks `$stage/stop`. It proceeds
to overwrite `$base/src`, `$base/synced.list`, and `$base/source.txt` (`with-lock.sh:139-141`), creates `$base/out/$id`
(`line 145`), and runs `prune_evidence` (`line 146`), evicting the oldest of the 20 retained evidence directories. Only
at line 150 does `scripts/test-worker/remote/run-workload.py:80-81` observe `$stage/stop` and return 143 without running
`run-$cmd.sh`. Line 152 then still atomically publishes `$out/finished` containing `143` for an empty directory with no
`summary.txt` or logs, so `collect` later succeeds (exit 0) while copying zero artifacts and an older real run's
evidence has been unnecessarily evicted. By contrast, a stop received before line 79 exits immediately at line 134 with
no source mutation, no pruning, and no empty `finished` directory. Fix direction: re-check
`[ ! -e "$stage/stop" ] || exit 143` before installing source (line 139) and before creating `$out`/calling
`prune_evidence` (line 145), and avoid publishing `$out/finished` (removing `$out` instead) if `run-workload.py` exits
on a pre-start `$stage/stop` without spawning the runner.

Reviewer metadata (g02-test-worker-isolation/f3): suggested bucket: other (cannot be in highest: maintainer test-worker
tooling defect causing unnecessary checkout mutation, eviction of one old test-worker run directory, and an empty
finished marker); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g02-test-worker-isolation/f3`.

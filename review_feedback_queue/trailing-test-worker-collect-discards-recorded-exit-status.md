# Test-worker collect discards the recorded exit status from finished

Reviewed commit: 022d0f010fe5fb0d5acd3e73e02b0569517ec1bc

## TLDR

Collecting a test-worker run's evidence later throws away the exit status recorded by the worker, so the local evidence
directory does not distinguish a passed run from a failed or force-killed one (and can display a stale graceful-stop
exit code instead of the forced kill).

## Details

**other.** Source inspection. Main reference `scripts/test-worker/test-worker.sh:191-193`.

`scripts/test-worker/remote/with-lock.sh:152` records the authoritative command exit code `$code` inside `$out/finished`
before publishing completion. However, nothing ever reads that value: `scripts/test-worker/remote/collect.sh:12-13` only
tests `[ -f "$result/finished" ]` and outputs `cat "$result/source"`, `fetch_out` in
`scripts/test-worker/test-worker.sh:192` excludes `/finished` (`--exclude '/source' --exclude '/finished'`), and
`scripts/test-worker/test-worker.sh:243` stamps `invocation.txt` unconditionally with `outcome: collected` and exits 0
without printing the exit status or summary (unlike a direct run at line 227, which writes `outcome: exit $code`). This
contradicts `.agents/test-worker.md:75` ("writes `invocation.txt` with the source commit and result") and visibly
misreports forced-stop runs: in the exact scenario exercised by `scripts/test-worker/test-test-worker.py:447-503`
(`test_forced_stop_leaves_a_collectable_invocation`), `run-run.sh` catches SIGTERM and writes `exit: 143` to
`summary.txt` before exiting, then `run-workload.py:72-75` SIGKILLs the surviving same-group child and returns `137`,
which `with-lock.sh:152` writes to `$result/finished`. When `collect` runs at `test-test-worker.py:502` (which does not
inspect the collected directory), `final/invocation.txt` records only `outcome: collected`, `/finished` is excluded, and
`final/summary.txt` still reports `exit: 143` (or is missing its `exit:` line altogether if the runner itself was
SIGKILLed). Fix direction: have `collect.sh` emit both the source commit and the recorded exit code from
`$result/finished`, stamp `outcome: collected (exit <code>)` in `invocation.txt`, and assert the collected exit outcome
in `test_forced_stop_leaves_a_collectable_invocation` and
`test_copy_failure_preserves_worker_exit_and_allows_collection`.

Reviewer metadata (g02-test-worker-isolation/f2): suggested bucket: other (cannot be in highest: maintainer test-worker
evidence collection metadata gap, not security or user-data loss); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g02-test-worker-isolation/f2`.

# Resize test cleanup signals historical PID after child reaping

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Resize test cleanup signals historical PID after child reaping.

## Details

`F16 / COR-RESIZE-HISTORICAL-PID` — **possible** — `crates/farhelm-supervisor/src/tmux.rs:6087` — Resize test cleanup
signals historical PID after child reaping

The resize timeout test can send SIGKILL to an unrelated process if its fixture's PID is reused. The test starts a
stand-in tmux client that records its PID and then sleeps. The resize operation times out with Tokio's `kill_on_drop`
enabled, which kills the abandoned child and permits Tokio to reap it. The test subsequently reads the recorded number,
polls it with `kill(pid, 0)`, and sends SIGKILL if that number still exists after five seconds.

Once the child has been reaped, its recorded PID no longer establishes ownership. If the kernel assigns that number to
another process before a poll sees it absent, the test treats the replacement as a surviving fixture and eventually
kills it. That could destroy unrelated work running under the test runner's account. The race is in test cleanup, not
the production resize command.

Remove the raw-PID kill fallback, or keep an owned child identity reserved until cleanup completes. If cleanup instead
uses a process start token, it must establish that token while the fixture is still known to be the child and verify it
before signaling; sampling the reused PID afterward is insufficient.

Suggested bucket: highest

Possible cover: none

Caveats: Reaping is not necessarily complete when resize returns, and reuse during this polling window has not been
reproduced. The runtime_data pass-two crosscheck agrees this finding is not new.

Restater note: The pinned Tokio 1.53.1 implementation kills on drop and either reaps the child or queues it for later
reaping, so the test cannot assume the PID stays reserved during its polling loop. Actual reuse and an unrelated kill
remain unverified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `runtime_general p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Reaping not necessarily synchronous before resize return; reuse timing unverified. runtime_data p2
crosscheck agrees not new.

## Filed reviewer metadata

- `runtime_general p2`: confidence as filed: **possible / likely**. Open premise: the fixture’s numeric PID is reused
  before or during the post-timeout existence check. Suggested bucket as filed: **highest**

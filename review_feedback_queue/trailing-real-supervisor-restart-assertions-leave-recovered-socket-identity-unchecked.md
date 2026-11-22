# Real supervisor restart assertions leave the recovered socket identity unchecked

Reviewed commit: 76ebed7af03dbd2a1ed896993d7a5b945f317e33

## TLDR

The executable-path restart tests show that the original host stays alive and the reattached session works. They can
miss a changed durable socket address because the two observations are never tied together after reattach.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/host_processes.rs:612`.

host_processes.rs:169-195,569-574 reads the original socket from the row. After starting the replacement supervisor,
:601-619 attaches by session ID, verifies old and fresh output, looks up the same old socket again, and checks only the
final row's outcome and kind. :334-343's attachment has no host identity. A bad write to terminal_address while the
runtime still serves the right terminal would satisfy those assertions. SPEC_impl.md:2223-2245 gives the handover's
existing-host adoption and the durable address's role in lookup. Compare the replacement's recorded socket with the
saved socket and correlate the reattached input with the retained host where practical. Bucket: other, test coverage.
Confidence: confirmed by source for the missing association and address assertion. No product regression or runtime
change observed. Possible coverage: host_restarts.rs:351-372 checks the in-process socket, host PID, child PID and
durable address, and observes serve's retained runtime. The real executable SIGTERM and SIGKILL legs do not make that
durable comparison. The only later in-range host_processes.rs change is fcb6fa5e4a40e5e1a4bcc505ad5ffa32dfa95b79, which
reuses grid_row_text and leaves the identity assertion intact. The comparison in host_restarts.rs:363-368 occurs before
its explicit serve start at :370, which is useful related coverage but not the executable-path post-attach row
assertion.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f8`.

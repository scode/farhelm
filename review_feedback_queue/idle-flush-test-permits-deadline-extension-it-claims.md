# Idle-flush test permits the deadline extension it claims to reject

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The idle-flush test allows the renewed deadline it is meant to reject.

## Details

F148 — **definite** — `crates/farhelm-supervisor/src/tmux/stream.rs:2159`;
`crates/farhelm-supervisor/src/tmux/stream.rs:2160` — Idle-flush test permits the deadline extension it claims to reject

Unrelated traffic can incorrectly reset the held output's deadline, yet the test still receives the expected bytes. Its
unrestricted final await lets paused time advance until the renewed timer expires, so eventual output cannot distinguish
the regression. Establish input-consumption boundaries and require completion by the original deadline, avoiding a final
wait that permits later time to satisfy the assertion.

## Evidence and triage context

- crates/farhelm-supervisor/src/tmux/stream.rs:2132 promises the original idle deadline.
- crates/farhelm-supervisor/src/tmux/stream.rs:2147 injects unrelated traffic, but :2160 merely awaits eventual
  completion and :2161 checks bytes.
- [private local path] documents paused-clock automatic advancement.
- crates/farhelm-supervisor/src/tmux/stream.rs:2138 uses paused Tokio time.
- crates/farhelm-supervisor/src/tmux/stream.rs:2147 advances time around unrelated messages; :2160 then awaits the
  reader without checking the original deadline.
- crates/farhelm-supervisor/src/tmux/stream.rs:1950 uses a real cat feeder, so yielding does not establish that the
  prefix reached the reader.
- crates/farhelm-supervisor/src/tmux/stream.rs:2132: the test explicitly claims unrelated notifications must not extend
  the original deadline.
- crates/farhelm-supervisor/src/tmux/stream.rs:2141: the fixture supplies a pending prefix, then sends four unrelated
  notifications between virtual-time advances at lines 2147–2157.
- crates/farhelm-supervisor/src/tmux/stream.rs:2159: after the last advance, line 2160 awaits the reader without a
  completion-time assertion; line 2161 checks only returned bytes.
- crates/farhelm-testtrace-macros/src/lib.rs:249: absent multi_thread, the macro selects CurrentThread; line 258
  preserves start_paused.
- crates/farhelm-testtrace/src/lib.rs:1132: the wrapper builds a current-thread runtime and applies start_paused at
  line 1139.
- Cargo.lock:5686 identifies Tokio 1.53.1. That dependency's src/time/clock.rs:120–125 documents automatic advancement
  to the next pending timer when the paused runtime has no work.
- crates/farhelm-supervisor/src/tmux/stream.rs:1217: production checks the absolute deadline, and line 1229 bounds the
  control-line read by it. Lines 1295–1316 update the deadline only while handling own-pane payload.
- crates/farhelm-supervisor/src/service/connection.rs:1477: the terminal forwarder calls next_output and consumes its
  returned bytes, making the timing contract relevant to live terminal output.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1053 describes bounded prefix retention; no exact existing test-defect coverage found.
- SPEC_impl.md:1053 accepts queries split beyond the idle interval, not unrelated traffic extending that interval.

Caveats:

- No mutation or runtime test was run.
- The current production implementation retains the intended own-pane deadline behavior.
- SPEC_impl.md:1053-1055 describes bounded prefix retention and idle flushing; it does not accept this test's failure to
  distinguish deadline extension.
- Input consumption is also not synchronized by the yields. Production deadline logic currently exists.
- No mutation test performed. Current production code preserves the relevant own-pane deadline behavior. The
  real-process scheduling premise is an additional weakness, not necessary to establish the missing deadline assertion.
- No mutant executed.
- The unestablished reader-readiness premise is additional to the unrestricted final await.
- No runtime or mutation reproduction was performed.
- Current production code preserves the deadline; this is a defective regression oracle, not an established production
  delay.
- The fixture also lacks an observation proving when the prefix was processed and when each unrelated notification was
  consumed.
- SPEC_impl.md:1053–1055 describes briefly retaining split prefixes and flushing after idle; it does not accept
  indefinite extension by unrelated traffic.
- TODO.md:33–43 plans connection-read-loop creation work, not this oracle. BUGS.md's terminal notification backlog
  concerns the separate input connection. No matching queue, filter, or ledger basis was found.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_cor_02:p1:F1`,
`gap_supervisor_runtime_sec_02:p1:F4`, `sr_systems:p3:F1`.

- `gap_supervisor_runtime_cor_02:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_runtime_sec_02:p1:F4`: confidence as filed: definite; suggested bucket as filed: other.
- `sr_systems:p3:F1`: confidence as filed: definite; suggested bucket as filed: other.

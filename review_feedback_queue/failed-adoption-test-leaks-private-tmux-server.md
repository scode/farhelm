# Failed adoption test leaks its private tmux server

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed adoption test can leave an unreachable tmux daemon running.

## Details

F147 — **definite** — `crates/farhelm-supervisor/src/tmux.rs:5321`; `crates/farhelm-supervisor/src/tmux.rs:5337` —
Failed adoption test leaks its private tmux server

After daemon startup, an assertion failure bypasses the fixture's only shutdown command. The driver has no destruction
cleanup, and removing its temporary directory does not stop the daemon; it instead removes the usual socket route for
cleanup. Establish a server guard before startup using the selected old binary, and keep the directory until guarded
shutdown completes.

## Evidence and triage context

- crates/farhelm-supervisor/src/tmux.rs:5321 creates an ordinary temporary directory and :5329 starts the old server.
- crates/farhelm-supervisor/src/tmux.rs:2032 configures exit-empty off.
- crates/farhelm-supervisor/src/tmux.rs:5337 contains assertions preceding the only explicit kill-server at :5373.
- crates/farhelm-supervisor/src/tmux/test_support.rs:64 demonstrates the existing guard-before-start ownership pattern.
- crates/farhelm-supervisor/src/tmux.rs:5321 creates a plain temporary directory and driver, then :5329 starts the
  daemon.
- crates/farhelm-supervisor/src/tmux.rs:5342 begins fallible assertions before the sole kill-server call at :5373.
- crates/farhelm-supervisor/src/tmux.rs:328 shows no server cleanup owner in TmuxDriver.
- crates/farhelm-supervisor/src/tmux/test_support.rs:68 demonstrates the existing guard-before-startup ownership
  pattern.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact existing coverage. Production adoption-preservation policy does not exempt fixture teardown.
- Preserving an operator's old server on production refusal does not authorize leaking a disposable test server. The
  hung-dependency filter does not apply.

Caveats:

- No failing test was executed.
- The leak requires startup to succeed and a subsequent failure that leaves the server alive.
- This concerns a private test daemon; no product-session loss is established.
- SPEC_impl.md:977-983 requires production adoption refusal to preserve an existing server. That requirement does not
  authorize abandoning a test-owned server during fixture teardown.
- Requires both old/new tmux availability and a failure after successful startup; no failure executed.
- Requires both old and new tmux binaries and a failure after daemon startup. The directly established consequence is
  leaked fixture resources, not loss of user sessions. No runtime reproduction performed.
- Requires the test's old/new tmux prerequisites and a failure after startup.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_runtime_cor_01:p1:F2`,
`gap_supervisor_runtime_sec_02:p1:F6`.

- `gap_supervisor_runtime_cor_01:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_runtime_sec_02:p1:F6`: confidence as filed: definite; suggested bucket as filed: other.

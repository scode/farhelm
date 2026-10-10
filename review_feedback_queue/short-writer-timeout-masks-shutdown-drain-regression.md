# Short writer timeout masks the shutdown-drain regression

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An independent writer timeout hides a broken shutdown-drain bound.

## Details

F265 — **definite** — `crates/farhelm/tests/e2e/session_lifecycle.rs:2792` — Short writer timeout masks the
shutdown-drain regression

The fixture's writer-stall timeout is shorter than the shutdown observation. It can terminate the writer on its own,
letting an unconditional shutdown join pass even after the intended drain bound is removed. Make the stall timeout
exceed the test deadline, establish blocked delivery and entry into shutdown's tail, and require the drain bound to end
the connection.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:2760-2778 identifies unconditional writer joining as the regression this
  test claims to reject.
- crates/farhelm/tests/e2e/session_lifecycle.rs:2791-2794 sets writer_stall to two seconds.
  crates/farhelm/tests/e2e/harness.rs:1633-1637 does not increase that field.
- crates/farhelm-supervisor/src/service/connection.rs:292-304 passes writer_stall into the writer task; :859-864 exits
  that task when write_frame_before_stall reports failure.
- crates/farhelm-proto/src/io.rs:119-125 returns an error after a complete window with no byte progress. An initial
  window containing partial progress can add another window, still well below the test's deadline.
- crates/farhelm-supervisor/src/service/connection.rs:115 defines the separate five-second drain window; :719-729 closes
  the writer queues, drains, and returns the read-loop result.
- crates/farhelm/tests/e2e/session_lifecycle.rs:2832-2838 half-closes requests and only requires successful connection
  completion within thirty seconds. Once read EOF has produced a successful result, independently timing out the writer
  can satisfy an unconditional join without exercising drain expiry.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/shutdown-expiry.md:14-24 concerns planned supervisor process exit before tmux output clients
  become safe, potentially losing sessions. This finding concerns a protocol-writer regression test; trigger,
  consequence and scope do not match.
- BUGS.md:8-44 concerns abrupt supervisor death and private-tmux crashes, explicitly distinguishing planned shutdown. It
  does not cover this test oracle.
- review_feedback_queue/FILTER.md:24-42 does not establish coverage: this is a systematically masked regression, not
  merely a rare safely retriable failure.

Caveats:

- No mutation experiment or runtime test was run.
- Writer failure occurring before read EOF could instead produce a test failure; that possibility does not remove the
  passing execution that masks drain removal.
- No production vulnerability or current user-work loss is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_07_sec:p2:F1`.

- `cli_installation_07_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.

# Readiness failure can leave the systemd probe child running

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Readiness failure could leave a systemd probe child without cleanup ownership.

## Details

F178 — **possible** — `crates/farhelm/tests/e2e/session_lifecycle.rs:5990` — Readiness failure can leave the systemd
probe child running

The probe child is spawned without kill-on-drop before a fallible readiness wait. Failure there can unwind while the
child and transient scope continue running. A normally executing fixture expires after 120 seconds, but a wrapper not
yet executing it could survive longer; neither timeout sequence was induced. Enable kill-on-drop before spawning and
keep explicit waiting on the successful path.

## Evidence and triage context

- crates/farhelm/tests/e2e/session_lifecycle.rs:5990-6004 spawns systemd-run without kill_on_drop.
- crates/farhelm/tests/e2e/session_lifecycle.rs:6006-6013 awaits readiness before :6015 arms PidKillGuard.
- crates/farhelm/tests/e2e/session_lifecycle.rs:6081 and :6091 introduce timeout/panic paths before that guard exists.
- crates/farhelm-fixtures/src/fake_agent.rs:2556-2589 installs the stubborn fixture and its 120-second sleep only after
  the wrapper has executed it.
- session_lifecycle.rs:5990-6004 spawns without kill_on_drop; :6006-6013 can fail before the guard at :6015.
  fake_agent.rs:2588 supplies a 120-second backstop only after fixture execution reaches that point. It does not bound a
  systemd-run wrapper that has not executed the fixture.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/FILTER.md:44-55 covers certain stalls caused by a hung systemd manager. It does not exactly
  cover this standalone test's child-ownership gap across all readiness failures; the test has no supervisor whose
  restart owns cleanup.

Caveats:

- No readiness timeout was induced.
- A normally running fixture self-expires after 120 seconds.
- An indefinite pre-exec wrapper survivor is possible rather than demonstrated.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_08_cor:p1:F5`,
`cli_installation_08_sec:p1:C1`.

- `cli_installation_08_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_08_sec:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.

# The live-agent restart test never establishes a live agent

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The live-agent restart test can run only the dead-agent branch.

## Details

F154 — **definite** — `crates/farhelm-supervisor/src/service/core.rs:18259` — The live-agent restart test never
establishes a live agent

The requested sleep agent cannot execute through the fixture's nonexistent shim. A live tab does not establish a live
agent, so Restart can take the dead-agent path while the test claims to protect live-agent stopping and tab
preservation. Use a functioning shim, verify agent identity and liveness before Restart, then verify its replacement and
the tab's preservation.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:18253–18301: the live-agent restart test constructs the supervisor with
  dummy_exe and proves only the tab process is live.
- crates/farhelm-supervisor/src/service/core.rs:15398–15404: dummy_exe is /nonexistent/farhelm.
- crates/farhelm-supervisor/src/launch.rs:617–654: the login shell executes that launch shim before the requested agent.
- crates/farhelm-supervisor/src/service/core.rs:10234–10307: live-agent stopping and dead-agent survivor cleanup are
  distinct branches.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- This does not establish a current product tab-killing regression.
- A transient launch shell can be alive, but that does not establish the intended live agent.
- A transient launch shell might briefly be live; that does not establish the intended sleep agent.
- No current product tab-loss regression is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_03:p1:F1`.

- `gap_supervisor_state_cor_03:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

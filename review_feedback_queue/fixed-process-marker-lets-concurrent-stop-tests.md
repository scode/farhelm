# Fixed process marker lets concurrent Stop tests kill each other’s children

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Concurrent Stop tests can kill each other's fixture processes.

## Details

F20 — **definite** — `crates/farhelm-supervisor/src/service/handlers.rs:4011` — Fixed process marker lets concurrent
Stop tests kill each other’s children

Every invocation of this Stop fixture uses the same session marker, while cleanup scans processes across the host.
Overlapping runs under the same account therefore cannot distinguish their children: either run can kill the other's
fixture and may falsely satisfy a disappearance check. The established impact is test interference, not loss of ordinary
product sessions or unrelated user processes. Generate a fresh UUID and use it for both the child marker and stored
session identity.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/handlers.rs:4011–4013,4035–4062: the child and session both use fixed s1.
- crates/farhelm-supervisor/src/procs.rs:1683–1700: the sleeper command explicitly applies that marker.
- crates/farhelm-supervisor/src/service/handlers.rs:942–953: terminal-less Stop invokes AgentOnly marker cleanup.
- crates/farhelm-supervisor/src/service/sweep.rs:125–156,252–259,422–453: process scanning is host-wide and matching
  uses the session marker.
- AGENTS.md:651–677: concurrent checkouts share the Unix account and other agents' owned processes must remain
  untouched.
- crates/farhelm-supervisor/src/service/handlers.rs:4011 fixes every invocation's session marker to s1; line 4070 stops
  that marker.
- crates/farhelm-supervisor/src/procs.rs:1695 removes ambient agent/tab markers and installs the supplied session
  marker.
- crates/farhelm-supervisor/src/service/sweep.rs:125 matches that marker, line 257 admits session-only legacy processes
  to AgentOnly, and line 422 scans the host process snapshot.
- crates/farhelm-supervisor/src/service/handlers.rs:4110 treats disappearance of its fixture as successful completion.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:3883–3899 covers supervisor ownership of Restart across connection cancellation, not marker
  collisions. SPEC_impl.md's accepted short PID-reuse residual is also a different trigger.
- No matching disposition. State-directory isolation cannot protect a host-wide marker sweep; deliberate same-account
  interference and PID-reuse exclusions do not apply to this accidental collision.

Caveats:

- The established victim is another concurrent fixture, not a demonstrated production session.
- Requires overlapping runs or another matching fixture under the same account.
- No runtime reproduction was performed.
- The established competing victim is another test fixture, not a production session.
- Requires overlapping matching fixtures under the same account.
- Highest is conservative because another invocation's processes are targeted.
- The demonstrated scope is test fixtures, not ordinary UUID-named product sessions.
- Concurrent execution was not reproduced.
- Requires overlapping invocations on the same host/account.
- No concurrent reproduction.
- Do not claim ordinary product sessions or arbitrary user processes are endangered by this fixed test marker.
- Independent completed supervisor adjudication limits victims to other test fixtures; no production/user-work loss
  demonstrated. Propagate the same-site downgrade to other across duplicate inputs.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_05:p1:F1`,
`gap_supervisor_state_sec_06:p1:F1`.

- `gap_supervisor_state_cor_05:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `gap_supervisor_state_sec_06:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

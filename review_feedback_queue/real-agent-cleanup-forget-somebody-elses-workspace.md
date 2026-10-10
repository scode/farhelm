# Real-agent cleanup can forget somebody else’s workspace

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed real-agent test can unregister another workspace.

## Details

F61 — **definite** — `e2e/tests/spawn.spec.ts:291–295`; `e2e/tests/spawn.spec.ts:293` — Real-agent cleanup can forget
somebody else’s workspace

The opted-in test uses the actual repository and a fixed spawned-workspace name. Once that name is assigned, a setup or
attachment failure can reach cleanup before the agent creates any workspace. Cleanup then forgets an existing
registration with that name without verifying its path or ownership. This removes metadata, not the workspace's physical
files. Use a unique explicit name and verify this run's registered path before forgetting it, preferably in an isolated
fixture repository.

## Evidence and triage context

- e2e/tests/spawn.spec.ts:217–228 gates this test on FARHELM_REAL_AGENT=1.
- e2e/tests/spawn.spec.ts:232 selects the actual repository.
- e2e/tests/spawn.spec.ts:249–250 assigns a scratch path with the fixed basename spawned-workspace.
- e2e/tests/spawn.spec.ts:251–261 performs fallible setup before asking the agent to create the workspace.
- e2e/tests/spawn.spec.ts:287–295 forgets that basename without checking creation success or the registered path.
- AGENTS.md:653–669 explicitly supports concurrent sibling jj workspaces and requires run-specific harness identities.
- SPEC_impl.md:3578–3580 explicitly describes the manually enabled real-Claude scenario.
- e2e/tests/spawn.spec.ts:232 selects the current repository.
- e2e/tests/spawn.spec.ts:249-250 assigns a unique scratch parent but the fixed basename spawned-workspace before any
  workspace creation.
- e2e/tests/spawn.spec.ts:263-266 asks Claude to create the workspace without an explicit unique workspace name.
- e2e/tests/spawn.spec.ts:287-295 forgets path.basename(workspace) whenever the path variable was assigned, including
  after earlier setup failure.
- e2e/tests/spawn.spec.ts:307 removes only the owned scratch tree; this does not establish ownership of the separately
  forgotten registration.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No matching Planned, BUGS, queue or ledger item found. FILTER.md expressly excludes wrong-target state-changing
  actions. The concurrent-workspace instructions reinforce the ownership boundary.
- SPEC.md:2152-2164 accepts deliberate same-account interference, not accidental mutation of unrelated user state by an
  acceptance test. FILTER.md:39 excludes actions applied to the wrong target.

Caveats:

- Requires FARHELM_REAL_AGENT=1 and a pre-existing same-name registration in the repository.
- Earlier cleanup errors can prevent reaching the forget command; they do not protect executions where that cleanup
  succeeds.
- Forgetting registration is established; deletion of the other workspace's physical files or irreversible loss of its
  contents is not.
- The fake-Claude launch problem does not refute this finding: attachment failure still enters the cleanup path.
- Highest priority is retained conservatively for wrong-target mutation of another worker's repository metadata.
- Requires opt-in execution and an existing registration with the fixed name.
- Earlier cleanup failures can prevent the forget command from being reached.
- The established consequence is loss of a workspace registration, not deletion of its physical files or demonstrated
  irreversible loss of commits.
- Downgraded from highest: the supplied evidence does not establish security compromise or data loss.
- Requires FARHELM_REAL_AGENT=1, a repository supporting the jj operation, a colliding workspace registration, and
  cleanup reaching the forget command.
- Forgetting removes registration; this finding does not claim recursive deletion of the unrelated workspace's files.
- The fake-Claude launch problem does not refute this finding because setup failure still enters cleanup.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_11_cor:p1:F1`,
`test_infrastructure_11_sec:p1:F1`.

- `test_infrastructure_11_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `test_infrastructure_11_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.

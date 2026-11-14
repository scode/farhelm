# Parity: Approvals and agent-spawned sessions

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Agent-spawned sessions, Agents and environments, One GUI at a time.

| Old spec                            | Tests | Covers                                                                         | Status         |
| ----------------------------------- | ----- | ------------------------------------------------------------------------------ | -------------- |
| `tests/approval-layout.spec.ts`     | 28    | explicit host add preserves edited fields and waits for its own answer receipt | not yet ported |
| `tests/agent-relay.spec.ts`         | 4     | agent relay: an agent clones its own session across hosts                      | not yet ported |
| `tests/spawn.spec.ts`               | 4     | a fake agent spawns children that appear without refreshing the observer       | not yet ported |
| `tests/session-view-claims.spec.ts` | 2     | navigation survives another client deleting a session with a Restart prompt    | not yet ported |

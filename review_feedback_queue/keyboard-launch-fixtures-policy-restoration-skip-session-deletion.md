# Keyboard-launch fixture’s policy restoration can skip session deletion

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Keyboard-launch teardown skips session deletion if policy restoration fails.

## Details

F262 — **definite** — `e2e/tests/sidebar.spec.ts:6029` — Keyboard-launch fixture’s policy restoration can skip session
deletion

The first teardown await restores and verifies policy. If it rejects, the deletion loop never runs despite already
knowing the launched session's identifier, leaving its processes in the shared stack. Attempt registered-session cleanup
independently of policy restoration success and preserve both failures, so one cleanup problem cannot suppress another
resource's release.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:6015-6020: the test waits for successful creation and registers the returned ID in created.
- e2e/tests/sidebar.spec.ts:6028-6030: policy restoration is awaited before the session-deletion loop; rejection
  prevents entry into that loop.
- e2e/tests/helpers/fleet.ts:321-332: the restoration helper can fail during verification after successfully restoring
  the setting.
- e2e/tests/helpers/fleet.ts:505-515: session stopping and deletion are independent requests that this failure path
  never attempts.
- e2e/playwright.config.ts:83-100,142-163 and e2e/start-stack.sh:242-265: the server stack is shared across tests and
  has a separate eventual teardown.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- {"basis": "TODO.md:255-261, Shared-checkout browser timeout and fixture cleanup", "comparison": "The TODO concerns
  restoring Git mappings after session-cleanup failure in another spec. It neither names nor covers policy-restoration
  failure suppressing session deletion here, and it is not a Planned item."}

Caveats:

- Requires a restoration failure after a session has been created and its ID recorded.
- No runtime reproduction was performed.
- The demonstrated retained resources are isolated test sessions and fake harness processes; user work loss is not
  established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p3:F4`.

- `test_infrastructure_10_cor:p3:F4`: confidence as filed: definite; suggested bucket as filed: other.

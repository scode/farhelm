# History fixture enables YOLO before installing its restoration guard

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

History-test setup can leak its changed confirmation policy.

## Details

F259 — **definite** — `e2e/tests/sidebar.spec.ts:4977` — History fixture enables YOLO before installing its restoration
guard

The fixture enables YOLO before entering the restoration guard. A committed change followed by failed readback returns
from setup without restoring the shared host's policy, altering later tests' confirmation assumptions. Enter the
restoration boundary before awaiting the enabling operation so even partially completed setup attempts restoration.

## Evidence and triage context

- e2e/tests/sidebar.spec.ts:4977-4978: the enabling await precedes try; rejection therefore bypasses the finally at
  5055-5058.
- e2e/tests/helpers/fleet.ts:321-332: the helper posts the setting, then separately calls listHosts and validates the
  returned value. listHosts at 298-301 can reject on request, status, or JSON failure.
- crates/farhelm-helm/src/store.rs:4905-4910: the policy update is committed independently of subsequent HTTP reads.
  crates/farhelm-helm/src/hosts.rs:538-540 also permits the POST response itself to fail while obtaining the post-commit
  host view.
- crates/farhelm-helm/src/yolo_guard.rs:61-69: later launches read the stored flag and allow unconfirmed YOLO when it is
  true.
- e2e/playwright.config.ts:83-100,142-163: tests use one shared helm; e2e/tests/helpers/evidence.ts:31-65 provides no
  policy restoration.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires failure after the write commits; occurrence was not reproduced.
- The helm uses fresh private state: e2e/start-stack.sh:123,431-435. Structured Codex launches resolve to the fake
  harness installed at 211-234. No user-installation security or work-loss consequence is established.
- The altered setting lasts until another explicit reset or stack teardown, not necessarily for the entire remaining
  run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_10_cor:p3:F1`.

- `test_infrastructure_10_cor:p3:F1`: confidence as filed: definite; suggested bucket as filed: other.

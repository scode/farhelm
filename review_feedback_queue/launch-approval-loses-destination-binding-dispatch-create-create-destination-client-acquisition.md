# Launch approval loses its destination binding before dispatch — Create destination-client acquisition

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An approved new session could launch on a replacement destination.

## Details

F4 — **possible** — `crates/farhelm-helm/src/agent_requests.rs:1565` — Launch approval loses its destination binding
before dispatch — Create destination-client acquisition

For an explicitly selected host, the helm checks the destination installation after approval, then separately acquires
the current connection when creating the session. The check does not bind that later connection to the approved
installation. A completed replacement in this gap could send the launch to a different machine. Template-selected
creates have an additional identity check; this finding concerns explicit-host creation. Carry the approved destination
identity and connection into dispatch and validate that binding.

## Evidence and triage context

- crates/farhelm-helm/src/agent_requests.rs:1455 captures the incarnation; :1465 compares it after approval; :1474
  performs another policy check and returns only ().
- Create site: crates/farhelm-helm/src/agent_requests.rs:1287 calls dispatch_agent_create after approval; :1565 acquires
  the then-current client. At :1568 the additional identity check applies only when template_identity exists.
- Clone site: crates/farhelm-helm/src/agent_requests.rs:2063 awaits source-owner revalidation, then :2071 independently
  acquires the destination's current client.
- crates/farhelm-helm/src/sessions.rs:1204 obtains current manager status; :1590 checks launch policy; :1618 sends
  through the supplied client. None compares that client with the approval's earlier incarnation.
- crates/farhelm-helm/src/agent_requests.rs:1465 checks incarnation; :1474 returns only the policy-check result.
- Explicit-create site: crates/farhelm-helm/src/agent_requests.rs:1565 selects the current destination client after
  approve_launch returned.
- Clone site: crates/farhelm-helm/src/agent_requests.rs:2063 awaits a source lookup before :2071 selects its destination
  client.
- crates/farhelm-helm/src/sessions.rs:1590 rechecks policy, then :1618 dispatches without an approved-target comparison.
- crates/farhelm-helm/src/agent_requests.rs:1539 makes the template identity guard conditional on a template-selected
  identity.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- TRIAGE_OUTCOMES.md:6973, template-successor.md, covers template-selected installation identity. Its :6982 expressly
  excludes explicit --host. SPEC.md:1748 accepts host reassignment between separate keyed attempts, not replacement
  during one approved dispatch.
- TRIAGE_OUTCOMES.md:6973 addresses only template-selected destinations; :6982 explicitly excludes explicit-host
  creates. SPEC.md:1748 addresses separate retries, not this within-request race.

Caveats:

- Requires destination replacement to complete between the last incarnation check and client acquisition. A disconnected
  replacement or a refused destination policy fails safely. Create and clone have distinct dispatch sites. No runtime
  reproduction.
- Requires a completed retarget/adoption between validation and acquisition. Template-selected creates retain their
  additional identity protection. No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_01_cor:p1:F4`,
`helm_state_provisioning_01_sec:p1:F4`.

- `helm_state_provisioning_01_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_01_sec:p1:F4`: confidence as filed: possible; suggested bucket as filed: highest.

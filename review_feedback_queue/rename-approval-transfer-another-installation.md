# Rename approval can transfer to another installation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A rename approval could change a replacement host's session title.

## Details

F3 — **possible** — `crates/farhelm-helm/src/agent_requests.rs:344` — Rename approval can transfer to another
installation

The helm discards the target connection before asking for rename approval and resolves ownership again afterward. Its
expected-title check protects against title changes, but cannot distinguish two installations with matching titles. If
ownership changes to a reachable successor with the expected title, the old approval could authorize a durable rename
there. Keep the original owner and connection across approval, while retaining the title check.

## Evidence and triage context

- crates/farhelm-helm/src/agent_requests.rs:336 discards the successful route; :339 builds the card; :344 dispatches
  after approval using only target ID and title precondition.
- crates/farhelm-helm/src/sessions.rs:2377 resolves the owner again; :2378 sends Rename with expected_title, without an
  earlier owner claim.
- crates/farhelm-helm/src/agent_requests.rs:1492 rechecks the requesting connection, not the target connection.
- crates/farhelm-helm/src/agent_requests.rs:336 discards the route; :339 constructs the action; :344 calls
  do_rename_session after approval.
- crates/farhelm-helm/src/sessions.rs:2377 takes a new route and :2378 forwards only the title precondition.
- crates/farhelm-helm/src/agent_requests.rs:1492 validates the requesting origin rather than the earlier target.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:1639 requires atomic title comparison, which this path supplies; that requirement does not accept redirecting
  approval to another installation.
- SPEC.md:1639 covers stale-title protection, not stale target ownership. No exact coverage found in Planned, BUGS,
  queue, or ledger.

Caveats:

- Requires sequential ownership change and a matching expected title. This finding demonstrates unauthorized durable
  mutation, not independently process loss. No runtime reproduction.
- Requires changed ownership and matching expected title. Does not independently establish process loss. No runtime
  reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_01_cor:p1:F3`,
`helm_state_provisioning_01_sec:p1:F2`.

- `helm_state_provisioning_01_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_01_sec:p1:F2`: confidence as filed: possible; suggested bucket as filed: highest.

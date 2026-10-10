# Stop approval does not retain the target’s owner

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A stop approval could apply to a replacement host instead of the host shown.

## Details

F1 — **possible** — `crates/farhelm-helm/src/agent_requests.rs:365`; `crates/farhelm-helm/src/agent_requests.rs:366` —
Stop approval does not retain the target’s owner

The helm checks which host owns the target session before asking for approval, then looks up the owner again when
sending Stop. It retains no binding to the host shown on the card. If ownership changes to a sole reachable successor
while the requester remains connected, approval could stop that successor's work. Checks for simultaneous ownership
conflicts do not cover this sequence. Retain the approved owner and connection, validate them after approval, and send
through that connection.

## Evidence and triage context

- crates/farhelm-helm/src/agent_requests.rs:359 discards the successful route; :362 builds the card; :366 calls
  do_stop_session after approval.
- crates/farhelm-helm/src/agent_requests.rs:1491 checks only the requesting origin after the approval wait.
- crates/farhelm-helm/src/sessions.rs:2041 resolves ownership again and :2042 dispatches Stop through that newly
  selected client.
- crates/farhelm-helm/src/sessions.rs:758 checks current contested claims; :775 resolves current live ownership. Neither
  compares against the card's earlier owner.
- crates/farhelm-helm/src/store.rs:5315 purges predecessor session-cache rows during adoption, allowing an earlier
  ownership record to disappear.
- crates/farhelm-helm/src/agent_requests.rs:359 discards the initial route, :363 describes the target, and :366 calls
  the freshly routing stop helper.
- crates/farhelm-helm/src/sessions.rs:2041 resolves the current owner before sending Stop.
- crates/farhelm-helm/src/approvals.rs:368 monitors requester liveness; crates/farhelm-helm/src/agent_requests.rs:1492
  likewise checks only the origin.
- crates/farhelm-helm/src/sessions.rs:758 and :775 protect against current competing ownership, without retaining the
  approval's original owner.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2235 permits some multi-GUI races, but :2243 explicitly preserves correctness for concurrent agent operations.
  No matching Planned item, BUGS entry, queue item, or ledger disposition was found.
- SPEC.md:2166 treats remote supervisors as untrusted; :2178 requires user authorization for cross-host actions. These
  support the finding rather than covering it.

Caveats:

- Requires a live requester distinct from a replaced target and another routable session with the same ID. Simultaneous
  conflicting ownership is refused. No runtime reproduction.
- Sequential ownership transition must leave a sole routable successor while the requesting connection survives.
  Simultaneous claims fail closed. No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_01_cor:p1:F1`,
`helm_state_provisioning_01_sec:p1:F1`.

- `helm_state_provisioning_01_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_01_sec:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.

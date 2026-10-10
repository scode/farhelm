# A failed registry reload can reconnect the previous destination under the edited host

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A failed host reload could reconnect the old destination under the edited registration.

## Details

F21 — **possible** — `crates/farhelm-helm/src/manager.rs:3288` — A failed registry reload can reconnect the previous
destination under the edited host

After retargeting, the connection worker can pick up the new connection generation but keep its old destination when a
registry reload fails. With both the retained row and peer lacking an installation identity, the old machine can then
publish a connection under the edited host. Session creation or input could reach the previous destination. The required
storage failure has not been reproduced. Fail this attempt on reload failure, or validate the dialed configuration
before publishing the connection.

## Evidence and triage context

- crates/farhelm-helm/src/hosts.rs:779–812: retarget commits, successfully reconciles, and can return success.
- crates/farhelm-helm/src/manager.rs:1582–1621: reconciliation updates the published row, withdraws the old connection,
  changes incarnation, and nudges the actor.
- crates/farhelm-helm/src/manager.rs:3281–3288,4319–4330: the actor samples the new incarnation but retains its old row
  when reload fails.
- crates/farhelm-helm/src/manager.rs:3581–3582: transport selection uses that retained row.
- crates/farhelm-helm/src/manager.rs:3664–3721: a row and peer lacking identity can connect without the identified-peer
  configuration comparison.
- crates/farhelm-helm/src/store.rs:5177–5183: identified-peer admission compares current configuration with the dialed
  configuration.
- crates/farhelm-helm/src/manager.rs:3867–3877,4450: publication checks incarnation, not dialed destination.
- crates/farhelm-helm/src/sessions.rs:677–702: session operations use the published client.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"matched": null, "possible": ["SPEC_impl.md:2687–2691 explicitly accepts a live actor retaining its old row when the
  registry is unreadable. Whether that also accepts publishing a newly admitted old-destination connection under
  successfully reconciled metadata remains ambiguous.", "SPEC.md:2021–2031 accepts filesystem-caused failures and halted
  progress, not expressly wrong-destination execution.", "TRIAGE_OUTCOMES.md:1025–1045 concerns pending-nudge
  publication; this path follows consumption of the retarget nudge.", "TRIAGE_OUTCOMES.md:5306–5326 concerns
  republishing a withdrawn client during refresh, not a new dial from a retained stale row.",
  "review_feedback_queue/FILTER.md excludes wrong-machine operations from its rare-race filtering boundary."]}

Caveats:

- Requires an identity-less retained row and peer, plus a registry-read failure at the specified point.
- The necessary storage failure was not reproduced.
- Identified peers encounter the additional configuration comparison.
- Requires a retained identity-less row, an identity-less peer, and a registry-read failure at the identified reload.
- That failure was not reproduced.
- Identified peers encounter the additional dialed-configuration check.
- Specification scope clarification is necessary before presenting this as an unconditional defect.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_03:p1:F2`.

- `gap_helm_connections_sec_03:p1:F2`: confidence as filed: possible; the required transient registry-read failure was
  not reproduced; suggested bucket as filed: highest.

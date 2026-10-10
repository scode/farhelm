# Logging test does not establish that a refresh completed

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The quiet-logging test could inspect logs before any refresh completes.

## Details

F172 — **possible** — `crates/farhelm-helm/src/manager.rs:10515` — Logging test does not establish that a refresh
completed

Connected status does not prove a steady-state refresh was published, and advancing several timer intervals together
does not force that many completed cycles. Scheduling can postpone refresh completion until cancellation or log
inspection, hiding an incorrectly noisy refresh logger. That passing schedule was not exercised. Observe completed
refresh publications before checking silence, advancing intervals separately when multiple cycles matter.
Destination-change assertions remain useful.

## Evidence and triage context

- crates/farhelm-helm/src/manager.rs:10507–10516: the test observes Connected, advances five intervals together, and
  yields.
- crates/farhelm-helm/src/manager.rs:3867–3877: Connected publication initially carries Pending refresh health.
- crates/farhelm-helm/src/manager.rs:3911–3916: retargeting can cancel the pending refresh.
- crates/farhelm-helm/src/manager.rs:3998–4000: cadence waits begin after the preceding refresh.
- crates/farhelm-helm/src/manager.rs:10518–10556: retarget precedes log inspection, which expects exactly two connection
  phase transitions.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": "SPEC_impl.md:2928–2937 requires current-destination transition logs and silence on
  unchanged-phase republishing. It states the behavior under test but does not establish that this fixture observes such
  a republish."}

Caveats:

- The destination-change assertions remain useful.
- No production logging defect established.
- No runtime mutation experiment demonstrating the suspect schedule.
- The destination and span-context assertions remain useful.
- No production logging defect is established.
- The faulty-logger passing schedule was not demonstrated at runtime.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_04:p1:F2`.

- `gap_helm_connections_sec_04:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.

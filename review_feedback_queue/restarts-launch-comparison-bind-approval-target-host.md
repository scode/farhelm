# Restart’s launch comparison does not bind approval to the target host

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A restart approval could interrupt matching work on a replacement host.

## Details

F2 — **possible** — `crates/farhelm-helm/src/agent_requests.rs:425` — Restart’s launch comparison does not bind approval
to the target host

Restart checks that the session's launch details still match those approved, but does not retain the installation that
owned the session. A successor host with the same session identifier, matching launch details, and a usable resume offer
can pass both comparisons. In that sequence, another machine's work could be stopped and resumed under the earlier
approval. Preserve and validate the approved owner and connection alongside the launch comparison.

## Evidence and triage context

- crates/farhelm-helm/src/agent_requests.rs:377 discards the target route; :400 constructs the card; :412 compares only
  cached launch contents.
- crates/farhelm-helm/src/agent_requests.rs:425 passes the target ID and expected launch to do_restart_session without
  an expected owner.
- crates/farhelm-helm/src/sessions.rs:2325 routes anew; :2332 sends the restart through that client.
- crates/farhelm-supervisor/src/service/core.rs:10058 compares expected_launch with the destination session's launch. At
  :10244 stop_if_running allows a working agent to proceed to stop_live_agent at :10253.
- crates/farhelm-helm/src/agent_requests.rs:377 discards the initial route; :412 compares cached launch contents; :425
  dispatches by session ID.
- crates/farhelm-helm/src/sessions.rs:2325 obtains a fresh owner/client; :2332 forwards expected_launch.
- crates/farhelm-supervisor/src/service/core.rs:10058 compares launch contents; :10244 and :10253 permit stopping the
  successor's live agent when consent is supplied.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC_impl.md:4057 describes expected-launch protection, but does not accept changed target ownership. SPEC.md:2243
  preserves agent/GUI concurrency correctness.
- SPEC_impl.md:4057 protects the shown launch but does not accept carrying approval to a different host.

Caveats:

- Requires sequential ownership change, matching launch contents, and a valid resumable successor session. Changed
  launch contents already cause refusal. No runtime reproduction.
- A sole successor owner must have the same launch and a usable resume offer. A launch mismatch refuses safely. No
  runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_01_cor:p1:F2`,
`helm_state_provisioning_01_sec:p1:F3`.

- `helm_state_provisioning_01_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
- `helm_state_provisioning_01_sec:p1:F3`: confidence as filed: possible; suggested bucket as filed: highest.

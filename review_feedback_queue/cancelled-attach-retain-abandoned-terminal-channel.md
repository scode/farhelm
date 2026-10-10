# Cancelled attach can retain an abandoned terminal channel

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Cancellation before attach transmission could retain local terminal-routing metadata.

## Details

F276 — **possible** — `crates/farhelm-helm/src/client.rs:3229`; `crates/farhelm-helm/src/client.rs:3230`;
`crates/farhelm-helm/src/client.rs:3213` — Cancelled attach can retain an abandoned terminal channel

Attachment registers routing state before waiting for writer capacity. If cancellation occurs before enqueue, no drop
guard removes that unsent registration, which can remain until connection retirement. Ordinary backpressure exceeding
the revocation grace while the connection survives was not reproduced, nor was accumulation measured. Give registration
cancellation-safe ownership, preserving pre-dispatch registration while distinguishing unsent cleanup from transmitted
attachment cleanup.

## Evidence and triage context

- crates/farhelm-helm/src/client.rs:3229-3254: inserts TerminalHandle before awaiting request; removal occurs only after
  that await returns an error.
- crates/farhelm-helm/src/client.rs:2500-2515: request first awaits writer reservation; Attach has not been enqueued
  during that wait.
- crates/farhelm-helm/src/terminal.rs:514-528: credential revocation waits only WS_TEARDOWN_GRACE for attachment, then
  returns and drops the pending future.
- crates/farhelm-helm/src/client.rs:1997-2023: closed-receiver cleanup requires an incoming event for the channel; an
  unsent Attach provides the supervisor no channel to answer.
- crates/farhelm-helm/src/client.rs:1574-1579: connection failure drains terminal entries.
- crates/farhelm-helm/src/client.rs:3229-3254: registration precedes the request await; error cleanup follows it.
- crates/farhelm-helm/src/client.rs:2500-2503: writer reservation is an await before transmission.
- crates/farhelm-helm/src/terminal.rs:518-528: attachment is abandoned after the credential-revocation grace expires.
- crates/farhelm-helm/src/client.rs:2013-2022: closed-receiver cleanup requires a subsequent terminal event.
- crates/farhelm-helm/src/client.rs:1574-1579: retirement/failure eventually drains the local entry.
- client.rs:3229-3254 registers before request; request waits for capacity at 2500-2503; terminal.rs:518-528 can abandon
  it. client.rs:2013-2022 requires a later event, unavailable for an unsent channel. SPEC.md:1895-1899 accepts admitted
  work across rotation but does not accept accumulating abandoned local registrations.
- client.rs:3229-3254 registers before request, whose capacity await is at 2500-2503; terminal.rs:523-528 can drop it.
  Incoming-event cleanup at client.rs:2013-2022 cannot clean an unsent channel. Retain only that possible remainder,
  matching hc_data:p1:F1.
- client.rs:3229-3254 inserts before the cancellable request; 2500-2503 can wait before sending; terminal.rs:518-528
  abandons admission after the grace. Cleanup at client.rs:2013-2022 requires an event for the abandoned channel.
  SPEC_impl.md:64-69 concerns selective peer nonresponse, not this unsent request.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:58-69 and TRIAGE_OUTCOMES.md:740-755 concern retained metadata and selectively unanswered dispatched
  requests. They do not cover repeated local registration abandoned before transmission.
- TRIAGE_OUTCOMES.md:835-852 concerns cancellation of an upstream Detach notification after local removal, not
  cancellation before Attach admission.
- SPEC_impl.md:64-69 excludes selectively unanswered requests, not local registration whose request never reached the
  supervisor.
- TRIAGE_OUTCOMES.md:740-755 has the same post-enqueue limitation.

Caveats:

- No runtime reproduction or accumulation measurement.
- Requires writer admission to remain blocked beyond the revocation grace while the connection subsequently survives.
- No security, wrong-target operation, or work loss established.
- Ordinary backpressure lasting beyond the grace while the connection survives remains an unverified premise.
- No runtime reproduction, sustained-growth measurement, or work loss established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_data:p1:F1`, `hc_systems:p1:F1`, `hc_general:p1:C3`,
`hc_lifecycle:p1:C2`, `hc_edges:p1:C7`.

- `hc_data:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `hc_systems:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `hc_general:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `hc_lifecycle:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `hc_edges:p1:C7`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.

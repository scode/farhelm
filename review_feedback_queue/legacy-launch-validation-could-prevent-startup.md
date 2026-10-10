# Legacy launch validation could prevent startup

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A historical launch shape could make the whole supervisor refuse startup.

## Details

F298 — **possible** — `crates/farhelm-supervisor/src/store.rs:2653` — Legacy launch validation could prevent startup

Decoding rejects an integrated legacy launch lacking a conversation-bearing resume template, and that row failure
propagates through the full session listing into supervisor construction. Whether a released historical writer could
produce an otherwise valid affected record remains unresolved. Establish that historical premise and reconcile migration
with the promised usability of legacy sessions, preserving the row while refusing unusable Resume rather than
unnecessarily blocking all startup.

## Evidence and triage context

- store.rs:2653–2661 rejects an integrated legacy launch without a conversation-bearing resume template.
- store.rs:2621–2626 propagates that failure from row decoding; :5532 collects all decoded rows into one fallible
  result; service/core.rs:5562 and :5331 propagate it through supervisor construction.
- store.rs:2300–2321 migrates decoded historical fields through SessionLaunch::from_pre_launch_kinds;
  crates/farhelm-proto/src/session_launch.rs:207–232 preserves legacy fields, including fallback from invalid structured
  launches.
- SPEC.md:1561–1572 promises legacy sessions remain usable while restricting Restart for unusable resume commands;
  SPEC_impl.md:3755–3756 says a legacy placeholder-free command is retained but never run.
- store.rs:11094–11118 deliberately constructs rejected records; it does not establish whether a released historical
  writer could create them. No matching accepted disposition was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_09:p1:C3`.

- `gap_supervisor_state_sec_09:p1:C3`: confidence as filed: possible; suggested bucket as filed: not separately tagged
  in candidate list.

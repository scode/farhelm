# Template discovery fails permanently once its reply exceeds 8 MiB

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A large valid template catalog permanently disables agent discovery.

## Details

F120 — **definite** — `crates/farhelm-helm/src/agent_requests.rs:1937` — Template discovery fails permanently once its
reply exceeds 8 MiB

Template discovery encodes the entire catalog into one reply without pagination or a byte budget. Individually valid
templates can together exceed the 8 MiB frame limit, causing the transport to return an error instead of any listing.
Retrying or reconnecting cannot recover until the catalog is reduced. Provide bounded pagination or another explicit
bounded discovery protocol that can retrieve the remaining templates.

## Evidence and triage context

- crates/farhelm-proto/src/launcher.rs:307 caps each template's serialized fields at 64 KiB; :336 checks that
  per-template size.
- crates/farhelm-helm/src/templates.rs:85 validates each write; crates/farhelm-helm/src/store.rs:3516 inserts without an
  aggregate catalog limit.
- crates/farhelm-helm/src/store.rs:3479 selects the entire catalog; crates/farhelm-helm/src/agent_requests.rs:1937
  collects every projected template into one reply.
- crates/farhelm-proto/src/lib.rs:1950 removes command texts but retains session-name fields; :388 sets MAX_FRAME_LEN to
  8 MiB.
- crates/farhelm-helm/src/client.rs:1157 detects an oversized frame and :1175 replaces the successful listing with
  Internal.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/template-catalog-error.md:14 concerns a GUI fetch error displayed as an empty catalog. This
  finding instead concerns an agent-protocol size failure that persists across retries. Trigger, consequence, and
  editable site differ.

Caveats:

- An unusually large catalog is required. For example, 140 templates containing 60 KiB names exceed 8 MiB while each
  passes storage validation. 'Permanently' means until catalog modification, not irreversible data loss. No runtime
  reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_01_cor:p2:F1`.

- `helm_state_provisioning_01_cor:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.

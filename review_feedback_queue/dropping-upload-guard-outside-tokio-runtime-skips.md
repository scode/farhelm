# Dropping an upload guard outside a Tokio runtime skips cleanup

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Dropping an upload owner outside runtime context could skip cleanup.

## Details

F324 — **possible** — `crates/farhelm-helm/src/client.rs:3786` — Dropping an upload guard outside a Tokio runtime skips
cleanup

The public upload guard retains its client but returns without cleanup when no current async runtime is available. If
the guard leaves that context while its connection remains live elsewhere, upload cleanup could be omitted. The
inspected shipped caller stays asynchronous and no product trigger or work loss was confirmed. Give cleanup ownership
access to its originating runtime or a defined fallback independent of the dropping context.

## Evidence and triage context

- client.rs:3780-3787 retains the client but returns without cleanup when Handle::try_current fails. UploadGuard is
  public at 3480 and begin_upload returns it at 3399-3404. The inspected shipped caller, uploads.rs:186-349, stays
  within async handling; no shipped out-of-runtime drop was established.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No current product trigger or work loss confirmed.
- Requires the guard to leave its runtime context while its connection remains live elsewhere.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_lifecycle:p2:C3`.

- `hc_lifecycle:p2:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.

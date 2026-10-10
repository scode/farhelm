# Shutdown leaves retained supervisor clients active

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Manager shutdown could leave retained clients active in a surviving runtime.

## Details

F278 — **possible** — `crates/farhelm-helm/src/manager.rs:2914`; `crates/farhelm-helm/src/manager.rs:2908`;
`crates/farhelm-helm/src/manager.rs:2907` — Shutdown leaves retained supervisor clients active

Shutdown aborts connection workers without explicitly retiring published clients. A retained client can continue
activity if its runtime remains alive, leaving the connection-shutdown contract dependent on later runtime or process
destruction. Normal standalone and desktop shutdown have no established persistent failure. Withdraw and retire
published clients during shutdown, with a focused transport-closure check using a retained clone.

## Evidence and triage context

- crates/farhelm-helm/src/manager.rs:2885-2915: promises to drop every connection but only marks shutdown, drains
  actors, and aborts their tasks.
- crates/farhelm-helm/src/manager.rs:2856-2872: host removal explicitly withdraws and retires the client, demonstrating
  the missing operation.
- crates/farhelm-helm/src/client.rs:918-936: destructor cleanup runs only when the final owning handle drops.
- crates/farhelm-helm/src/client.rs:1677-1679: explicit retirement aborts answer work and signals transport shutdown
  despite retained handles.
- crates/farhelm-helm/src/lib.rs:1993-2002: serving invokes manager shutdown immediately before returning.
- crates/farhelm-ui/src/desktop.rs:448-466: the shipped embedded caller destroys its dedicated runtime after
  run_embedded returns.
- manager.rs:2907-2915 only aborts actors; client.rs:925-935 cleans on final owner drop. desktop.rs:448-466 destroys its
  runtime, but SPEC_impl.md:1310-1313 requires withdrawn-client retirement. TRIAGE_OUTCOMES.md:550-563 covers host
  removal only.
- manager.rs:2885-2915 promises connection shutdown but only aborts actors; client.rs:925-935 needs final-owner drop,
  whereas 1677-1679 explicitly retires retained clients. desktop.rs:448-466 bounds shipped desktop impact.
  TRIAGE_OUTCOMES.md:550-563 is a different removal path.
- manager.rs:2907-2915 aborts actors without calling retire; client.rs:918-935 depends on final-owner drop.
  lib.rs:2001-2002 returns after shutdown, and desktop.rs:448-466 destroys the runtime. SPEC_impl.md:1310-1313 and
  TRIAGE_OUTCOMES.md:550-563 do not accept omission at this separate site.
- manager.rs:2907-2915 only aborts actors, unlike stop_actor at 2856-2872. client.rs:925-935 requires final-owner drop.
  SPEC_impl.md:23-40 protects already accepted mutations, while 1310-1313 separately requires withdrawn transports to
  close despite retained handles.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:550-563 fixes removed-host paths, not global manager shutdown.
- SPEC_impl.md:1306-1313 requires explicit retirement of withdrawn connections; it supports the concern rather than
  accepting the omission.
- SPEC_impl.md:2484-2486 specifies terminal reconciliation behavior, not permission to retain client transports.

Caveats:

- No persistent failure established in normal standalone or desktop shutdown.
- A surviving runtime and retained client are necessary for continuing activity.
- No unauthorized mutation or work loss established; the agent-request origin check also rejects a removed manager
  entry.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `hc_lifecycle:p1:F1`, `hc_data:p1:C2`, `hc_general:p1:C1`,
`hc_systems:p1:C3`, `hc_edges:p1:C6`.

- `hc_lifecycle:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
- `hc_data:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `hc_general:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `hc_systems:p1:C3`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `hc_edges:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.

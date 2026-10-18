# Create can accept another host's session ID

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A faulty or hostile host can answer Create with another host's existing session ID. Farhelm can report success and then
open the other session, sending subsequent terminal input or session actions there.

## Details

`crates/farhelm-helm/src/sessions.rs:1405–1440` propagates only the durable store's `SessionOwnerAmbiguous` error from
best-effort recording. Two paths avoid that error:

- An identity-less host's in-memory recording path (`manager.rs:1981–2125`) does not check foreign cached ownership.
  Disconnecting afterward clears its live claim.
- A delivered Create reply survives transport closure (`client.rs:1822–1824,2576–2607`). Disconnect changes the
  connection incarnation (`manager.rs:4559–4592`), causing recording to fail its freshness check before reaching the
  store. That generic failure is logged and swallowed.

The store's foreign-owner check requires a cached row with the returned ID and a different host (`store.rs:6429–6447`).
Neither bypass reaches that check or overwrites the honest owner's row. Once the creating host's live claim disappears,
lookup can select that honest owner.

Create completion selects the returned ID (`crates/farhelm-ui/src/list/view.rs:3943–4008`, `lib.rs:1782–1785`). The
detail view fetches by ID without comparing the creating host (`session_view.rs:943–1022,1137`), and terminal attachment
routes by ID alone (`crates/farhelm-helm/src/terminal.rs:182–204`). The honest host's metadata can become visible; this
does not claim permanent concealment or exfiltration to the creating host.

Clone shares the acceptance boundary. Fresh reconciliation shares the stale-claim case; managed-checkout creation
requires an identity, so the identity-less variant is not claimed there. Replace rejects replay of its source ID and
deletes through the original connection: this finding does not redirect that Delete to the foreign session.

Check foreign ownership independently of cache backend and cache-write freshness before returning actionable success. A
fake-peer regression should keep host A connected with cached session S and exercise both bypasses from B, requiring a
conflict instead of selecting S.

Suggested bucket: **highest**, for cross-host wrong-target input and lifecycle actions. Confidence: **confirmed static
control flow**; no runtime reproduction. A faulty or hostile peer must know the foreign ID. No registry migration,
HostId reuse or waiting approval is required.

Possible cover: `TRIAGE_OUTCOMES.md`, `create-reply-foreign-id-misroutes.md:1744–1771`, records a completed fix for
propagating the durable collision error. These paths bypass that check. SPEC_impl.md:3246–3262 expressly requires a
conflicting Create reply to be refused. The earlier item's `other` assessment relied on agents being allowed to stop
other hosts without approval; current SPEC.md:2877–2907 requires user authority for those actions. This changed premise
motivates the conservative bucket; no new runtime harm was demonstrated. No exact Planned, known-bug or filter coverage
was found for the remainder.

Proposed drop: none. Provenance: persistent Astra/high review, `helm-session-operations-1/f1`, including later-turn
tracing of the GUI and mutation callers.

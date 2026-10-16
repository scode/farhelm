# Checkout approval can cross hosts after registry recovery

Reviewed commit: 88fbdb474f3f84104199e7dfed1e2d6aa1ca6d06

## TLDR

After recovering Farhelm with a fresh host registry, a different remote host may be able to finish a checkout creation
approved for an earlier host, without obtaining its own approval.

## Details

Suggested bucket: `highest`. Confidence: likely. Key equality and the approval-bypassing recovery path are established
statically; recovery with a fresh registry and a surviving eligible reservation was not exercised end to end. Source:
separate adversarial trailing reviewer, with an independent fresh-context reviewer agreeing to retain it. Proposed drop:
none.

`crates/farhelm-helm/src/agent_requests.rs:1163–1182` binds the retry key to numeric `HostId`, the supplied requesting
session ID and the raw retry key. `store.rs:478–489,1982–1983` gives host rows SQLite AUTOINCREMENT identities, which
avoid reuse within one database but do not distinguish independently created registries.

Consider an approved managed create from host A, registry ID 2, targeting supervisor T. Stop that helm, recover with a
fresh registry, register different host B as ID 2 and reconnect T. If B repeats A's supplied session ID, retry key and
exact request, the scoped key and request identity match. The request digest has no helm namespace or authenticated
requester identity (`agent_requests.rs:859–939`); supplied session IDs reach it through `client.rs:1960–1978`.
Reconciliation precedes approval (`agent_requests.rs:1381–1406`), and the target's permanent reservation can select
`Resolution::Relaunch` (`crates/farhelm-supervisor/src/service/core.rs:7082–7083,7198–7224,7260–7281,7320`).

This requires knowledge or a guess of the original tuple and an eligible surviving reservation. It does not allow
arbitrary new launch contents. Verify with isolated sequential helms and one retained target supervisor, establishing
the reservation's relaunch premise before the second request. A durable namespace for each helm database, included in
the retry scope, would distinguish fresh registries while preserving existing-row retargeting.

Possible cover: the completed `checkout-recovery-requester-not-bound-to-host.md` ledger outcome fixes collisions between
rows in one registry, not independent registries. SPEC.md, “Topology,” explicitly supports recovery after losing the
registry; its concurrent-helm prohibition does not cover this sequential case. No exact Planned item, BUGS entry, queue
item or filter acceptance was found.

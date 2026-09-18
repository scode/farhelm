# Host list briefly pairs a new identity with its resolved mismatch

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Immediately after adopting a host's identity, one refresh of the host list can briefly show the new identity alongside
the mismatch warning it just resolved.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

`host_views` (hosts.rs:305-340) joins an awaited registry read (`list_hosts`) with a LATER snapshot read
(`manager.snapshots()`), so an adopt commit landing between the two yields one transient read pairing the NEW identity
(registry) with the STALE `IdentityMismatch` (snapshot) — the exact "adopt what was just adopted" render adopt's own
docs forbid ("must not find the adopted identity sitting beside the mismatch it just resolved", manager.rs:2322-2324 — a
guarantee that covers bump-woken re-readers, not a reader already mid-read when adopt commits). Microsecond window,
self-heals on the next read, no cheap fix (would need a lock spanning store+map). Possible, low.

No fix proposed beyond documenting the transient (or snapshotting both under one critical section if a cheap one
emerges).

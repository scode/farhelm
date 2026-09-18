# Ambiguous session-owner check compares only the first claimant

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When two hosts both claim the same session, an action aimed at one machine (like stopping a session) can silently land
on the other machine instead of being refused as ambiguous — depending only on the numeric ordering of the host ids.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Reviewer verified reachability; coordinator
confirmed it.

`resolve_owner` (`crates/farhelm-helm/src/sessions.rs:475`) promises to fail closed "where two hosts claim one id ...
picking one would mean a stop aimed at one machine landing on another" (sessions.rs:467-472), but the check
(sessions.rs:485-498) compares the cached owner against `contested.first()` only.

Reachability: contested entries are per-host published refresh state (`manager.rs` `ActorStatus::contested`); a mutation
seed (`remember_session`, manager.rs:1798) writes the cache WITHOUT clearing contested; only a new refresh, a same-host
delete (manager.rs:2037-2047), or adoption clears it. So: X (id 1) and Y (id 2) both contested on session I, X seeds I
from a mutation reply → claimants [1,2], owner 1, `first() == owner` → routed to X while Y's last refresh still shows a
live same-id session. With any other claimant composition the same code refuses — the outcome depends on id ordering.
The pinned test covers only the single-claimant case.

Suggested fix: refuse `SessionOwnerAmbiguous` when ANY claimant differs from the cached owner
(`contested.iter().find(|c| **c != owner)`); a sole self-contest still routes.

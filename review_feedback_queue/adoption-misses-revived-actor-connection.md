# Adoption misses a revived actor's connection

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

Adoption can report success while the host entry briefly retains a connection to its previous installation. A new launch
could select that connection before its reconnect request is handled. The necessary actor failure, endpoint change and
scheduling have not been observed.

## Details

`crates/farhelm-helm/src/manager.rs:2472–2486` captures a connection worker's status cell before awaiting durable
adoption (`2498–2501`). Retry can revive a worker that fails meanwhile: `spawn_actor` creates a different status cell
(`1719–1732`) and `revive` replaces the map entry (`2711–2772`). If that replacement accepts the predecessor identity
before adoption commits, adoption subsequently invalidates only its retained, detached cell (`2542–2559`). Publication
guards compare the replacement's own cell (`4063,4157,4635–4647`), so invalidating the old cell does not fence it.

The retry after adoption merely sends a nudge to a live worker (`2670–2685`); it neither withdraws the current map
entry's client synchronously nor waits for consumption. Routing reads that current entry (`1919–1931`), and host-scoped
create takes its client without comparing the durable installation identity
(`sessions.rs:1232–1264,1467–1497,1514–1517, 1678–1695`). A fresh request can therefore conditionally choose the
superseded installation after adoption succeeds.

The window ends when the worker handles the nudge and withdraws (`manager.rs:4097–4100,4215–4218,4259–4266`), or an
identity-bound refresh fails. Store identity checks reject predecessor cache writes after adoption
(`store.rs:6898–6914`), and session-specific operations normally fail after the cache purge (`sessions.rs:840–855`). No
permanent stale routing, durable cache corruption, successful post-purge Stop/Delete, or actual lost work was
established. Consider serializing adoption with replacement, or invalidating the actual current entry after commit while
preserving retirement behavior.

Suggested bucket: **provisional highest**, for conditional wrong-install execution. Confidence: **likely structural
race**. A natural worker panic, revival before the adoption write, the endpoint again answering as the predecessor, and
an operation landing before nudge handling are unverified premises. Source inspection only; no reproduction was
performed or requested after the user's stop.

Possible cover: `SPEC.md:217–226` accepts in-flight operations following the logical host to its new installation; this
case is fresh routing to its predecessor. The rare-fault exception at `1333–1345` and durable-first convergence at
`SPEC_impl.md:3806–3815` may inform triage but do not clearly settle this ordering. Ledger
`adopt-publishes-unconditionally-after-commit.md` (`TRIAGE_OUTCOMES.md:189–210`) fixed preservation of Retired and
explicitly left broader adoption concurrency out. `retarget-race-republishes-old-client.md` (`5294–5316`) fixed
same-cell publication; `host-write-lock-split-on-actor-respawn.md` (`6456–6476`) fixed cache-lock ownership, not this
retained cell. No exact Planned, BUGS, queue or filter match was found. Independent assessment retained the finding.
Proposed drop: none.

Provenance: persistent reviewers `A19-adoption/A19-F2` and `A20-refresh-publication/A20-publication-f2`, independent
`static-dispositions-11`, parent source/coverage verification. These are one underlying defect.

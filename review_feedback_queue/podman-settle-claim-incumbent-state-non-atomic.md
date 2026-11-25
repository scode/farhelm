# Place-claim transfer reads the incumbent's identity refusal non-atomically

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

In a narrow race, two host entries that reach the same container host can swap which one is allowed to start new
containers, right as one of them finishes verifying its identity. No container, file or process is touched — only the
entry that new launches route through changes.

## Details

**other.** Source inspection; commissioner confirmed the non-atomicity. The established consequence is a benign
routing-preference shift: the twin query (store/podman.rs:418-429) matches on machine_id+uid (both entries provably
reach the same Podman store) and referenced_place_reason (store/podman.rs:337-376) blocks transfer if the incumbent
holds any container/image/helper. What would move it to highest: a shown path where the transfer moves a claim between
different machines or displaces an incumbent holding resources — neither is possible given those two guards.

`crates/farhelm-helm/src/environment/podman_probe.rs:593-625` (`settle_claim`): line 609 reads the incumbent's LIVE
actor state (`ordinary_state(&incumbent)` → `state_for_registry_row`) to decide it has a current identity refusal
(IdentityMismatch/IdentityUnverified with matching recorded identity) or is a Duplicate pointing at the challenger.
Lines 622-624 then call `claim_podman_place(row, identity, Some(verified), Some(&incumbent))` to transfer the unused
claim. The store transfer (`crates/farhelm-helm/src/store/podman.rs:431-447`) re-checks the incumbent's REGISTRY facts
in-transaction (id, entry_uuid, host_identity, DialedAs) and referenced_place_reason(owner), but NOT the live actor
state. So if the incumbent's IdentityUnverified resolves to Connected with the same recorded identity between line 609
and 622, the transfer still proceeds. (An adoption or retarget in that window IS caught, because those change
host_identity/DialedAs, which the comparison re-checks.) SPEC_impl.md "Podman places, creation and images" says the
unused twin yields when it "has a current identity refusal"; the check is not atomic with the transfer.

Not covered by the related queued items: trailing-podman-refresh-settles-claims-with-stale-pre-probe-row is a stale
REGISTRY row passed across probe() (fix: re-read the row under claim_publication) — re-reading the row would not close
this race, which is the incumbent's LIVE actor state changing between the read and the transfer;
trailing-podman-settle-claim-refuses-third-host-duplicate-transfer and
trailing-podman-place-retains-unused-claim-after-twin concern other guards/consequences.

Verify: script the incumbent actor to transition IdentityUnverified → Connected(same identity) between the two claim
calls and observe the claim move. Fix shape: re-read the incumbent's live state under claim_publication immediately
before the transfer, or carry the observed refusal into the store transaction as an extra fence.

Reviewer metadata (area 6 chunk B, host registry/place probe): suggested bucket: other; confidence: likely
(non-atomicity confirmed by reading; benign consequence from guard analysis; exact race not executed); proposed_drop:
none; possible_cover: related Podman place-claim items exist but are distinct (above). Commissioner validation:
settle_claim confirmed at the cited lines; coverage searched (queue incl. the three related items read, TODO.md,
BUGS.md, TRIAGE_OUTCOMES.md, plans/queue) — no exact match.

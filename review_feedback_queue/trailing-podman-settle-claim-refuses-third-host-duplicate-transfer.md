# PodmanPlaces::settle_claim refuses to transfer an unused claim when the incumbent is Duplicate against a third host

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When a stale host entry holds an unused container host claim and is refused as a duplicate of a third host, the verified
host that actually owns the container host cannot automatically take over the unused claim and is refused for new
containers until the stale entry is removed or retargeted.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:609`.

`SPEC.md:344-346` states: "When Farhelm has verified which host owns the ordinary identity, that entry can take an
unused container host claim from a second entry whose ordinary identity is absent or refused." `SPEC_impl.md:6243-6244`
states: "An unused twin yields to the verified ordinary owner when the twin lacks a recorded ordinary identity or has a
current identity refusal." In `crates/farhelm-helm/src/store.rs:8706-8711`, `record_first_contact` checks `claimant_of`
before `Mismatch`, so an incumbent entry that previously recorded a `host_identity` and whose endpoint later reaches a
third host `C`'s supervisor enters `HostState::Duplicate { twin: C, identity: id_C }` rather than
`HostState::IdentityMismatch`. However, in `crates/farhelm-helm/src/environment/podman_probe.rs:609-618`
(`PodmanPlaces::settle_claim`), `HostState::Duplicate` is only matched with `if twin == row.id`:

```rust
let verified = match self.ordinary_state(&incumbent) {
    Some(HostState::Duplicate { twin, identity }) if twin == row.id => Some(identity),
    Some(HostState::IdentityMismatch { recorded, .. })
    | Some(HostState::IdentityUnverified { recorded })
        if incumbent.host_identity.as_ref() == Some(&recorded) =>
    {
        verified_supervisor
    }
    _ => None,
};
```

When `incumbent` has a recorded `host_identity`, holds an unused `podman_place_claims` row, and is currently refused as
`HostState::Duplicate { twin: C, .. }` with `C != row.id`, `verified` evaluates to `None` even when `row` has a live
`verified_supervisor = Some(...)` (so `row` is the verified ordinary owner and `incumbent`'s ordinary identity is
refused). Adding `Some(HostState::Duplicate { .. }) => verified_supervisor` (after the `twin == row.id` arm) satisfies
`SPEC.md:344-346` and `SPEC_impl.md:6243-6244`.

Reviewer metadata (g35-podman-identity-probe/l2): suggested bucket: other (cannot be in highest: refuses new
container/image work on a verified host in a narrow duplicate-entry topology, with no data loss or security impact).;
confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/environment/podman_probe.rs:609` (g37-podman-retarget-probe-store/f3):

Commit 86be6c582492cddb7413e5a176d84bb29eb76da1 updated `SPEC.md:344-346` ("When Farhelm has verified which host owns
the ordinary identity, that entry can take an unused container host claim from a second entry whose ordinary identity is
absent or refused") and `SPEC_impl.md:6243-6244` ("An unused twin yields to the verified ordinary owner when the twin
lacks a recorded ordinary identity or has a current identity refusal"). In `PodmanPlaces::settle_claim`
(`crates/farhelm-helm/src/environment/podman_probe.rs:609-618`), the match on `self.ordinary_state(&incumbent)` handles
`Some(HostState::Duplicate { twin, identity }) if twin == row.id => Some(identity)` and
`Some(HostState::IdentityMismatch { recorded, .. }) | Some(HostState::IdentityUnverified { recorded }) if incumbent.host_identity.as_ref() == Some(&recorded) => verified_supervisor`,
but falls through to `_ => None` when the incumbent is in `HostState::Duplicate { twin, .. }` with `twin != row.id`. If
an unused incumbent has a recorded `host_identity`, holds the unused Podman place claim for `row`'s machine/account, and
now has a `Duplicate` refusal against a third entry while `row` is connected with `verified_supervisor = Some(...)`,
`settle_claim` refuses to transfer the unused claim even though `row` is the verified ordinary owner and the incumbent
has a current identity refusal. Fix: add a fallback arm `Some(HostState::Duplicate { .. }) => verified_supervisor` after
the `twin == row.id` arm.

Reviewer metadata (g37-podman-retarget-probe-store/f3): suggested bucket: other (not highest: refuses new container
admission on one host until the duplicate entry is removed or retargeted; no data loss or security impact); confidence:
confirmed; possible cover: none (interface overlap with g35-podman-identity-probe; SPEC.md and SPEC_impl.md wording
updated in 86be6c582492cddb7413e5a176d84bb29eb76da1).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g35-podman-identity-probe/l2`,
`g37-podman-retarget-probe-store/f3`.

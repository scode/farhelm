# A host reaching another entry's identity is stuck on a failing adopt

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After pointing a host entry at a machine another entry already manages, the host keeps asking to adopt the new identity
and every attempt fails with a conflict until it is edited or removed by hand.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F11 / COR-MISMATCH-NEVER-DUPLICATE`, tagged **definite**. Anchors and title: `store.rs:4133-4149`, `store.rs:4232`,
`manager.rs:3004-3016`, `manager.rs:2375`, `store.rs:1130-1136` — A host that reaches another entry's identity is stuck
on an adopt offer that always fails

When a host connects, `record_first_contact` (`store.rs:4111`) classifies the identity it reports. The order of checks
is the problem (`store.rs:4133-4149`). If the row already has a recorded identity and it differs from the reported one,
the result is `Mismatch` straight away. The check "does another registry row already own this identity?" (`claimant_of`,
which yields `Collision` and leads to the **duplicate** host view) runs only when the row has _no_ identity yet. So a
row that has ever learned an identity can never be classified as a duplicate. Two common triggers:

- retargeting a known entry onto a machine another entry already manages;
- re-adding a reinstalled host as a new entry while the old entry still points at it.

Either way, the actor publishes `IdentityMismatch` and freezes with no timer (`manager.rs:3004-3016`). The user is
offered "adopt the new identity". But `adopt_identity` does check `claimant_of` (`store.rs:4232`) and refuses with
`IdentityClaimed` (409). Nothing changes state, and the offer comes back. The `IdentityClaimed` docs
(`store.rs:1130-1136`) promise that re-rendering the host will show the duplicate-resolution view, but nothing ever
publishes that state.

SPEC.md says two destinations reaching one identity are the same host, shown once, and that errors must be actionable.
Here the only offered remedy can never succeed.

Suggested fix: in `record_first_contact`, check `claimant_of` before comparing against the recorded identity and return
`Collision { owner }`. Alternatively, have `adopt` (around `manager.rs:2375`) republish the host as `Duplicate` when the
store answers `IdentityClaimed`.

User-visible consequence: after pointing a host entry at a machine that another entry already manages, the host keeps
asking to adopt the new identity, and every attempt fails with a conflict until the entry is edited or removed by hand.

# SPEC_impl.md still states leftover-sandbox local key and ownership reads are unbounded

Reviewed commit: de4302008ae668ca4ee7f569e7fad36e55521285

## TLDR

The implementation specification contradicts itself about whether listing Tensorlake sandboxes not on this helm bounds
its local database reads, which can mislead future changes to discovery timeouts.

## Details

**other.** Source inspection. Main reference `SPEC_impl.md:6454-6459`.

In commit de4302008ae668ca4ee7f569e7fad36e55521285, `Tensorlake::discover_orphan_sandboxes_by`
(`crates/farhelm-helm/src/tensorlake/orphans.rs:126-133`) was changed to wrap both `self.api_key()` and
`self.known_sandboxes()` in `tokio::time::timeout_at(reply_by, ...)`, and `SPEC_impl.md:7047-7048` (under "Tensorlake
recipe tunnels and unowned sandboxes"; lines 7115-7116 at `1b3fe49af86979aebc2719965682a17f8c6366ac`) was updated to
state: "Discovery's reads of the local key, the provider, and local ownership use the same reply deadline." However,
`SPEC_impl.md:6454-6459` (under "Tensorlake account"; lines 6527-6532 at `1b3fe49af86979aebc2719965682a17f8c6366ac`) was
not updated and still states: "It stops listing at the UI channel's host reply bound, 20 seconds after the request
arrived (the stored key's read counts against it, but nothing bounds that local read itself), and a listing cut off
there is a transport failure saying Tensorlake did not answer within that time; the listing of leftover sandboxes is cut
off the same way (see \"The UI channel\"), and its ownership reads come after the listing and are not bounded either, so
that a sandbox created during the listing, whose name its host recorded first, is never shown as not on this helm." How
to verify: compare `SPEC_impl.md:6454-6459` against `SPEC_impl.md:7047-7048` and
`crates/farhelm-helm/src/tensorlake/orphans.rs:126-133`. Fix: update the leftover-sandboxes clause in `SPEC_impl.md`
under "Tensorlake account" to state that leftover-sandbox discovery bounds its local key read, provider listing, and
post-listing ownership reads under the same reply deadline (while keeping the unbounded local-read statement scoped to
`POST /api/tensorlake/test`, where `Tensorlake::test` in `crates/farhelm-helm/src/tensorlake/mod.rs:361-375` still
leaves the local key read unbounded).

Reviewer metadata (g16_tensorlake_leftovers_discovery/l1): suggested bucket: other — internal implementation
specification drift with no security, data-loss, or user-visible runtime impact.; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g16_tensorlake_leftovers_discovery/l1`.

# Recovery can delete a different Tensorlake sandbox under a reused name

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

If an interrupted Tensorlake create never reached the provider or its original sandbox was removed, retrying a build or
deleting a host or recipe can terminate a replacement sandbox using the reserved name.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R9-5.

Source references: `crates/farhelm-helm/src/tensorlake/build.rs:1389–1395, 1473–1517, 1680–1740`,
`crates/farhelm-helm/src/tensorlake/hosts.rs:568–590`, `crates/farhelm-helm/src/provisioning/tensorlake_add.rs:116–135`,
`crates/farhelm-helm/src/tensorlake/config.rs:82, 92`.

The helm saves a pending-create timestamp before dispatch and has no canonical ID yet. If another sandbox takes the name
after this point, recipe recovery records the first matching ID and sweeps it and its snapshots. Host Delete likewise
promotes the pending-name match to a saved ID and terminates it. Reply recovery for a normal create therefore supplies
authority over a replacement. Retrying a build can take this path without a separate destructive confirmation.

Sol rated this source-confirmed and critical. Opus verification R9-5 confirmed the mechanism and rated it noncritical
because another creator must use this helm's reserved name and the build helpers themselves are disposable. Highest
reflects loss of the unrelated sandbox, its processes and its snapshots. SPEC_impl.md lines 6242–6257 and 6482–6486
explicitly allow name recovery with an outstanding create. Whether that accepts a later sandbox independently created
under the same name is unclear. SPEC.md lines 541–552 reserves the prefixes for explicit orphan cleanup, which has its
own confirmation.

Bound recovery to the specific create where the provider permits it, for example with a create nonce or a verified
provider creation time relative to the durable marker and settle interval. Test a matching name whose creation
demonstrably falls outside that request.

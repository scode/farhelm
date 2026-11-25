# Leftover container listing still reads Podman's full ps report

Reviewed commit: de6aaf3ae254c51cecd06feba47cdfcd58b6c379

## TLDR

The fix made deleting a leftover container work even when the container carries a lot of metadata. But Settings →
Containers can only offer that Delete after listing the host's leftovers, and the listing still reads Podman's full
description of every Farhelm container on the host in one reply. With enough containers on one host, or containers built
from an image with many labels, that reply exceeds Farhelm's output limit. The whole listing then fails for every
container host, and the leftovers, which have no host row and no other delete route, stay until removed by hand.

## Details

`Podman::list_owned` (`crates/farhelm-helm/src/environment/podman.rs:1601-1640` at the tip) runs
`podman ps --all --no-trunc --filter label=farhelm.helm=<tag> --format=json` through `require` (64 KiB cap,
`provisioning/backend.rs:106`) and then reads only each item's `Id`/`ID` before calling the newly projected `inspect`.
Podman's ps JSON carries each container's full label map (including inherited image labels), command, mounts, names,
networks, ports and timestamps, so it grows with container count and label volume, the "unrelated labels" case the
commit's `CONTAINER_INSPECTION` doc names. Its only caller is the leftover listing (`podman.rs:919`), and
`list_leftovers` joins every place with `futures_util::future::try_join_all` (`podman.rs:917-922`), so one host over the
cap fails the whole list and `delete_leftover` (`podman.rs:840-880`), the route the commit fixed, cannot be reached for
any host.

Fix: project the listing too, for example `--format '{{.ID}}'` with one ID per line, as `build.rs:854-860` already does
for `ps --filter ancestor=`; keep the per-item `inspect` for ownership. Consider joining places so one host's failure
does not hide the others.

Suggested bucket: `other` (cleanup unavailable, resources left on the container host; no user work lost). Confidence:
likely. Unverified after investigation: that a real host's ps JSON for Farhelm containers reaches 64 KiB (it needs
dozens of containers, or images with large label sets). Related: `trailing-podman-image-inspect-hits-output-cap.md`.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g06-podman-cleanup-checkout-names` record `f3`.

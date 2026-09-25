# A failed download leaves its .part file behind

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If a release download fails because the disk filled or the connection dropped, a partial download of up to 1 GiB stays
in the helm's state directory and can keep its disk full until the helm restarts.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F20 / COR-PART-LEFT-ON-FAILURE`, tagged **possible**. Anchors and title: `provisioning/release_payloads.rs:729-764` — A
failed asset download leaves its partial .part file in helm state

`download_verified` streams a release asset into `<asset>.part` in the helm's state directory, hashing as it goes, and
renames it into place only if the hash matches. It deletes the `.part` file on two failure paths only: the size cap (1
GiB) and a checksum mismatch. The other failures return early through `?` and leave it behind
(release_payloads.rs:729-764): a transport error in the middle of the body, a failed `write_all` (typically a full
disk), or a failed `flush`/`sync_all`. The partial file, up to the 1 GiB cap, stays until the same asset is requested
again (the next lookup's cache check discards it) or until housekeeping in the next helm process.

The likeliest write failure is a full disk, and the leftover keeps it full; the same state directory holds `helm.db`, so
unrelated helm writes keep failing until someone retries provisioning or restarts the helm. It also contradicts the
module's own rule that unverified bytes are never kept. Suggested change: remove the `.part` file on every error after
creating it (a scope guard), and fold any cleanup failure into the reported error.

User-visible consequence: if a release download fails because the disk filled or the connection dropped, a partial
download of up to 1 GiB stays in the helm's state directory and can keep its disk full until the helm restarts or the
download is retried.

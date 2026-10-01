# A failed release download leaves a partial file in helm state

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

When downloading a release for host setup fails partway (a network error, or a full disk), the partial file stays in the
helm's state directory until a later retry or helm restart, which on a full disk keeps the disk full.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F35 / COR-PARTIAL-DOWNLOAD`, tagged **definite**. Anchor and title:
`crates/farhelm-helm/src/provisioning/release_payloads.rs:729` — a failed release download leaves its partial file in
helm state.

When the helm sets up or updates a remote host, it can download Farhelm release assets (the `farhelm` binary archive,
the private tmux build) from the release server and cache them in the helm's state directory. The download routine,
`download_verified` in `crates/farhelm-helm/src/provisioning/release_payloads.rs` (lines ~729–764), streams the asset
into a staging file named `<asset>.part`, hashing as it goes, and only renames it to its final name once the hash
matches the signed checksum list. Its own comment says a partial download is removed rather than kept because "it is
unverified content sitting in helm state", and a test in the same file asserts "unverified bytes must not survive,
staged or published".

The cleanup is only done on two of the failure paths: when the download grows past the size cap, and when the finished
file's checksum does not match. Every other failure after the `.part` file is created returns early through Rust's `?`
operator and leaves the file behind: an error from the network body stream mid-download, a write error (for example, a
full disk), a flush error, or an fsync error. The file stays until a later attempt at the same asset overwrites it, or
until the next helm start's housekeeping sweeps stale `.part` files.

The practical impact is low, but it is a direct contradiction of the routine's own contract, and the worst case is
unpleasant: on a full disk, a failed download leaves up to tens of megabytes of partial archive sitting in the helm's
state directory, keeping the disk full. The fix is to wrap file creation, streaming, flush and fsync in a single block,
and on any error drop the file handle and call the existing `remove_if_present(&part)`, folding any cleanup failure into
the returned error the same way the size and checksum refusals already do.

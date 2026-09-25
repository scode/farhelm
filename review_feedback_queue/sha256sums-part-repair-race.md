# Concurrent manifest repairs race on SHA256SUMS.part

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After the helm's download cache has been damaged, one of several concurrent setups or updates can fail with an odd
rename error and then succeed on retry.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F19 / COR-SUMS-PART-RACE`, tagged **possible**. Anchors and title: `provisioning/release_payloads.rs:653-680`,
`provisioning/release_payloads.rs:1153-1168`, `provisioning/release_payloads.rs:1255-1260` — Concurrent cache-manifest
repairs race on the fixed SHA256SUMS.part staging name

Release downloads are cached in helm state together with the signed checksum list (`SHA256SUMS` and
`SHA256SUMS.minisig`). Each payload lookup first re-verifies those two cached files. If they fail verification but this
process still holds a verified copy in memory, `cached_controls_verified` (release_payloads.rs:653-680) repairs them by
calling `publish_controls`, which writes each file to a _fixed_ staging name (`SHA256SUMS.part`,
`SHA256SUMS.minisig.part`) with `File::create` (which truncates) and then renames it into place (`publish_bytes`,
1153-1168; `publish_controls`, 1255-1260).

The only lock around a lookup is per asset, and a cold host resolves its farhelm and tmux payloads concurrently (so do
parallel runs). Two repairs can therefore interleave on the same staging path: B truncates the `.part` file that A is
writing, A renames it into place while B is still writing into that same file, and B's rename then fails with ENOENT.
That error propagates, so B's payload lookup fails and its provisioning run fails. A reader in between can also see a
truncated `SHA256SUMS` (it then re-verifies and fails, so no unverified content is trusted). The trigger requires the
on-disk control files to be damaged after this process verified them, so it is rare, and it lands on exactly the path
meant to recover from a damaged cache without a network round trip. Suggested change: serialize control-file publication
with one lock shared by all assets, or use unique staging names.

User-visible consequence: after the helm's download cache has been damaged, one of several concurrent setups or updates
can fail with an odd rename error and then succeed on retry.

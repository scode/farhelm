# --payload-dir must be writable by the helm

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Pointing `--payload-dir` at a read-only or shared copy of the release files makes every host setup and update fail with
a "creating …/.extracted" permission error.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F9 / COR-PAYLOAD-DIR-WRITABLE`, tagged **definite**. Anchors and title: `provisioning/payloads.rs:103-109`,
`provisioning/payloads.rs:156-165`, `provisioning/payloads.rs:270-337` — --payload-dir must be writable, because every
run materializes into <payload-dir>/.extracted/

`--payload-dir` points the helm at a directory of release files staged by the operator, instead of downloading them —
the documented mechanism for air-gapped installs and mirrors. `DirectoryPayloads::path` does not just read it. On every
call it runs `ensure_private_extracted_dir(<payload dir>/.extracted)` (payloads.rs:163-164, 270-337), which creates that
subdirectory or, if it exists, forces it to mode 0700, and then writes a fresh `<asset>.<uuid>.bin` into it (extracting
the farhelm binary from its archive, or copying tmux). Old copies are pruned only when older than an hour, by a later
call.

So the payload directory must be writable by the helm's user. A read-only mirror, removable or read-only media, or a
root-owned or Nix-store staging directory fails with EROFS or EACCES. A group-shared mirror works for the first user
only: their helm creates `.extracted` as 0700 under their uid, and every other user's helm then fails trying to chmod a
directory it does not own. The failure happens in `prepare_payloads`, before any host is touched, so every setup and
update fails.

The extraction output is private scratch space and has no reason to live inside operator input. Suggested change:
extract into a private 0700 directory under the helm's own state (for example
`<state_dir>/payloads/directory-<hash of the payload dir>/`) or a private temporary directory, and treat `--payload-dir`
as read-only.

User-visible consequence: pointing `--payload-dir` at a read-only or shared copy of the release files makes every host
setup and update fail with a "creating …/.extracted" permission error.

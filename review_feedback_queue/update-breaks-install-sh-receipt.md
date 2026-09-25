# UPDATE invalidates install.sh's ownership receipt

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After updating a host from the hosts panel, `farhelm uninstall` on that host can fail with a digest mismatch on its own
binary.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F14 / COR-INSTALLSH-RECEIPT`, tagged **possible**. Anchors and title: `provisioning/plan.rs:458-470`,
`crates/farhelm/src/uninstall/ownership.rs:182-197`, `crates/farhelm/src/uninstall/ownership.rs:395-421` — UPDATE
replaces an install.sh-owned binary, invalidating install.sh's ownership receipt so uninstall refuses

`install.sh` puts `farhelm` in `~/.local/bin` and writes a receipt, `.farhelm-installation`, beside it that records the
SHA-256 of the binary it installed. `farhelm uninstall` uses the receipt to prove which files it owns: it hashes the
binary and refuses with "… does not match its recorded SHA-256 digest" if it differs (`verify_payload`,
`crates/farhelm/src/uninstall/ownership.rs:395-421`, called at 182-197).

UPDATE targets the row's recorded binary (plan.rs:458-470). For an install.sh host that is `~/.local/bin/farhelm`, and
the install step renames the helm's release binary over it without touching the receipt. After that, `farhelm uninstall`
on that host fails the digest check on its own binary. One supported operation silently breaks another that SPEC.md
"Install and uninstall" promises will work.

Suggested change (any of): when the target directory holds a `.farhelm-installation` receipt, refuse UPDATE with
"managed by install.sh"; update the receipt's digest in the same step; or install into `~/.local/lib/farhelm/` and
repoint the unit and row.

Restater note: the reviewer did not run uninstall end to end. I confirmed the receipt check and its refusal text in the
code, but not the exact message the user sees or whether uninstall offers any repair path for a mismatched binary.

User-visible consequence: after updating a host from the hosts panel, `farhelm uninstall` on that host can fail with a
digest mismatch on its own binary.

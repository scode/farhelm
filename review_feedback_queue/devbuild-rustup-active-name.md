# Mac devbuild misparses Rustup 1.28's active-toolchain output

Reviewed commit: af44ffdf30876b002687e0add4bb3504ac8628ee

## TLDR

Mac devbuild can abort before building even when its pinned compiler and targets are already installed. The new
preflight misparses Rustup 1.28.0's multiline output. Current Rustup 1.29.3 uses a different one-line output and is
unaffected by this specific parsing route.

## Details

`scripts/devbuild-mac.sh:383–389` captures `rustup show active-toolchain`. Line 390 removes everything beginning at the
first literal space; line 391 uses the remainder as `--toolchain` for a target query and exits on failure.

[Rustup 1.28.0 rustup_mode.rs:1084–1105](https://github.com/rust-lang/rustup/blob/1.28.0/src/cli/rustup_mode.rs#L1084-L1105)
prints the name followed by a newline and `active because: <reason>`, even without verbose mode. The shell expansion
therefore leaves `<name>\nactive`, including the newline, as the argument. The CLI's `PartialToolchainDesc`
(`rustup_mode.rs:388–403`, `dist/mod.rs:587–599`) reaches an
[anchored parser](https://github.com/rust-lang/rustup/blob/1.28.0/src/dist/mod.rs#L284-L340) that cannot consume the
newline and returns `InvalidToolchainName`. The previous script queried targets without parsing this display output.

Select the first output line before removing any same-line reason suffix, accommodating both observed output forms. The
failure occurs before the previous staged-build marker is removed (`devbuild-mac.sh:457–458`), so loss of an earlier
completed build is not established.

Priority: `other`, maintainer tooling. Confidence: confirmed exact-version output/parser and shell semantics; no runtime
reproduction. The compiler can already be installed, making this distinct from the CentOS missing-pin issue. Proposed
drop: none. Possible cover: no exact version restriction, planned repair, known bug, queue item, ledger decision or
filter match was found. Rustup 1.29.3's nonverbose output at `rustup_mode.rs:1324–1351` is one line; this is not a claim
that the script fails with every version.

Provenance: trailing review through `25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `toolchain/toolchain-l6`.

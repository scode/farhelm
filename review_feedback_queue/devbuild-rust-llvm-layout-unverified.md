# Pinned Rust may need the release LLVM repair in local Mac devbuilds

Reviewed commit: af44ffdf30876b002687e0add4bb3504ac8628ee

## TLDR

Unverified after investigation: a clean Mac development build may fail while stripping its web UI because the newly
pinned Rust's `rust-objcopy` cannot find LLVM. The local script omits a repair used by release builds, but the exact
distributed Rust artifact's loader paths remain unverified.

## Details

`rust-toolchain.toml:4` pins 1.99.1. `scripts/devbuild-mac.sh:472–474` directly runs the release web build through
Dioxus 0.7.10 (`Cargo.toml:322–324`). That pinned Dioxus implementation strips WebAssembly through the host toolchain's
`rust-objcopy`. `.github/dist-build-setup.yml:413–445` contains the release repair for the Darwin LLVM layout, which the
local script does not perform. The concern is this web-strip route, not native desktop stripping alone.

Investigation checked the pinned Dioxus strip path, the local/release scripts, and primary Rust metadata and source. The
[1.99.1 release manifest](https://static.rust-lang.org/dist/channel-rust-1.99.1.toml) identifies compiler commit
`58cf8f9c35b355199727627d07ae34a4e2846d67` and the Darwin compiler archive dated 2026-10-15, with SHA-256
`23948a0a9c162fd2952defb3fb0462c44e8fcfd1a4ab8a274a1e1bdb2cf810a3`. The release's
[Darwin rpath fix](https://github.com/rust-lang/rust/pull/163954.patch) changes LLD configuration only; it does not by
itself settle objcopy's loader paths.

At that compiler commit,
[dist.rs:579–621](https://github.com/rust-lang/rust/blob/58cf8f9c35b355199727627d07ae34a4e2846d67/src/bootstrap/src/core/build_steps/dist.rs#L579-L621)
places LLVM in the runtime library location and objcopy under `rustlib/<host>/bin`; lines 2574–2604 distinguish
target/runtime library locations. Its LLVM submodule is `1b9c0d5ff9bbe7634aead059efe6b11a7eeba145`, whose
[AddLLVM.cmake:2685–2698](https://github.com/rust-lang/llvm-project/blob/1b9c0d5ff9bbe7634aead059efe6b11a7eeba145/llvm/cmake/modules/AddLLVM.cmake#L2685-L2698)
gives Apple builds a default installed rpath of `loader_path/../lib` unless configured otherwise. These sources support
the mechanism but do not attest the final package's load commands or all compensating library locations.

The missing premise is the hash-identified distributed `rust-objcopy`'s actual dependency/rpath together with absence of
a usable library at its lookup location. Metadata and build source cannot exclude packaging overrides. Reports about
1.99.0 do not prove 1.99.1 behaves the same way. No compiler archive download, install mutation or Mac runtime probe
ran. Exact-artifact load-command/member evidence or upstream confirmation would settle it. If confirmed, share the
release repair with the local build path or use a verified toolchain layout.

Priority: `other`, maintainer tooling. Confidence: **unverified after investigation**, conditional on the missing
artifact premise. Proposed drop: none. Possible cover: no exact recording coverage found; the landed compiler-pin plan
does not accept breaking local builds. No security, user-work loss or production release failure is asserted.

Provenance: trailing review through `25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `toolchain/toolchain-l3`. The complete checks and retained primary
sources remain with that record. This is distinct from the source-refuted missing-Darwin staging allegation.

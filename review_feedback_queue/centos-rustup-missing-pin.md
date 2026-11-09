# Standalone CentOS validation cannot bootstrap the pin with Rustup 1.28

Reviewed commit: af44ffdf30876b002687e0add4bb3504ac8628ee

## TLDR

With Rustup 1.28.0 already installed and the pinned compiler absent, standalone CentOS provisioning validation exits
before building its payload. Current Rustup 1.29.3's automatic installation avoids this specific failure; the finding is
limited to the established older-version route.

## Details

`scripts/test-provision-centos.sh:262–268` enters the checkout and asks Rustup for installed targets, then adds the musl
target before compiling. The new repository pin selects 1.99.1, but this path does not explicitly install an absent
compiler before target operations.

Versioned
[Rustup 1.28.0 rustup_mode.rs:1114–1149](https://github.com/rust-lang/rustup/blob/1.28.0/src/cli/rustup_mode.rs#L1114-L1149)
resolves target operations through `DistributableToolchain::from_partial` (`distributable.rs:35–43`), then
`config.rs:503–515,738–748` and `toolchain.rs:106–115` require the selected toolchain to be installed. The script's
target-add at line 265 therefore fails. Its preceding target-list failure is hidden by the grep condition, not repaired.
Earlier release steps can mask this if something else has already installed the pin.

The same version's [Cargo proxy](https://github.com/rust-lang/rustup/blob/1.28.0/src/cli/proxy_mode.rs#L13-L36) also
resolves synchronously without automatically installing the compiler. An earlier suggestion that simply moving a Cargo
call first would bootstrap it was corrected during review. Explicitly install the selected compiler before the target
commands, using the upstream-documented compatible show/install sequence or an equivalent explicit path.

Priority: `other`, maintainer validation availability. Confidence: confirmed exact-version source route, no runtime
reproduction. Current 1.29.3's default resolver auto-installs; no blanket claim about all Rustup versions or fresh
installers is made. Proposed drop: none. Possible cover: no minimum Rustup version or exact accepted exception was found
in the governing build/testing contracts, and no other recording coverage matched. This is distinct from Mac devbuild's
active-toolchain output parsing defect.

Provenance: trailing review through `25d8a9435662e6c832bc2e7fbf4b03a3ebbe1652`, run
`fc41a3a3-d917-41d0-bf2f-3aad9a7d6f3a`, original `toolchain/toolchain-l4`; its complete event history preserves the
Cargo-bootstrap correction and current-version caveat.

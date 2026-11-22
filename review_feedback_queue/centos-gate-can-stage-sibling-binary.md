# The CentOS gate can stage a sibling checkout’s musl binary from a shared target

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The CentOS gate can stage a sibling checkout's supervisor from a shared Cargo output directory.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d scripts/test-provision-centos.sh:60–65,262–271,493–502;
crates/farhelm-helm/src/provisioning.rs:12693–12701,13049–13057,13695–13697,13944–13950;
crates/farhelm-helm/src/provisioning/installation.rs:21–44,68–78

A local gate can exercise compatible supervisor bytes from another checkout and attribute coverage to the source under
test. With a shared absolute CARGO_TARGET_DIR, permitted across sibling checkouts by AGENTS.md:79–80, Cargo and the
fixture name the same mutable path. Cargo finishes before the static check and test, and the ELF checker establishes
machine and linkage rather than source identity (scripts/check-static-elf.sh:48–100). The recorder preserves
FARHELM_TEST_BINARY, the Rust fixture checks existence and returns the same path, and the installation helper stages
that path only when preparing the provisioning action. This stage fixes the bytes for that action, but cannot repair a
replacement before staging. UPDATE also explicitly rereads the original path when creating its fixture-owned payload.
The Podman producer isolates build and intermediate directories for this same shared-path issue
(scripts/build-podman-test-payloads.sh:29–40). Shipped hosted release use (.github/workflows/release.yml:271–278) does
not establish another checkout sharing the target. Actual concurrent replacement, compatibility, and an affected result
remain unverified. Isolate the CentOS build and provide a run-owned, validated artifact before leaving the build phase.
The reviewer suggested other, confirmed source route under documented sharing, , Possible coverage: related to trl-12
but distinct trigger.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `test-and-release-tools/trl-09`.

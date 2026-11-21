# Direct golden execution bypasses Cargo’s runtime setup

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Custom screenshot builds can succeed but fail to run when they depend on Cargo’s runner or shared-library setup.

## Details

The supported interface forwards additional Cargo build options before -- (scripts/check-gui-goldens.py:27–28, :286–293
and .agents/validation.md:728–734). The previous runner invoked cargo test directly
(dbb42b527d2ebd7918aaa2f4e0c87f06f78bcc47:scripts/check-gui-goldens.py:116–125). The pinned version invokes Cargo only
for artifact discovery at scripts/check-gui-goldens.py:90–110. Inventory at :113–118 runs the binary with base_env, and
render at :186–189 and :255–259 runs it with the renderer environment, which adds no library search paths at :69–87. The
Cargo Book's Dynamic library paths section specifies that cargo test adds the Rust sysroot, build output and eligible
build-script paths. A custom artifact with a dylib that is available through those paths, but unavailable in the
ordinary loader environment or its rpath, fails at inventory before capture deletion or reference fetch. One supported
way to reach the premise is custom Rust compiler flags enabling dynamic linking, if that build actually links a
toolchain dylib that requires Cargo's path. The rustc book documents dynamic linking preferences and rpath defaults.
Independently, a target.<triple>.runner or target.<cfg>.runner configured for a supported build target is bypassed under
the Cargo Book's Configuration contract. A native build can still run successfully if that runner was optional. The
documented test recorder preserves non-FARHELM environment options, including Cargo and Rust configuration variables at
scripts/record-test-run.py:1162–1170. The checked-in Cargo.toml:296–393 optimizations only set opt-level, so they do not
establish either premise for a normal build. Preserve Cargo's execution environment and configured launcher, or enforce
an explicit supported build contract that refuses incompatible settings before executing.

Confidence: confirmed by source and documented tool contracts, with the specified dynamic-linking or configured-runner
premise.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `goldenRunner/l2`.

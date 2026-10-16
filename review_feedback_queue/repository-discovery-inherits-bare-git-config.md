# Inherited Git configuration can hide repository suggestions

Reviewed commit: 5998041edc804b73a0166729b52b16364f677896

## TLDR

An inherited Git configuration selector can prevent existing repositories from appearing as launcher suggestions. Manual
repository selection remains available.

## Details

Suggested bucket: `other`. Confidence: likely. Inheritance is confirmed statically; Git's refusal is supported by the
plan report and installed documentation, without independently repeating its experiment. Source: separate adversarial
trailing reviewer. This is a pre-existing production omission exposed while reviewing the fixture fix. Proposed drop:
none.

`crates/farhelm-supervisor/src/repository_discovery.rs:232–258` removes `GIT_CONFIG_*` and repository-location variables
but leaves bare `GIT_CONFIG` inherited, and permits it through `child_env`. Inspection uses `git config --local`
(`212–225`). Git treats `GIT_CONFIG` as a file selector, conflicting with `--local`.
`plans/reports/rust-test-portability.report.md:24–32` at `d617e89a7e07e2a3261a3662ca889d57e09e84d4` records three
production discovery assertions failing under that input. Only the fixture helper was changed (`989–995`). The
production omission persists at the review tip.

Verify with a scanner given a child-only `GIT_CONFIG` selector and an owned repository whose GitHub origin would
otherwise be discovered. Remove the selector from production inspection children as well as fixture children; do not
modify the test process's environment.

Possible cover: the completed `git-discovery-fixtures-inherit-bare-git-config.md` ledger outcome addresses fixture
writes and requests production parity, but describes production as unaffected. This record preserves the different
production consequence. SPEC_impl.md, “Checkout configuration and discovery,” requires inspection children to discard
ambient Git configuration/location overrides. No matching acceptance, Planned item, BUGS entry, queue item or filter was
found.

# Release gate maintenance guidance still describes the stock workflow

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The release workflow’s maintainer instructions can leave the generated workflow stale for the next tag.

## Details

The relevant source is `.github/dist-build-setup.yml:7`. Reviewed commit: 67ca3574905469ea431302b6e82e809a41cefb61.
TLDR: Following the build gate's regeneration instructions can leave changes out of the generated release workflow and
cause the next tag's plan check to fail. Details: .github/dist-build-setup.yml:5–8 still directs maintainers to
`dist init --yes`. With the new allow-dirty = ["ci"] (dist-workspace.toml:219–225), pinned cargo-dist 0.32.0
`src/init.rs:384–393` calls inferred generation, `src/lib.rs:850–887` skips allowed-dirty inferred modes and
`src/config/mod.rs:879–885` confirms that CI is skipped. Farhelm's wrapper removes that setting in its render copy and
adds the required overlay (releasing/generate-workflow.py:32–55,95–104,127–170). The tag plan and on-demand CI compare
against the overlaid result (.github/workflows/release.yml:78–84 and .github/workflows/ci.yml:493–507). The adjacent
rationale at .github/dist-build-setup.yml:16–25 and releasing/.agents/workflow.md:22–24 also still implies a skipped
artifact build can create an empty GitHub release. Its host job may still start after a skip, but it now refuses missing
artifacts before publication (.github/workflows/release.yml:465–553, releasing/check-artifact-handoff.sh:19–40). No
later in-range commit corrected these descriptions. Suggested mitigation: point both instructions to the wrapper and
describe the pre-build test gate and artifact completeness gate at their respective boundaries.

Confidence: confirmed by pinned source inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `releaseGate/f1`.

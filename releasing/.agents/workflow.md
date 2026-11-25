# The release workflow

How the tag-triggered release workflow is generated and gated, what a change to release plumbing can check locally, how
to rehearse the gate, and where the recovery procedure for a broken GitHub release lives. It is the main file of the
release workflow edits reading list in `releasing/AGENTS.md`, and every release kind's reading list includes it for the
gate and the recovery procedure. A change to the release workflow also updates the check inventory in
`.agents/validation.md` in the same change (`AGENTS.md`, "Tests and validation").

Rustup selects `rust-toolchain.toml` from the checked-out release source, installing its exact compiler and components
on first use. The generated build jobs add their native or musl target to that toolchain. Toolchain bumps are manual PRs
changing the version in that file; the runner image's default stable does not select the release compiler.

`.github/workflows/release.yml` is generated from cargo-dist's template by `python3 releasing/generate-workflow.py`,
with the pinned dist on PATH. Edit the config, the build setup or the generator at the source. It runs on tag pushes
only (`pr-run-mode = "skip"`, D19), so a PR exercises none of it: the release path is validated when a tag is cut, by
the gate `.github/dist-build-setup.yml` puts at the top of every build job — the tag-shape assertion, the changelog
format and announcement checks, then the retained Rust targets, the desktop app's tests, the GPUI end-to-end smoke, the
OMP reporter's JS scenarios and CentOS provisioning on x86_64 Linux. The macOS job runs standalone uninstall validation
and the desktop app's and GPUI crates' tests. The Linux workspace selection includes the full farhelm integration suite,
with rootless Podman and its offline Ubuntu image prepared before execution. The GPUI smoke runs the debug app through
its automation bridge on a private headless Mutter display with Mesa's software renderer. Its recorder output goes to
the same failure evidence collection as the other release tests. The gate lives inside the build jobs rather than in a
`plan-jobs` workflow because dist 0.32 lets a failed plan job skip the build jobs, and its `host` job accepts a skip. A
gate failure inside the build job gives `host` a failed dependency directly. Farhelm's added artifact handoff checks
also block publication when the required outputs are missing.

The artifact downloader can succeed with the artifacts GitHub listed at that instant. Platform builds fetch the plan by
name. The checksum and host jobs require the plan, all three build manifests, all four archives and their sidecars
before dist consumes them. They retry the download once if the check fails. The host also requires the global manifest
and unified checksum. It stages the exact release inventory from that download and its own final manifest before
creating the release. The validator allows a short delay for GitHub's release read to catch up and downloads the
payloads by asset URL from the complete inventory it saw.

What a change to release plumbing CAN be checked locally is `dist plan` (the config parses and the asset list is what
you expect), `python3 releasing/generate-workflow.py --check` (the generated workflow is current), the release scripts'
own `--self-test` modes (`scripts/check-release-archive.py`, `scripts/check-static-elf.sh`,
`scripts/build-desktop-binary.sh`, `releasing/check-changelog.py`, `releasing/finalize-release-notes.py`,
`releasing/rehearse-release-gate.py`, `releasing/generate-workflow.py`), and `shellcheck` over the scripts the workflow
calls.

To get the gate's verdict in GitHub without cutting a release, `releasing/rehearse-release-gate.py` derives a throwaway
workflow from `release.yml` that runs the plan and per-target build jobs on a push to a `rehearsal/release-gate/...`
branch and publishes nothing; its docstring says what it changes and what a rehearsal therefore does not check (the two
tag assertions). `.agents/run-all-tests.md` is the procedure that pushes it. The generated file is never merged.

When a tag produces a GitHub release that is incomplete or failed validation, the recovery procedure is in
`dist-workspace.toml`'s header ("RECOVERY: a GitHub release that is incomplete or failed validation"). It is
maintainer-run, before anything for that version is signed or published on get.farhelm.io: delete the release, never the
tag, then re-run the workflow.

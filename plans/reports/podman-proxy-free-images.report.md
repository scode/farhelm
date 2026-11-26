## What this was about

When Farhelm builds a container recipe's images on a container host (the software image with the recipe's packages and
the agents, then the prepared image with Farhelm installed), it wrote the host's current proxy settings into a file the
container's user session reads at every boot. Both image captures kept that file. So the host account's Podman storage
held the build host's proxy settings, proxy passwords included, inside every recipe image. A container started from one
of those images booted with the build-time settings until its own setup replaced them, so for that window it could use a
proxy the host had since removed.

Your decision for this plan: scrub the file before new captures only. Do not track which images are proxy-free, and do
not force existing images to be rebuilt.

What the plan did: image builds no longer write the file at all. The software build gets the proxies only through each
build command's own private environment. The step that builds the prepared image (the preparer) also hands them to its
container's running user session, in memory only, so Farhelm's installer inside it sees them. Right before each software
and prepared image is saved, Farhelm removes the file. If the removal fails, it refuses to save the image. The removal
is also what hides the file from containers started from a prepared image made on a software image an earlier release
built; the file itself stays in that software image's layers (see below). New session containers still write the file
for themselves, with the host's current settings, as before.

## Things you should know

- After upgrading, the next launch of a recipe prepares a new image, because the Farhelm version changed. It builds on
  the recipe's existing software image, so new containers stop seeing the old file without a Rebuild. The old software
  image is not rebuilt, though, and its layers still hold the file, password included, in the host account's Podman
  storage. The new prepared image shares those layers, so the file stays on disk even though containers no longer see
  it. A Rebuild of the recipe replaces the software image and removes this leftover. That matches your decision not to
  force rebuilds, and SPEC_impl.md and the changelog draft both say so.
- The software build had a proxy step before. It is gone entirely. Every software build command already got the proxies
  through its own environment, and none of them runs under the container's user session.
- SPEC.md's "Container sessions" gained one sentence: recipe images Farhelm builds do not keep the building host's proxy
  settings, though one built by an earlier release keeps them until a Rebuild. SPEC_impl.md's proxy paragraph under
  "Podman places, creation and images" describes the new build behavior and the leftover.
- The TODO entry "Keep host proxy settings out of Podman recipe images." is removed in this PR.

## Open questions and possible follow-ups

- If you want the leftover credentials gone from existing hosts without users clicking Rebuild, that is the
  provenance-tracking and forced-rebuild work this plan left out by your decision. A narrower option: on upgrade, mark
  software images built before this release as needing a Rebuild in Settings, without forcing anything.

## PRs

- #5095 fix: keep host proxy settings out of podman recipe images (single PR: code, tests, SPEC.md and SPEC_impl.md,
  TODO removal, changelog fragment)

## Checks run, reused and skipped

- Run now: the helm's container recipe and launch-failure tests through the test recorder:
  `cargo nextest run -p farhelm-helm --lib -E 'test(/environment::podman::/) | test(/provisioning::session_launch_failure_tests::/)'`.
  The final run, 872069a1-a2ae-42eb-add8-0792141ca5d4, passed 44 of 44, including four new regression tests. The new
  tests cover these cases:
  - Builds write no boot file, and each capture removes it before it is saved.
  - A prepared image made on a software image that has the file comes out without it.
  - A failed removal refuses to save the image.
  - A new session after the host's proxies changed gets only the current settings, including a removed setting and a
    different one set to empty. This one starts from the upgraded state with an old software image.
- Run now: two deliberate mutation runs with the preparer's removal deleted, to show the tests catch its loss. Both
  failures were expected, and the code was restored after each:
  - c1609692-2d62-4a7c-8949-4fb2206700a4: three of the four new tests failed; the new-session test did not, which the
    review then flagged.
  - b259bb25-bd7a-4953-9a5b-99732674657a, after the new-session test was reworked to start from an old software image:
    it now fails too.
- Run now: `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test-sleep
  checker (0 unannotated delays), `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`.
- Skipped: the real Podman environment e2e suite (`podman_environments`). It needs a prepared rootless Podman host, the
  managed base image and network installs. The change is in the order of build steps, which the in-process stand-ins
  above cover command by command. What they cannot show is the removal and the in-memory proxy handoff running in a real
  rootless container's user session; the landing monitor may choose to run that suite for this.
- Skipped: the workspace-wide runs. Nothing outside the helm's container recipe code changed.

## Review gate

The review was a fresh-context Opus 5.5 review at high effort, asked for correctness against the acceptance criteria,
design fit and idiomatic code. It found no correctness bug and confirmed three things:

- Dropping the software build's proxy step breaks no build command.
- The removal runs only after Farhelm has re-checked that it is still talking to the same container host and account the
  recipe is being built on, like every other build command.
- No provenance or forced-rebuild machinery was added.

It raised two medium findings and four low ones, and all six were fixed:

- SPEC_impl.md had overclaimed that a prepared image made on an old software image no longer keeps the credentials. It
  does still keep them, in shared layers.
- The new-session test could not fail on its image premise. It now starts from an old software image that has the file.
  The second mutation run above shows it now fails when the preparer's removal is missing.
- The low findings covered three inaccurate comments and the changelog draft's wording.

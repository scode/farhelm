### What this was about

When a helm sets up or updates a host, it needs the Farhelm binaries to push to that host. A helm built for distribution
(with the web UI embedded, as releases are built) downloads them from the GitHub release matching its own version.
Builds of main carry the version `0.0.0-unreleased`, which no release will ever have. Such a helm, given neither
`--payload-dir` nor `--release-base-url`, got a 404 and told the user the release was not published yet and to retry in
a few minutes, which could never help.

The TODO entry behind this asked for a guard against provisioning a host with payloads older than the helm's own
protocol. During planning you were told that since main started carrying `0.0.0-unreleased`, a build from main can no
longer download an older release, and that the separate attach-refusal plan covers staged payloads (it makes the helm's
refusal of a host running an older supervisor name the protocol mismatch). You chose a small fix: make the failure say
plainly that the build is unreleased and to pass `--payload-dir`, and remove the TODO entry.

What the plan did: such a helm now refuses when it first needs a payload, before any download and before any change to
the host. The message is "this farhelm is an unreleased build (version 0.0.0-unreleased), so no published release
carries its provisioning payloads; pass --payload-dir <dir> holding payloads built from the same commit". Helm startup
is unaffected. `--release-base-url` still works on any build. A real release version downloads exactly as before.

### Things you should know

- **A plain developer build of main still gives the old advice.** A build without the embedded web UI (an ordinary
  `cargo build`) also carries `0.0.0-unreleased`, but it hits the older developer-build refusal first. That refusal says
  to pass `--payload-dir` "holding the release files", which is the advice this change avoids, because a release's files
  are the older-protocol payloads the helm would then refuse. Making every build of main give the new message would
  widen the change, so it was left alone.
- **The desktop app cannot follow the advice.** A desktop app built from main has no way to pass `--payload-dir`, so the
  new message, like the old one, names a remedy it cannot use. This is not a regression.
- **A `0.0.0-<prerelease>` release would now refuse its own payloads.** The check treats any `0.0.0` version with a
  prerelease as a build of main. If a release were ever tagged with such a version, its helm would refuse to download
  its own published payloads. Nothing in the release process forbids that version today.

### Open questions and possible follow-ups

- Should a plain developer build of main also give the new "unreleased build" message, instead of the "release files"
  advice? The recommendation is yes, as a small follow-up: it is the same situation, and the current advice points at
  payloads the helm would refuse.
- Whether to make the release process refuse a `0.0.0-<prerelease>` version. This is low priority, since such a tag is
  unlikely.

### The PRs

- #1500 (draft): fix: say plainly that an unreleased helm has no payloads to download.

### Checks

Run now:

- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings`: clean.
- Recorded nextest runs db9cc4f6 and 38e967c3 (after the review fixes): the helm's payload, provisioning-selection and
  build-order tests, 91 of 91.
- The changelog format check and `dprint check` on the changed Markdown.

Skipped:

- Every other test suite. The change is confined to how the helm picks its payload source, and those tests were run. The
  new refusal makes no network request, so no download test is involved.

### Review gate

A fresh-context Opus 5.5 reviewer at high effort, reviewing adversarially, found no correctness defect. It raised the
three points under "Things you should know", stale descriptions in two docstrings and the module overview, a test
docstring that overstated what it drove (a production-wiring check was added), an unsourced rationale in a docstring
(removed), and two nits, both fixed. Every code finding was fixed except the three noted above. The developer-build
message was deliberately not changed and is raised above as a follow-up.

# Cutting an RC release

The RC procedure: settling base and version (with the version default), the bump, the pre-tag checks, the tag, the
signing-and-install line for the maintainer, and closing the bump PR. It is the main file of the RC release reading list
in `releasing/AGENTS.md`, and the dev release reading list includes it, because a dev release follows it with `dev` in
place of `rc`.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

An RC exists so the maintainer can `curl | sh` a candidate onto a real machine before anything merges. The TAG, not any
merge, is what triggers the release workflow, so an RC can be cut from the tip of an unmerged PR stack — that is a
normal move here, not a trick (v0.2.1-rc.1 and rc.2 shipped this way on 2026-09-01, trialing the Farhelm.app bundle and
the clipboard fix before either landed).

An RC carries no curated notes. Its tag finds no `## vX.Y.Z` section (the section is written when the stable release is
cut), so dist publishes the download tables alone. The one exception is an RC cut after the stable section for the same
version has already landed on main: dist then falls back to that section and titles it with the RC version, which the
`announce` check reports as a note rather than a failure.

When asked for an RC, settle TWO choices first. Ask about each unless the request states it explicitly or the version
default described next applies; guessing wrong publishes the wrong binaries or version to a real, public prerelease:

- The BASE: cut from main, or from the current in-flight PR stack's tip? "cut an rc with this stack" is explicit; "cut
  an rc" is not.
- The VERSION: is this the next attempt at the SAME candidate — a previous `X.Y.Z-rc.N` exists for the target version
  and this continues it, so it is `X.Y.Z-rc.N+1` — or the FIRST rc of a new target version? If the latter, which
  component bumps is the maintainer's semantic call, not something to infer from the diff: patch (`X.Y.Z+1-rc.1`), minor
  (`X.Y+1.0-rc.1`), or major. The default described next makes that call "minor" unless the request names another
  version.

An explicit request for an RC release without a version has a default, decided by the most recently published release.
Check published releases including prereleases, ordered by publication time; GitHub's `releases/latest` endpoint
excludes prereleases and cannot answer this question.

- If the most recent release is `X.Y.Z-rc.N`, the request continues that candidate: `X.Y.Z-rc.N+1`. Do not fall back to
  an older RC.
- Otherwise (the most recent release is stable, a dev release, or anything other than an RC), the request starts a new
  minor candidate. Take the most recent stable `vA.B.C` and cut `A.B+1.0-rc.1`. The maintainer asked for this default on
  2026-09-29 so that starting a new candidate does not stop to ask for a version in the common case.

Announce the exact version and proceed without asking about it. Stop and ask instead when there is no published stable
release to bump from, or when the default's tag already exists (a dev release published after an RC can make the
"otherwise" branch land on a version that was already cut). An explicit version always wins, so a patch or major
candidate is requested by naming it. The base still needs to be stated or confirmed, and an existing tag must never be
reused.

With both settled, the process is:

- Start the risk report (`releasing/.agents/risk-report.md`, "The release risk report") and let it run while the rest of
  this list proceeds; it does not hold up the tag.
- Bump the version to `X.Y.Z-rc.N` (N increments per attempt; never reuse a tag name) in the root `Cargo.toml`'s
  `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, and refresh `Cargo.lock` (running `cargo metadata`
  suffices). The release commit is exactly those three files with the message `chore: release X.Y.Z-rc.N` — the shape
  every release commit here has (#304, #311, #314).
- Before tagging, run `cargo clean` (see `releasing/.agents/build-outputs.md`, "Build outputs"), then sanity-check the
  announce: the version-parity tests (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'`, through
  the recorder) and `dist plan` naming the rc version with BOTH packages under it — a version mismatch makes the desktop
  archive silently vanish from the release. Also `python3 releasing/check-changelog.py format`: the build gate lints
  every fragment on every tag, so a malformed fragment anywhere in the stack fails the rc build and costs an `rc.N`.
- Give the bump its own PR like any other commit (stacked on the stack tip, or based on main), but do not merge anything
  for the release's sake: push the tag `vX.Y.Z-rc.N` at the bump commit and the workflow runs from the tag. Its build
  gate runs the retained Rust targets, pinned shutdown regression, JS, CentOS, and native desktop checks while excluding
  the tmux e2e suite (see `AGENTS.md`, "Tests and validation", and the inventory in `.agents/validation.md`); that gate
  is the release's validation, so local slow-battery reruns are not a prerequisite for tagging.
- Watch the workflow run to completion rather than fire-and-forgetting it, then verify the GitHub release: every asset
  the release contract names present, no `SHA256SUMS` or `SHA256SUMS.minisig`, and the release marked prerelease
  (cargo-dist does that for `-rc.N` versions on its own).
- Hand the maintainer one copy-paste line that signs and publishes the release and then installs it. The signing half is
  `./bin/dev-release.sh vX.Y.Z-rc.N`, the maintainer's signing tooling (outside this repository, run on their trusted
  host from that tooling's own directory, never by an agent), which exits 0 once the release is signed and published at
  its version path on get.farhelm.io. The install half runs only after that succeeds, and uses the installer
  get.farhelm.io serves FOR THAT TAG (the one signed with the release) with the version pinned on the far side of the
  pipe:

  ```
  ./bin/dev-release.sh vX.Y.Z-rc.N && curl -fsSL https://get.farhelm.io/vX.Y.Z-rc.N/install.sh | FARHELM_VERSION=vX.Y.Z-rc.N sh
  ```

  The maintainer asked for this one line on 2026-10-07 so that trying an RC costs them only running it in the right
  directory and supplying the signing key's passphrase. Remind the maintainer that it runs from the signing tooling's
  directory, and to quit the desktop app before running it and relaunch after.
- Once the maintainer reports that it ran, check that `https://get.farhelm.io/vX.Y.Z-rc.N/SHA256SUMS`, its `.minisig`
  and `install.sh` answer, and that `https://get.farhelm.io/latest` still names the last stable release, so ordinary
  installs are unaffected. Then close the version-bump PR without merging it. The tag preserves the release commit; the
  PR does not need to stay open for the RC to remain available. Keep the tag and published release intact.
- Then `cargo clean` again (see `releasing/.agents/build-outputs.md`, "Build outputs").
- A failed tag build publishes nothing; fix forward (`releasing/.agents/fixing-forward.md`, "Fixing forward") and cut
  `rc.N+1`. The stale tag stays (tags are never deleted; the incomplete-release recovery in
  `releasing/.agents/workflow.md`, "The release workflow", deletes only a GitHub release, and keeps the tag).
- Close the version-bump PR without merging when its release attempt is permanently abandoned, including when a fix
  requires another RC and a new version-bump PR will supersede it. A temporary pause or a recoverable workflow rerun is
  not permanent abandonment; keep the PR open while that same release attempt remains active.
- One jj side effect to expect: once the rc tag is fetched, jj treats every commit under it as immutable, so a later
  mid-stack rewrite of those commits (a fixup round after the trial, say) needs `--ignore-immutable`. That is safe —
  rewriting creates new commits and the tag keeps pointing at what it tagged — but it will otherwise refuse with
  "immutable commits are used to protect shared history" at exactly the moment a trial's feedback wants applying.

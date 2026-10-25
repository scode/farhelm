# Releasing Farhelm

The index for cutting releases, publishing release advisories, curating the changelog and changing the release workflow:
the rules every release follows, then one reading list per role. The procedures themselves are in `releasing/.agents/`.
`dist-workspace.toml`'s header explains the release machinery itself and why it is shaped the way it is; these files are
the procedure that drives it. The rule every PR follows (leave a changelog fragment) is in `AGENTS.md`, "Changelog
fragments", not here.

After a compaction or resume, re-read every file in your role's reading list before your next action.

A release session is long-running. Keep its working log in the parent directory of the checkout, named
`farhelm-release-log-<checkout>.md` (`<checkout>` is the checkout directory's name), and record in it the active flow,
the release version, the reading list you are following, the step you are on, and the session's scratch directory. After
any compaction or resume, read that log first.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Rules for every release

Three kinds of release exist. A stable release `X.Y.Z` is cut from main, gets curated release notes, and is what
`https://get.farhelm.io/latest` names and the installer installs by default. An RC `X.Y.Z-rc.N` and a dev release
`X.Y.Z-dev.N` are prereleases for trying a build on a real machine; they carry no curated notes. The tag, not any merge,
triggers the release workflow.

No release tag is pushed before its base and its version are settled with the maintainer per the procedure for its kind,
a tag name is never reused, and a tag is never deleted. Each procedure settles both when the release is requested, by
its defaults or by asking right then, before any other work. After that, nothing before the tag's build waits for the
maintainer: the build gate takes the longest and finds the failures to fix forward, so it starts as soon as the pre-tag
checks pass.

The workflow builds the release, validates it, and publishes its archives on a GitHub release. It signs nothing: CI
never sees a signing key. A release reaches users only once the maintainer has signed its `SHA256SUMS` (the six payloads
plus its `install.sh`) and published it on get.farhelm.io, from a trusted host with tooling that deliberately lives
outside this repository; agents do not author, run, or replace that step. Until then nothing installs it: the installer
and the helm read get.farhelm.io only. `dist-workspace.toml`'s header and SPEC_impl.md's "Verification chain (D3)" say
why.

The installed app also reads `https://get.farhelm.io/advisories` and `advisories.minisig` on each update check. This is
a separate, mutable publication; changing `/latest` or a release's signed checksums is not a substitute. The
maintainer's external publishing tooling needs to support it. Agents do not author or run that tooling, sign with
release keys, or publish either file.

Main's version is always `0.0.0-unreleased`, in both the root `Cargo.toml` and `packaging/farhelm-desktop/dist.toml`.
Every kind of release sets its real version in a release commit that lives only under its tag: a stable one on its
`release-X.Y.Z` branch, an RC or dev one in a bump PR that is closed without merging. Never merge a release commit into
main, and never let a bump ride into main inside another PR (a squash merge of a PR stacked on a bump commit does
exactly that). Anything built from main then says plainly that it is not a release; the root `Cargo.toml` explains what
a release number on main would break.

When making a PR whose title type is `feat`, `fix`, `perf`, `style` or `revert`, or carries `!`, or whose change has a
user-facing effect, read `AGENTS.md`, "Changelog fragments", before committing the change. Fence: the fragment is added
in the same commit as the change.

Before changing release plumbing (`.github/workflows/release.yml`, `dist-workspace.toml`,
`.github/dist-build-setup.yml`), read `releasing/.agents/workflow.md` in full, and the rest of `releasing/AGENTS.md`,
"Release workflow edits", before the first edit. Fence: `release.yml` is generated: never hand-edit it; change the
config and regenerate. Changing the release workflow means updating the validation inventory in `.agents/validation.md`
in the same change.

## PR merges these procedures authorize

These are the only merges a release request authorizes. Each is spelled out, with its conditions, in the file cited.

- The stable release's changelog PR, and its finalization PR: `releasing/.agents/stable.md`, "Cutting a stable release",
  steps 3 and 4.
- A stable re-cut's changelog PR: `releasing/.agents/stable.md`, "Re-cutting a stable release".
- A fix forward's PR, marked ready and merged when the fix is straightforward and unambiguous:
  `releasing/.agents/fixing-forward.md`, "Fixing forward".

Nothing else merges for a release's sake. The release screenshot PR stays a draft until the maintainer asks for it to
land, an RC or dev bump PR is closed without merging, and a stable release commit never reaches main.

## Reading lists

Every list is complete: read each file it names in full before the first action of that role. Files marked as root
`AGENTS.md` sections are already in context for an agent started at the checkout root.

### Stable release ("cut a stable release", "release vX.Y.Z")

Before step 1 of `releasing/.agents/stable.md`, "Cutting a stable release" (the fragment sweep), read these in full:

- `releasing/.agents/stable.md`
- the `interactive-artifact` skill
- `releasing/.agents/changelog.md`
- `releasing/EDITORIAL_GUIDANCE.md`
- `releasing/.agents/risk-report.md`
- `releasing/.agents/build-outputs.md`
- `releasing/.agents/fixing-forward.md`
- `releasing/.agents/workflow.md`
- `releasing/AGENTS.md`, "Release screenshot subagent (fixed prompt)" (the files you name in that subagent's prompt)
- `releasing/AGENTS.md`, "Risk-report researchers (fixed prompt)" (when you split the risk report's research)
- `AGENTS.md`, "Agent scratch space", "Changelog fragments", "Talking to the user" and "Tests and validation" (root)
- `.agents/validation.md`
- `.agents/test-evidence.md` and `docs/test-run-evidence.md` (the version-parity tests run through the recorder)

Phase files: on reproducing a failed pre-tag check or gate failure, read `.agents/narrow-tests.md`. On rehearsing the
release gate, read `.agents/run-all-tests.md`.

Fence: every rule in `releasing/AGENTS.md`, "Rules for every release"; agents never run the maintainer's signing or
publish line and never move `/latest`; the procedure is not finished until the notes are finalized or the maintainer
said to leave that for later.

### RC release ("cut an rc", "cut an rc with this stack")

Before settling the base and version, read these in full:

- `releasing/.agents/rc.md`
- `releasing/.agents/risk-report.md`
- `releasing/.agents/build-outputs.md`
- `releasing/.agents/fixing-forward.md` (its pointer to `releasing/.agents/stable.md` applies to stable releases only)
- `releasing/.agents/workflow.md`
- `releasing/AGENTS.md`, "Risk-report researchers (fixed prompt)" (when you split the risk report's research)
- `AGENTS.md`, "Changelog fragments", "Talking to the user" and "Tests and validation" (root)
- `.agents/validation.md`
- `.agents/test-evidence.md` and `docs/test-run-evidence.md`

Phase files: on reproducing a failed pre-tag check or gate failure, read `.agents/narrow-tests.md`.

Fence: every rule in `releasing/AGENTS.md`, "Rules for every release"; ask about the base unless the request states it;
the bump PR is closed without merging; agents never run the signing and install line they hand the maintainer.

### Dev release ("cut a dev release", "cut a dev release with this stack")

A dev release follows the RC procedure, so this list is the RC list plus `dev.md`. Before settling the base and version,
read these in full:

- `releasing/.agents/dev.md`
- `releasing/.agents/rc.md`
- `releasing/.agents/risk-report.md`
- `releasing/.agents/build-outputs.md`
- `releasing/.agents/fixing-forward.md` (its pointer to `releasing/.agents/stable.md` applies to stable releases only)
- `releasing/.agents/workflow.md`
- `releasing/AGENTS.md`, "Risk-report researchers (fixed prompt)" (when you split the risk report's research)
- `AGENTS.md`, "Changelog fragments", "Talking to the user" and "Tests and validation" (root)
- `.agents/validation.md`
- `.agents/test-evidence.md` and `docs/test-run-evidence.md`

Phase files: on reproducing a failed pre-tag check or gate failure, read `.agents/narrow-tests.md`.

Fence: as for an RC, except that the RC version default does not apply: ask for the base and version unless the request
states them or a fix forward settled them.

### Fixing forward and stable re-cuts ("fix forward", "re-cut the release")

When a release attempt of any kind fails, before starting the fix, read these in full, on top of the reading list of the
release kind in progress:

- `releasing/.agents/fixing-forward.md`
- `AGENTS.md`, "Tests and validation" (root), and `.agents/validation.md`
- `.agents/test-evidence.md` and `.agents/narrow-tests.md`
- for a stable release: `releasing/.agents/stable.md` (the next attempt follows `releasing/.agents/stable.md`,
  "Re-cutting a stable release"), `releasing/.agents/changelog.md`, `releasing/EDITORIAL_GUIDANCE.md` and
  `releasing/.agents/risk-report.md`

Fence: a fix that needs a design decision, grows in scope, changes product behavior the specs do not settle, or has more
than one reasonable answer goes to the maintainer; a pushed tag's name is spent and is never reused or deleted.

### Release advisories ("withdraw release vX.Y.Z", "revoke vX.Y.Z", "lift the withdrawal")

Before preparing an advisories file, read these in full:

- `releasing/.agents/advisories.md`

Fence: agents prepare the content only; they do not author or run the publishing tooling, sign with release keys, or
publish either file.

### Release workflow edits

Before editing `.github/workflows/release.yml`, `dist-workspace.toml` or `.github/dist-build-setup.yml`, read these in
full:

- `releasing/.agents/workflow.md`
- `dist-workspace.toml`'s header
- `AGENTS.md`, "Tests and validation" (root), and `.agents/validation.md`
- `.agents/run-all-tests.md` (it pushes the gate rehearsal, and its lists change with the workflow)

Fence: `release.yml` is generated: never hand-edit it; change the config and regenerate. The generated rehearsal
workflow is never merged. A change to the release workflow updates the check inventory in `.agents/validation.md`, and
`.agents/run-all-tests.md`'s lists where they name the gate, in the same change.

### Changelog curation

Outside a stable release, before editing `CHANGELOG.md`, read these in full:

- `releasing/.agents/changelog.md`
- `releasing/EDITORIAL_GUIDANCE.md`
- `releasing/.agents/stable.md` (curation is its steps 1 to 4, and the changelog rules cite it)
- the `interactive-artifact` skill
- `AGENTS.md`, "Agent scratch space" and "Changelog fragments" (root)

Fence: a published release's section changes only as a correction a task specifically targets; wording the maintainer
approved is reproduced exactly.

### Release screenshot subagent (fixed prompt)

The stable procedure's step 4 gives this subagent a fixed prompt (`releasing/.agents/stable.md`, "Cutting a stable
release"). Its prompt also tells it to read these in full before its first action, starting with the root `AGENTS.md`
when the subagent runs outside a checkout or starts without it:

- `AGENTS.md` (root, in full)
- `website/AGENTS.md`
- `website/.agents/release-notes.md` and `website/.agents/screenshots.md`
- `website/EDITORIAL_RULES.md`
- `docs/docs-shots/SPEC.md`
- `docs/readme-hero/SPEC.md` (its staging rules govern the docs shots too)
- `releasing/AGENTS.md` (it governs `CHANGELOG.md`, which the Release notes page is generated from)
- `.agents/docs-media.md`
- `.agents/validation.md` (the website build)

Fence: work in a new jj workspace under the scratch directory, never the release checkout; no `cargo clean`, tags,
release branch or get.farhelm.io; the PR stays a draft and is never merged; no preview server.

### Risk-report researchers (fixed prompt)

`releasing/.agents/risk-report.md`, "Delegating the research", gives these read-only subagents a fixed instruction.
Their prompt also tells them to read these in full before their first action, starting with the root `AGENTS.md` when
the subagent runs outside a checkout or starts without it:

- `AGENTS.md` (root, in full)
- `releasing/.agents/risk-report.md`

Fence: no `git checkout`, `git switch`, `git reset`, `git stash`, any `jj` command, or anything else that moves HEAD,
the working tree, the index or refs; other revisions are read only through `git show`, `git diff`, `git log` and
`git grep <pattern> <rev>`.

### Release tooling edits

Before editing `releasing/check-changelog.py`, `releasing/finalize-release-notes.py`, `releasing/notes-review-page.py`,
`releasing/rehearse-release-gate.py` or `releasing/EDITORIAL_GUIDANCE.md`, read these in full:

- `releasing/.agents/changelog.md` (the checker must stay compatible with older sections)
- `releasing/.agents/stable.md` (the steps these scripts serve)
- `AGENTS.md`, "Tests and validation" (root), and `.agents/validation.md` (their self-tests)

Fence: a change to the changelog process applies to sections curated after it; published sections are never rewritten to
fit it.

### Mac VM test edits

Before editing anything under `releasing/mac-vm-test/`, read `releasing/mac-vm-test/BRICK-TEST.md` in full, including
"Validate changes to this coordinator".

### Release brick test ("brick test release vX.Y.Z", "stop after P0 checks", "stop after P1 checks", "did P0 checks succeed?", "did P1 checks succeed?")

When asked for a brick test, read `releasing/mac-vm-test/BRICK-TEST.md` in full before inspecting Tart state, then the
files `releasing/mac-vm-test/BRICK-TEST.md`, "Release brick tests", names.

Fence: a test request never authorizes publication or release promotion; a permission block during a test is a harness
bug: stop, and do not approve manually or rescue it through host UI.

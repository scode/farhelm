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
its defaults or by asking right then, before any other work. An RC or stable request also settles its in-app
announcements in that same reply, as described in `releasing/AGENTS.md`, "In-app announcements"; even a request that
states the base and version needs that answer. Dev releases are exempt, and fix-forward attempts and stable re-cuts
carry the settled announcements over without asking again. After those request-time decisions, nothing before the tag's
build waits for the maintainer: the build gate takes the longest and finds the failures to fix forward, so it starts as
soon as the pre-tag checks pass.

At request time, compare `UPGRADE_FLOOR` in `crates/farhelm-proto/src/version.rs` between the previous promoted stable
and the proposed product-source base. A previous release without the constant has the earlier documented v0.23.0
boundary. Tell the maintainer whether the floor differs. For a raise, show the old and new floors and the new floor
release's dated stable heading in `CHANGELOG.md`. Label that date as when the notes were written, not when the release
was promoted: the stable procedure writes notes before building and promotion. Show the actual promotion date and its
age when the available history establishes them; otherwise state that they are unknown, with the changelog date only as
context. Also show how many later stable releases were promoted through the previous stable, excluding RCs, dev
releases, draft sections and unpromoted tags. If the available history cannot establish promotion or that count, say so
rather than inferring either from a heading or a tag. The maintainer accepts the raise or chooses an earlier base before
release work proceeds. There is no minimum age or release count. Settle this alongside the request's other decisions,
never as a deferred pre-tag question. If the proposed product-source base changes, compare its floor again before
continuing: an unchanged floor reuses the answer, while a different floor needs the decision settled for that new base.
A re-cut follows the same rule. A change that needs earlier upgrade logic raises the constant and
`releasing/mac-vm-test/brick_test.py`'s matching literal in its own PR and explains the raise in its PR and changelog
fragment.

For a stable release, choosing an earlier base pauses the default main-based procedure before step 1. Settle a
compatible release path with the maintainer before continuing; this decision does not authorize silently copying,
cherry-picking or backporting source into the ordinary main-based release.

The workflow builds the release, validates it, and publishes its archives on a GitHub release. It signs nothing: CI
never sees a signing key. A release reaches users only once the maintainer has signed its `SHA256SUMS` and published it
on get.farhelm.io, from a trusted host with tooling that deliberately lives outside this repository; agents do not
author, run, or replace that step. Until then nothing installs it: the installer and the helm read get.farhelm.io only.
`dist-workspace.toml`'s header and SPEC_impl.md's "Verification chain (D3)" say why.

The signed `SHA256SUMS` lists exactly five files, sorted by name: the four archives
`farhelm-aarch64-apple-darwin.tar.gz`, `farhelm-aarch64-unknown-linux-musl.tar.gz`,
`farhelm-desktop-aarch64-apple-darwin.tar.gz` and `farhelm-x86_64-unknown-linux-musl.tar.gz`, then the release's
`install.sh`. Earlier releases also listed two bare tmux builds, `tmux-aarch64-unknown-linux-musl` and
`tmux-x86_64-unknown-linux-musl`; no release built from this source publishes them, and the `sign-sums` job refuses a
release that carries either, so signing tooling that still expects them must stop expecting them. That job's last step
prints the five digests in the signed file's form, to compare against.

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
- Its prepared screenshot PR, once the maintainer confirms the release is good and going out:
  `releasing/.agents/stable.md`, "Cutting a stable release", step 8. Preparation starts at notes approval, not at that
  confirmation.
- An RC's small announcements PR, before the tag, when the maintainer selected additions or changes:
  `releasing/.agents/rc.md`, "Cutting an RC release". It changes the announcement table on main, never the version.
- A stable re-cut's changelog PR: `releasing/.agents/stable.md`, "Re-cutting a stable release".
- A fix forward's PR, marked ready and merged when the fix is straightforward and unambiguous:
  `releasing/.agents/fixing-forward.md`, "Fixing forward".

Nothing else merges for a release's sake. The release screenshot PR stays a draft until the maintainer confirms the
release is good and going out; that confirmation authorizes the release agent to land it without a separate merge ask.
The screenshot subagent never merges it. An RC or dev bump PR is closed without merging, and a stable release commit
never reaches main. The temporary announcements PR beneath a stack-based RC's bump is closed without merging too; the
announcement text has already landed through the separate PR on main.

## In-app announcements

Only announcements the maintainer explicitly selects go into a release. Never select them from the changelog's Breaking
category or from fragments. The table is `crates/farhelm-helm/src/release_announcements.json`, initially `[]`; each
entry has `version`, `id`, `title` and `body`. It is built into the helm, so selected text must be in the release's
source before the tag, rather than added during release-note finalization.

In the reply that settles or announces the first RC's version and base, ask: "Which in-app announcements should be
included in this release candidate? None is fine." Ask even for a fully specified request such as "cut an rc with this
stack", before any other work. At every later RC and at the stable request, read main's current table and show the
entries for the whole target version cycle (`X.Y.Z` and its prereleases), with their exact versions, ids, titles and
bodies, or say that the list is empty. Ask: "Do you want to add or change any in-app announcements for this release? No
is enough." A stable request with no earlier RC asks the same question in its request-settling reply. This is the
cycle's selected list, never a prediction of what a particular helm will show. Wait for the answer before starting the
release work; the base, version and announcement questions belong together, not at separate later stops.

When offering to change an announcement, explain that an entry that already shipped keeps its id, and helms that already
enqueued it keep the old text. Rewording is not a way to show an announcement a second time. Reproduce the maintainer's
title and body exactly; ask about ambiguous wording or a validation problem rather than silently polishing it. Bodies
are plain text, with blank lines separating paragraphs.

Attach a new entry to the exact release being cut, without a leading `v`: `X.Y.Z-rc.N` at an RC, `X.Y.Z` at stable. Mint
its never-reused id as `<version>-<short-slug>`. Retain earlier entries and their original versions and ids. At an RC,
additions or changes land on main in a small PR before the tag; stable additions or changes ride in step 4's changelog
PR. No selected changes means no table edit or announcements PR on main; a stack-based RC may still need the temporary
table-carrying PR described in `releasing/.agents/rc.md`, "Cutting an RC release". Dev releases attach no announcements
and ask no announcement question. Fix-forward attempts and stable re-cuts carry the table over without new questions or
new copies of its entries, even when the next attempt's version changes.

Before merging a table edit, run the table validation with the pinned nextest setup from `docs/test-run-evidence.md`:

```
python3 scripts/record-test-run.py --runner nextest --kind release --selection 'release announcement table' --concurrency '4 nextest slots; one selected test; retries 0' --tmux none -- cargo nextest run -p farhelm-helm --lib -E 'test(release_announcement_table_is_valid)'
```

It checks unique ids, valid versions and non-empty text. Keep the procedure's existing changelog checks too. Use the
`chore` type for the small RC announcements PR; selecting announcement text needs no separate changelog fragment. The
underlying product changes still follow the ordinary fragment rules. Run `python3 releasing/check-changelog.py format`
before merging that PR so announcement work cannot carry a malformed fragment into the tag build. Mark it ready and
merge it under the release request's authorization after these checks pass. Never include a version bump in it.

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

Fence: work in a new independent colocated jj checkout (its default workspace) under the scratch directory, never the
release checkout; no `cargo clean`, tags, release branch or get.farhelm.io; the subagent keeps the PR draft and never
merges it; no preview server. The release agent lands it only after the maintainer confirms the release is good and
going out.

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

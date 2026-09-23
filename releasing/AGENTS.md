# Releasing Farhelm

Everything an agent needs to cut a release or to feed the changelog, in one place. The root `AGENTS.md` points here and
keeps only the one rule every PR has to follow (leave a changelog fragment). `dist-workspace.toml`'s header explains the
release machinery itself and why it is shaped the way it is; this file is the procedure that drives it.

Three kinds of release exist. A stable release `X.Y.Z` is cut from main, gets curated release notes, and is what
`install.sh` and `releases/latest` serve. An RC `X.Y.Z-rc.N` and a dev release `X.Y.Z-dev.N` are prereleases for trying
a build on a real machine; they carry no curated notes. The tag, not any merge, triggers the release workflow.

# The changelog

`CHANGELOG.md` at the repository root holds one section per stable release, newest first. cargo-dist finds the file on
its own (`CHANGELOG*` at the workspace root; the `auto-includes = false` setting only governs archive contents), takes
the section whose heading names the tagged version, and puts it in the GitHub release body above the download tables. It
also uses the heading text as the release's name. `releasing/check-changelog.py` holds the mechanical rules below, and
the release gate runs it, so a file dist would misread fails the tag build instead of publishing a release with the
wrong notes.

## Format

The layout is stipulated, not suggested. The checker refuses deviations, so read this before editing the file.

- The file opens with `# Changelog` and a short paragraph of prose. Nothing else sits above the first release.
- Each release is `## vX.Y.Z - YYYY-MM-DD`: the version first, with its `v`, then the date the section was written.
  dist's parser accepts nothing before the version except a `v`, `Version`, or `Release` prefix, so an emoji or any
  other word there hides the section from it, and everything after the version becomes part of the GitHub release name.
  There is no `Unreleased` section; pending changes live in fragments (below), which is what keeps a prerelease tag from
  picking up half-written notes.
- Inside a release, categories appear in this order and with these emoji, each omitted when it has nothing:

  | Heading             | Holds                                                                       |
  | ------------------- | --------------------------------------------------------------------------- |
  | `### ✨ Highlights` | prose subsections for anything that needs more than a sentence              |
  | `### 💥 Breaking`   | `feat!` and any other type marked `!`; anything a user must act on          |
  | `### 🚀 Added`      | `feat`                                                                      |
  | `### 🔄 Changed`    | `style`, and `feat` or `perf` that alters existing behavior                 |
  | `### 🔧 Fixed`      | `fix`, `perf` that fixes a performance problem                              |
  | `### 🗑️ Removed`     | removals that are not breaking in practice; breaking ones go under Breaking |

  Why these emoji: 💥 rather than 🔥 for Breaking because 🔥 is the common mark for removed code, and 🔄 rather than ♻️
  for Changed because recycling reads as refactoring. Category headings sit below the release level, so the parser never
  looks at them and the emoji stay out of the release name.
- Highlights is prose: one `####` subsection per item, each a title and one or more paragraphs, no bullets at the top
  level. Every other category is bullets: one `-` item per entry, one paragraph, ending in its PR reference as `(#N)` or
  `(#N, #M)`.
- One paragraph per physical line, however long. `CHANGELOG.md`, the fragments, and local curation drafts are excluded
  from dprint in `dprint.json` for this reason: GitHub renders a release body the way it renders a comment, with every
  newline as a line break, so a section hard-wrapped at 120 columns shows up ragged on the release page. The checker
  tolerates indented continuation lines as part of an item, but do not write them.
- An item that gets a highlight also gets its one-line bullet in its category. The category lists are complete on their
  own; Highlights is a reading aid, and a release with nothing worth a paragraph has no Highlights heading.
- A Breaking entry says what the user must do about it (update both halves together, re-run provisioning, drop a flag),
  not only what changed.
- Entries are written for someone running Farhelm. Name the feature as the UI or CLI names it, say what changed for
  them, and leave out module names, internal mechanisms, and the reason a simpler fix was rejected. Caveats and
  limitations belong in the entry ("launch only; no conversation tracking"), since the alternative is the user finding
  them.

## What gets an entry

Every commit on main whose Conventional Commit type is `feat`, `fix`, `perf`, `style`, or `revert`, and every commit of
any type marked `!`, is expected in the changelog of the next stable release. `style` is on the list because in this
repository it means UI styling, which users see. `docs`, `refactor`, `test`, `chore`, and `ci` are not expected; add one
by hand during curation when it matters to a user (a tmux pin change, say). A `revert` of something that shipped in a
stable release is an entry; a revert of something that never shipped only removes the original's fragment.

The rule of thumb is exactly that: if a required commit turns out to have nothing user-facing, the maintainer excludes
it at curation time and nothing else has to happen.

## Fragments: what a PR leaves behind

`releasing/changelog.d/` holds one file per pending change. A PR whose title has a required type adds one in the same
commit as the change, without asking; the agent writes it. A PR of any other type adds one when its change has a
user-facing effect, and otherwise nothing. A `fix` or `feat` that amends work not yet released (a follow-up to an
unreleased PR) edits that PR's fragment instead of adding a second entry; the sweep counts an edited fragment as
coverage. The file name is a free mnemonic (`cursor-launch.md`, `fix-detach-notification.md`), nothing parses it, and
`README.md` there is not a fragment.

```
---
kind: added
---

Cursor is a harness choice in the session launcher. This is launch only: Farhelm does not track a Cursor session's
conversation and cannot resume one.
```

The front matter is a leading `---` block, YAML-style, because a `---` line directly under text is a setext heading to
Markdown (the first draft of this format had dprint rewrite `kind: added` over a `---` into `## kind: added`); a leading
block is the one shape every Markdown tool treats as metadata. `kind:` is one of `breaking`, `added`, `changed`,
`fixed`, `removed`, or `none`, naming the category the entry lands in. `none` records a required commit that was
considered and has nothing user-facing; its body says why, so curation can tell an omission from a decision. An optional
`pr: 123` (or `pr: 123, 456`) line claims PR numbers when the fragment is written after its change merged, or covers
several PRs at once; a fragment added in the change's own commit needs no `pr:` because the sweep pairs it with the
commit that added it.

The body is a draft in user-facing voice, written after reading `releasing/EDITORIAL_GUIDANCE.md`, the maintainer's
accumulated wording rules: the first paragraph is the one-liner candidate, further paragraphs are material for a
highlight. Err toward including caveats. It is not reviewed at PR time and it is not the final text; curation rewrites
it. When the draft rests on a guess (a PR with no description, say), say so in the body so the curator verifies it.

## The checker

`releasing/check-changelog.py` has three modes and a `--self-test`; none of them writes anything.

- `format` lints `CHANGELOG.md` and every fragment against the rules above. The release gate runs it on every tag, and
  it belongs in the validation of any PR that touches either.
- `fragments` walks the commits since the merge base with the last stable `vX.Y.Z` tag (stable releases are tagged on a
  release branch that never merges, so the merge base is the honest "since"), reports each required commit as covered by
  a fragment it added or changed, or one that claims its PR, or as `MISSING`, and lists `STALE` fragments whose adding
  commit predates the range. A stale fragment means a change that shipped without notes: either a previous curation
  forgot to consume it, or its PR merged between the changelog PR and the release branch cut (which step 5 below exists
  to prevent). Bring it to the user; it is not automatically material for the next release. It needs the tags fetched.
- `announce --tag vX.Y.Z` compares what dist computes for the tag (`announcement_title` and `announcement_changelog` in
  its manifest, obtained by running `dist plan` locally or read from `--manifest`/`--manifest-env` in the gate) with the
  checker's own reading of the file. A stable tag must land on the newest section exactly; a prerelease must land on
  nothing, or on its own version's stable section when that already exists, which is dist's fallback and is tolerated.
  This is the check that catches a misparse before a release exists, and it only works once the workspace version
  matches the tag, so it runs after the version bump.

# Cutting a stable release

A stable release is cut from main. The changelog section lands on main FIRST, as its own PR; the release branch is cut
after it merges, so the bump commit keeps its three-file shape and main is the only place the notes ever live.

1. Fetch tags, then run `python3 releasing/check-changelog.py fragments`. Go through its report with the user: every
   required commit since the last stable release is included unless the user explicitly excludes it, `MISSING` ones get
   an entry written now from the commit and PR, `kind: none` fragments are shown to the user as proposed exclusions (the
   fragment author's judgment is not the user's decision), and `STALE` ones are raised as described under the checker.
   This is the sweep the fragment rule exists to make cheap.
2. Read `releasing/EDITORIAL_GUIDANCE.md`, then draft the section from the fragments and from whatever the user and the
   agent agree on in conversation, following that guidance. Decide which items earn a highlight. Write the whole
   proposed `## vX.Y.Z - YYYY-MM-DD` section to `releasing/drafts/vX.Y.Z.md` and give the maintainer its absolute path.
   This ignored local Markdown file is the review surface. The maintainer edits it in a text editor and tells the agent
   when to read it back. Read the file again after each editing round; do not reconstruct its contents from conversation
   or an earlier read. Continue until the maintainer approves the text. Keep the file until the changelog section is
   committed, then delete it.

   Treat the file's text and any wording the maintainer supplies as authoritative. Reproduce supplied wording and file
   edits exactly: do not summarize, paraphrase, polish, reorder, or silently correct them. If the agent proposes wording
   and the maintainer gives edits in conversation, apply precisely those edits to the proposal, without interpreting
   them as permission for other changes. An explicit request to write or rewrite text grants that latitude only for the
   requested text. Ask when an instruction is ambiguous. Format or checker problems must be brought back to the
   maintainer rather than silently changing reviewed wording. When transferring the approved section into
   `CHANGELOG.md`, copy the section verbatim, including its headings, whitespace, and line breaks; add only the
   separation needed between it and the surrounding changelog sections. Verify the copied section against the draft. Do
   not run a formatter over the draft or the approved section.
3. While iterating, watch for feedback that generalizes beyond the entry it was given on: a word the maintainer calls
   internal jargon, a shape of sentence they keep rewriting, a kind of detail they keep cutting or adding. Two ways a
   rule gets into `releasing/EDITORIAL_GUIDANCE.md`, and only two: the maintainer states it as a rule in so many words
   ("X is internal jargon, use Y"), in which case record it; or the agent infers a generalization from the edits, in
   which case ASK, naming the rule it would write, and record it only on a yes. Never write an inferred rule on the
   agent's own judgment; the file is the maintainer's opinions, and a guessed one would steer every future draft. Record
   each rule in the same changelog PR, with the example that prompted it. Feedback that only fixes the one entry is
   applied and not recorded.
4. Make the changelog PR: the approved `## vX.Y.Z - <today>` section at the top of `CHANGELOG.md`, every fragment under
   `releasing/changelog.d/` deleted (including `kind: none` ones; they were for this sweep), any guidance gathered in
   step 3, `dprint fmt`, and `python3 releasing/check-changelog.py format` passing. Merge it before going on.
5. Start a `release-X.Y.Z` branch at the changelog PR's merge commit EXACTLY, not at whatever main has become since:
   anything merged after it would ship in X.Y.Z without notes and leave its fragment behind as `STALE`. Confirm
   `releasing/changelog.d/` holds only `README.md` at that commit. Then bump the version to `X.Y.Z` in the root
   `Cargo.toml`'s `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, refresh `Cargo.lock`
   (`cargo metadata` suffices), and commit exactly those three files as `chore: release X.Y.Z`.
6. Before tagging: the version-parity tests (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'`
   through the recorder), `dist plan --tag vX.Y.Z` naming BOTH packages under the version (a mismatch makes the desktop
   archive silently vanish), and `python3 releasing/check-changelog.py announce --tag vX.Y.Z`, which must print that
   dist's announcement matches.
7. Push the tag `vX.Y.Z` at the bump commit and watch the workflow to completion. Then verify the release: every asset
   present including `SHA256SUMS` and `SHA256SUMS.minisig`, the release NOT marked prerelease, `releases/latest`
   pointing at it, and the release page showing the changelog section above the download tables with the heading text as
   its name.
8. Hand the maintainer the ordinary install command and remind them to quit the desktop app before updating.

A failed tag build publishes nothing; fix on main and cut again with the next patch version, since a tag name is never
reused. That means another changelog PR before the new branch: retitle the `## vX.Y.Z` section to the new version (a
stable `announce` fails unless the newest section names the tag exactly) and fold in the fix commits' fragments. The
abandoned release branch is left as it is, like the earlier ones.

# Cutting an RC release

An RC exists so the maintainer can `curl | sh` a candidate onto a real machine before anything merges. The TAG, not any
merge, is what triggers the release workflow, so an RC can be cut from the tip of an unmerged PR stack — that is a
normal move here, not a trick (v0.2.1-rc.1 and rc.2 shipped this way on 2026-09-01, trialing the Farhelm.app bundle and
the clipboard fix before either landed).

An RC carries no curated notes. Its tag finds no `## vX.Y.Z` section (the section is written when the stable release is
cut), so dist publishes the download tables alone. The one exception is an RC cut after the stable section for the same
version has already landed on main: dist then falls back to that section and titles it with the RC version, which the
`announce` check reports as a note rather than a failure.

When asked for an RC, settle TWO choices first. Ask about each unless the request states it explicitly or the version
default below applies; guessing wrong publishes the wrong binaries or version to a real, public prerelease:

- The BASE: cut from main, or from the current in-flight PR stack's tip? "cut an rc with this stack" is explicit; "cut
  an rc" is not.
- The VERSION: is this the next attempt at the SAME candidate — a previous `X.Y.Z-rc.N` exists for the target version
  and this continues it, so it is `X.Y.Z-rc.N+1` — or the FIRST rc of a new target version? If the latter, which
  component bumps is the maintainer's semantic call, not something to infer from the diff: patch (`X.Y.Z+1-rc.1`), minor
  (`X.Y+1.0-rc.1`), or major. Name the exact resulting version string when asking, so the answer is a version, not a
  category.

An explicit request for an RC release without a version means `X.Y.Z-rc.N+1` IF AND ONLY IF the most recently published
release is `X.Y.Z-rc.N`. Check published releases including prereleases, ordered by publication time; GitHub's
`releases/latest` endpoint excludes prereleases and cannot answer this question. Announce the exact next version and
proceed without asking about it. If the most recent release is stable, a dev release, or anything other than an RC — or
there is no published release — still ask about the version. Do not fall back to an older RC. An explicit version always
wins, the base still needs to be stated or confirmed, and an existing tag must never be reused.

With both settled, the process is:

- Bump the version to `X.Y.Z-rc.N` (N increments per attempt; never reuse a tag name) in the root `Cargo.toml`'s
  `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, and refresh `Cargo.lock` (running `cargo metadata`
  suffices). The release commit is exactly those three files with the message `chore: release X.Y.Z-rc.N` — the shape
  every release commit here has (#304, #311, #314).
- Before tagging, sanity-check the announce: the version-parity tests
  (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'`, through the recorder) and `dist plan`
  naming the rc version with BOTH packages under it — a version mismatch makes the desktop archive silently vanish from
  the release. Also `python3 releasing/check-changelog.py format`: the build gate lints every fragment on every tag, so
  a malformed fragment anywhere in the stack fails the rc build and costs an `rc.N`.
- Give the bump its own PR like any other commit (stacked on the stack tip, or based on main), but do not merge anything
  for the release's sake: push the tag `vX.Y.Z-rc.N` at the bump commit and the workflow runs from the tag. Its build
  gate runs the retained Rust targets, pinned shutdown regression, JS, CentOS, and native desktop checks while excluding
  the tmux e2e suite (see "Finishing work" in the root `AGENTS.md`); that gate is the release's validation, so local
  slow-battery reruns are not a prerequisite for tagging.
- Watch the workflow run to completion rather than fire-and-forgetting it, then verify the published release: every
  asset present including `SHA256SUMS` and `SHA256SUMS.minisig`, and the release marked prerelease (cargo-dist does that
  for `-rc.N` versions on its own — `releases/latest` must still point at the last stable, so ordinary installs are
  unaffected).
- Finish by handing the maintainer the exact copy-paste command, with the installer fetched FROM THE TAG — when the rc
  comes from a stack, main does not have the rc's installer — and the version pinned on the far side of the pipe:

  ```
  curl -fsSL https://raw.githubusercontent.com/scode/farhelm/vX.Y.Z-rc.N/scripts/install.sh | FARHELM_VERSION=vX.Y.Z-rc.N sh
  ```

  Remind the maintainer to quit the desktop app before updating and relaunch after.
- A failed tag build publishes nothing; fix on the stack and cut `rc.N+1`. The stale tag stays (tags are never deleted;
  the unsigned-release recovery below is the one exception's procedure, and even it keeps the tag).
- One jj side effect to expect: once the rc tag is fetched, jj treats every commit under it as immutable, so a later
  mid-stack rewrite of those commits (a fixup round after the trial, say) needs `--ignore-immutable`. That is safe —
  rewriting creates new commits and the tag keeps pointing at what it tagged — but it will otherwise refuse with
  "immutable commits are used to protect shared history" at exactly the moment a trial's feedback wants applying.

# Cutting a dev release

A dev release is an RC under another name: `X.Y.Z-dev.N`, tagged `vX.Y.Z-dev.N`, cut by exactly the procedure above with
`dev` in place of `rc` everywhere — the bump commit is `chore: release X.Y.Z-dev.N`, N increments per attempt and a tag
name is never reused, the workflow runs from the tag and marks the release a prerelease (any semver prerelease suffix
does; `releases/latest` still points at the last stable), and `scripts/install.sh` accepts
`FARHELM_VERSION=vX.Y.Z-dev.N` the same way it accepts an `-rc.N`. Settle the same two choices first, base and version,
and ask when the request does not state them; the RC version default above does not apply to dev releases. The `-dev.N`
and `-rc.N` counters are independent, so `0.3.0-dev.2` and `0.3.0-rc.1` can both exist. The name is the whole
difference: it tells whoever reads the tag list later that the build was a trial of work in progress, not a claim that
this is what will ship as `X.Y.Z`.

# The release workflow

`.github/workflows/release.yml` is generated by cargo-dist from `dist-workspace.toml` — never hand-edit it; change the
config (or `.github/dist-build-setup.yml`) and run `dist init --yes && dprint fmt` to regenerate. It runs on tag pushes
only (`pr-run-mode = "skip"`, D19), so a PR exercises none of it: the release path is validated when a tag is cut, by
the gate `.github/dist-build-setup.yml` puts at the top of every build job — the tag-shape assertion, the changelog
format and announcement checks, then the retained Rust targets, pinned shutdown regression, JS harness, CentOS
provisioning, desktop unit test, and desktop smoke on the x86_64 Linux one; an Apple CLI compile, standalone uninstall
validation and desktop unit test on macOS. The tmux-driven e2e suite remains out of the Linux gate until its
load-sensitive tests are deflaked; TODO.md lists the condition for putting it back. That gate lives inside the build
jobs rather than in a `plan-jobs` workflow because dist 0.32 lets a failed plan job SKIP the build jobs, and its `host`
job accepts a skip; a failure inside a build job is the only kind it refuses.

What a change to release plumbing CAN be checked locally is `dist plan` (the config parses and the asset list is what
you expect), `dist generate --check` (the generated workflow is current), the release scripts' own `--self-test` modes
(`scripts/check-release-archive.py`, `scripts/check-static-elf.sh`, `scripts/check-desktop-assets.sh`,
`releasing/check-changelog.py`), and `shellcheck` over the scripts the workflow calls.

When a tag produces a public release that never got its `SHA256SUMS`, the recovery procedure is in
`dist-workspace.toml`'s header ("RECOVERY: a release that exists but was never signed"). It is maintainer-run: delete
the release, never the tag, then re-run the workflow.

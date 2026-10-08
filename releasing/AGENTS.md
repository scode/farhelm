# Releasing Farhelm

Everything an agent needs to cut a release or to feed the changelog, in one place. The root `AGENTS.md` points here and
keeps only the one rule every PR has to follow (leave a changelog fragment). `dist-workspace.toml`'s header explains the
release machinery itself and why it is shaped the way it is; this file is the procedure that drives it.

Three kinds of release exist. A stable release `X.Y.Z` is cut from main, gets curated release notes, and is what
`https://get.farhelm.io/latest` names and the installer installs by default. An RC `X.Y.Z-rc.N` and a dev release
`X.Y.Z-dev.N` are prereleases for trying a build on a real machine; they carry no curated notes. The tag, not any merge,
triggers the release workflow.

The workflow builds the release, validates it, and publishes its archives on a GitHub release. It signs nothing: CI
never sees a signing key. A release reaches users only once the maintainer has signed its `SHA256SUMS` (the six payloads
plus its `install.sh`) and published it on get.farhelm.io, from a trusted host with tooling that deliberately lives
outside this repository; agents do not author, run, or replace that step. Until then nothing installs it: the installer
and the helm read get.farhelm.io only. `dist-workspace.toml`'s header and SPEC_impl.md's "Verification chain (D3)" say
why.

Main's version is always `0.0.0-unreleased`, in both the root `Cargo.toml` and `packaging/farhelm-desktop/dist.toml`.
Every kind of release sets its real version in a release commit that lives only under its tag: a stable one on its
`release-X.Y.Z` branch, an RC or dev one in a bump PR that is closed without merging. Never merge a release commit into
main, and never let a bump ride into main inside another PR (a squash merge of a PR stacked on a bump commit does
exactly that). Anything built from main then says plainly that it is not a release; the root `Cargo.toml` explains what
a release number on main would break.

# The changelog

`CHANGELOG.md` at the repository root holds one section per stable release, newest first. cargo-dist finds the file on
its own (`CHANGELOG*` at the workspace root; the `auto-includes = false` setting only governs archive contents), takes
the section whose heading names the tagged version, and puts it in the GitHub release body above the download tables. It
also uses the heading text as the release's name. `releasing/check-changelog.py` holds the mechanical rules below, and
the release gate runs it, so a file dist would misread fails the tag build instead of publishing a release with the
wrong notes.

The docs website's Release notes page is built from the same file, every release section in it, by
`website/scripts/release-notes.mjs` on each website build. Nothing extra is needed for a release to appear there, but a
release heading the generator cannot read (anything at the `##` level other than the format below) fails the website
build. Screenshots for a release live on the website only (`website/AGENTS.md`, "Release notes"), never in this file;
step 7 of the stable procedure below has them added for each release.

Curated release sections are historical records. Once a section has been approved or published, do not rewrite it to
adopt a later layout rule, wording convention, or category set. A change to this process applies to sections curated
after the change; the checker must retain compatibility with older sections so a release in flight does not require
retroactive edits. When a new format is introduced, update the checker and its fixtures to recognize both the new format
and the older formats that remain in the file.

The current compatibility registry preserves the Highlights layout in `v0.12.0` and `v0.13.0`. It is a record of
already-curated sections, not permission to use that layout in a new release.

## Format

The layout below is the current format for new sections. It is a curation guide, not a migration instruction for older
sections; the checker accepts the historical formats that are already present.

- The file opens with `# Changelog` and a short paragraph of prose. Nothing else sits above the first release.
- Each release is `## vX.Y.Z - YYYY-MM-DD`: the version first, with its `v`, then the date the section was written.
  dist's parser accepts nothing before the version except a `v`, `Version`, or `Release` prefix, so an emoji or any
  other word there hides the section from it, and everything after the version becomes part of the GitHub release name.
  There is no `Unreleased` section; pending changes live in fragments (below), which is what keeps a prerelease tag from
  picking up half-written notes.
- Inside a release, categories appear in this order and with these emoji, each omitted when it has nothing:

  | Heading           | Holds                                                                       |
  | ----------------- | --------------------------------------------------------------------------- |
  | `### 💥 Breaking` | `feat!` and any other type marked `!`; anything a user must act on          |
  | `### 🚀 Added`    | `feat`                                                                      |
  | `### 🔄 Changed`  | `style`, and `feat` or `perf` that alters existing behavior                 |
  | `### 🔧 Fixed`    | `fix`, `perf` that fixes a performance problem                              |
  | `### 🗑️ Removed`   | removals that are not breaking in practice; breaking ones go under Breaking |

  Why these emoji: 💥 rather than 🔥 for Breaking because 🔥 is the common mark for removed code, and 🔄 rather than ♻️
  for Changed because recycling reads as refactoring. Category headings sit below the release level, so the parser never
  looks at them and the emoji stay out of the release name.
- Every category is bullets: one `-` item per entry, one paragraph, ending in its PR reference as `(#N)` or `(#N, #M)`.
- One paragraph per physical line, however long. `CHANGELOG.md`, the fragments, and local curation drafts are excluded
  from dprint in `dprint.json` for this reason: GitHub renders a release body the way it renders a comment, with every
  newline as a line break, so a section hard-wrapped at 120 columns shows up ragged on the release page. The checker
  tolerates indented continuation lines as part of an item, but do not write them. The curation draft is the one
  exception: it is wrapped for reading and unwrapped when it moves into `CHANGELOG.md` (step 2 of the stable release
  procedure).
- A Breaking entry says what the user must do about it (update both halves together, re-run provisioning, drop a flag),
  not only what changed.
- That a release cannot be downgraded from is not Breaking, and it gets no entry at all. Most releases upgrade the
  helm's or the supervisor's database, so an older build refusing to open it afterwards is the normal case, not news.
  The maintainer ruled this on 2026-10-07. The risk report's downgrade check still tells the maintainer, who is the one
  reader that needs it. Sections up to and including `v0.23.0` carry such entries; like every curated section, they stay
  as they are.
- A release that changes the helm–supervisor protocol version opens its Breaking category with exactly this entry, and a
  release that does not change it never carries it:

  ```
  - This release *requires* you to update your remote hosts. (#N, #M)
  ```

  The references are the PRs that changed the protocol version, in merge order. Nothing else varies: no explanation of
  the change, no reworded variant, and no second entry saying the same thing in other words. Builds on different
  protocol versions refuse to connect to each other (SPEC.md's compatibility rules), so every such release strands the
  hosts left on the old version at `needs update`. The fixed wording and the rule that it appears every time are the
  point: a reader who does not find the line can rely on its absence to mean their hosts keep working unchanged, which
  holds only while no release omits it. Sections up to and including `v0.16.0` predate the rule, and several of them
  changed the protocol without saying so; their silence means nothing, and like every curated section they stay as they
  are.
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
accumulated wording rules: the first paragraph is the entry candidate, and further paragraphs supply context for
curation. Err toward including caveats, except that the release cannot be downgraded from, which no entry mentions (see
the format). It is not reviewed at PR time and it is not the final text; curation rewrites it. When the draft rests on a
guess (a PR with no description, say), say so in the body so the curator verifies it.

## The checker

`releasing/check-changelog.py` has three modes and a `--self-test`; none of them writes anything.

- `format` lints `CHANGELOG.md` and every fragment against the current rules plus the historical section formats the
  checker still supports. It checks structure and references; it does not rewrite, normalize, or migrate curated release
  text. The release gate runs it on every tag, and it belongs in the validation of any PR that touches either.
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

   In the same step, ALWAYS check whether the helm–supervisor protocol version changed since the last stable release,
   and tell the user the answer either way. Compare `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` at the merge
   base the sweep uses and at the release base, and list the commits that changed it:

   ```
   base=$(git merge-base vPREV origin/main)
   git log --oneline -G 'PROTOCOL_VERSION: u32 = [0-9]+' "$base"..origin/main -- crates/farhelm-proto/src/lib.rs
   ```

   (`vPREV` is the last stable tag.) Neither the sweep nor the fragments can be trusted to surface this: a bump often
   rides in a `refactor`, which needs no fragment, or inside a feature whose fragment is about something else. When the
   version changed, the section carries the fixed remote-hosts entry described under the format, citing those commits'
   PRs; when it did not, it carries no such entry.

   Start the risk report ("The release risk report" below) now too. It must reach the maintainer before the tag is
   pushed in step 7.
2. Read `releasing/EDITORIAL_GUIDANCE.md`, then draft the section from the fragments and from whatever the user and the
   agent agree on in conversation, following that guidance. Write the whole proposed `## vX.Y.Z - YYYY-MM-DD` section to
   `releasing/drafts/vX.Y.Z.md` and give the maintainer its absolute path. This ignored local Markdown file is the
   review surface. Unlike `CHANGELOG.md`, the draft is hard-wrapped at 120 columns, with each continuation line of an
   entry indented two spaces under its `-`, so the maintainer can read and edit it as raw Markdown. Keep that wrapping
   through every editing round, and rewrap an entry the maintainer's edits push past 120 columns only by moving line
   breaks, never by changing words. The maintainer edits it in a text editor and tells the agent when to read it back.
   Read the file again after each editing round; do not reconstruct its contents from conversation or an earlier read.
   Continue until the maintainer approves the text. Keep the file until the changelog section is committed, then delete
   it.

   Treat the file's text and any wording the maintainer supplies as authoritative. Reproduce supplied wording and file
   edits exactly: do not summarize, paraphrase, polish, reorder, or silently correct them. If the agent proposes wording
   and the maintainer gives edits in conversation, apply precisely those edits to the proposal, without interpreting
   them as permission for other changes. An explicit request to write or rewrite text grants that latitude only for the
   requested text. Ask when an instruction is ambiguous. Format or checker problems must be brought back to the
   maintainer rather than silently changing reviewed wording. When transferring the approved section into
   `CHANGELOG.md`, the one permitted transformation is unwrapping: join each entry's continuation lines onto its `-`
   line, replacing each line break and the indentation after it with a single space, so every paragraph is one physical
   line as the format requires. Copy everything else verbatim, including headings, blank lines, and every word; add only
   the separation needed between the section and the surrounding changelog sections. Verify the copied section by
   unwrapping the draft the same way and comparing it against the copy. Do not run a formatter over the draft or the
   approved section.
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
6. Before tagging: `cargo clean` (see "Build outputs" below), then the version-parity tests
   (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'` through the recorder),
   `dist plan --tag vX.Y.Z` naming BOTH packages under the version (a mismatch makes the desktop archive silently
   vanish), and `python3 releasing/check-changelog.py announce --tag vX.Y.Z`, which must print that dist's announcement
   matches.
7. Push the tag `vX.Y.Z` at the bump commit and watch the workflow to completion. Then verify the GitHub release: every
   asset the release contract names present and NO `SHA256SUMS` or `SHA256SUMS.minisig` (those belong on get.farhelm.io;
   the workflow refuses them), the release NOT marked prerelease, and the release page showing the changelog section
   above the download tables with the heading text as its name. Tell the maintainer it is ready to sign and publish at
   its version path, and that `/latest` must not move yet. Once they have, check get.farhelm.io (read-only):
   `https://get.farhelm.io/vX.Y.Z/SHA256SUMS`, its `.minisig` and `install.sh` answer, and
   `https://get.farhelm.io/latest` still names the previous stable release.

   When you tell the maintainer the release is ready to sign, start a subagent in that same turn to give the release's
   notes their screenshots on the docs website, so they are ready by the time the release is promoted. Do not wait for
   the signing. The notes' text is already there, built from the changelog section step 4 merged; what the subagent adds
   is screenshots of the release's visible changes, following `website/AGENTS.md`, "Release notes", and
   `website/EDITORIAL_RULES.md`. Give it this, in so many words:
   - Work in a new jj workspace based on the latest `main`, under a subdirectory of your scratch directory, never in the
     release checkout, which is busy with the release branch. Do not run `cargo clean` or touch tags, the release
     branch, or anything on get.farhelm.io.
   - Read the `## vX.Y.Z` section of `CHANGELOG.md` and decide which entries change something a reader would see: a new
     dialog or control, or a redesign. Most entries do not, and a release with none gets no PR; say so and stop.
   - For each that does, add a shot to `e2e/docs-shots/release-notes.spec.ts` and an item to
     `website/src/release-notes/vX.Y.Z.json`, run the full capture (`scripts/docs-screenshots.sh`, no `--only`), look at
     every image it prints and fix placement, then run `scripts/publish-docs-shots.sh` and confirm the website builds.
   - Open the result as a draft PR per the `jjstack` skill, with its commit message cold-read per the
     `scode-commit-msg-reviewer` skill, and do not merge it. It starts no preview server, being unattended.

   Pass the PR link on to the maintainer when the subagent reports it, with a note that the website changes only when
   someone deploys it. The subagent's result does not hold up the steps below.
8. Stop for the brick test. The release is now staged: anyone can install it by naming its version, and nothing installs
   it by default. Tell the maintainer it is ready to brick test, and give them the line to hand the agent on the Mac
   test host: `brick test release vX.Y.Z` (`releasing/mac-vm-test/BRICK-TEST.md`). That test runs on the maintainer's
   Mac, in Tart VMs, so this session does not run it; it installs the staged release and upgrades the previous stable
   releases to it, which is the check that the release does not brick existing installations (SPEC.md, "Upgrade
   compatibility and client scale"). Give the maintainer the risk report's ranked risks and manual checks as material
   for the test's per-release addendum. Then wait for the maintainer's verdict; do not treat silence, or a test that
   could not run, as a pass.
   - Pass: the maintainer moves `/latest`, which no agent does. Once they say it has moved, check that
     `https://get.farhelm.io/latest` names `vX.Y.Z`, and go on.
   - Fail: `/latest` stays on the previous release and the release is never promoted. Handle it like a failed tag build
     (below): fix on main and cut the next patch version. Whether to take the staged version off get.farhelm.io is the
     maintainer's call.
   - Could not run: ask the maintainer whether to fix the test environment and run it again, or to promote without it.
     Only the maintainer decides to skip the test.
9. Hand the maintainer the ordinary install command and remind them to quit the desktop app before updating.
10. `cargo clean` again (see "Build outputs" below).

A failed tag build publishes nothing; fix on main and cut again with the next patch version, since a tag name is never
reused. A release that failed its brick test is the same case: it was signed and staged but never promoted, and the next
attempt is the next patch version. That means another changelog PR before the new branch: retitle the `## vX.Y.Z`
section to the new version (a stable `announce` fails unless the newest section names the tag exactly) and fold in the
fix commits' fragments. The abandoned release branch is left as it is, like the earlier ones.

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
  (`X.Y+1.0-rc.1`), or major. The default below makes that call "minor" unless the request names another version.

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

- Start the risk report ("The release risk report" below) and let it run while the rest of this list proceeds; it does
  not hold up the tag.
- Bump the version to `X.Y.Z-rc.N` (N increments per attempt; never reuse a tag name) in the root `Cargo.toml`'s
  `[workspace.package]` and `packaging/farhelm-desktop/dist.toml`, and refresh `Cargo.lock` (running `cargo metadata`
  suffices). The release commit is exactly those three files with the message `chore: release X.Y.Z-rc.N` — the shape
  every release commit here has (#304, #311, #314).
- Before tagging, run `cargo clean` (see "Build outputs" below), then sanity-check the announce: the version-parity
  tests (`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::assets)'`, through the recorder) and
  `dist plan` naming the rc version with BOTH packages under it — a version mismatch makes the desktop archive silently
  vanish from the release. Also `python3 releasing/check-changelog.py format`: the build gate lints every fragment on
  every tag, so a malformed fragment anywhere in the stack fails the rc build and costs an `rc.N`.
- Give the bump its own PR like any other commit (stacked on the stack tip, or based on main), but do not merge anything
  for the release's sake: push the tag `vX.Y.Z-rc.N` at the bump commit and the workflow runs from the tag. Its build
  gate runs the retained Rust targets, pinned shutdown regression, JS, CentOS, and native desktop checks while excluding
  the tmux e2e suite (see "Finishing work" in the root `AGENTS.md`); that gate is the release's validation, so local
  slow-battery reruns are not a prerequisite for tagging.
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
- Then `cargo clean` again (see "Build outputs" below).
- A failed tag build publishes nothing; fix on the stack and cut `rc.N+1`. The stale tag stays (tags are never deleted;
  the incomplete-release recovery below deletes only a GitHub release, and keeps the tag).
- Close the version-bump PR without merging when its release attempt is permanently abandoned, including when a fix
  requires another RC and a new version-bump PR will supersede it. A temporary pause or a recoverable workflow rerun is
  not permanent abandonment; keep the PR open while that same release attempt remains active.
- One jj side effect to expect: once the rc tag is fetched, jj treats every commit under it as immutable, so a later
  mid-stack rewrite of those commits (a fixup round after the trial, say) needs `--ignore-immutable`. That is safe —
  rewriting creates new commits and the tag keeps pointing at what it tagged — but it will otherwise refuse with
  "immutable commits are used to protect shared history" at exactly the moment a trial's feedback wants applying.

# Cutting a dev release

A dev release is an RC under another name: `X.Y.Z-dev.N`, tagged `vX.Y.Z-dev.N`, cut by exactly the procedure above with
`dev` in place of `rc` everywhere — the bump commit is `chore: release X.Y.Z-dev.N`, N increments per attempt and a tag
name is never reused, the workflow runs from the tag and marks the release a prerelease (any semver prerelease suffix
does; get.farhelm.io's `latest` still names the last stable), and `scripts/install.sh` accepts
`FARHELM_VERSION=vX.Y.Z-dev.N` the same way it accepts an `-rc.N`. Settle the same two choices first, base and version,
and ask when the request does not state them; the RC version default above does not apply to dev releases. The `-dev.N`
and `-rc.N` counters are independent, so `0.3.0-dev.2` and `0.3.0-rc.1` can both exist. The name is the whole
difference: it tells whoever reads the tag list later that the build was a trial of work in progress, not a claim that
this is what will ship as `X.Y.Z`.

# The release risk report

Every release attempt, stable, RC or dev, comes with a risk report for the maintainer, delivered without being asked. It
answers two questions: which of the changes going out are most likely to break the release badly, and whether the
release blocks a downgrade. The maintainer asked for it as a default on 2026-10-02, after requesting it by hand for
v0.22.0-rc.1 and v0.22.0-rc.2.

NOTE: The report is research done by reading diffs, not validation. It does not replace the tag's build gate, and it
does not gate an RC or dev tag: start it as soon as the base and version are settled, let it run alongside the bump and
the pre-tag checks, and deliver it when it is ready. A stable release is different only in timing: have the report in
front of the maintainer before pushing the tag (step 7 of the stable procedure), since a stable tag reaches every
ordinary install.

## Which commits

The report covers what is new since the previous published release of any kind, prereleases included (the same lookup
the RC version default uses). A continued candidate therefore reports only what landed since the last `rc.N`, and a
first candidate or a stable release cut right after a stable one covers everything since that stable release. Release
commits live only under their tags, and an RC may have come from an unmerged stack, so take the range from the merge
base rather than from the tag itself:

```
git log --oneline "$(git merge-base vPREV BASE)"..BASE
```

`vPREV` is the previous published release's tag and `BASE` is the commit the release is cut from. Docs-only commits get
a quick `git diff --stat` to confirm they touch nothing shipped, and nothing more.

## Downgrade check

Compare against the last stable release, because that is what an ordinary install falls back to, and also against the
previous published release when that is a different one. Report the numbers either way, including when nothing moved:

```
for rev in vPREV BASE; do
  git show "$rev:crates/farhelm-helm/src/store.rs" | grep 'const SCHEMA_VERSION'
  git show "$rev:crates/farhelm-supervisor/src/store.rs" | grep 'const SCHEMA_VERSION'
  git show "$rev:crates/farhelm-proto/src/lib.rs" | grep 'PROTOCOL_VERSION: u32'
done
```

The two schema versions are what actually block a downgrade. The helm and the supervisor each migrate their database
forward when they open it, and each refuses to open a database whose version is newer than its own ("refusing to open it
rather than risk misreading it"). So once a build with a higher number has run against a state directory, an older build
cannot use that directory, and the half whose number moved cannot be downgraded in place. A protocol version change
blocks nothing on its own, but builds on different protocol versions refuse to connect to each other, so a downgrade has
to take the helm and every host it talks to back together, and hosts left on the other side sit at `needs update`. For a
stable release this is the same protocol question step 1 already asks; answer it once and use it for both.

The constants do not cover every persisted format. Read the diff for anything else an older build would fail to read: a
versioned state file whose version moved, a new variant in an enum that is stored rather than only sent over the wire, a
renamed or relocated file. Say what was checked and what was not.

## Risk review

Rank the changes most likely to significantly break the release: the helm or a supervisor failing to start or refusing
to connect, adding, installing, updating or provisioning hosts breaking, the desktop app failing to start or sign in,
state or sessions lost or corrupted, running agents killed, or the UI wedged across the fleet. Look hardest at code that
runs unconditionally at startup or on every request, at new refusals and validation that could reject input real
installs already rely on, and at anything that changes how different versions of the helm and supervisor interact.
Narrow edge cases with mild failure modes go in the low-risk bucket.

For each ranked risk, give the change and its PR, a concrete failure scenario, what in a real deployment would trigger
it, how likely that seems, and a manual check the maintainer can run on the release to smoke it out. Close with one line
naming the rest as low risk. Write it in the terms of "Talking to the user" in the root `AGENTS.md`: what the user sees,
not internal names.

## Delegating the research

Splitting the commit range across read-only subagents works well and keeps the diffs out of the main context. Tell every
one of them, in so many words, not to run `git checkout`, `git switch`, `git reset`, `git stash`, any `jj` command, or
anything else that moves HEAD, the working tree, the index or refs, and to read other revisions only through `git show`,
`git diff`, `git log` and `git grep <pattern> <rev>`. The release checkout is busy while they work: on 2026-10-02 a
research agent asked only to read ran `git checkout` to look at another revision, swapped the release checkout's files
back to the previous release in the middle of the version-parity build, and that run had to be thrown away and repeated
from `cargo clean`. Verify a delegate's headline claims (a version number, a refusal the review says is new) against the
code before passing them on.

# Build outputs

Cargo files every workspace crate's build outputs under a hash that includes the package version, and every release
attempt changes the version. So each attempt's builds land beside the previous attempt's instead of replacing them, and
nothing ever removes the old ones. A checkout used for releases grew past 100 GB that way in about two weeks: dozens of
copies of the version-parity test binary with their incremental caches, plus the reproductions of gate failures run
between attempts. A release attempt therefore starts and ends with `cargo clean` in the checkout it is cut from:

- Right before the version-parity tests, the attempt's first build. This also clears what an earlier attempt left
  behind, including one whose session ended before its own cleanup. It costs little: the version bump rebuilds every
  workspace crate regardless, so only the dependencies are rebuilt that would otherwise have been reused.
- When the attempt is over: the release is published and verified, or the attempt is abandoned. Not while a gate failure
  is still being reproduced in that checkout, since the reproduction wants its warm build.

`cargo clean` removes the checkout's whole `target/`, including the dx bundle and any earlier development build there.
The pinned tmux and nextest (`.ci-tmux`, `.ci-nextest`) live outside it and stay.

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

When a tag produces a GitHub release that is incomplete or failed validation, the recovery procedure is in
`dist-workspace.toml`'s header ("RECOVERY: a GitHub release that is incomplete or failed validation"). It is
maintainer-run, before anything for that version is signed or published on get.farhelm.io: delete the release, never the
tag, then re-run the workflow.

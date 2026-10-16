# The changelog

How `CHANGELOG.md` is laid out, which commits it must cover, and what `releasing/check-changelog.py` checks. It is on
the stable release and changelog curation reading lists in `releasing/AGENTS.md`. The rule every PR follows (leaving a
changelog fragment) is in `AGENTS.md`, "Changelog fragments"; "Fragments: what a PR leaves behind" here has its full
text.

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

`CHANGELOG.md` at the repository root holds one section per stable release, newest first. cargo-dist finds the file on
its own (`CHANGELOG*` at the workspace root; the `auto-includes = false` setting only governs archive contents), takes
the section whose heading names the tagged version, and puts it in the GitHub release body above the download tables. It
also uses the heading text as the release's name. `releasing/check-changelog.py` holds the mechanical rules in
`releasing/.agents/changelog.md`, "Format", and the release gate runs it, so a file dist would misread fails the tag
build instead of publishing a release with the wrong notes.

The docs website's Release notes page is built from the same file, every release section in it, by
`website/scripts/release-notes.mjs` on each website build. Nothing extra is needed for a release to appear there, but a
release heading the generator cannot read (anything at the `##` level other than the format in
`releasing/.agents/changelog.md`, "Format") fails the website build. Screenshots for a release live on the website only
(`website/.agents/release-notes.md`), never in `CHANGELOG.md`; step 4 of `releasing/.agents/stable.md`, "Cutting a
stable release", has them added for each release.

Curated release sections are historical records. Once the maintainer has approved a section, do not rewrite it to adopt
a later layout rule, wording convention, or category set. A section that still opens with the draft notice (see
`releasing/.agents/changelog.md`, "Format") has not been approved yet, even though its tag may already be public, and
replacing it with the approved text is the procedure working as intended, not a rewrite. The same goes for a stable
re-cut putting the notice back on a section it extends (`releasing/.agents/stable.md`, "Re-cutting a stable release"):
the approved entries in it stay word for word. More generally, a published release's section is never part of a change
by default, a sweep or a rename included; it changes only as a correction a task specifically targets. A change to this
process applies to sections curated after the change; the checker must retain compatibility with older sections so a
release in flight does not require retroactive edits. When a new format is introduced, update the checker and its
fixtures to recognize both the new format and the older formats that remain in the file.

The current compatibility registry preserves the Highlights layout in `v0.12.0` and `v0.13.0`, and the single
`### 🔧 Fixed` category in every section up to and including `v0.25.0`. It is a record of already-curated sections, not
permission to use those layouts in a new release.

## Format

The layout this section lists is the current format for new sections. It is a curation guide, not a migration
instruction for older sections; the checker accepts the historical formats that are already present.

- The file opens with `# Changelog` and a short paragraph of prose. Nothing else sits above the first release.
- Each release is `## vX.Y.Z - YYYY-MM-DD`: the version first, with its `v`, then the date the section was written.
  dist's parser accepts nothing before the version except a `v`, `Version`, or `Release` prefix, so an emoji or any
  other word there hides the section from it, and everything after the version becomes part of the GitHub release name.
  There is no `Unreleased` section; pending changes live in fragments (`AGENTS.md`, "Changelog fragments"), which is
  what keeps a prerelease tag from picking up half-written notes.
- While the maintainer is still reviewing a section's wording, its first line, directly under the heading, is exactly
  `**DRAFT release notes - not yet finalized**`. The stable tag is pushed with that draft (see
  `releasing/.agents/stable.md`, "Cutting a stable release"), and the notice travels with it into the GitHub release, so
  anyone who reads the release page before it is finalized knows the text will change. Only the newest section may carry
  it, and the approved text never does. The checker holds the canonical copy.
- Inside a release, categories appear in this order and with these emoji, each omitted when it has nothing:

  | Heading                           | Holds                                                                               |
  | --------------------------------- | ----------------------------------------------------------------------------------- |
  | `### 💥 Breaking`                 | `feat!` and any other type marked `!`; anything a user must act on                  |
  | `### 🚀 Added`                    | `feat`                                                                              |
  | `### 🔄 Changed`                  | `style`, and `feat` or `perf` that alters existing behavior                         |
  | `### 🔧 Fixes worth highlighting` | the fixes a reader should not miss (see `releasing/.agents/changelog.md`, "Format") |
  | `### 🗑️ Removed`                  | removals that are not breaking in practice; breaking ones go under Breaking         |
  | `### 🩹 Misc fixes`               | every other fix                                                                     |

  Why these emoji: 💥 rather than 🔥 for Breaking because 🔥 is the common mark for removed code, 🔄 rather than ♻️ for
  Changed because recycling reads as refactoring, and 🩹 for Misc fixes because a bandage reads as a small repair.
  Category headings sit below the release level, so the parser never looks at them and the emoji stay out of the release
  name.
- Fixes are split in two. Fixes worth highlighting holds a fix (a `fix`, or a `perf` that fixes a performance problem)
  when a non-trivial share of users is likely to notice it, meaning user-facing behavior on a path people commonly take
  rather than an obscure corner, or when it fixes a genuinely serious bug: lost or corrupted state or sessions, killed
  agents, a helm or host that will not start or connect, a security problem. A serious bug that only an extremely rare
  edge case could ever hit does not qualify. Everything else goes under Misc fixes, which is the kind of thing most
  projects sum up as one "various bug fixes" bullet; the maintainer wants the complete list published anyway, for
  transparency. The curating agent makes the initial split on its own judgment and the maintainer reviews it with the
  rest of the draft. Fragments do not decide it: `kind: fixed` covers both categories, because whether a fix stands out
  depends on the rest of the release.
- Misc fixes always opens with exactly this paragraph, then its bullets, and like every category it is omitted when it
  has nothing:

  ```
  *Fixes that would normally be summed up as a single "various bug fixes" line. In the interest of transparency, the complete list is included.*
  ```

  The wording is fixed so that it is not one more thing to review per release; the checker holds the canonical copy.
- Every category is bullets, after Misc fixes' fixed opening paragraph: one `-` item per entry, one paragraph, ending in
  its PR reference as `(#N)` or `(#N, #M)`.
- One paragraph per physical line, however long. `CHANGELOG.md` and the fragments are excluded from dprint in
  `dprint.json` for this reason: GitHub renders a release body the way it renders a comment, with every newline as a
  line break, so a section hard-wrapped at 120 columns shows up ragged on the release page. The checker tolerates
  indented continuation lines as part of an item, but do not write them.
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

`AGENTS.md`, "Changelog fragments", carries the part of this section every PR needs, as a shorter selection of the same
sentences; this is the full text. When you change a rule here, check the root copy too.

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
`fixed`, `removed`, or `none`, naming the category the entry lands in (`fixed` lands in one of the two fix categories,
chosen at curation). `none` records a required commit that was considered and has nothing user-facing; its body says
why, so curation can tell an omission from a decision. An optional `pr: 123` (or `pr: 123, 456`) line claims PR numbers
when the fragment is written after its change merged, or covers several PRs at once; a fragment added in the change's
own commit needs no `pr:` because the sweep pairs it with the commit that added it.

The body is a draft in user-facing voice, written after reading `releasing/EDITORIAL_GUIDANCE.md`, the maintainer's
accumulated wording rules: the first paragraph is the entry candidate, and further paragraphs supply context for
curation. Err toward including caveats, except that the release cannot be downgraded from, which no entry mentions (see
the format). It is not reviewed at PR time and it is not the final text; curation rewrites it. When the draft rests on a
guess (a PR with no description, say), say so in the body so the curator verifies it.

## The checker

`releasing/check-changelog.py` has three modes and a `--self-test`; none of them writes anything.

- `format` lints `CHANGELOG.md` and every fragment against the current rules plus the historical section formats the
  checker still supports, including the draft notice on the newest section. It checks structure and references; it does
  not rewrite, normalize, or migrate curated release text. The release gate runs it on every tag, and it belongs in the
  validation of any PR that touches either.
- `fragments` walks the commits since the merge base with the last stable `vX.Y.Z` tag (stable releases are tagged on a
  release branch that never merges, so the merge base is the honest "since"), reports each required commit as covered by
  a fragment it added or changed, or one that claims its PR, or as `MISSING`, and lists `STALE` fragments whose adding
  commit predates the range. A stale fragment means a change that shipped without notes: either a previous curation
  forgot to consume it, or its PR merged between the changelog PR and the release branch cut (which step 5 of
  `releasing/.agents/stable.md`, "Cutting a stable release", exists to prevent). Bring it to the user; it is not
  automatically material for the next release. It needs the tags fetched.
- `announce --tag vX.Y.Z` compares what dist computes for the tag (`announcement_title` and `announcement_changelog` in
  its manifest, obtained by running `dist plan` locally or read from `--manifest`/`--manifest-env` in the gate) with the
  checker's own reading of the file. A stable tag must land on the newest section exactly; a prerelease must land on
  nothing, or on its own version's stable section when that already exists, which is dist's fallback and is tolerated.
  This is the check that catches a misparse before a release exists, and it only works once the workspace version
  matches the tag, so it runs after the version bump.

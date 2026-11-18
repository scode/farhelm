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
  | `### 🩹 Misc fixes`               | historical category for other fixes                                                 |

  Why these emoji: 💥 rather than 🔥 for Breaking because 🔥 is the common mark for removed code, 🔄 rather than ♻️ for
  Changed because recycling reads as refactoring, and 🩹 for Misc fixes because a bandage reads as a small repair.
  Category headings sit below the release level, so the parser never looks at them and the emoji stay out of the release
  name.
- Fixes worth highlighting covers repairs likely to matter to a typical user and critical fixes that affect a small
  group of users. Examples include lost or corrupted sessions, killed agents, a helm or host that will not start or
  connect, or a security problem. The maintainer reviews the selection with the draft. Older releases used Misc fixes
  for the remaining fixes. The checker still accepts those sections.
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
- Releases through v0.39.0 used the following entry to flag a helm–supervisor protocol change. From v0.40.0, updating
  remote hosts is treated as part of an ordinary upgrade:

  ```
  - This release *requires* you to update your remote hosts. (#N, #M)
  ```

  Those references name the PRs that changed the protocol version, in merge order. Builds on different protocol versions
  refuse to connect (SPEC.md's compatibility rules). Keep checking the version for the maintainer's release testing.
- v0.40.0 opens with a one-time `### Changes to release notes going forward` section. Its bullets describe editorial
  policy and carry no PR references. The first Changed entry also omits references because the desktop migration spanned
  hundreds of PRs.
- Entries are written for someone running Farhelm. Name the feature as the UI or CLI names it, say what changed for
  them, and leave out module names, internal mechanisms, and the reason a simpler fix was rejected. Caveats and
  limitations belong in the entry ("launch only; no conversation tracking"), since the alternative is the user finding
  them.

## What gets an entry

Review every commit on main whose Conventional Commit type is `feat`, `fix`, `perf`, `style`, or `revert`, and every
commit of any type marked `!`, when curating the next stable release. `style` includes UI styling users see. Use
`releasing/EDITORIAL_GUIDANCE.md` to select changes for the public notes, including changes outside those types when
they matter to the reader (a raised requirement on a tool users install, say). A `revert` of something that shipped in a
stable release is an entry; a revert of something that never shipped only removes the original's fragment.

A fix is listed as a fix only when the bug it fixes reached users in a stable release. Release candidates and dev builds
do not count; their changes reach users under the stable release that follows them. So before sorting fixes into the two
fix categories, check each one against the previous stable tag: when the code or feature it fixes is not in that tag, it
gets no fix entry. A fix to something new in this release folds into that feature's entry, which cites its PR and
describes the feature as it now behaves; anything else is dropped. A fix that is partly both keeps a fix entry for the
part that shipped, and only that part. A fragment's `kind: fixed` does not settle this: a fix to unreleased work should
have edited that work's fragment instead (`AGENTS.md`, "Changelog fragments"), and often has not. The maintainer made
this the default for every release on 2026-10-16, after the v0.29.0 sweep turned up ten fixes whose bugs had only ever
existed between stable releases.

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

## Plans' release notes

Work built through the planning system (`plans/AGENTS.md`) gets its release note settled while it is planned, before
anyone has written a line of code. The planner offers an entry on the plan's questions page, and the maintainer accepts
it, edits it, writes their own, or opts out in one of two fixed ways (`plans/.agents/planning.md`, "Asking the up-front
questions on a local page"). The answer goes into the plan file as one `## Release note` section, whose body is exactly
one of:

- the entry: its first paragraph is the entry text, and any further paragraphs are context for the release agent, as in
  a fragment. It is the maintainer's wording, whether they wrote it or accepted the planner's proposal.
- `No release note needed.`
- `No release note now; ask during the release.`

The two fixed answers are spelled exactly so, because `releasing/check-changelog.py plans` tells them from an entry by
their text. A plan file written before this rule has no section; curate its work from the fragments as before. A
`## Decisions` entry that changes the release note wins over the section, like any Decision over the plan body.

The plan file is usually gone by the time of the release, since approving a landed plan deletes it.
`python3 releasing/check-changelog.py plans` reads every plan whose landing was recorded since the last stable release
from the commit that recorded it, where the file still exists. For each plan it prints the PRs that landed it, split by
whether they merged in this release's range or before it, its release note, whether it has Decisions, whether an earlier
round landed before the last stable release, and a `git show` line for the plan file as it stood at that landing. Read
the Decisions of every plan that has them before acting on its note, whatever the note says: a block answer or a
follow-up can replace an opt-out with wording, or wording with an opt-out.

Which release a plan's work belongs to is decided by where its PRs merged, not by when its landing was recorded. The
landing monitor merges a plan's PRs and records the landing afterwards, and a plan that blocks partway through its
landing has PRs on main and no record at all. So:

- A plan whose every PR merged before this range shipped in an earlier release: its note is not this release's, and it
  gets no entry here.
- A plan still listed in `plans/queue/INDEX.md` can already have PRs on main (a landing in progress, or one that blocked
  partway). The `plans` mode does not list it. Curate those PRs from their fragments, and say in the handover message
  which plan they belong to, so the maintainer knows its note was not used.

At curation, each plan's answer works like this:

- An entry is the default entry for the work it describes, used word for word: the maintainer supplied that wording, and
  `releasing/.agents/stable.md`, "Cutting a stable release", step 2, applies to it. The only changes allowed without
  asking are the format's: one `-` bullet on one physical line (dprint wraps plan files, so the paragraph arrives
  hard-wrapped), and its PR references. It cites the plan's PRs that merged in this range and that do what the entry
  describes, which by default is every one of them that carries a required type or a fragment. The plan's entry takes
  the place of those PRs' fragments that describe the same work, and of any entry step 1 would write for one of them
  that is `MISSING`. A fragment from one of the plan's PRs that describes something else (a pre-existing bug fixed along
  the way, say) is curated as usual. The agent sorts the entry into a category by the rules in
  `releasing/.agents/changelog.md`, "Format".
- `No release note needed.` leaves the plan's work out of the draft: the fragments that describe it, and any entry step
  1 would write for a `MISSING` commit of the plan's. This is the maintainer's decision, not a proposed exclusion, but
  it is listed with the proposed exclusions in step 2's handover message so the maintainer sees what it removed.
- `No release note now; ask during the release.` means the maintainer wants to write this one at release time. Draft an
  entry from the fragments (or from the PRs, when they left only `kind: none` fragments or none) as for any other work,
  and mark it on the review page as the plan the maintainer deferred, asking them to supply their own text or approve
  the draft.
- A malformed note (the `plans` mode says so: an empty section, or more than one) is treated as no note: curate the work
  from the fragments, and call it out so the maintainer can supply the entry.

Two rules in this file fix what an entry must or must not say, and they win over a plan's wording, because a reader
relies on them holding in every release: the rule in `releasing/.agents/changelog.md`, "What gets an entry", that a fix
to unreleased work folds into that work's entry rather than standing as a fix, and the rule in
`releasing/.agents/changelog.md`, "Format", that no entry mentions downgrades. Where one applies, the draft follows the
rule (folding the fix, cutting the caveat), and the page shows the plan's wording exactly as it was, beside the callout
that says which rule changed it. Nothing else changes the maintainer's wording.

Before handing over the review page, check every plan entry, and every "no release note needed", against what actually
landed, and call out the obvious inaccuracies you can detect. The entry was written against a plan, and plans drift: a
block answer or a follow-up changes the scope, an executor takes a different fork than the plan expected, a name in the
UI comes out differently, a part never lands. The evidence is the plan file and its Decisions at the landing, the plan's
report (`git show` it at the same commit, under `plans/reports/`), the PRs' diffs and descriptions, and their fragments,
which say where the built work departs from the plan's entry (`plans/.agents/executing.md`, "Executing one plan", step
8). The checks, at least:

- the entry describes behavior, a name, a control, a flag or a limitation that differs from what landed;
- it describes something that did not land in this release, or that an earlier round already shipped in a stable release
  (the `plans` mode flags a plan with an earlier round, and PRs that merged before the range);
- it would be a fix entry for a bug that never reached a stable release;
- it breaks a rule in `releasing/.agents/changelog.md`, "Format", or `releasing/EDITORIAL_GUIDANCE.md` (internal names,
  a missing action for a Breaking entry), including the rules above that win over its wording;
- it says the same thing as another entry in the draft;
- "no release note needed" on work whose PRs made a user-facing change.

A callout is a highlight on the review page, never an edit to the maintainer's text except where a rule above wins: say
what looks wrong and the evidence (a PR, a Decision, a line of the report), and offer replacement wording when it is
obvious, for the maintainer to accept as a suggestion. The scratch Markdown file keeps the maintainer's entry as it was
until they change it. A plan with no bullet in the draft (one whose note said no release note was needed, a deferred one
with nothing to draft from, a malformed one) still gets a row on the page with its PR links, so a callout on it has a
place to sit and the maintainer can still add an entry there. Name every plan with a callout in the message that hands
over the page, too. "Obvious" sets the bar on purpose: this is a check for mistakes a careful reader of the PRs would
catch, not a license to second-guess the maintainer's choice of words.

## The checker

`releasing/check-changelog.py` has five modes and a `--self-test`; none of them writes anything.

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
- `plans` lists the plans whose landing was recorded since that same merge base, with each one's release note as its
  plan file held it at the landing and its PRs split by whether they merged in the range
  (`releasing/.agents/changelog.md`, "Plans' release notes"). It exits non-zero for a malformed `## Release note`
  (empty, or more than one) or a landing record that does not match what the queue script writes. It needs the tags
  fetched.
- `plan-note FILE...` checks that each plan file carries exactly one non-empty `## Release note` section. The planning
  flow runs it on every plan file a planning PR adds or revises (`plans/.agents/planning.md`, "Planning: "plan to
  implement <TODO items>"").
- `announce --tag vX.Y.Z` compares what dist computes for the tag (`announcement_title` and `announcement_changelog` in
  its manifest, obtained by running `dist plan` locally or read from `--manifest`/`--manifest-env` in the gate) with the
  checker's own reading of the file. A stable tag must land on the newest section exactly; a prerelease must land on
  nothing, or on its own version's stable section when that already exists, which is dist's fallback and is tolerated.
  This is the check that catches a misparse before a release exists, and it only works once the workspace version
  matches the tag, so it runs after the version bump.

# Release notes

This file covers the docs site's generated Release notes page and the screenshots a release's entries can show. The
fence that published release material is frozen is in `website/AGENTS.md`, "Published release notes are frozen", and
governs everything here. It belongs to the "Release notes work" reading list in `website/AGENTS.md`.

The Release notes page (`/docs/release-notes/`, the sidebar entry under Overview) is generated from the repository's
`CHANGELOG.md` by `website/scripts/release-notes.mjs` on every build and preview start, and is gitignored. Do not edit
the page; a change to the notes themselves is a change to `CHANGELOG.md`, which `releasing/AGENTS.md` governs. Each
release has a stable anchor named after its version, such as `/docs/release-notes/#v0.23.0`.

An entry can show screenshots of what it changed, behind a closed **show screenshot** link under the entry's text.
`website/src/release-notes/<version>.json` (for example `website/src/release-notes/v0.25.0.json`) lists them, naming
each entry by a pull request number the entry cites; the generator places them, and fails the build when no entry of
that release cites the number. `CHANGELOG.md` and the GitHub release stay text only. The screenshots are ordinary docs
shots: tests in `e2e/docs-shots/release-notes.spec.ts` named `release-notes/v<X-Y-Z>-<what>`, captured and published
with every other shot until their first publish, after which they are frozen (`website/AGENTS.md`, "Published release
notes are frozen"). Give a screenshot only to an entry whose change is visible: a new dialog or control, or a redesign,
not a fix or behaviour a picture cannot show. Most entries get none, and a release with no visible change gets no file.

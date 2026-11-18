# Release notes

This file covers the docs site's generated Release notes page and the screenshots a release's entries can show. The
fence that published release material is frozen is in `website/AGENTS.md`, "Published release notes are frozen", and
governs everything here. It belongs to the "Release notes work" reading list in `website/AGENTS.md`.

The Release notes page (`/docs/release-notes/`, the sidebar entry under Overview) is generated from the repository's
`CHANGELOG.md` by `website/scripts/release-notes.mjs` on every build and preview start, and is gitignored. Do not edit
the page; a change to the notes themselves is a change to `CHANGELOG.md`, which `releasing/AGENTS.md` governs. Each
release has a stable anchor named after its version, such as `/docs/release-notes/#v0.23.0`. A release from v0.42.0 on
can end with its folded full list below the highlights, an HTML details block titled All other changes
(`releasing/.agents/changelog.md`, "Format"); the generator passes its frame lines through as markup, so it starts
closed on the page as it does on GitHub, and Starlight's own styling for details blocks draws it.

An entry can show screenshots of what it changed, behind a closed **show screenshot** link under the entry's text.
`website/src/release-notes/<version>.json` (for example `website/src/release-notes/v0.25.0.json`) lists them, naming
each entry by a pull request number the entry cites; the generator places them, and fails the build when no entry of
that release cites the number. `CHANGELOG.md` and the GitHub release stay text only. Add a `Shot` named
`v<X-Y-Z>-<what>` in `scripts/docs-shots/release-notes.py` and put its full name, `release-notes/v<X-Y-Z>-<what>`, in
the listing for `vX.Y.Z`. On Linux, run `scripts/docs-screenshots.sh --local --only release-notes` while drafting, then
`scripts/docs-screenshots.sh --local` for the full 2x capture that publication needs. The first publication freezes the
shot (`website/AGENTS.md`, "Published release notes are frozen"). See `docs/docs-shots/SPEC.md`, "Adding or changing a
shot", for staging more of the fleet. Give a screenshot only to a highlighted entry whose change is visible, such as a
new dialog or control or a redesign; entries in the folded full list get none, and the generator fails the build on a
listed shot whose PR only the full list cites. Most entries get none, and a release with no visible change gets no file.

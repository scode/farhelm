// Builds the docs website's Release notes page from the repository's
// CHANGELOG.md, every time the site is built or the preview server starts
// (astro.config.mjs runs it from an integration hook).
//
// CHANGELOG.md stays the one source of release notes: it is written for
// someone running Farhelm, and cargo-dist copies each release's section from
// it into the GitHub release. Generating the page from it, instead of keeping
// a copy under website/, means the website, the GitHub releases, and the file
// cannot disagree. The generated files are gitignored; nothing here is meant
// to be edited by hand.
//
// What it writes:
//
// - src/generated/release-notes/<version>.md, one per release: that release's
//   section of CHANGELOG.md, below its "## vX.Y.Z - date" heading, verbatim.
//   Plain Markdown, so nothing a changelog entry contains (a `{placeholder}`, a
//   `<path>`) is ever read as MDX.
// - src/content/docs/docs/release-notes.mdx, the page: a short introduction,
//   then each release newest first, as a heading the page owns (so the
//   right-hand "On this page" column lists the versions) followed by that
//   release's Markdown.
//
// The changelog's own opening paragraph is left out: it describes how the
// file is produced and curated, which is for the people writing it.
//
// Each release also gets a stable anchor named after its version, so
// /docs/release-notes/#v0.23.0 links to one release. The heading's automatic
// slug drops the dots ("v0230"), which nobody would guess.
//
// Screenshots and other website-only material: a file at
// src/release-notes/<version>.mdx (for example src/release-notes/v0.26.0.mdx)
// is shown at the top of that release, above its changelog text. That is
// where annotated screenshots of a release's new dialogs or visual changes go,
// written in MDX like any docs page. Such a file is the website's addition to
// a release and is never sent to GitHub; CHANGELOG.md stays text only.

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDir = join(dirname(fileURLToPath(import.meta.url)), '..');

/** The repository's changelog, the source this page is built from. */
export const changelogPath = join(websiteDir, '..', 'CHANGELOG.md');

const generatedDir = join(websiteDir, 'src', 'generated', 'release-notes');
const extrasDir = join(websiteDir, 'src', 'release-notes');
const pagePath = join(websiteDir, 'src', 'content', 'docs', 'docs', 'release-notes.mdx');

/**
 * Split CHANGELOG.md into its releases, newest first, as the file lists them.
 * A release starts at a "## vX.Y.Z - YYYY-MM-DD" heading and runs to the next
 * "## " heading; releasing/check-changelog.py holds the file to that layout,
 * so anything else at that level is a mistake worth failing the build over.
 */
function releases(changelog) {
  const found = [];
  let current = null;
  for (const line of changelog.split('\n')) {
    if (line.startsWith('## ')) {
      const match = /^## v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?) - (\d{4}-\d{2}-\d{2})$/.exec(line);
      if (!match) throw new Error(`CHANGELOG.md: unexpected release heading: ${line}`);
      current = { version: match[1], date: match[2], lines: [] };
      found.push(current);
    } else if (current) {
      current.lines.push(line);
    }
  }
  if (found.length === 0) throw new Error('CHANGELOG.md: no release sections found');
  return found;
}

/** An identifier MDX accepts as a component name for a release's import. */
function componentName(prefix, version) {
  return `${prefix}_${version.replace(/[^0-9A-Za-z]/g, '_')}`;
}

/** Write the per-release Markdown and the page. Synchronous, so the config hook can call it. */
export function writeReleaseNotes() {
  const all = releases(readFileSync(changelogPath, 'utf8'));

  rmSync(generatedDir, { recursive: true, force: true });
  mkdirSync(generatedDir, { recursive: true });

  const imports = [];
  const sections = [];
  for (const release of all) {
    const tag = `v${release.version}`;
    writeFileSync(join(generatedDir, `${tag}.md`), `${release.lines.join('\n').trim()}\n`);

    const notes = componentName('Notes', release.version);
    imports.push(`import ${notes} from '../../../generated/release-notes/${tag}.md';`);

    let extra = '';
    if (existsSync(join(extrasDir, `${tag}.mdx`))) {
      const name = componentName('Extra', release.version);
      imports.push(`import ${name} from '../../../release-notes/${tag}.mdx';`);
      extra = `<${name} />\n\n`;
    }

    sections.push(
      `<a id="${tag}"></a>\n\n## ${tag}\n\nReleased ${release.date}.\n\n${extra}<${notes} />\n`,
    );
  }

  const page = `---
title: Release notes
description: What changed in each stable release of Farhelm, newest first.
---

{/* Generated from CHANGELOG.md by website/scripts/release-notes.mjs; do not edit. */}

${imports.join('\n')}

What changed for you in each stable release of Farhelm, newest first. Release candidates are not listed separately:
their changes appear under the stable release that follows them. Farhelm on your Mac
[updates itself](/docs/using/update-and-uninstall/) to each new stable release.

${sections.join('\n')}`;

  mkdirSync(dirname(pagePath), { recursive: true });
  writeFileSync(pagePath, page);
}

// Running the file directly regenerates the page, for a look at the output.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  writeReleaseNotes();
}

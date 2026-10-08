// Builds the docs website's Release notes page from the repository's
// CHANGELOG.md, every time the site is built or the preview server starts
// (astro.config.mjs runs it from an integration hook).
//
// CHANGELOG.md stays the one source of release notes: it is written for
// someone running Farhelm, and cargo-dist copies each release's section from
// it into the GitHub release. Generating the page from it, instead of keeping
// a copy under website/, means the website, the GitHub releases, and the file
// cannot disagree. The generated page is gitignored; nothing here is meant to
// be edited by hand.
//
// It writes one file, src/content/docs/docs/release-notes.mdx: a short
// introduction, then each release newest first, as a heading the page owns
// (so the right-hand "On this page" column lists the versions) followed by
// that release's section of CHANGELOG.md. The changelog's own opening
// paragraph is left out: it describes how the file is produced and curated,
// which is for the people writing it.
//
// Each release also gets a stable anchor named after its version, so
// /docs/release-notes/#v0.23.0 links to one release. The heading's automatic
// slug drops the dots ("v0230"), which nobody would guess.
//
// Screenshots: a release can show a screenshot with any of its entries, behind
// a closed "show screenshot" link under that entry. Which shots go with which
// entries is in src/release-notes/<version>.json (for example
// src/release-notes/v0.25.0.json), which names each entry by a pull request
// number the entry cites:
//
//   { "screenshots": [
//       { "entry": 1691, "name": "release-notes/v0-25-0-quick-switcher",
//         "alt": "The quick switcher, ..." } ] }
//
// That file is the website's addition to a release; CHANGELOG.md, and with it
// the GitHub release, stays text only. A screenshot whose entry number no
// entry of that release cites fails the build, rather than quietly showing
// nowhere.
//
// The changelog text goes into MDX, so `{`, `}` and `<` outside code spans are
// escaped: MDX would otherwise read them as an expression or a tag, and an
// entry like "the <path> argument" would break the build or vanish.

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDir = join(dirname(fileURLToPath(import.meta.url)), '..');

/** The repository's changelog, the source this page is built from. */
export const changelogPath = join(websiteDir, '..', 'CHANGELOG.md');

/** Where each release's screenshot list lives, named `<version tag>.json`. */
export const screenshotsDir = join(websiteDir, 'src', 'release-notes');

const pagePath = join(websiteDir, 'src', 'content', 'docs', 'docs', 'release-notes.mdx');

/** A docs shot name, `<page>/<shot>` in lowercase kebab case (docs/docs-shots/SPEC.md). */
const SHOT_NAME = /^[a-z0-9]+(-[a-z0-9]+)*\/[a-z0-9]+(-[a-z0-9]+)*$/;

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

/**
 * Escape what MDX would read as syntax, outside inline code spans: `{` and `}`
 * (expressions) and `<` (tags). Code spans are left alone; MDX shows their
 * contents literally. The changelog has no fenced code blocks, and a line is
 * escaped on its own, so a code span never spans lines here.
 */
export function escapeMdx(line) {
  return line
    .split(/(`+[^`]*`+)/)
    .map((part, i) => (i % 2 === 1 ? part : part.replace(/[{}<]/g, (c) => `\\${c}`)))
    .join('');
}

/** The pull request numbers an entry cites in its closing "(#N, #M)". */
function citedNumbers(entry) {
  const refs = /\(((?:#\d+)(?:,\s*#\d+)*)\)\s*$/.exec(entry);
  return refs ? refs[1].split(',').map((ref) => Number(ref.trim().slice(1))) : [];
}

/** A release's screenshot list, validated, or none when it has no file. */
function screenshotsFor(tag) {
  const path = join(screenshotsDir, `${tag}.json`);
  if (!existsSync(path)) return [];
  const where = `website/src/release-notes/${tag}.json`;
  const shots = JSON.parse(readFileSync(path, 'utf8')).screenshots;
  if (!Array.isArray(shots)) throw new Error(`${where}: expected a "screenshots" list`);
  for (const shot of shots) {
    if (!Number.isInteger(shot.entry)) throw new Error(`${where}: "entry" must be a pull request number`);
    if (typeof shot.name !== 'string' || !SHOT_NAME.test(shot.name)) {
      throw new Error(`${where}: shot name ${JSON.stringify(shot.name)} is not <page>/<shot> in lowercase kebab case`);
    }
    if (typeof shot.alt !== 'string' || shot.alt.trim() === '') throw new Error(`${where}: ${shot.name} needs alt text`);
  }
  return shots;
}

/**
 * One release's section as MDX: the changelog lines, escaped, with each entry
 * that has screenshots followed by a <ReleaseScreenshots> placed inside its
 * list item (indented under the bullet, after a blank line).
 */
function releaseBody(tag, lines) {
  const shots = screenshotsFor(tag);
  const placed = new Set();
  const out = [];
  for (const line of lines) {
    // A category heading ("### 🚀 Added") goes one level down, so the page's
    // "On this page" column lists the releases alone, not every category.
    out.push(line.startsWith('### ') ? `#${escapeMdx(line)}` : escapeMdx(line));
    if (!line.startsWith('- ')) continue;
    const cited = citedNumbers(line);
    const mine = shots.filter((shot) => cited.includes(shot.entry));
    if (mine.length === 0) continue;
    for (const shot of mine) placed.add(shot);
    const list = JSON.stringify(mine.map(({ name, alt }) => ({ name, alt })));
    out.push('', `  <ReleaseScreenshots shots={${list}} />`, '');
  }
  const unplaced = shots.filter((shot) => !placed.has(shot));
  if (unplaced.length > 0) {
    const numbers = unplaced.map((shot) => `#${shot.entry}`).join(', ');
    throw new Error(`website/src/release-notes/${tag}.json: no entry of ${tag} cites ${numbers}`);
  }
  return out.join('\n').trim();
}

/** Write the page. Synchronous, so the config hook can call it. */
export function writeReleaseNotes() {
  const sections = releases(readFileSync(changelogPath, 'utf8')).map((release) => {
    const tag = `v${release.version}`;
    return `<a id="${tag}"></a>\n\n## ${tag}\n\nReleased ${release.date}.\n\n${releaseBody(tag, release.lines)}\n`;
  });

  const page = `---
title: Release notes
description: What changed in each stable release of Farhelm, newest first.
---

{/* Generated from CHANGELOG.md by website/scripts/release-notes.mjs; do not edit. */}

import ReleaseScreenshots from '../../../components/ReleaseScreenshots.astro';

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

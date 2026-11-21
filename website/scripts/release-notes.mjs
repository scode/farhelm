// Builds the docs website's Release notes page and the desktop app's upgrade
// highlights from CHANGELOG.md. The Astro config runs it before Astro loads
// the content collection or copies the public assets.
//
// CHANGELOG.md stays the one source of release notes: it is written for
// someone running Farhelm, and cargo-dist copies each release's section from
// it into the GitHub release. Generating the page from it, instead of keeping
// a copy under website/, means the website, the GitHub releases, and the file
// cannot disagree. The generated page is gitignored; nothing here is meant to
// be edited by hand.
//
// The page, src/content/docs/docs/release-notes.mdx, starts with a short
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
//
// The one exception is the folded full list. A release section from v0.42.0 on
// can end with a `<details>` block ("All other changes") holding everything
// the maintainer did not highlight, and its three frame lines must reach MDX
// as real HTML so the list starts folded here as it does on GitHub. Only those
// exact lines pass through unescaped (releasing/check-changelog.py holds the
// file to that frame); a `<` anywhere else still goes through escapeMdx.
//
// Screenshots belong to highlights only, so entries inside the fold are never
// matched against a release's screenshot list: a shot whose PR only the fold
// cites is unplaced and fails the build, rather than showing up folded away
// (or twice, when a PR has a highlight and an unrelated full-list entry).
// The same pass gives each screenshot disclosure a version-and-PR anchor.
// The upgrade feed uses those anchors, and omits draft notes so a new app
// can wait for the site to publish its own release's finalized highlights.

import { fromMarkdown } from 'mdast-util-from-markdown';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteDir = join(dirname(fileURLToPath(import.meta.url)), '..');

/** The repository's changelog, the source this page is built from. */
export const changelogPath = join(websiteDir, '..', 'CHANGELOG.md');

/** Where each release's screenshot list lives, named `<version tag>.json`. */
export const screenshotsDir = join(websiteDir, 'src', 'release-notes');

const pagePath = join(websiteDir, 'src', 'content', 'docs', 'docs', 'release-notes.mdx');
const upgradeNotesPath = join(websiteDir, 'public', 'whats-new.json');

const DRAFT_NOTICE = '**DRAFT release notes - not yet finalized**';
const HIGHLIGHT_NAMES = new Map([
  ['### 💥 Breaking', 'Breaking'],
  ['### 🚀 Added', 'Added'],
  ['### 🔄 Changed', 'Changed'],
  ['### 🔧 Fixes worth highlighting', 'Fixes worth highlighting'],
  ['### 🗑️ Removed', 'Removed'],
]);

/**
 * The folded full list's frame lines, exactly as releasing/check-changelog.py
 * requires them (`FOLD_OPEN`, `FOLD_SUMMARY`, `FOLD_CLOSE` there). They are
 * the only changelog lines the page passes to MDX as markup.
 */
const FOLD_FRAME = new Set(['<details>', '<summary>All other changes</summary>', '</details>']);

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
 *
 * Canonical Farhelm docs links also become site paths outside code spans.
 * CHANGELOG.md needs absolute links on GitHub; the website's link validator
 * requires those same destinations as site paths so it can check their anchors.
 * Only inline Markdown targets under https://farhelm.io/docs/ are adapted.
 */
export function escapeMdx(line) {
  return line
    .split(/(`+[^`]*`+)/)
    .map((part, i) => (i % 2 === 1 ? part : part
      .replace(/\]\(https:\/\/farhelm\.io(?=\/docs\/)/g, '](')
      .replace(/[{}<]/g, (c) => `\\${c}`)))
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
 * Only stable releases using the highlights and folded full-list format
 * belong in the app's feed. Older releases still go on the website.
 */
function hasUpgradeNotes({ version, lines }) {
  const stable = /^(\d+)\.(\d+)\.\d+$/.exec(version);
  return stable !== null && (Number(stable[1]) > 0 || Number(stable[2]) >= 42)
    && lines.find((line) => line.trim()) !== DRAFT_NOTICE;
}

/** Convert one highlight to styled text. Link labels can contain styling, but their destinations stay on the page. */
function entryPieces(entry) {
  const pieces = [];
  function append(node, style) {
    if (node.type === 'strong') style = 'bold';
    if (node.type === 'emphasis') style = 'plain';
    if (node.type === 'inlineCode') style = 'code';

    if (node.children) {
      for (const child of node.children) append(child, style);
      return;
    }
    const text = node.value ?? node.alt;
    if (!text) return;
    const previous = pieces.at(-1);
    if (previous?.style === style) previous.text += text;
    else pieces.push({ style, text });
  }
  append(fromMarkdown(entry), 'plain');
  return pieces;
}

/** The changelog checker accepts indented continuations, even after blank lines. */
function continuationEnd(lines, start) {
  let end = start;
  for (let next = start + 1; next < lines.length; next++) {
    if (!lines[next].trim()) continue;
    if (!lines[next].startsWith('  ')) break;
    end = next;
  }
  return end;
}

/**
 * Render a release section and collect its app highlights in the same pass.
 * Eligibility, screenshot placement and the folded list determine what the
 * app can link to, so the page and feed share those decisions here.
 */
function releaseBody(release, shots) {
  const { version, date, lines } = release;
  const tag = `v${version}`;
  const notes = hasUpgradeNotes(release) ? { version, date, highlights: [], additionalChanges: 0 } : null;
  const placed = new Set();
  const citationCounts = new Map();
  const out = [];
  let inFold = false;
  let entries = null;
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];
    if (FOLD_FRAME.has(line)) {
      if (line === '<details>') inFold = true;
      if (line === '</details>') inFold = false;
      out.push(line);
      continue;
    }
    // A category heading ("### 🚀 Added", or "#### 🔧 Fixed" inside the fold)
    // goes one level down, so the page's "On this page" column lists the
    // releases alone, not every category. Outside the fold a "####" line is
    // one of v0.12.0's or v0.13.0's Highlights subsections, and stays as it
    // always rendered: published release notes are frozen.
    const category = line.startsWith('### ') || (inFold && line.startsWith('#### '));
    out.push(category ? `#${escapeMdx(line)}` : escapeMdx(line));
    if (notes && category && !inFold) {
      const name = HIGHLIGHT_NAMES.get(line);
      entries = name ? [] : null;
      if (entries) notes.highlights.push({ name, entries });
    }
    if (!line.startsWith('- ')) continue;
    if (inFold) {
      if (notes) notes.additionalChanges++;
      continue;
    }
    const end = continuationEnd(lines, index);
    const continuation = lines.slice(index + 1, end + 1);
    out.push(...continuation.map(escapeMdx));
    index = end;
    const entry = [line.slice(2), ...continuation].map((part) => part.trim()).filter(Boolean).join(' ');
    const cited = citedNumbers(entry);
    const mine = shots.filter((shot) => cited.includes(shot.entry));
    // Published entries can cite the same PR. Number repeated citations
    // even if an entry has no shots, so screenshot metadata cannot move a link.
    const first = cited[0];
    const occurrence = (citationCounts.get(first) ?? 0) + 1;
    citationCounts.set(first, occurrence);
    const suffix = occurrence === 1 ? '' : `-${occurrence}`;
    const anchor = mine.length ? `${tag}-pr-${first}${suffix}` : null;
    if (entries) {
      const pieces = entryPieces(entry);
      entries.push(anchor ? { pieces, screenshotUrl: `https://farhelm.io/docs/release-notes/#${anchor}` } : { pieces });
    }
    if (mine.length === 0) continue;
    for (const shot of mine) placed.add(shot);
    const list = JSON.stringify(mine.map(({ name, alt }) => ({ name, alt })));
    out.push('', `  <ReleaseScreenshots anchor="${anchor}" shots={${list}} />`, '');
  }
  const unplaced = shots.filter((shot) => !placed.has(shot));
  if (unplaced.length > 0) {
    const numbers = unplaced.map((shot) => `#${shot.entry}`).join(', ');
    throw new Error(`website/src/release-notes/${tag}.json: no entry of ${tag} cites ${numbers}`);
  }
  return { body: out.join('\n').trim(), notes };
}

/** Generate the page and the versioned JSON together so a feed link always names an anchor on that page. */
export function renderReleaseNotes(changelog, loadScreenshots = screenshotsFor) {
  const data = { formatVersion: 1, releases: [] };
  const sections = releases(changelog).map((release) => {
    const tag = `v${release.version}`;
    const { body, notes } = releaseBody(release, loadScreenshots(tag));
    if (notes) data.releases.push(notes);
    return `<a id="${tag}"></a>\n\n## ${tag}\n\nReleased ${release.date}.\n\n${body}\n`;
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

  return { page, data };
}

/** Write both generated files synchronously before Astro reads them. */
export function writeReleaseNotes() {
  const { page, data } = renderReleaseNotes(readFileSync(changelogPath, 'utf8'));
  mkdirSync(dirname(pagePath), { recursive: true });
  writeFileSync(pagePath, page);
  writeFileSync(upgradeNotesPath, `${JSON.stringify(data)}\n`);
}

// Running the file directly regenerates both files for inspection.
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  writeReleaseNotes();
}

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { renderReleaseNotes } from './release-notes.mjs';

const changelog = `# Changelog

## v0.44.0 - 2026-11-24

**DRAFT release notes - not yet finalized**

### 🚀 Added

- A draft change. (#12)

## v0.43.0 - 2026-11-20

### 💥 Breaking

- Keep [its **name**](https://example.com/notes), run \`farhelm update\`,
  and retain *this*. (#7, #8)

### 🚀 Added

- A new menu. (#9)

<details>
<summary>All other changes</summary>

#### 🔧 Fixed

- Fixed a label. (#10)
- Fixed another label. (#11)

</details>

## v0.42.0 - 2026-11-19

### 🚀 Added

- An older new menu. (#6)

## v0.41.0 - 2026-11-18

### 🩹 Misc fixes

- A legacy fix. (#5)
`;

/** The screenshot belongs to the second PR, so the URL cannot depend on which screenshots a release has. */
function loadScreenshots(tag) {
  return tag === 'v0.43.0' ? [{ entry: 8, name: 'release-notes/v0-43-0-menu', alt: 'The new menu' }] : [];
}

/**
 * The app's feed comes from the same changelog as the page, while draft and legacy entries stay on the website.
 * v0.43.0's fold left its highlights out, so its total counts both tiers; v0.42.0 has no fold at all.
 */
test('exports finalized highlights and counts every change in releases whose fold omits the highlights', () => {
  const { page, data } = renderReleaseNotes(changelog, loadScreenshots);
  assert.match(page, /A draft change/);
  assert.match(page, /A legacy fix/);
  assert.deepEqual(data, {
    formatVersion: 1,
    releases: [
      {
        version: '0.43.0',
        date: '2026-11-20',
        highlights: [
          {
            name: 'Breaking',
            entries: [{
              pieces: [
                { style: 'plain', text: 'Keep its ' },
                { style: 'bold', text: 'name' },
                { style: 'plain', text: ', run ' },
                { style: 'code', text: 'farhelm update' },
                { style: 'plain', text: ', and retain this. (#7, #8)' },
              ],
              screenshotUrl: 'https://farhelm.io/docs/release-notes/#v0.43.0-pr-7',
            }],
          },
          { name: 'Added', entries: [{ pieces: [{ style: 'plain', text: 'A new menu. (#9)' }] }] },
        ],
        totalChanges: 4,
      },
      {
        version: '0.42.0',
        date: '2026-11-19',
        highlights: [{ name: 'Added', entries: [{ pieces: [{ style: 'plain', text: 'An older new menu. (#6)' }] }] }],
        totalChanges: 1,
      },
    ],
  });
  assert.match(page, /and retain \*this\*\. \(#7, #8\)\n\n  <ReleaseScreenshots anchor="v0\.43\.0-pr-7" shots=/);
});

/** A full-list entry cannot acquire an app link to a folded screenshot that the page never places. */
test('a screenshot must match a highlighted entry', () => {
  function foldedShot(tag) {
    return tag === 'v0.43.0' ? [{ entry: 10, name: 'release-notes/v0-43-0-label', alt: 'The fixed label' }] : [];
  }
  assert.throws(() => renderReleaseNotes(changelog, foldedShot), /no entry of v0\.43\.0 cites #10/);
});

/** Some published entries cite the same PR. Their screenshot links must address distinct entries. */
test('entries that cite the same PR retain separate screenshot anchors', () => {
  function repeatedShots(tag) {
    return tag === 'v0.43.0' ? [{ entry: 7, name: 'release-notes/v0-43-0-menu', alt: 'The new menu' }] : [];
  }
  const repeated = changelog.replace('A new menu. (#9)', 'A new menu. (#7)');
  const { page, data } = renderReleaseNotes(repeated, repeatedShots);
  const urls = data.releases[0].highlights.flatMap(({ entries }) => entries.map(({ screenshotUrl }) => screenshotUrl));
  const anchors = [...page.matchAll(/<ReleaseScreenshots anchor="([^"]+)"/g)].map((match) => match[1]);
  assert.deepEqual(anchors, ['v0.43.0-pr-7', 'v0.43.0-pr-7-2']);
  assert.deepEqual(urls.map((url) => new URL(url).hash), anchors.map((anchor) => `#${anchor}`));
});

/**
 * From v0.44.0 the fold lists every change, highlighted or not, so the maintainer can remove or merge highlights
 * without changing it. The app's total must then be the fold's own count: adding the highlights would count their
 * changes twice, and a merged highlight shows that no subtraction could undo that.
 */
test('a release whose fold lists every change counts the fold alone', () => {
  const complete = changelog.replace(
    '**DRAFT release notes - not yet finalized**\n\n### 🚀 Added\n\n- A draft change. (#12)\n',
    `### 🚀 Added

- Two menus, summed up as one highlight. (#12, #13)

<details>
<summary>All changes</summary>

#### 🚀 Added

- A new session menu. (#12)
- A new host menu. (#13)

#### 🔧 Fixed

- Fixed a tooltip. (#14)

</details>
`,
  );
  const { page, data } = renderReleaseNotes(complete, () => []);
  assert.deepEqual(data.releases[0], {
    version: '0.44.0',
    date: '2026-11-24',
    highlights: [{
      name: 'Added',
      entries: [{ pieces: [{ style: 'plain', text: 'Two menus, summed up as one highlight. (#12, #13)' }] }],
    }],
    totalChanges: 3,
  });
  assert.match(page, /\n<details>\n<summary>All changes<\/summary>\n/);
});

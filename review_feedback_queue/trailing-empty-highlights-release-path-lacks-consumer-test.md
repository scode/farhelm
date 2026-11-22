# The empty-highlights release path lacks a consumer test

Reviewed commit: 6bc33a60962889301e266588cbf8d4fe9e3e3e05

## TLDR

The release checker accepts a version with every entry in All changes. The generator, desktop reader and dialog lack an
assertion that this release remains usable.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/whats_new.rs:239`.

releasing/check-changelog.py:348–355 accepts a fold-only section and its existing assertion at lines 1354–1360 removes
both highlight categories before checking acceptance. website/scripts/release-notes.mjs:170–178, 225–262, 292 and
297–303 exports that release with highlights empty and totalChanges from the complete fold, while retaining the folded
body on the site. crates/farhelm-desktop/src/whats_new.rs:99–141 accepts the empty array, keeps the running release in
range and checks presence by version. The GPUI section renders its header unconditionally and iterates zero category
rows (crates/gui/farhelm-gui/src/whats_new.rs:238–273), sums all release counts and leaves All release notes available
(338–378). The dismissal hook still records the running release (125–146). SPEC.md:836–851 says to show the available
releases and gives the footer a total, with no alternate behavior for an empty selection. The current subtitle still
says Highlights from when every selected release has an empty list. In the reviewed source the two eligible published
sections have highlights. This is a supported future curation path. Existing JS and GUI assertions cover total counts
with populated highlights (website/scripts/release-notes.test.mjs:123–155 and whats_new/tests.rs:130–158), leaving
empty-array feed and GUI coverage unexercised. Bucket: other. Confidence: confirmed by source, with no rendering
observed. proposed_drop: spec-accepted selection and presentation, with a test coverage gap for the empty range. Could
be highest: no. Possible coverage: none.

The independent check established a separate coverage question. DISAGREE via
6bc33a609:releasing/check-changelog.py:346–357, farhelm-gui/src/whats_new.rs:236–265, SPEC.md:834–853,
release-notes.test.mjs:121–150, and whats_new/tests.rs:128–157. The checker accepts fold-only notes, the GUI builds the
version and date header unconditionally, and the spec counts every change. The cited fixtures use populated highlights.
The proposed gap concerns releases with empty highlight lists. An empty release range would omit the required running
release.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `release/l4`.

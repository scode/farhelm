# Indented blank lines defeat release-note PR coverage checks

Reviewed commit: 6bc33a60962889301e266588cbf8d4fe9e3e3e05

## TLDR

A separator containing only spaces can make the gate overlook a highlighted PR missing from All changes, or reject a
list that already cites it.

## Details

**other.** Source inspection. Main reference `releasing/check-changelog.py:430`.

parse_changelog preserves body whitespace at releasing/check-changelog.py:241–260. _check_bullets skips whitespace-only
lines at 569–575 and validates the joined text at 582–587. _cited_prs at 420–436 instead appends a space when the
separator starts with two spaces. The end-anchored pattern at 209 then misses the preceding citation if it finishes the
bullet. The set comparison at 360–367 can therefore undercount either tier. This difference is present for standard
Markdown blank separators, independent of authoring hard wraps: a whitespace-only line between complete single-line
entries or between the last entry and a heading or fold boundary is a blank block separator. The generator treats it as
blank when finding the end of the entry (website/scripts/release-notes.mjs:202–210), trims and filters continuation
pieces before parsing PR numbers (264–270), and sends only the original whitespace to the page (237–252, 293). A
two-space blank line adds no bullet or highlight and does not change its citation for screenshot matching.
SPEC.md:836–840 and releasing/.agents/changelog.md:107–112, 162–166 keep the display and checker under these
single-paragraph entries and PR coverage rules. The release gate and finalizer call the checker at
.github/workflows/release.yml:154, releasing/check-changelog.py:1698–1705, and
releasing/finalize-release-notes.py:130–136. Existing checker tests at 1419–1440 cover merged and wrapped citations and
missing references, but no such separator. Share logical bullet assembly or apply the same whitespace rule. Bucket:
other, a release-tooling correctness gap. Confidence: confirmed by pinned source and Markdown blank-line semantics, with
no runtime or published occurrence observed. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `release/l2`.

# Redrawn-path browser test does not wait for its new successful stat

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The file-link redraw regression can issue its only download click while the replacement file is still being checked,
then treat the correctly ignored click as a download regression.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed missing
readiness oracle, not a measured flake; actual failure frequency is unmeasured and not part of the retained claim.

Source locations: 7509285e647ea86ccd2cee7905675dc87dad9b6f:e2e/grid-client/grid-attach.spec.ts:898-901.

test line 899 waits for b.txt only, whereas terminal-files.js:428-438 clears the old hover, sets info null and
synchronously displays Checking plus the new path before awaiting stat; only :440-462 installs successful info and the
ready action hint. download :602-605 returns immediately unless that info is ready. The test first waits for 10 B before
clicking a.txt (:890), and its separate OSC relabel case likewise waits for 10 B after the second request (:1091-1094);
neither readiness witness exists for the b.txt click. routeFiles :770-783 returns requests rather than a completion
barrier for stat. Suggested repair: require b.txt plus a successful ready disclosure, or use a gated stat to test the
unsettled click separately.

The file-link redraw regression can issue its only download click while the replacement file is still being checked,
then treat the correctly ignored click as a download regression. Exact

Possible coverage and caveats: none exact found. At pinned tip SPEC.md :2358-2390 explicitly distinguishes checking from
successful click admission; it does not accept this test premise. Focused searches covered SPEC_impl.md, TODO.md
(Planned remains empty at :23-25), BUGS.md, queue INDEX and unlanded plans (plans/queue/INDEX.md empty). Ledger
download-regression-fixtures-arm-cleanup-too-late.md :14935-14957 addresses real-session cleanup ownership in
e2e/tests/terminal-files.spec.ts, and file-stat-test-compares-unresolved-temp-path.md :14984-15007 addresses canonical
path spelling in a supervisor test; neither covers this grid test readiness gap. Queue file/hover/readiness entries have
different triggers and oracles; no matching queue item found. FILTER.md does not establish an exclusion: rarity is
unmeasured and this is an absent test premise rather than an accepted production diagnostic. Possibly highest: no.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g165/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f.
Persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb:e2e/grid-client/grid-attach.spec.ts:906-909.. This
was a source-only review; runtime occurrence and frequency were not measured.

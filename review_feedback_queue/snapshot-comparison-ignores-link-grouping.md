# Snapshot comparison accepts changed hyperlink identity and grouping

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Snapshot regression checks can pass after terminal hyperlink identities or boundaries change.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed statically. No
runtime reproduction or mutation experiment was performed.

Source locations:
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:crates/farhelm-terminal-emulator/tests/common/compare.rs:194-204.

Snapshot regression checks can pass after terminal hyperlink identities or boundaries change. This weakens checks for
preserving terminal links across supervisor restarts; no production snapshot corruption is established. Source basis at
both C and T: tests/common/compare.rs:177-187 compares cells independently, and :194-204 equates every pair of numeric
_alacritty IDs with equal URI. A link split across distinct generated IDs therefore remains equal cell by cell, and
explicit IDs using that spelling are also ignored. At C, vendor/alacritty_terminal/src/term/cell.rs:279-290 preserves
explicit IDs and generates the same spelling for absent IDs; the same constructor persists at T. The painter explicitly
preserves stored IDs, at C src/snapshot/v1/paint.rs:844-852 and T :890-899. tests/common/roundtrip.rs:303-311 directly
compares the live original with its restore, where no generated-ID normalization is needed; later comparisons
(:315-317,346) use independently replayed references and legitimately need consistent renaming. At T
tests/common/synthetic.rs:149-160 actually generates id-less hyperlinks; snapshot_golden.rs:23-25 deliberately uses
explicit IDs for deterministic blobs, so it does not replace the anonymous-link comparison. User-visible observability
is confirmed at T crates/farhelm-ui/assets/grid-buffer.js:286-287,300-320: equal URI but unequal ID yields separate
hover/underline groups.

Possible coverage and caveats: none exact. At T SPEC_impl.md Terminal snapshots (:2549-2597) requires restoration and
describes these property tests; SPEC.md:2306-2331 makes link extents observable. TODO.md Planned (:23-25) is empty. Its
Near term repeated-link warning (:51-68), and g163/f3 (also identified by the parent as duplicating g171/f3), concern
ordinary repeated anonymous OSC starts already producing distinct IDs, not a snapshot oracle accepting a changed
partition. TRIAGE_OUTCOMES.md repeated-terminal-links-acquire-mismatch-warning.md (:16915-16932) fixes repeated complete
addresses with the same explicit ID across rows; it likewise does not change this comparison or its
identity-preservation contract. SPEC_impl.md:1671-1704 accepts limits in hover comparison, not loss of identity during
snapshot restoration or an oracle's failure to detect it. Prior complete coverage-source searches of BUGS.md,
plans/queue, queue INDEX/matching files and TRIAGE_OUTCOMES.md found no exact test-helper item; resource-growth and
upstream-xterm dispositions (:7594-7643,11205-11215) do not cover it. FILTER.md does not filter a deterministic false
equality in this Farhelm regression helper. Fix direction: use strict IDs for the direct restore comparison; preserve a
consistent bijection for generated IDs across independently replayed references, including cursor templates and cells,
and compare explicit IDs exactly where provenance is known.

Additional reviewed locations: g078/f1:
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a:crates/farhelm-terminal-emulator/tests/common/roundtrip.rs:308-317.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g075/f1`, `g078/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a (C); persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb (T)..
This was a source-only review; runtime occurrence and frequency were not measured.

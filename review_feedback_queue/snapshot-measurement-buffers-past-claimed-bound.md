# Measurement retains a whole wide row beyond the snapshot stream limit

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Sizing a checkpoint can retain a temporary stream beyond its claimed 16 MiB limit, even though final blob fitting
remains correct.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
behavior; no physical RSS, allocator peak or availability measurement. At tip paint.rs:832-834 appends unchecked,
1018/1024 starts measurement with 64 KiB, and 1077-1105 clears only between rows; 1145-1147 prints all cells before
another clear. A supported 10,000-column row with different 250-byte explicit link IDs and 2083-byte URIs emits at least
23,440,000 bytes (10,000 times 2343 OSC bytes plus one glyph), about 22.35 MiB. Initial wording over 23 MiB was a
decimal/binary unit error, corrected here. The live extra-storage charge is about 26.21 MB: (250+2083+128+160)*10,000,
below 64 MiB; tip vendor/alacritty_terminal/src/term/cell.rs:135-142 and term/mod.rs:53-111,2077-2098 establish
admission. A maximal 4096-entry title stack adds 16,822,272 bytes after the last row (paint.rs:303-312), so retained
length can exceed 38 MiB; initial over 39 MiB wording is also corrected. Rust 1.99.1 source commit
58cf8f9c35b355199727627d07ae34a4e2846d67: library/alloc/src/vec/mod.rs:3605-3606 calls spec_extend,
vec/spec_extend.rs:50-57 calls append_elements for u8, vec/mod.rs:2959-2963 reserves and appends, raw_vec/mod.rs:503-533
grows to max(2*capacity, required). The inspected implementation therefore requests 32 MiB for the row and 64 MiB after
the titles. Vec::clear at vec/mod.rs:3052-3085 retains capacity. These are source-derived requested capacities for that
inspected standard library, not a repository-pinned toolchain or measured RSS claim. The limit breach already follows
from retained len >16 MiB independently of the precise growth implementation. snapshot/mod.rs:797-833 measures the full
content before deciding what to drop, and v1/mod.rs:282-309 returns only measurements, so later fitting does not prevent
the transient allocation.

Source locations: crates/farhelm-terminal-emulator/src/snapshot/v1/paint.rs:1024-1048,1088-1090 (introduction);
tip:832-834,1012-1024,1077-1105,1145-1147,303-312.

Mode::Measure starts Writer.out with 64 KiB capacity (paint.rs:962-974); Writer::raw appends without a measurement
threshold (paint.rs:786-788); drop_measured clears only at the start of each row (paint.rs:1024-1029,1047-1048), while
the complete row is encoded before another clear. A supported 10,000-column row with a distinct explicit OSC 8 link
before every cell can emit over 23 MiB for that row alone: URI 2083 bytes plus id and delimiters per cell
(set_link:844-853; paint_unit:908-917). Ten thousand such links cost well under the vendored 64 MiB extras cap
(vendor/alacritty_terminal/src/term/mod.rs:55-100), so the cap does not prevent the premise. The last wide linked row
can also remain buffered while the title stack is emitted (paint.rs:278-287); a maximal 4096-entry stack adds about 16
MiB after the over-23-MiB row, so measurement can reach a roughly 64 MiB stream allocation. The Vec then grows by
doubling beyond the 16 MiB blob limit before the row ends. This contradicts Mode::Measure's reason for existing
(paint.rs:111-115: never hold a stream larger than the limit), even though later fitting correctly bounds the serialized
blob.

sizing a checkpoint can retain a temporary stream beyond its claimed 16 MiB limit, even though final blob fitting
remains correct.

Possible coverage and caveats: SPEC.md at tip:3991-3994 accepts hostile same-account availability effects; this finding
concerns an explicit encoder buffering contract (paint.rs:117-120; snapshot/mod.rs:791-796), not an established denial
of service. SPEC_impl.md:2588-2597 gives an approximate budget at 200 columns, so it alone is not a universal
wide-terminal budget. TODO.md:104-132 describes latency work and repeats the below-limit constraint, without specifying
this per-row repair. No exact coverage was found in SPEC/SPEC_impl, empty TODO Planned, unlanded plans, BUGS, queue
INDEX/matching files or TRIAGE_OUTCOMES. At ordinary widths too, the maximal title stack alone emits 16,822,272 bytes,
exceeding 16,777,216 (16 MiB), because the title loop has no measurement flush. The wide-row example establishes the
larger capacity, not the existence of the limit breach.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g071/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.. This
was a source-only review; runtime occurrence and frequency were not measured.

# Private recording loader rejects valid JSON field spacing

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A developer can provide a private terminal recording in the documented JSON-lines format and have the snapshot test
reject it before checking the recording.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
behavior; no runtime reproduction.

Source locations: crates/farhelm-terminal-emulator/tests/snapshot_roundtrip.rs:193-198.

snapshot_roundtrip.rs:165-169 documents each size event as a JSON object with offset, cols and rows. The field parser at
193-198 searches for an exact quoted-name immediately followed by colon, so JSON whitespace between the closing quote
and colon causes a no-field panic although it is legal JSON. Subsequent numeric parsing accepts whitespace after the
colon, but that does not fix the key lookup. The same formatting fails on every retry until the fixture is rewritten.

a developer can provide a private terminal recording in the documented JSON-lines format and have the snapshot test
reject it before checking the recording.

Possible coverage and caveats: initial pass cited FILTER.md's rare safe-failure/retry rule, but that exclusion is
withdrawn: unchanged input fails deterministically, and the workaround changes the fixture rather than retrying the same
operation; the filter match is at least unclear and therefore cannot justify dropping. Rechecked exact-term searches at
the pinned tip across SPEC.md, SPEC_impl.md, TODO Planned (empty at 23-24), unlanded plans/queue, BUGS.md, the queue
including INDEX and all matching files, and TRIAGE_OUTCOMES.md found no matching accepted contract or tracking item.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g088/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a;
unchanged through tip ca9b0a19856dc136f63ad2d14f1076027edce1eb by static git diff and direct tip source read.. This was
a source-only review; runtime occurrence and frequency were not measured.

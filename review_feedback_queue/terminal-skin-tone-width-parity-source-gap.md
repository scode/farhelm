# Skin-tone emoji width parity retains a compressed-table source gap

Reviewed commit: fdfcc98cf4062345c4488a388459fcc19d9f595a

## TLDR

A newer hand emoji with a skin-tone modifier may use fewer browser cells than tmux, displacing later output or the
replayed cursor. Its exact shipped add-on classifications remain unverified.

## Details

The new provider activates at `crates/farhelm-ui/assets/terminal.js:3578`. In
`assets/vendor/addon-unicode-graphemes.js:1`, zero-based byte offsets 385 and 7375 locate character properties and the
joining predicate. Verified tmux 3.7c assigns U+1FAF1 and U+1F3FD two cells each (`utf8.c:94–98,208`), but its
modifier-base list omits U+1FAF1 (`utf8-combined.c:122–200`), so `screen-write.c:2706–2721` does not combine the pair.
The add-on joins Extend characters, but this pair's entries in its shipped compressed trie were not independently
decoded and read. `tmux/stream.rs:1115–1158,1294–1308` forwards live bytes and reconstructs replay from tmux before
restoring its cursor.

Suggested bucket: `other`. Confidence: likely source lead with a missing concrete-table premise, not a measured
rendering failure. Whether it predates this provider is also unestablished. Existing six-glyph tests exclude skin-tone
sequences. Possible cover: `SPEC_impl.md:1119–1122` directs vendored-bundle upgrades for nonsecurity bugs; applicability
to this cross-component mismatch is ambiguous. Provenance: `g072/f1`. No browser, tmux or decompression experiment ran.

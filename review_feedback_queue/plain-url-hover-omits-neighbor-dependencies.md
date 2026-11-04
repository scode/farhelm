# A hovered printed URL can keep its old destination after a neighboring edit

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A program can extend a hovered URL onto the next wrapped row while a click continues opening the old, shorter address,
even after repaint. The hostname-continuation route is source-confirmed and differs from the separate before-paint
issue.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; runtime occurrence and frequency unmeasured

Source locations:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1238-1242,1608-1612.

Original g160/f1 and g159/f2 provenance must remain. Plain rowLinks URLs retain matched ranges only; OSC links
additionally retain evidence rows. Broad answerLines protects a pending provider answer, but retained hoverRows captures
only matched/evidence rows. A neighboring edit outside the old match changes the recomputed plain URL without changing
captured rows. lookUpLink/refreshHover retain the old still-covering hover; release activates it. grid_output sends only
differing rows, while the emulator supports wrapping and edits restricted to the neighboring row. Parent personally
checked exact source and SPEC2324–2332/3961–3982. This persists after repaint, so subsecond pre-redraw acceptance is not
exact cover. Captured-row replacement refutations remain narrow. Subsequent source checks by a092, a098 and a099
corroborated the omitted-neighbor route and corrected the earlier broader exclusion. This is distinct from the
before-paint finding.

At reviewed tip ca9b0a19856dc136f63ad2d14f1076027edce1eb,
crates/farhelm-ui/assets/grid-buffer.js:182–210,300–334,436–464 detects plain URLs using neighboring wrapped rows, but
returns only matched ranges. Unlike OSC links, those links retain no dependency evidence. Its answerLines guard at
:389–406 protects a pending provider reply, not an existing hover. grid-terminal.js:1234–1247,1334–1347 captures only
the old matched/evidence rows; :1272–1283,1380–1390 retains the old hover while its old ranges still contain the
pointer; :1614–1618 activates it. An edit confined to a neighboring wrapped row can extend the printed hostname without
changing any captured row. grid_output.rs:404–451 sends only differing rows, preserving the old row identity. Vendored
term/mod.rs:1099–1160,1235–1328,1347–1362 supports wrapping and those neighboring edits. terminal.js:1233–1252,1299–1312
keeps found.uri and supplies no separate ordinary printed-URL disclosure. The visible hostname can therefore change
while activation opens the older origin, including after repaint. Capture all rows that established the match, or
revalidate that match before activation. SPEC.md:2330–2331 promises the printed text is what opens; :3966–3969 concerns
acting before redraw and does not exactly cover this postpaint route. Vendored-xterm acceptance is not first-party grid
acceptance.

Possible coverage and caveats: none needed for refutation; no spec acceptance asserted. T SPEC.md:2306-2331 and
SPEC_impl.md:989-1019 were checked. Historical vendored-xterm partial-repaint outcomes(T TRIAGE_OUTCOMES.md:7680-7721)
and TODO.md:1033-1036 do not cover grid code.

Additional reviewed locations: g159/f2:
crates/farhelm-ui/assets/grid-terminal.js:1244-1247,1297-1307,1340-1342,1614-1618; g161/f2:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1238-1241,1608-1612; g163/f4:
7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-terminal.js:1238-1242,1291-1301,1608-1612;
g171/f4: 7509285e647ea86ccd2cee7905675dc87dad9b6f:crates/farhelm-ui/assets/grid-buffer.js:389-406.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g160/f1`, `g159/f2`,
`g161/f2`, `g163/f4`, `g171/f4`, `a091/f1`, `a092/f1`, `a098/f1`, `a099/f1`. Original append-only events, revisions,
withdrawals and independent judgments are retained in that run's catalogs. Locations refer to the named reviewed
revisions; the original revision context is 7509285e647ea86ccd2cee7905675dc87dad9b6f (R); refutation persists at
ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a source-only review; runtime occurrence and frequency were not
measured.

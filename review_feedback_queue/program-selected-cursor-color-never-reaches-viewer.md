# Program-selected cursor colors never reach grid viewers

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A program that changes its text cursor color continues to show the theme's fixed cursor color, even though Farhelm's
emulator accepts the new color.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior, no runtime frequency/experiment premise required.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:348-355,462-477.

A program that changes its text cursor color continues to show the theme's fixed cursor color, even though Farhelm's
emulator accepts the new color. Details/source basis at tip: vendor/vte/src/ansi.rs:1471-1498 maps OSC12 to
NamedColor::Cursor and calls set_color; queries.rs:283-295,342-343 delegates it unchanged;
vendor/alacritty_terminal/src/term/mod.rs:1855-1863 stores it. Assigned grid_output.rs:348-355 serializes
position/visibility/shape/blink, while :462-477 serializes only default foreground/background colors. Protocol
grid.rs:969-1006,1052-1083 has no cursor-color field. Browser grid-style.js:220-239 gets cursor color solely from
theme.cursor or defaults.fg; grid-terminal.js:763-775 (read in synthesis) paints with that mapper color. No transmitted
value can represent the accepted OSC12 change, on live update or reconnect. R's parser and mapper excerpts independently
confirm the same route before tip.

Possible coverage and caveats: none found. Required coverage searches at pinned tip across SPEC.md, SPEC_impl.md, TODO
Planned, all unlanded plans/queue, BUGS.md, queue INDEX and TRIAGE_OUTCOMES.md found no matching accepted exception,
cause-removing plan or duplicate. Retained full SPEC.md Terminal experience2080-2391 promises color fidelity
at2085-2086. Full SPEC_impl grid-client1706-1736/stream2442-2482/emulator2517-2548 sections do not accept ignoring
cursor colors; queries.rs:1-146 explicitly lists OSC12 queries as the program's own color if set, with no
unsupported-setting caveat. The protocol's closed Cursor/TerminalState schemas document the missing channel, not
explicit acceptance of its user-visible loss. TODO Planned23-25 remains empty. TRIAGE_OUTCOMES11316-11326 concerns
save/restore formatting wholly inside vendored xterm.js, not this Rust-to-grid omission; read in full and not treated as
coverage. FILTER rare-self-correcting rule does not cover an ordinary program color change that stays wrong after
refresh/reconnect. Cross-run dedup search of current findings.jsonl files for cursor-color/OSC12 matched only this f4.
Distinct from g173/f1 (dropped styled leading cell) and g158/f1 (raw/effective bright palette selection), and no
existing local ID is reused to claim novelty.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g173/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41
(assigned encoder introduction). Persists at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb: empty diff for
grid_output.rs, grid-style.js and vendor/vte ansi parser; emulator query/term diffs only add unrelated documentation..
This was a source-only review; runtime occurrence and frequency were not measured.

# An initial terminal size above 1000 cells cannot be streamed

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A terminal fitted to over1000 cells can fail to open repeatedly because its initial dimensions exceed the grid stream's
bound.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior; runtime frequency unmeasured, no unresolved source premise.

Source locations: crates/farhelm-supervisor/src/grid_output.rs:330-359.

grid_output.rs:333-338,357-359 copies emulator size. grid_stream.rs:423-449,499-513 sends Hello after the original
attach size is reached; :818-821 aborts when encode rejects that size. At tip, terminal.js:4013-4015,4076-4078 fits
xterm and sends its measured dimensions; vendor/addon-fit.js:1 has lower bounds only, and targeted complete resize
snippets from vendor/xterm.js:1 show public resize forwarding to CoreTerminal.resize (Math.max minimum only), then
BufferService storing the dimensions. The physical condition is exact: after subtracting padding/scrollbar,
usableWidth >=1001*measuredCellWidth or usableHeight >=1001*measuredCellHeight. app.css:8110-8143 gives panes the full
container and terminal padding4px; terminal.js:415-424 supports terminal fonts down to9px; SPEC.md:2102-2112 permits
independent browser page zoom. No artificial input/control is needed. A particular 8K display's OS scale or glyph
metrics were not measured; the initial illustrative display example is replaced by this source-defined CSS-geometry
condition. Host admission still accepts 10000 (terminals.rs:267-279, hosts/terminal.rs:2251-2259,
terminal-host/protocol.rs:102,135-145); grid protocol Hello accepts1000 (grid.rs:955-963,1180-1196). grid adoption/fit
at terminal.js:5299-5396 and grid-terminal.js:214-224 only happens after the stream notice travels back, so cannot
rescue a sender that encoded Hello before the resulting clamped resize round trip. onopen also sends the pre-grid xterm
size (terminal.js:6028-6036). Retry persistence: :1757-1802,1830-1893,5140-5232 rebuilds a fresh mount with the same
spec and redoes the uncapped initial fit; it does not carry the previous grid size into the next URL. Therefore retries
at unchanged geometry can repeatedly hit the same failure; a clamped resize that arrives early can rescue an individual
attempt, so failure on every retry is not asserted.

grid_output::screen copies emulator.size into Screen without enforcing GRID_MAX_DIMENSION.
service/grid_stream.rs:423-449 waits for the attach's requested size and immediately sends that size in Hello; :818-821
ends attachment on encode failure. The tip's shipped browser initially uses xterm.js's measured size unchanged in its
socket URL (crates/farhelm-ui/assets/terminal.js:4076-4078), before it mounts the grid component; handlers.rs:1843-1849
requests that size before starting output. terminals.rs:267-279 forwards it to HostTerminal::resize, which accepts
Size::is_valid (:2251-2259); terminal-host protocol admits each dimension up to 10000. The grid component's later fit
clamp to 1000 does not protect the first Hello: the sender begins when the original attach size is reached. This can
occur on an 8K-width viewport with small terminal text, and a same-dimension row may also fail protocol validation.

A terminal fitted to over1000 cells can fail to open repeatedly because its initial dimensions exceed the grid stream's
bound. Static source route: grid_output.rs:333-338,357-359 serializes emulator dimensions;
grid_stream.rs:423-449,499-513 waits for original attach size then encodes Hello; :818-821 ends attachment on encoding
error. Browser terminal.js:4013-4015,4076-4078 fits xterm and sends that size; addon-fit.js:1 and complete targeted
xterm.js:1 resize snippets have no upper1000 clamp. Exact geometry condition: usable width >=1001 measured cells or
usable height >=1001 measured cells after padding/scrollbar. app.css:8110-8143 gives the terminal its full pane and4px
padding, terminal.js:415-424 supports font sizes9..28, and SPEC.md:2102-2112 leaves browser zoom independent. No
measured8K-display claim is retained. Additional admission synthesis confirms no hidden clamp: helm
terminal.rs:193-197,315-350 accepts u16 dimensions and forwards q.cols/q.rows; helm client.rs:3919-4032 forwards them
unchanged into ControlMsg::Attach; supervisor handlers.rs:1410-1438,1440-1535,3079-3106 passes them through without
normalizing, and :1843-1849 requests that size before starting output. Host terminal size validation accepts1..10000
(terminals.rs:267-279, hosts/terminal.rs:2251-2259, terminal-host/protocol.rs:102,135-145), while grid Hello validation
rejects above1000 (grid.rs:955-963,1180-1196). Later browser grid.fit clamps1000 only after the stream notice returns
(terminal.js:5299-5396, grid-terminal.js:214-224); the sender can already have failed Hello. Retry
terminal.js:1757-1802,1830-1893,5140-5232 remounts and redoes xterm fit at unchanged geometry, so the mismatch can
repeat. A sufficiently early clamped resize can rescue an individual attempt; no claim every scheduling outcome fails.

Possible coverage and caveats: none exact after required searches; full SPEC.md Terminal experience2080-2391 and
SPEC_impl grid client1706-1736/Grid streams2442-2482 promise fit equivalence/replay, not this mismatch. TODO
Planned23-25 empty. No exact plan/BUGS/queue/ledger coverage. Rare-safe-retry filter does not fully match because retry
repeats the incompatible producer.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g173/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 70cd68a03c101493e0adbf00bb27bb8004adfe41;
persists unchanged through ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime occurrence
and frequency were not measured.

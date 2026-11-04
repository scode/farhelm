# X10-only terminal mouse reporting is silently lost

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Programs that request only classic X10 mouse tracking lose clicks in terminal-host terminals, unlike the live raw-stream
browser terminal.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source.

Source locations: e1e6821d07f2d174c520690296961ac5c0bb064c:crates/farhelm-supervisor/src/grid_input.rs:49-60,678-684.

grid_input.rs:48-60 explicitly documents the loss, and :678-684 drops clicks when mouse reporting is off. At pinned tip,
vendor/vte/src/ansi.rs:935-955 maps private mode 9 to Unknown; vendor/alacritty_terminal/src/term/mod.rs:2160-2167
ignores Unknown; crates/farhelm-terminal-emulator/src/modes.rs:12-27 has no X10 variant and :83-92 produces Off. In
contrast, crates/farhelm-ui/assets/vendor/xterm.js:1 sets activeProtocol=X10 for case 9 and its X10 restrict allows
button presses (not releases, wheel or motion, with modifiers cleared).
crates/farhelm-terminal-emulator/src/modes.rs:51-54 also explicitly states that the previous raw-stream browser terminal
honors it; SPEC_impl.md:2889-2895 explains that raw pane bytes reach xterm.js and mouse reports return as input.
SPEC.md:2085 promises terminal fidelity, and SPEC_impl.md:1726-1729 says a program reads the same keys and clicks in
both kinds.

Possible coverage and caveats: no exact accepted exception, Planned item, unlanded plan, BUGS entry, queue item or
ledger decision found. SPEC.md:2085 and SPEC_impl.md:1706-1735, especially :1726-1729, promise fidelity and equivalent
clicks without accepting X10 loss. Implementation comments acknowledging the omission are not specification acceptance.
Source chain at T: vendor/vte/src/ansi.rs:935-955 maps mode 9 to Unknown;
vendor/alacritty_terminal/src/term/mod.rs:2160-2167 ignores it;
crates/farhelm-terminal-emulator/src/modes.rs:12-27,83-92 has no X10 state and produces Off;
crates/farhelm-supervisor/src/grid_output.rs:462-471 publishes MouseMode::Off;
crates/farhelm-ui/assets/grid-terminal.js:1683-1709 therefore selects locally rather than forwarding the press, and
assets/grid-input.js:531-535 refuses Off mouse reports. If a client nevertheless sends a press, T grid_input.rs:681-687
drops it. The vendored xterm.js line 1 has case 9 enabling X10 and a press-only unmodified X10 reporting rule (read
during initial pass). The prior live path is not blocked by tmux failing to track X10: parent-supplied pinned official
tmux 3.7c window.c:1140-1163 queues control output before input_parse_pane; control.c:474-523,614-651 queues and
octal-escapes the original pane bytes. At T, tmux/stream.rs:1294-1309 decodes those bytes and the fixed query filter in
tmux/query_strip.rs:44-55,78-123 does not remove mode-setting sequences. Thus the mode reaches the older live xterm
renderer even when tmux does not recognize it for replay. Existing queue
classic-mouse-snapshot-copy-blocks-later-links.md (full file) and ledger :19375-19385 concern retained selection after
copying a frozen classic-encoding snapshot, not mode-9 click loss; their trigger, consequence and scope do not cover
this. Filter rare self-correcting glitches does not fully match: the mode remains unsupported for that program, rather
than retry/refresh repairing it. Fix direction correction: an internal X10 reporting variant and the proper encoder rule
are needed, but a wire-format enum change is not necessarily needed: publish the existing Click state so the client
forwards presses, then suppress releases/wheel and modifiers in the supervisor's exact X10 mode. Add X10 parity cases;
the mouse-table protocol list currently excludes it.

Additional reviewed locations: g102/f1:
e1e6821d07f2d174c520690296961ac5c0bb064c:crates/farhelm-ui/js-tests/xterm-mouse-table.js:66,139.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g099/f2`, `g102/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
e1e6821d07f2d174c520690296961ac5c0bb064c (C); persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb (T).. This was a
source-only review; runtime occurrence and frequency were not measured.

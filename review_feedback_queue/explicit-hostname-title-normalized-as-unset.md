# Title normalization rejects matching explicit host-name titles

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Emulator parity validation can fail on machines named codex or workspace even when tmux and the emulator show the same
explicitly set title.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static
composition, no runtime reproduction and no production user-work consequence claimed.

Source locations:
20ebb7a84b101b52dc808d11e9cd90c23d6ffbd5:crates/farhelm-supervisor/src/emulator_parity/harness.rs:626-631.

harness.rs:626-631 replaces every tmux title equal to host with empty, instead of restricting that allowance to an unset
emulator title. Static title ingestion verified in synthesis: official tmux 3.7c input.c:2675-2710,
options-table.c:1105-1111, screen.c:246-258, tmux.c:285-299 and window.c:1083-1084; pinned-tip
vendor/vte/src/ansi.rs:1398-1409 and vendor/alacritty_terminal/src/term/mod.rs:2450-2474. Discovery reconciliation:
g051/f1 is a semantic duplicate with a concrete tracked-fixture producer. Independently read pinned-tip
crates/farhelm-supervisor/src/emulator_parity/fixtures.rs:1-110: lines 38-39 explicitly repaint non-placeholder titles,
81-82 feed them to this harness, and 91-94 accumulate its false title failure. Tracked
crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/idle-fresh.title:1 is codex and
muse/1.4.14/idle-resumed.title:1 is workspace. This adds existing-test trigger evidence under the original ID; it is
neither a new discovery nor a second queue item. The original recording route remains valid at
recordings.rs:145-147,333-337, but no private recording or current machine hostname is assumed.

emulator parity validation can fail on machines named codex or workspace even when tmux and the emulator show the same
explicitly set title.

Possible coverage and caveats: none found in prior exact-trigger coverage searches of SPEC.md, SPEC_impl.md, TODO.md
Planned (empty), unlanded plans/queue, BUGS.md, queue INDEX and matching text, or TRIAGE_OUTCOMES.md. Full applicable
pinned-tip SPEC_impl.md subsection 'The supervisor's terminal emulator' (2517-2548) does not accept false equal-title
failures. A deterministic fixture failure repeating on the same hostname is not a self-correcting diagnostic. Global
duplicate alias: g051/f1; consolidate, preserving both original IDs. g071/f3 is distinct because it requires a capped
long title and snapshot recovery.

Additional reviewed locations: g051/f1:
20ebb7a84b101b52dc808d11e9cd90c23d6ffbd5:crates/farhelm-supervisor/src/emulator_parity/fixtures.rs:81-94.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g053/f2`, `g051/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
20ebb7a84b101b52dc808d11e9cd90c23d6ffbd5; persistence at ca9b0a19856dc136f63ad2d14f1076027edce1eb verified in synthesis
by an empty complete-file diff and direct branch read.. This was a source-only review; runtime occurrence and frequency
were not measured.

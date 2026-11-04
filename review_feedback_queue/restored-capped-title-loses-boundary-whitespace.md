# Snapshot replay trims title whitespace exposed by the title-length cap

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Recovery can change a retained terminal title and its saved title-stack entries by removing trailing whitespace exposed
by the title-size cap.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
composition, no missing source premise for the byte mismatch. At tip vendor/vte/src/ansi.rs:1398-1409 trims the full
incoming title, then vendor/alacritty_terminal/src/term/mod.rs:2450-2467 truncates it to 4096 bytes without another
trim. 4095 letters followed by a space and a nonspace suffix therefore leaves a stored trailing space. paint.rs:308-312
sends that stored title through OSC 2 again, so recovery removes the newly terminal space. Title-stack push/pop at
term/mod.rs:2478-2497 retains that altered value. The exact value is externally readable through Emulator::title
(emulator.rs:284-295), HostTerminal::title (hosts/terminal.rs:2149-2154), and Terminal::title
(service/terminals.rs:333-339). The inspected shipped Codex status reader only checks title prefixes
(agent_kind/screen_reader.rs:705-708,758-774), so this trailing-only difference does not establish a status change; do
not claim wrong status, process loss, security harm, or an observed visible GUI regression. snapshot/mod.rs:394-405
explicitly includes titles and title stacks in the observable-state contract.

Source locations: crates/farhelm-terminal-emulator/src/snapshot/v1/paint.rs:283-287 (introduction); tip:308-312.

vendor/vte/src/ansi.rs:1402-1408 trims the original full OSC title before passing it to Term::set_title;
vendor/alacritty_terminal/src/term/mod.rs:2452-2459 then cuts it at 4096 bytes without trimming. Thus an ordinary OSC 2
payload with 4095 ASCII letters, a space, then a non-space suffix leaves a stored 4096-byte title ending with a space.
paint.rs:283-287 writes that stored title as OSC 2, so replay's same title parser trims the newly terminal space and
restores 4095 bytes. Title-stack entries take the same path. This violates the whole-state round-trip contract for
retained titles; it does not involve permitted dropping of oldest title-stack entries to fit the blob.

recovery can change a retained terminal title and its saved title-stack entries by removing trailing whitespace exposed
by the title-size cap.

Possible coverage and caveats: emulator.rs:284-293 accepts vte's leading/trailing trim as a tmux difference, but that is
ingestion normalization, not a second normalization across snapshot recovery. No exact acceptance, Planned item,
unlanded plan, BUGS entry, queue item or ledger outcome was found; SPEC_impl.md:2549-2586 allows title-stack entry
removal for size, not changes within retained entries. FILTER.md's rare diagnostic/self-correcting display rule does not
clearly cover a retained state mismatch that ordinary refresh does not restore.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g071/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.. This
was a source-only review; runtime occurrence and frequency were not measured.

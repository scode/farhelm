# Clearing empty scrollback leaves a restored missing-history notice active

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

When a recovered checkpoint contains no scrollback because it omitted all the old history, an explicit clear-scrollback
command does not dismiss the missing-output notice; the warning remains until enough new lines scroll or a full terminal
reset occurs.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source; producer, erase parser, handler, counter and display route established without runtime reproduction.

Source locations: crates/farhelm-terminal-emulator/src/emulator.rs:330-337.

emulator.rs:330-337 ends the mark only when the primary grid history_clears counter changes or the line countdown
expires. QueryHandler delegates clear_screen directly (20ebb7a84b101b52dc808d11e9cd90c23d6ffbd5:queries.rs:328, retained
at tip). At tip vendor/alacritty_terminal/src/term/mod.rs:2008-2017 calls clear_history for CSI 3 J only when
history_size is positive; an empty restored history takes the no-op branch, so the counter and mark remain.
snapshot/mod.rs Size permits dropping oldest history up to all of it; the restored mark is independent of retained
history size. SPEC.md:2163-2164 and SPEC_impl.md:2578-2582 promise the notice until the program clears scrollback or
history turns over.

When a recovered checkpoint contains no scrollback because it omitted all the old history, an explicit clear-scrollback
command does not dismiss the missing-output notice; the warning remains until enough new lines scroll or a full terminal
reset occurs. Trigger requires checkpoint trimming with no separate runtime journal-gap flag. Exact route at reviewed
tip: vendor/vte/src/ansi.rs:1665-1677 decodes CSI 3 J as ClearMode::Saved;
crates/farhelm-terminal-emulator/src/queries.rs:330 delegates clear_screen directly.
vendor/alacritty_terminal/src/term/mod.rs:2008-2017 only calls clear_history when current history_size is positive,
otherwise it does nothing. Its grid/mod.rs:441-443 increments history_clears only inside clear_history.
emulator.rs:330-337 therefore sees no erase and retains the trim mark. The all-history-dropped producer is established:
snapshot/mod.rs:450-467 plus fit_history at :845-885 can skip all retained lines; the last-resort path :469-491
explicitly records all history as dropped while supplying trimmed_for(history > 0). snapshot/v1/mod.rs:225-239 encodes a
positive trim count independently of history length (Nothing records history 0), and :488-498 reestablishes that count
after restore. The viewer route is already verified in f1: service/grid_stream.rs:866-872; ui
assets/grid-model.js:508-510; grid-terminal.js:489-498. SPEC.md:2139-2165 and SPEC_impl.md Terminal snapshots
(:2549-2597, especially :2578-2582) explicitly end the checkpoint-trimming notice when the program clears history;
snapshot Size (:44-75) says the same. Existing tests/snapshot.rs:765-770 exercise clearing a nonempty restored history,
so they do not refute this empty case.

Possible coverage and caveats: FILTER.md Rare, self-correcting glitches and imprecise diagnostics might be suggested for
warning-only behavior, but its persistent-wrong-state exclusion applies to the trim state that is carried across
snapshots/restores; no clear exclusion established. Coverage searches of SPEC.md, SPEC_impl.md, TODO.md Planned (empty
:23-24), unlanded plans/queue, BUGS.md, queue INDEX and all queue files, and TRIAGE_OUTCOMES.md found no matching
acceptance, planned fix, duplicate or triage disposition for this exact trigger/consequence/scope.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g059/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
trim-mark logic introduced in assigned c345799ed43ddbd6e0c577f8f2a0add4dd051c6a.. This was a source-only review; runtime
occurrence and frequency were not measured.

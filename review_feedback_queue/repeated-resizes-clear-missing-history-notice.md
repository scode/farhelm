# Repeated height resizes can clear the lost-history warning without replacing the missing history

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After restoring a checkpoint that omitted old scrollback, repeated taller/shorter resizes can remove the missing-output
notice without replacing the old history.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static source
behavior; no runtime reproduction or measurement performed or required to establish this source route.

Source locations: crates/farhelm-terminal-emulator/src/emulator.rs:330-337.

emulator.rs:330-337 subtracts the monotonic lines_scrolled_into_history counter from the trim mark;
vendor/alacritty_terminal/src/grid/resize.rs:57-71 pulls history into the screen without decrementing that counter, and
:86-89 pushes those same rows back through scroll_up on shrinking. grid/mod.rs:325-328 increments the counter on every
such re-entry. Thus cycles consume the remaining warning count repeatedly while retaining the same old rows.
snapshot/v1/mod.rs:470-480 establishes the trim mark after restoring, and snapshot/mod.rs:425-432 persists its remaining
count. SPEC_impl.md at pinned tip ca9b0a19856dc136f63ad2d14f1076027edce1eb:2578-2582 requires the mark until history is
cleared or turned over.

emulator.rs:330-337 subtracts lines_scrolled_into_history from the trim mark. At this commit
vendor/alacritty_terminal/src/grid/resize.rs:57-71 pulls rows out of history without decrementing that counter; :86-89
returns those same rows through scroll_up when shrinking. grid/mod.rs:325-328 counts every re-entry.
snapshot/v1/mod.rs:470-480 establishes the mark after replay and snapshot/mod.rs:425-432 saves the decreased count. Thus
a restored terminal with a bottom cursor and some retained history can cycle height down and back up, consuming the
count repeatedly without evicting old history. The effect can be persisted in later snapshots; the mark does not recover
by reopening. The shipped runtime exposes the mark directly at
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/terminal.rs:1332-1338. Contract: tip
SPEC.md:2162-2166 and SPEC_impl.md:2578-2582 require the notice until the program clears scrollback or it turns over.

After restoring a checkpoint that omitted old scrollback, repeated taller/shorter resizes can remove the missing-output
notice without replacing the old history. Exact trigger: the restored primary screen has a bottom cursor and retained
history; growing the height pulls rows from history and shrinking it pushes those same rows back. Source at tip:
emulator.rs:330-337 consumes the monotonic lines_scrolled_into_history delta; grid/resize.rs:57-71 pulls history back
without subtracting that delta, :86-89 sends the same rows through scroll_up, and grid/mod.rs:325-328 counts their
re-entry. Its counter contract at grid/mod.rs:174-190 explicitly says it does not reflect rows pulled back from history
and callers must account for resize shape changes. snapshot/mod.rs:578-589 saves the consumed count;
snapshot/v1/mod.rs:488-498 restores it. Viewer route verified: service/grid_stream.rs:866-872 forwards the runtime mark,
ui/assets/grid-model.js:508-510 replaces state with decodeState (:314-327), and grid-terminal.js:485-498 shows the
notice directly from that flag. Scope clarification: hosts/terminal.rs:1332-1338 ORs an independent history_gap flag; a
runtime with that flag set keeps its separate gap warning. This finding concerns checkpoint-history trimming when no
independent gap masks it. Contract: tip SPEC.md:2139-2165 distinguishes gaps from checkpoint trimming and explicitly
requires the latter notice until history is cleared or turned over; SPEC_impl.md Terminal snapshots (:2549-2597,
especially :2578-2582) agrees. snapshot/mod.rs Size (:44-75) specifies NEW lines; tests/snapshot.rs:773-788 and
:1177-1200 exercise new output, not repeated resize re-entry.

Possible coverage and caveats: SPEC_impl.md:2580's lines-entering-history wording is qualified by turnover in the same
paragraph, and snapshot Size explicitly says new lines. No specification acceptance of resize re-entry was found.
FILTER.md Rare, self-correcting glitches and imprecise diagnostics does not clearly cover the snapshot-persisted
incorrect countdown; its persistent-wrong-state exclusion is relevant. Coverage sources searched again: Planned in
TODO.md:23-24 is empty; no trigger/consequence/scope match in BUGS.md, queue INDEX/items, unlanded plans/queue, or
TRIAGE_OUTCOMES.md.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g059/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c345799ed43ddbd6e0c577f8f2a0add4dd051c6a. Later
synthesis verified persistence at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb: git diff between these
revisions is empty for emulator.rs, vendor/alacritty_terminal/src/grid/mod.rs, and grid/resize.rs.. This was a
source-only review; runtime occurrence and frequency were not measured.

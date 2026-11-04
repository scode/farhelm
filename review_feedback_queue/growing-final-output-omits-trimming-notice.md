# Growing final output can omit the Restart trimming notice

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Final buffered output can grow the old terminal's history between Restart's count and capture, making the replacement
omit older lines without the promised trimming notice. This does not allege loss beyond the accepted history capacity.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; occurrence and frequency unmeasured

Source locations: fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm-supervisor/src/tmux.rs:4153-4158,4175-4176;
service/status.rs:606-625.

Original g058 prose exclusion overturned after real Stop/status and official pinned tmux3.7c read-callback checks. cut
uses first history count; second guarded capture rechecks identity, dimensions and nonempty history but not count. Seed
capacity can be below tmux12000. Parent independently inspected exact source; R→Ttmux diff empty. Diagnostic-filter
coverage ambiguous, retain. a028 completed another credible empty discovery pass after this productive pass.

if final buffered output pushes a tmux terminal's history above what its new terminal can retain between the two reads,
Restart can omit older history without the promised trimmed-history indication. Source: R sweep.rs:1976-1995 waits for
process teardown then samples optional exit code, while status.rs:606-625 permits no exit code and never waits for tmux
pane-dead/drained output. Official supplied tmux 3.7c window.c:1139-1163 parses PTY output in its read callback;
:374-386 retains a pane while FIONREAD reports pending bytes; format.c:2075-2084 defines pane_dead using closed fd plus
status ready. server.c:264-280 drains command queues separately from event-driven pane reads, so stopping a writer is
not a terminal-drain fence. The ordinary producer is already-written final PTY output, not deliberate private-server
manipulation or output from a still-running agent. R tmux.rs:4153-4158 sets cut only from the first history count;
:4175-4176 rechecks identity, dimensions and nonempty history but not whether history now exceeds lines. The
newest-lines range still truncates to lines. A capacity below tmux's 12000 limit is admitted by
snapshot/mod.rs:598-607,691-711 (restore-cell budget and screen rows); no assumed arbitrary deployment or payload
required. For the conditional case initial_count<=lines<later_count and an otherwise untrimmed small capture, the byte
cap and seed encoder need not add the missing cut flag.

Possible coverage and caveats: FILTER.md Rare, self-correcting glitches and imprecise diagnostics may cover a narrow
timing-window missing indication; whether a retained terminal's trimming indicator counts as that diagnostic is
interpretive, so AMBIGUOUS on filter exclusion and retain. SPEC.md:1374-1382 promises a notice when old output is left
out; SPEC_impl.md Moving an old session into a host:2195-2265 accepts row/byte limits and certain style/link omissions
but does not explicitly accept a missing cut mark from a stale first count. Planned empty at TODO.md:23-25. Targeted
searches across SPEC/SPEC_impl, TODO, BUGS, queue INDEX/files, plans/queue and TRIAGE_OUTCOMES found no exact
history-between-reads trim-notice decision. Host-terminal settle-timeout family g131/f1 and g183/f1 is a different
substrate and does not cover this tmux indication issue.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `a028/f7`,
`g058: history-growth prose candidate`. Original append-only events, revisions, withdrawals and independent judgments
are retained in that run's catalogs. Locations refer to the named reviewed revisions; the original revision context is
fba73468a9cb09c217a39da2be7f2c7cea064cb8; contracts searched/read at pinned ca9b0a19856dc136f63ad2d14f1076027edce1eb.
Original exact prose candidate g058 'History grows between reads without reporting truncation', repeated turn1 through
turn4. Verdict DISAGREE with the proposed source refutation 'agent stopped, no supported producer'; retain only the
conditional missing-notice remainder.. This was a source-only review; runtime occurrence and frequency were not
measured.

# Restart fixture types before its safe-to-type barrier

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The restart tests can overwrite their startup marker with early test input and then report a healthy supervisor restart
as having lost terminal contents.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source
mechanism; occurrence/frequency unmeasured, no observed runtime flake or reproduction claimed.

Source locations: crates/farhelm/tests/e2e/host_processes.rs:675-678.

host_processes.rs:425-428 waits for FAKE-AGENT READY and types before-restart; :449-454 later requires READY again.
Same-commit crates/farhelm-fixtures/src/fake_agent.rs:1667-1674 expressly defines > as the SAFE-TO-TYPE barrier and
explains that input after READY alone can overwrite its row; :1682-1684 emits READY then >. Investigated transport
premise: terminal-host/src/host.rs:503-542 journals nonblocking PTY reads without line framing; supervisor
hosts/terminal.rs:3222-3304 feeds each chunk and calls after_record, :3784-3816 publishes it, with no
prompt/complete-line fence. Grid stream read_home (:516-559) reads the current emulator rather than waiting for the
prompt. These checks do not rule out the documented split marker/line-termination schedule.

tip host_processes.rs:675-678 sends before-restart once FAKE-AGENT READY appears, while :700-704 later requires that
marker. Same-tip fake_agent.rs:1667-1684 documents > as the SAFE-TO-TYPE barrier and emits READY followed by CR/LF
before that prompt. Static mechanism established during synthesis: Linux v6.8 drivers/tty/n_tty.c:521-576 processes the
block before an ONLCR newline, writes marker plus CR and releases output_lock; :2377-2392 invokes process_output
separately for LF; :491-496 locks/unlocks separately, and :779-808 permits receive-side input echo under that same
per-operation lock. Input can therefore be echoed at column zero on the marker row between CR and LF. tty_io.c:124-129
defines OPOST|ONLCR/ECHO defaults; tip terminal-host/src/spawn.rs:370-405 preserves defaults except erase/IUTF8. PTY
output reads are chunks (terminal-host host.rs:503-542), and supervisor hosts/terminal.rs:3222-3304,3784-3816 publishes
each record without a complete-line barrier.

the restart tests can overwrite their startup marker with early test input and then report a healthy supervisor restart
as having lost terminal contents.

Possible coverage and caveats: FILTER.md:24-42 Rare, self-correcting glitches and imprecise diagnostics; exact ambiguity
is whether :32-34's safely retryable operation includes regression-test false failures and whether safe test retry is
sufficient. Parent resolves that uncertainty under FILTER.md:18-20: unclear filter coverage does not match, so retain.
Earlier withdrawal events remain historical; their exclusion recommendation is superseded. No exact SPEC, TODO Planned,
unlanded plan, BUGS, queue or ledger coverage was found in the recorded searches. g153/f1 is related but distinct:
post-echo expected-screen capture before its separately flushed prompt, rather than our startup-marker overwrite by
input.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g190/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced in 5e467f4d64f8c815173ce95cb6d180e83aa67704 at :425-428 and retained by
58094288259b0c29f74ea25936d4fe006c72128f at :675-678. Parent disposition correction under original g190/f1; this is not
a new discovery.. This was a source-only review; runtime occurrence and frequency were not measured.

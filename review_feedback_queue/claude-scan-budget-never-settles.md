# Claude scan capture never settles in large project directories

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A Claude session in a heavily used directory may never offer "resume" and keeps costing background disk work.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F15 / COR-CLAUDE-SCAN-BUDGET`, tagged **possible**. Anchors and title: `agent_kind/capture.rs:388-428`,
`agent_kind/capture.rs:463-465`, `service/capture.rs:1090-1101`, `service/capture.rs:1228-1281` — Claude record-scan
capture can never complete in a large project directory, and those sessions are rescanned forever

Restart can offer "Resume" only if the supervisor knows which conversation the agent was in. For Claude that id normally
comes from a hook report. When no report arrives, the capture pass falls back to scanning Claude's per-project record
directory, `~/.claude/projects/<munged cwd>/`. This happens when the user passes their own `--settings` or disables
hooks. The scan looks only at direct entries (`record_depth` 0) and picks the `.jsonl` whose creation time falls in a
window around the session's first input, from 5 s before to 60 s after.

`scan_records` enforces a budget of `MAX_VISITED_ENTRIES` = 4096 entries per scan, plus a 750 ms time limit. Every
directory entry counts toward the budget: every old `.jsonl`, and every per-conversation subdirectory Claude creates.
The mtime floor that would discard old files is applied only after the entry has been counted and stat'ed
(capture.rs:463-465). If the budget runs out, the scan returns `complete: false` with no candidates. `capture_pass`
commits a claim, or settles as `UncapturedFinal` ("looked, found nothing, stop looking"), only when the scan is complete
and the window has passed (service/capture.rs:1228-1281). A session in a project directory with more than about 4096
entries therefore never captures and never settles. It gets rescanned on every ticker pass and every `ListSessions`
reply for its whole life. Each rescan costs thousands of `stat` calls, made while holding the global capture-pass lock
that list replies wait on.

The user-visible result is that a Claude session in a heavily used directory never offers "resume this conversation" at
restart. Nothing in the log explains this, because an incomplete scan is silent, unlike an ambiguity. The session also
keeps costing background disk work. The open premise is how common project directories with more than 4096 entries are
among sessions that depend on the scan fallback rather than the hook. The suggested fix is to stop counting entries that
the mtime floor would drop, or to give them a separate, much larger budget. Also, once a session is past its horizon, an
incomplete scan should settle to `UncapturedFinal` with a log line. That claims nothing, so it stays safe, and it stops
the endless rescans.

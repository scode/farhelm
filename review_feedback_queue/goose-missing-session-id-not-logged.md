# Goose reporter logs nothing without AGENT_SESSION_ID

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When Goose doesn't provide its session id, the Farhelm session silently never becomes resumable and the hook log gives
no hint why.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F20 / COR-GOOSE-MISSING-ID-NO-LOG`, tagged **possible**. Anchors and title: `crates/farhelm/src/main.rs:1041` — With
the Goose reporter enabled but AGENT_SESSION_ID missing, nothing is reported and nothing is logged

For Goose, Farhelm's conversation reporter runs inside `farhelm internal goose-hook`, which Goose starts as an MCP
extension. It reads Goose's conversation id from the `AGENT_SESSION_ID` environment variable and passes it to
`hook::run_with`, the function that sends the report and writes the hook log line. That call sits entirely inside
`if let Ok(goose_id) = std::env::var("AGENT_SESSION_ID")` (main.rs:1041) with no `else`. If the variable is missing or
not valid UTF-8, the helper sends no report _and_ writes no hook-log line, even though the log path is already computed.

hook.rs documents the hook log as the one place a reporter failure shows, with exactly one line per run. Here the
session silently never becomes resumable and the log is empty. Suggested change: in the missing-variable case, append a
`bad-payload missing-agent-session-id` line through the same log path, still exiting 0 and still serving MCP.

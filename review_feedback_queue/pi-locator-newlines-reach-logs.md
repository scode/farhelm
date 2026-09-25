# Pi conversation locators can carry newlines into logs

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A session process can inject line breaks into supervisor logs.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F18 / COR-PI-LOCATOR-LOG`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/core.rs:13726-13753`, `farhelm-supervisor/src/agent_kind/mod.rs:281-284` — Pi
conversation locators can carry raw newlines into log lines

For the Pi agent, the "conversation id" an agent reports is not a bare id but a **locator**: a vendor prefix followed by
a small JSON object with a version, session id, and session-file path. The supervisor validates it with `parse_locator`
(`agent_kind/mod.rs:281-284`), which strips the prefix and hands the rest to `serde_json::from_str`. JSON allows
whitespace between tokens, and newlines and tabs count as whitespace. A locator like `<prefix>{\n"version":1,…}`
therefore validates. After a successful report, the supervisor logs the **original** string with `%` (Display) in its
`info!` lines (`core.rs:13726-13753`), so those newlines reach the log raw.

A session process can therefore break supervisor log lines. The injected content is tightly limited: everything after a
newline must still be valid JSON that forms a plausible locator. Forging a convincing fake entry is much harder than in
F17, but splitting lines is easy. The Codex and Grok paths do not have this problem, because they store and log a
re-encoded, canonical form. The suggested fix is to do the same for Pi: store and log the locator re-serialized by
`encode_locator`.

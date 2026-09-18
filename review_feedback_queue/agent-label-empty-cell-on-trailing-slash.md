# Agent label renders an empty cell on trailing-slash invocations

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A session whose recorded command ends in a slash shows a blank agent name in the session list, where every other row
shows a label.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: possible, minor. Coordinator verified the
basename path and the empty-invocation chain against the documented promise.

`agent_label` (`crates/farhelm-helm/src/agent_requests.rs:1558-1576`) promises "a slightly uglier label beats either an
empty cell or a parse failure" (:1556-1557), but two degenerate raw invocations still produce `""`: a first token ending
in `/` (e.g. `"/bin/"`, `"/"`) makes `basename` (`program.rsplit('/').next()`, :1580-1582) yield `""`, and an empty
invocation flows through `shell_words::split` → `Ok(vec![])` → `None` → whitespace fallback `None` →
`unwrap_or_default()` → `""`. Impact is cosmetic: the `agent` column renders empty in `farhelm agent sessions` for that
row (and any agent-side matching on the field sees an empty string). Reachable whenever a raw session's recorded
invocation has one of these shapes — the trailing-slash case stands regardless of whether empty invocations are
precluded upstream.

Suggested fix: make the empty result impossible at the end of the chain, e.g. fall back to the unparsed program spelling
when the basename is empty (`if base.is_empty() { program } else { base }`), which matches the documented intent without
ever surfacing an argument.

# The hook-log test parser drops the identity pair from write-failed and bad-payload lines

Reviewed commit: a333cb197dc0542c0d970031d2596479d9a50e4b

## TLDR

No user-visible effect. Only the test tooling is affected: the new structured reader for the per-session hook log
misreads two kinds of hook line that do name a conversation. One test now asserts that such a line names no
conversation. A regression that dropped or corrupted the conversation on those lines would go unnoticed, and a future
reader that trusts the parser would draw wrong conclusions.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t05f-trust-hooks-backend` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept (maintainer test tooling). No coverage source covers it.

### Reviewer record

Location: `crates/farhelm-supervisor/src/hook_report.rs:575`

At a333cb197d, `crates/farhelm/src/hook.rs:396-440` (`finish_report`) renders
`write-failed <io error> <conversation> <source>` and, for sub-agent refusals,
`bad-payload <reason> <conversation> <source>` (hook.rs:403). The module docs at hook.rs:142 give
`write-failed No such file or directory (os error 2) conv-1 -` as an example, and the documented line shape
`[<outcome detail> ][<conversation-id> <source>]` applies to every outcome. The new `parse_log_line`
(`crates/farhelm-supervisor/src/hook_report.rs:575-617`) only splits out `about` for `acked`, `written` and `refused`.
Every other word falls to `_ => (None, payload)`, so the conversation and source of a write-failed or sub-agent
bad-payload line end up inside `detail` and `about` is `None`. The `HookLogLine` doc says these are 'the fields readers
can rely on'. In `crates/farhelm/tests/e2e/hook_identity.rs`,
`antigravity_hook_writes_json_on_early_exit_and_write_failure` now calls
`assert_sole_hook_log_outcome(state.path(), "sess-blocked", "write-failed", None, None)`, which asserts
`entry.about == None` for a line that does carry the reported conversation. Before this commit the test only checked
`starts_with("write-failed ")`. The new assertion encodes the parser's blind spot as the expected format. Verify by
reading `finish_report` (the `.about(...)` call applies to the write-failed outcome) beside the parser's match arms.
Fix: parse the trailing identity pair with the `rsplitn(3, ' ')` arm for `write-failed` too, and for `bad-payload` when
it has two trailing tokens. Note that a one-word `bad-payload` reason has no pair, so the arm has to tell the two shapes
apart, for example by requiring the documented identity shape. Then change the sess-blocked assertion to expect the
reported identity.

Suggested bucket: other. The harm is confined to test tooling: a weakened, misleading assertion and a parser contract
that does not match the documented format.

Confidence: confirmed (source-established from hook.rs finish_report and the parser's match arms; the test was not run).

proposed_drop: none

possible_cover: none

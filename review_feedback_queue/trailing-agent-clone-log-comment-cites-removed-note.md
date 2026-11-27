# The agent clone log's comment cites a note 443eac14b0 removed

Reviewed commit: 443eac14b0f1f76339817f9c9a07cd20ecfc25e3

## TLDR

No user-visible effect. In the helm's agent-request code, the clone dispatch's log call tells readers to see the create
dispatch's note on why its logged values are safe, but 443eac14b0 removed that note and changed the create site to
escape the value itself, so the cross-reference now points nowhere and the two sites disagree.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t09a-sessions-lifecycle` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Narrowed at recording to the dangling cross-reference and the create/clone inconsistency. The double-escaped create log
line the reviewer also reported fully matches FILTER.md's "Rare, self-correcting glitches and imprecise diagnostics"
(shipped clients never produce such session IDs; the consequence is an imprecise log line), and the independent drop
check AGREED, so that part is not queued. The reviewer record below still describes both parts.

### Reviewer record

Location: `crates/farhelm-helm/src/agent_requests.rs:3117`

At 443eac14b0, `dispatch_agent_create` (crates/farhelm-helm/src/agent_requests.rs:3117 at the tip) changed
`asking = asking_session` to `asking = escape_for_log(asking_session).as_str()` and deleted the comment above it. That
comment was the target of the clone site's cross-reference at agent_requests.rs:4150 (tip):
`// See `create_for_agent`'s own note on why both logged values are safe.`, followed by `asking = asking_session` and
`source = request.source_session_id.as_str()`, both unescaped. That reference now points nowhere, and next to the
escaped create line it reads as if clone were the missed case.

On the escaping itself: SPEC_impl.md, "Session IDs in logs", says "Safe escaping by the configured logging formatter
also satisfies the requirement without a separate escaping helper at each call site ... do not add duplicate escaping or
new infrastructure where the logger already provides it." The helm logs through tracing-subscriber 0.3.23's default fmt
formatter (crates/farhelm/src/main.rs:2788, crates/farhelm-desktop/src/lib.rs:486). Its DefaultVisitor::record_str
(tracing-subscriber-0.3.23/src/fmt/format/mod.rs:1264-1273) writes every non-message `&str` field through
`record_debug(field, &value)`, i.e. `str`'s Debug, which quotes the value and escapes newlines, other controls and
non-printable format characters such as U+202E (Rust's `char::escape_debug`). So the field was already escaped before
this commit, and passing the `escape_for_log` output turns `\u{202e}` into `\\u{202e}`. The new test
`ordinary_agent_create_escapes_the_asking_id_in_its_log` checks the captured field value before formatting, so it cannot
show whether the formatted line was ever unsafe. Several older call sites (agent_requests.rs:1476, 2457, 3425, ...)
already use `escape_for_log` this way, so the create change follows an existing local pattern; the pattern itself is the
drift from the spec rule.

Verify: read the two call sites and the tracing-subscriber record_str source; format a `&str` holding U+202E with
`{:?}`.

Fix: either drop `escape_for_log` at log sites and rely on the formatter, as the spec prefers, or keep it and restore an
accurate comment at the clone site (and escape clone's two IDs for consistency). At minimum, replace the dangling
cross-reference at the clone site.

Suggested bucket: other. A diagnostics and comment-accuracy issue with no security effect: the formatter already
prevents log injection on both sites.

Confidence: confirmed for the dangling reference and the create/clone inconsistency. That Debug escapes U+202E rests on
Rust's documented escape_debug behavior for Cf characters, which I did not check by running anything.

proposed_drop: none (low value; the commissioner may fold it into a broader logging-escape cleanup).

possible_cover: SPEC_impl.md "Session IDs in logs" (the rule it drifts from); the archived outcome
`helm-agent-create-dispatch-logs-asking-session-unescaped.md` that this commit closed asked for the create change.

# Grok Restart and named start put `--resume` after a Custom `--` tail

Reviewed commit: 06138dd5c3bd19fe5ed85e378e5162d3f77e821d

## TLDR

A Grok session started with Custom permission words that include `-- <words>` still reports its conversation, so Farhelm
offers Restart (and named starts). But Restart's `--resume <conversation>` is placed after the user's `--`, where Grok
reads it as prompt text. Restart then starts a new Grok conversation with an odd first message instead of resuming the
one the user was working in, and the session follows that new conversation from then on.

## Details

Priority bucket: `high`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t05b-trust-hooks-backend` `l1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Recorded in the reviewer's suggested `high` bucket (Restart silently starts a fresh conversation in place of the working
one; the old one stays in Grok's own store). Confidence is `likely`: the argument order is source-established, Grok's
`--` handling comes from Farhelm's own encoding and attribution tests, not from running Grok. FILTER.md's
non-first-class harness filter was considered (Grok is not first-class and a Custom tail ending in `--` is unusual), but
whether "Restart resumes nothing" is within that filter's "missing or delayed Resume offer" consequence is unclear, so
it is recorded. It would move to `other` if triage applies that filter or judges bare `--` Custom launches unrealistic.

### Reviewer record

Location: `crates/farhelm-helm/src/launches.rs:882`

Lead l1 promoted. At the reviewed commit (and unchanged at tip), crates/farhelm-helm/src/launches.rs
`custom_positional_tail_len` (tip 882-904) returns the `--` tail length only for Antigravity; every other harness gets
0, so `compile` (tip 657-675) appends `resume_arguments` after the whole Custom word list. For Grok, start is
`grok --no-leader <custom words> {farhelm_args}` (launches.rs:534) and resume is
`grok --no-leader <custom words> --resume {conversation} {farhelm_args}` (launches.rs:1048). With Custom words
`--always-approve -- note`, Restart runs `grok --no-leader --always-approve -- note --resume <id>`. Premise that Grok
ends options at `--`: Farhelm already relies on it in two places — `LaunchHarness::initial_prompt_arguments`
(crates/farhelm-proto/src/launch.rs:174-193) gives Grok `["--", prompt]` because "Positional prompts follow an option
terminator, so leading hyphens cannot become agent options", and the Grok attribution test procs.rs:3079-3124 says "An
option after `--` belongs to the prompt" and refuses `grok -- --no-leader`. Route to the consequence: Grok reports
through persistent hooks the user installed, and `inject_hook_argv_tail`
(crates/farhelm-supervisor/src/agent_kind/mod.rs:771-806) returns before the bare-`--` refusal for an empty tail, so
unlike Claude/Codex/Muse (whose argv hooks are skipped on a bare `--`, leaving no capture and no Restart) a Grok Custom
`--` launch is captured and Restart is offered. `--no-leader` stays before the `--`, so attribution still admits the
report. The doc on `resume_arguments` (tip launches.rs:1038-1040) claims "The compiler inserts them before any Custom
positional tail", which holds only for Antigravity, and the drift test's `delimited_custom`
(crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs:153-165) returns None for Grok, so nothing would
catch it. Verify: compile a Grok selection with Custom `["--always-approve", "--", "note"]` and inspect
`IntegrationSnapshot::of(&launch).filled_resume_argv("id")` and `named_start`; the selector follows `note`. Fix: return
the `--` tail length for Grok as well (and audit Cursor, Pi, OMP, Goose, OpenCode, whose CLIs may also end options at
`--`), or refuse Custom words containing a bare `--` for harnesses whose resume selector is an option. The same
misplacement applies to Grok's initial prompt (`-- note -- <prompt>`), where the second `--` becomes part of the
message.

Suggested bucket: high — Restart silently starts a fresh conversation in place of the user's working one; the old
conversation stays in Grok's own store, so it is not data loss. Would move to other if Grok Custom launches with a bare
`--` are judged too unusual to matter, but rarity alone does not lower the bucket.

Confidence: likely — the argv order is source-established; Grok's `--` handling is taken from Farhelm's own encoding and
attribution tests rather than from running Grok.

proposed_drop: none

possible_cover: none known.

# Antigravity Restart puts the conversation selector after a Custom trailing `--`

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

If a user launches an Antigravity session with Custom permission flags that end in a bare `--` (a shape Farhelm
explicitly supports for Antigravity), plain Restart and Restart with a named conversation relaunch Antigravity with the
conversation choice placed after that `--`. Antigravity treats everything after `--` as plain text rather than options
(going by the repository's own notes; not observed), so Restart does not reopen the captured or named conversation. The
new live drift check written to guard this ordering therefore fails on every Antigravity run with today's code.

## Details

The product defect is pre-existing, in the helm's launch compiler (`crates/farhelm-helm/src/launches.rs`, `compile`).
60f24ea6ea adds the delimited Antigravity Custom drift case and `require_resume_words`; 54b4c96330 moves it behind
`delimited_custom` (`crates/farhelm/tests/e2e/real_agent_capture/drift/permission_modes.rs`: `delimited_custom`,
`require_resume_words`, `check_custom_restart`). Main references: `launches.rs:679-705` (Custom words, trust splice,
resume = argv + suffix) and `:1004-1027` (`resume_arguments` and its doc).

The launcher accepts `--` as a Custom word: `split_permission_flags` (`crates/farhelm-proto/src/launch.rs:1022-1032`)
refuses only empty lists, bad quoting and NUL. `compile()` appends Custom permission words (`launches.rs:679-689`),
splices trust words before any Custom tail that starts at `--` (`trust_custom_tail_len`, `:868-890`, which exists
because a Custom `--` is supported; pinned by `crates/farhelm-helm/src/workspace_trust.rs:218-255`), then builds resume
as `argv.clone()` + `resume_arguments(harness)` + `{farhelm_args}` (`:700-705`). For Antigravity `resume_arguments` is
`["--conversation", CONVERSATION_PLACEHOLDER]` (`:1020`), so the selector lands after the user's `--`. The doc of
`resume_arguments` (`:1006-1011`) asserts "the start commands this module composes carry no positional prompt or `--`
they could fall behind", which is false for Antigravity Custom words.

Nothing downstream reorders it. `IntegrationSnapshot::of` takes `resume_template` from `launch.resume_argv()`
(`crates/farhelm-supervisor/src/agent_kind/mod.rs:1986`); `filled_resume_argv` and `filled_named_resume_argv`
(`mod.rs:2130-2190`) fill the placeholder in place; `ensure_resume_template` (`mod.rs:2652-2670`) checks only emptiness,
executability and placeholder-as-program; the supervisor's `relaunch_argv`
(`crates/farhelm-supervisor/src/service/core.rs:3717-3753`) execs `snapshot.resume_argv` verbatim. Antigravity reports
through persistent hook files (`agent_kind/antigravity.rs`, `persistent_hook_files`), not an argv tail, so the bare-`--`
hook skip in `inject_hook_argv_tail` (`agent_kind/mod.rs:809`) does not apply. SPEC.md's Antigravity admission checks
("positional attribution and pin checks", transcript first record) concern the reporting process, not the resume argv,
so capture succeeds and plain Restart is offered with the broken command. Antigravity offers named conversation starts
(`crates/farhelm-proto/src/launch.rs:499-511`), which use the same template.

The drift test's `require_resume_words` checks that `--conversation` occurs before the first `--` in the captured resume
argv. Per the code above it never does, so `check_custom_restart` returns Failed for the delimited mode on every
Antigravity run that gets as far as a capture, and `execute()` aggregates that into a failed case. No unit test pins
resume placement relative to a Custom `--`.

Verify by reading `compile()` for
`{harness: Antigravity, permissions: Custom, permission_flags: [--mode, auto, --settings, {}, --]}`: resume is
`[agy, ..., --mode, auto, --settings, {}, --, --conversation, <placeholder>, {farhelm_args}]`. Fix: for harnesses whose
CLI ends options at `--`, insert the resume selector before the Custom tail that starts at `--` (the offset
`trust_custom_tail_len` already computes), add a helm unit test pinning it for plain and named Restart, and correct the
`resume_arguments` doc.

Two related observations outside this finding's commits. The supervisor appends Antigravity's initial prompt as
`--prompt-interactive=<text>` at the very end (`crates/farhelm-supervisor/src/service/core.rs:14689-14695`;
`crates/farhelm-proto/src/launch.rs:144-160`), so with a Custom trailing `--` that option also becomes positional text.
Grok's resume (`--resume <id>`) has the same append-after shape, and Grok captures through its own hooks (an empty argv
tail returns before the `--` check), but whether Grok's CLI treats `--` as end-of-options is unverified, and
`delimited_custom` names only Antigravity.

Bucket: other. Restart (plain or named) of a rarely configured Antigravity Custom launch fails to resume, and the vendor
conversation itself is not deleted; it is also a deterministic false red in maintainer drift tooling. It would move to
high if a common launcher path or template produced a trailing `--`. Not highest: nothing is lost (no data, credentials
or processes); the conversation stays in Antigravity's store and the user can name it explicitly once the ordering is
fixed.

Confidence: confirmed from source for the argv ordering and the absence of any reordering or refusal downstream. That
Antigravity's CLI treats `--` as end-of-options is taken from the repository's own statements (`launches.rs:835`,
`:868`; the drift test docstrings), not observed.

Possible cover: none known; the drift test added in 60f24ea6ea is the only detector. A drop argument triage may weigh:
SPEC.md, Custom permissions paragraph, "The user is responsible for keeping these flags limited to permissions; Farhelm
neither checks their effect nor infers a safety mode". Against it, the helm deliberately supports a Custom `--` for
Antigravity (`trust_custom_tail_len`), and the same paragraph says "plain Restart or Replace reuses the stored command",
which presumes a working resume.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g03 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.

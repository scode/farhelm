# Antigravity's initial prompt still lands after a Custom `--`

Reviewed commit: 06138dd5c3bd19fe5ed85e378e5162d3f77e821d

## TLDR

An Antigravity launch whose Custom permission words end in `-- <operands>` and that carries an initial prompt (fresh or
named start) hands the prompt to agy after the `--`, so agy reads `--prompt-interactive=<text>` as a literal operand
instead of the interactive prompt option. The user's first message is not delivered as Farhelm promises (SPEC.md:
"Initial prompts work for both fresh and named starts"). The commit fixed exactly this placement for the conversation
selector and trust option but left the third Farhelm-owned option on the wrong side.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t05b-trust-hooks-backend` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. FILTER.md's non-first-class harness filter may apply (Antigravity is not first-class,
the configuration is unusual); whether an undelivered first message is within its consequence list is unclear, so it is
recorded for triage. No other coverage source covers it.

### Reviewer record

Location: `crates/farhelm-supervisor/src/service/core.rs:14674`

Revision (turn 2): adds the refusal check; conclusion, bucket and confidence unchanged.

The commit (crates/farhelm-helm/src/launches.rs compile, tip lines ~657-675) inserts trust words and the resume selector
before `custom_positional_tail_len`, because "the CLI reads the rest as operands" (tip launches.rs:878-880). The initial
prompt is added later in the supervisor: crates/farhelm-supervisor/src/service/core.rs:14670-14679 (tip) does
`argv.extend(harness.initial_prompt_arguments(text))` after the {farhelm_args} fill, and
crates/farhelm-proto/src/launch.rs:187 makes Antigravity's argument `--prompt-interactive=<prompt>`, an option. With the
commit's own test shape (`--mode auto --settings {} -- "literal operand"`) plus a prompt, the executed argv is
`agy --mode auto --settings {} [--conversation X] -- 'literal operand' --prompt-interactive=hello`. Nothing refuses this
combination: `SessionLaunch::validate_initial_prompt` (crates/farhelm-proto/src/session_launch.rs:271-291), which both
helm and supervisor run before create, checks only `harness.supports_initial_prompt(named)`; `supports_initial_prompt`
(proto launch.rs:155) is harness-only; `check_permission_flags` (launch.rs:1055) accepts `--`. Verify: extend the
commit's test `antigravity_custom_delimiter_keeps_both_conversation_selectors_in_options` with an initial prompt through
the supervisor spawn path, or add a prompt to the `delimited_custom` drift case in `permission_modes.rs`. Fix: insert
the prompt option before the Custom positional tail for harnesses whose prompt is an option (Antigravity), e.g. have the
integration expose the tail boundary, or refuse an initial prompt on such launches in `validate_initial_prompt` with a
clear reason.

Suggested bucket: other — the first message is not delivered or is misread on a rare Custom shape; no data or security
impact. Would move up only if agy is shown to act on stray operands in a harmful way.

Confidence: likely — source establishes the argv order and the absence of a refusal; the premise that agy treats words
after `--` as operands is the one the commit and its drift test (`permission_modes.rs` DelimitedCustom) already rely on.

proposed_drop: none

possible_cover: none known. The append site predates the reviewed commit, but the commit claims to keep Farhelm's
options before the tail and leaves this one out.

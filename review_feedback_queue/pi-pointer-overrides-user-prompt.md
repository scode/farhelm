# Pi's instructions pointer may override the user's own appended system prompt

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

A Pi profile that passes its own `--append-system-prompt` may have that text silently dropped on every launch with agent
instructions on, because Farhelm appends its own after it.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F55 / COR-PI-APPEND-PROMPT`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/agent_kind/pi.rs:31` — Pi's instructions pointer replaces the user's own
--append-system-prompt.

When "agent instructions" are on, Farhelm adds a short line to a supported agent's system prompt telling it that the
`farhelm agent` command exists (the "instructions pointer"). For Pi, the supervisor's launch rewriting in
`crates/farhelm-supervisor/src/agent_kind/pi.rs:31-36` always appends `--append-system-prompt <pointer>` to the user's
command line.

The user may already pass that option themselves, for example `pi --append-system-prompt "review against the spec"`. The
OMP integration (OMP is a Pi derivative) handles this case explicitly:
`crates/farhelm-supervisor/src/agent_kind/omp.rs:395-408` skips the pointer when the user's command already carries
`--append-system-prompt`, because OMP's argument parser keeps only the last occurrence and a second one would silently
replace the user's text. A test, `omp_pointer_yields_to_a_users_own_appended_system_prompt`, pins that. Pi has no such
check, so Farhelm's pointer is added after the user's option.

If Pi's parser also keeps only the last occurrence, as OMP's does, the user's own system-prompt instructions are
silently discarded on every Pi launch with instructions enabled. That premise is not confirmed: the reasoning rests on
OMP's documented behavior and the shared lineage, not on Pi's own parser.

The suggested fix is to apply OMP's rule to Pi: detect a user-supplied `--append-system-prompt` (both the
separate-argument and `=` forms), and in that case let only the pointer yield (the reporter extension still loads); add
the same pinning test for Pi.

# validate_model_or_effort permits leading hyphens in custom agent --model and --effort values

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

Custom agent model and effort values can begin with a hyphen (for example `--resume` or `-v`), which Farhelm passes as
separate `--model <value>` and `--effort <value>` arguments. Standard argument parsers in custom agent binaries or
wrappers (such as clap, Python argparse, Go flag, or getopt) can reject these values as missing arguments or
misinterpret them as options, inconsistent with Farhelm's non-option guarantee for built-in model IDs, custom
executables, and conversation IDs.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/custom_agents.rs:74`.

`crates/farhelm-proto/src/custom_agents.rs:74-84` (`validate_model_or_effort`) checks only that `value` is 1..=256 bytes
of `[a-z0-9_-]` and omits a `!value.starts_with('-')` check. By contrast, every other literal argv value validator in
Farhelm rejects leading hyphens: `validate_model_id` (`crates/farhelm-helm/src/launches.rs:1239`), `validate_command`
(`crates/farhelm-proto/src/custom_agents.rs:60`), `is_plausible_conversation_id`
(`crates/farhelm-proto/src/conversation.rs:42`), and `validate_id` (`crates/farhelm-proto/src/custom_agents.rs:40`).
`compile_custom` (`crates/farhelm-helm/src/launches.rs:754-764`) emits `["--model", value]` and `["--effort", value]` as
two separate argv elements (per the custom-agent contract). Leading-hyphen values reach `compile_custom` from the GUI
launcher/template editor and from the CLI (`crates/farhelm/src/main.rs:1042` sets `allow_hyphen_values = true` on
`--model`, and `--effort=--resume` passes `parse_effort` at `crates/farhelm/src/main.rs:952-959`).

How to verify: Call `farhelm_proto::custom_agents::validate_model_or_effort("--resume")` or
`validate_model_or_effort("-x")` — both return `Ok(())`.

Fix: Reject `value.starts_with('-')` in `validate_model_or_effort` (and update `SPEC.md` and
`website/src/content/docs/docs/agents/custom-agent-contract.md` + its Contract changelog accordingly, plus a test case
in `custom_model_and_effort_are_bounded_literals`).

Reviewer metadata (g41a_custom_agents_proto_types/f1): suggested bucket: other — correctness and wrapper-contract
consistency issue. Does not reach `highest`: every custom agent launch is classified as YOLO
(`crates/farhelm-proto/src/yolo.rs:58-63` via `LaunchHarness::Custom.stored_permission`), so the agent YOLO guard
refuses custom agent launches on hosts that ask before YOLO; on hosts where YOLO is allowed without asking, approval
cards show the compiled command verbatim and an agent with both commands-without-asking and YOLO-without-asking can
already run arbitrary `--command ... --yolo`.; confidence: confirmed (source analysis).; possible cover:
`SPEC.md:493-494` ("accept literal model and effort values of up to 256 bytes from `[a-z0-9_-]`") and
`website/src/content/docs/docs/agents/custom-agent-contract.md:53-56` ("Value is a non-empty string matching
`^[a-z0-9_-]+$` (max 256 bytes)."). Neither explicitly states intent to allow leading hyphens as option-like arguments
(unlike Section 3 of `custom-agent-contract.md`, which explicitly forbids leading `-` on conversation IDs)..

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41a_custom_agents_proto_types/f1`.

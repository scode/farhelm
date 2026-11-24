# CLI --effort rejects valid leading-hyphen custom values and drops effort-choice guidance while SPEC_impl.md still describes closed-set parsing

Reviewed commit: 0ae1f6de79be7b712ec33dfd71b3a38edf3b1399

## TLDR

Passing a custom agent effort value that starts with a hyphen as a separate argument (such as
`farhelm agent create --agent acme-code --effort -fast`) fails locally as an unrecognized flag, even though
`--effort=-fast` and `--model -fast` succeed and the custom-agent contract permits leading hyphens. Separately,
`SPEC_impl.md` still describes `--effort` as a closed-set flag (note: built-in `--effort` typo guidance was fixed
post-tip in `#4752`).

## Details

**other.** Source inspection. Main reference `crates/farhelm/src/main.rs:1042`.

Post-tip note (`ff379d4cc` / `#4752`): After pinned review tip `1b3fe49af86979aebc2719965682a17f8c6366ac`, `#4752`
updated `parse_effort` and helm refusal messages so built-in `--effort` typos now list the resolved model's valid effort
choices. Two parts of this finding remain open on `main`: `LaunchFlags::effort` in `crates/farhelm/src/main.rs` still
omits `allow_hyphen_values = true` (so `--effort -fast` fails locally while `--effort=-fast` succeeds), and
`SPEC_impl.md:7466` still describes `--agent`, `--effort`, and `--permissions` as closed-set flags.

- `SPEC.md:492` (commit `db311ca0e8bfabcc05422eecffdb690ec450e1d5`) and
  `website/src/content/docs/docs/agents/custom-agent-contract.md:59` define custom model and effort values as 1–256
  bytes matching `^[a-z0-9_-]+$`, and `crates/farhelm-proto/src/custom_agents.rs:72-81` (`validate_model_or_effort`)
  accepts leading `-` (for example `-fast`).
- In `crates/farhelm/src/main.rs:1038-1044`, `LaunchFlags::model` carries `allow_hyphen_values = true`, so
  `--model -fast` is accepted, while `LaunchFlags::effort`
  (`#[arg(long, value_name = "LEVEL", value_parser = parse_effort)]`) omits `allow_hyphen_values = true`. As a result,
  `farhelm agent create --agent acme-code --effort -fast` (and `farhelm spawn` / `farhelm agent template create|edit`)
  is rejected locally by clap as an unexpected flag, whereas `--effort=-fast` succeeds.
- `SPEC_impl.md:7420` still documents that `--agent`, `--effort`, and `--permissions` omit `allow_hyphen_values` because
  "each takes a word from a closed set, and an unknown one is refused listing them all". Since commit
  `0ae1f6de79be7b712ec33dfd71b3a38edf3b1399`, `--effort` accepts free-text `[a-z0-9_-]` values in
  `crates/farhelm/src/main.rs:949-957` (`parse_effort`), malformed spellings only report
  `use 1 to 256 lowercase letters, digits, hyphens, or underscores` (omitting built-in choices and `default`), and
  well-formed typos such as `--agent codex --effort huge` reach the helm and fail in
  `crates/farhelm-proto/src/launcher.rs:1021-1033` with `effort: huge is not offered by codex for this model` without
  listing valid choices.
- How to verify: inspect `crates/farhelm/src/main.rs:949-957, 1038-1044`,
  `crates/farhelm-proto/src/custom_agents.rs:72-81`, and `SPEC_impl.md:7420`, or parse
  `["farhelm", "agent", "create", "--agent", "acme-code", "--effort", "-fast"]` with `Cli::try_parse_from` vs
  `"--effort=-fast"`.
- Fix: Decide whether leading hyphens are intentionally allowed in custom `--model`/`--effort` values. If leading
  hyphens should be forbidden (matching `--resume <id>` and `command` so custom-agent wrappers using standard flag
  parsers cannot misparse a value as a flag), reject leading `-` in `validate_model_or_effort` and update `SPEC.md` and
  `custom-agent-contract.md` (and its Contract changelog). If leading hyphens are intended to be allowed, add
  `allow_hyphen_values = true` to `LaunchFlags::effort` in `crates/farhelm/src/main.rs:1042` and warn wrapper authors in
  `custom-agent-contract.md`. In either case, update `SPEC_impl.md:7420` (and consider listing built-in effort choices /
  `default` in `parse_effort` or the helm's built-in effort refusal).

Reviewer metadata (g43_custom_agents_cli_hooks/f1): suggested bucket: other (CLI flag-parsing inconsistency and
spec/diagnostic drift; custom agents are already YOLO and gated by the YOLO launch rules, so no security or data-loss
boundary is crossed).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g43_custom_agents_cli_hooks/f1`.

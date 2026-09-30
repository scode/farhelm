# An `env NAME=value` prefix hides a YOLO launch from the sensitive-host check

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

A command or profile that starts with `env SOMEVAR=value` before the agent (say
`env CLAUDE_CONFIG_DIR=… claude --dangerously-skip-permissions`) launches an agent with its approval prompts off on a
host marked sensitive, without the confirmation Farhelm is supposed to require, and the sidebar shows no YOLO badge for
it.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F1 / COR-YOLO-ENV`, tagged **definite**. Anchor and title: `crates/farhelm-proto/src/yolo.rs:141` — a command
wrapped in `env NAME=value` skips the sensitive-host YOLO confirmation.

SPEC.md requires that launching an agent with its vendor's approval-skipping flag ("YOLO", e.g. Claude's
`--dangerously-skip-permissions` or Codex's `--yolo`) on a host the user has marked sensitive be explicitly confirmed,
and the helm enforces this. For a raw command line or a profile's invocation, the helm splits the text into words and
asks a shared classifier (`argv_is_yolo` in `crates/farhelm-proto/src/yolo.rs:175-210`) whether it is a YOLO launch.
That classifier decides which vendor's rules apply purely from the first word's file name (`program_basename`,
`yolo.rs:141-149`). So `env CLAUDE_CONFIG_DIR=/x claude --dangerously-skip-permissions` or `env FOO=1 codex --yolo` has
`env` as its program, matches no vendor, and is classified "not YOLO". The guard in
`crates/farhelm-helm/src/yolo_guard.rs:56-58` therefore lets it through without asking, and the supervisor runs it
directly (no shell; `env` is an ordinary program that sets the variables and execs the agent), so the agent starts with
its approval prompts turned off. The sidebar's YOLO badge uses the same first-word lookup (`invocation_marker`,
`yolo.rs:152-161`, shown by `crates/farhelm-ui/src/list/row.rs:579`), so it does not show the badge either. The same gap
applies to `farhelm agent create` from inside an agent without `--allow-yolo-on-sensitive-host`.

This is not an exotic shape. Farhelm itself treats a leading `env NAME=value …` as a supported way to launch an agent:
the supervisor's `effective_program_index` (`crates/farhelm-supervisor/src/agent_kind/mod.rs:783-799`) deliberately
looks past such a prefix to find the real program when injecting hooks and building resume commands, and Farhelm's own
Goose and Grok launches build exactly that shape via `with_launch_environment` (`mod.rs:660`). A raw `env X=1 claude …`
typed into New is treated as a generic agent, but a profile that declares its kind (say Claude or Pi) and uses an env
prefix is integrated as that agent while still escaping the YOLO check. Env prefixes are an ordinary way to set an API
key or config directory, so a user can bypass the one control between a permission-skipping agent and a sensitive host
by accident; nothing in the spec or tests exempts this case. Two reviewers found this independently. It is filed under
correctness, but it is a bypass of a safety control.

The fix is to have the shared classifier skip a leading simple `env NAME=value …` prefix, using the same rule as
`effective_program_index` (move that rule into the shared protocol crate so both sides use one copy, or keep two copies
with a test that they agree), before looking up vendor rules, in both the guard's classifier and the badge's. An `env`
followed by an option (`env -i …`), which the supervisor's rule deliberately does not interpret, should be treated as
YOLO or unknown rather than "not YOLO", since this is a guard. Add tests for
`env A=1 claude --dangerously-skip-permissions`, `/usr/bin/env A=1 codex --yolo`, and `env A=b pi` (Pi is always YOLO).

## Additional detail merged from a second review (de774a1ee8815ce833da77deac593a55d82f7be3)

A separate whole-codebase review found the same gap independently; its item was folded into this one and
`yolo-guard-misses-codex-option-form.md` when both landed. It adds:

- Reach: every create path that is not a structured launch goes through the same classifier: REST raw `invocation`,
  user-edited catalog profiles (`profiles.rs` only runs `validate_profile_fields`), `mode_from_source` for clone/replace
  of a raw or profile-backed source, agent `create --invocation`/`--profile`, and `ResolveProfile` for `farhelm spawn`
  (`crates/farhelm-helm/src/agent_requests.rs:353`). Structured composer launches are unaffected.
- The proto docs themselves name `env claude` as a motivating shape (`crates/farhelm-proto/src/lib.rs` ~1536, ~2453), so
  this is an expected command form, not an exotic one.
- Arbitrary wrappers (scripts, `sh -c '…'`) stay beyond any argv classifier. One sentence in SPEC_impl.md could record
  that limit when this is fixed. Shell syntax such as `true; claude …` or `$(…)` cannot produce a YOLO launch, because
  the launch shim execs argv directly with no shell (`crates/farhelm-supervisor/src/launch.rs` `agent_command`).
- Related: `yolo-guard-misses-equivalent-spellings.md` (Cursor's short `-f` and an explicit Pi agent kind) shares this
  classifier and fix site.

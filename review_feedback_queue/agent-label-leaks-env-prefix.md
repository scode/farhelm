# The fleet agent label leaks a leading NAME=secret

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A key typed as `KEY=… claude` appears in every agent's session listing on every host until the failed session is
deleted; composer-built Goose sessions show as `env`.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F1 / COR-AGENT-LABEL`, tagged **definite**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:1579-1603` — The fleet-wide `agent` label takes the first shell word, leaking a
leading `NAME=secret` and mislabelling `env …` launches as `env`

`farhelm agent sessions` lists every session in the fleet to any process holding any session's credential. Each row has
an `agent` column that is meant to be safe to show fleet-wide. The `AgentSession::agent` doc
(`farhelm-proto/src/lib.rs:2155-2169`) says it is either the profile's name or, for a raw-invocation session, "the
basename of the invocation's program … and nothing else", because users put credentials on command lines. The helm
computes it in `agent_label`: if the session came from a profile it uses the profile name. Otherwise it shell-splits the
invocation, takes the **first word**, and strips everything up to the last `/`. The first word is not always the
program, and two real shapes break that assumption.

- **A leading environment assignment leaks the secret.** A user who types a raw invocation like
  `ANTHROPIC_API_KEY=sk-… claude` gets the label `ANTHROPIC_API_KEY=sk-…`: the first word has no `/`, so all of it
  survives. The supervisor accepts this invocation at create time. `ensure_executable_argv`
  (`farhelm-supervisor/src/agent_kind/mod.rs:2833`) only refuses an empty argv, an empty first element, or a NUL byte.
  Launch then fails, because the launch shim `exec`s the argv directly with no shell (`launch.rs::window_command`), so
  `ANTHROPIC_API_KEY=sk-…` is looked up as a program. The session row stays anyway, in error status, until someone
  deletes it. For that whole time the key sits in the `agent` column of every agent's `sessions` table and `--json`
  output on every host. Nobody had to ask for it.
- **`env` launches all read as `env`.** The helm's own launch composer builds Goose sessions that have an effort or
  permission setting as `env GOOSE_THINKING_EFFORT=… GOOSE_MODE=… goose session …`
  (`farhelm-helm/src/launches.rs:333-349`). The supervisor explicitly supports a raw `env NAME=value program` prefix
  (`effective_program_index`, `farhelm-supervisor/src/agent_kind/mod.rs:567-586`). Neither kind of session carries a
  profile, so both get the label `env`, and the column stops telling sessions apart. Composer-built sessions do record
  the harness choice in `SessionInfo::launch`, but `agent_label` never looks at it.

The first case breaks an explicit redaction contract and SPEC's rule that discovery never exposes credentials. The
second defeats the column's purpose. The suggested fix has three parts:

- When the session has a structured launch record (`info.launch`), use its harness name.
- Otherwise skip a leading `env` and every leading `NAME=value` word, the same way `effective_program_index` does,
  before taking the basename. Fall back to a fixed placeholder if nothing is left.
- Optionally, refuse a raw create whose first word contains `=`, with a hint to write `env NAME=value program`.

Extend the existing test `a_sessions_agent_is_its_profile_name_or_its_program_basename` to cover both shapes.

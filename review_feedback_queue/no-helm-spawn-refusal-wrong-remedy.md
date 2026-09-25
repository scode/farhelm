# The no-helm spawn refusal advises a remedy that fails

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent in an unattached session following the advice gets a second error.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F9 / COR-NOHELM-ADVICE`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:525-533` — The "no helm attached" refusal for a named spawn advises a remedy
that fails

`farhelm spawn --agent NAME` and `--profile-id ID` need the helm to resolve the profile. When no helm is attached to the
session, `resolve_restricted_profile` refuses with: "an attached helm is needed to resolve a profile name; omit --agent
to reuse the asking session's agent" (`handlers.rs:525-533`). That advice no longer works:

- The CLI requires exactly one of `--agent`, `--profile-id`, or `--inherit-agent` (clap `required_unless_present_any` at
  `farhelm/src/main.rs:78-85`), so omitting `--agent` is a usage error.
- The supervisor also refuses a restricted create with zero selectors (`handlers.rs:3205`).

The remedy SPEC_impl.md (around lines 1112-1115) names for this refusal is `--inherit-agent`, which copies the asking
session's stored launch bundle and works with no helm. The message also says "profile name" even when the user passed
`--profile-id`. A unit test pins the wrong wording (`handlers.rs:4549` asserts the message contains "omit --agent").

SPEC requires every failure to carry an actionable remedy, and this is the one refusal SPEC defines for this path. An
agent that follows it just gets a second error. The fix is to name `--inherit-agent`, word the message to cover both
`--agent` and `--profile-id`, and update the test.

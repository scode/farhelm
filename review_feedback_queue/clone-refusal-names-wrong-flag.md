# Clone refusals name --session instead of --source-session

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

`farhelm agent clone --source-session ""` tells the agent to fix a `--session` flag clone does not have.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F12 / COR-CLONE-FLAG`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:3777-3803`, `farhelm-supervisor/src/service/handlers.rs:3886`,
`farhelm/src/main.rs:261-264` — Clone refusals for a bad source id name the wrong flag and give the wrong remedy

In `validate_agent_verb`, the Clone arm checks the source session id with `validate_target` (`handlers.rs:3886`). That
helper was written for the lifecycle verbs (rename, stop, restart), whose target flag is `--session`. Its messages are
all worded for that flag (`handlers.rs:3777-3803`):

- "--session must not be empty; name the asking session explicitly to act on it"
- "an explicit --session target is N bytes, exceeding …"
- "an explicit --session target must not contain control characters"

`farhelm agent clone` has no `--session` flag. Its flag is `--source-session` (`main.rs:261-264`), and the source can be
any session, so "name the asking session" is the wrong remedy as well. The helm's own check for the same field does name
`--source-session` correctly, so the two hops contradict each other depending on which one refuses. For example,
`farhelm agent clone --source-session "" --host H` tells the agent to fix a `--session` flag that clone does not have.
SPEC requires concrete, actionable errors, and agents script against this CLI.

The fix is to pass the flag name into `validate_target`, or to add a clone-specific wrapper, and to drop the "name the
asking session" remedy for clone. Add a test for `Clone { source_session_id: Some("") }`.

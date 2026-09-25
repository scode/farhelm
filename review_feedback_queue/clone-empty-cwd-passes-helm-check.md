# The helm's clone validation accepts an empty cwd

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Same as F10.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F11 / COR-CLONE-CWD-HELM`, tagged **definite**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:647-654`, `farhelm-helm/src/agent_requests.rs:1237` — The helm's own clone
validation also accepts an empty cwd

This is the helm-side twin of F10. The helm runs its own check on every agent request, `validate_authoritative_verb`,
because it is supposed to treat supervisors as untrusted and not rely on their checks. For Clone
(`agent_requests.rs:647-654`) that check requires only a non-empty source session and host. It says nothing about `cwd`.
So whenever the supervisor's check is missing or bypassed (a compromised or older supervisor, or F10 as it stands
today), `cwd: Some("")` reaches `request.cwd.unwrap_or(source.cwd)` at line 1237. The empty string then replaces the
source's directory, with the same confusing far-end refusal and burned idempotency key described in F10.

It is a separate fix at a separate layer. Refuse an empty clone cwd here too, mirroring the Create arm's
`required(Some(cwd), "--cwd")`. Alternatively, normalize `Some("")` to `None` and document that choice.

# agent clone --cwd "" passes the supervisor relay check

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A confusing error and possibly a burned idempotency key.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F10 / COR-CLONE-CWD-SUP`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:3879-3894`, `farhelm-supervisor/src/service/handlers.rs:3933-3937` —
`agent clone --cwd ""` passes the supervisor's relay check

`farhelm agent clone --source-session S --host H [--cwd DIR]` copies session S onto host H. With no `--cwd`, the copy
uses S's directory. Before relaying, the supervisor shape-checks the request in `validate_agent_verb`. For **Create**,
it refuses an empty cwd. For **Clone** (`handlers.rs:3879-3894`), it passes `cwd.as_deref().unwrap_or_default()` into
the shared size check and never looks at emptiness, so `Some("")`, which is what `--cwd ""` produces, goes through. The
comment on `validate_create_fields` (`handlers.rs:3933-3937`) says a `""` cwd there "is the wire's `None`". That is true
only after this function has collapsed `None` into `""`. An explicit `Some("")` looks the same at that point, but it is
not `None` on the wire, and the helm treats it differently.

At the helm, `clone_for_agent` does `request.cwd.unwrap_or(source.cwd)` (`farhelm-helm/src/agent_requests.rs:1237`).
`Some("")` therefore **replaces** the source's directory with an empty string instead of falling back to it. The target
supervisor then refuses with "working directory is not absolute: " followed by nothing. That is a confusing far-end
error for input the first hop should have refused. If the caller passed an idempotency key, the refusal is recorded
against it (`create_session_admitted` routes validation refusals through `record_refused_create`), so that key is
burned: a corrected retry under it is refused as key reuse.

The fix is to refuse an empty clone `--cwd` in the Clone arm, as the Create arm does, and correct the comment.

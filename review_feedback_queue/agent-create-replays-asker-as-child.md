# A keyed agent create can report the asker as the created session

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent can be told its own session is one it just created, and a routine clean-up can then stop or restart itself.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F5 / COR-CREATE-SELF-REPLAY`, tagged **possible**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:1085-1104`, `farhelm-helm/src/agent_requests.rs:1252-1309`,
`farhelm-supervisor/src/service/core.rs:1382`, `farhelm/src/main.rs:813-842` — A keyed `farhelm agent create` can report
the asking session itself as the newly created session

`farhelm agent create` accepts an optional idempotency key. If a retry arrives with the same key and the same request,
the target supervisor returns the session the first attempt made instead of creating another. Two properties combine
here:

- An agent create reaches its target over the helm's connection, so the target stores the key with the permanent scope
  any helm-made create gets (SPEC_impl says so explicitly).
- The replay match is a fingerprint of the request (`create_fingerprint`, `core.rs:1382`): working directory, resolved
  launch bundle, title, and parent. Agent creates never carry a parent. Nothing in the fingerprint records **who
  asked**.

The failure goes like this. Session P runs `farhelm agent create --host H --cwd D --profile X --idempotency-key K` and
gets child C on host H. Later C's own agent runs the identical command. That is plausible when a parent hands its
command or script to the child it spawned. The target finds K, the fingerprint matches, and it replays C. The helm
returns C as `Created`, with `current: true` because C is the asker. The CLI prints C's own id on stdout as "the new
session" and ignores `current` (`main.rs:813-842`).

Agents script against that printed id. A later "clean up the helper I created" then stops or restarts the agent itself.
The CLI warns before a self-restart only when the `--session` value equals its own `FARHELM_SESSION_ID`, and an agent
that believes the id belongs to a child will not expect that.

The clone path already guards against exactly this. `clone_for_agent` passes an `accept_result` check,
`reject_clone_replay` (`agent_requests.rs:1252-1309`), that refuses a replayed result equal to the asker or the source.
It returns `Conflict` and tells the caller to use a fresh key. `create_for_agent` passes `accept_result: None`
(`agent_requests.rs:1085-1104`), and its comment claims a keyed replay "is the caller's own earlier create coming back".
That claim is wrong when the caller is the child.

The open premise is how often a child actually re-issues its parent's exact keyed create. The suggested fix:

- Give `create_for_agent` an `accept_result` that refuses `created.id == asking_session` with the clone guard's
  `Conflict` wording. It runs inside `do_create_session`'s acceptance step, so a refused replay never lands in the
  helm's cache.
- Correct the comment.
- Optionally, make the CLI refuse or warn when a `Created` reply's row has `current: true`.

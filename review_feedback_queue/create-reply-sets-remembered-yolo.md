# A remote create reply sets the remembered permission and trust defaults

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Creating one session on a compromised remote machine can quietly switch your remembered launch defaults to "yolo" with
workspace trust, so the next new session on any machine starts with approvals off unless you notice.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0535-2b597e9-a2a7` (audit of Area 7, helm state) as
`F24 / SEC-CREATE-REPLY-SETS-DEFAULTS`, tagged **definite**. Anchors and title: `sessions.rs:1815`, `sessions.rs:1835`,
`store.rs:5036`, `store.rs:5086-5099` — A remote supervisor's create reply sets the helm-wide remembered permission and
workspace-trust defaults

The New-session dialog preselects a **remembered permissions level** (such as "yolo", meaning no approvals) and a
**remembered workspace-trust** choice. These are helm-wide singletons in the `preferences` table, applied to every host.
SPEC_impl (line 106) calls them "facts of accepted launches rather than claims from one client".

After any user-initiated create, `accept_created_session` passes the _supervisor's reply_ (`SessionInfo`) to
`record_create_history_with_destination` with `remember_launch_choices = true` (`sessions.rs:1815`, flag at `:1835`).
This covers structured, raw, profile and plain-Replace creates. In the store (`store.rs:5036` onward), that function
reads `entry.launch` from the reply, a field the remote peer controls. It writes `launch_history`, and it overwrites
`preferences.remembered_permissions` and `remembered_workspace_trust` from it (`store.rs:5086-5099`).

Nothing compares the reply's `launch` with the selection the helm actually sent. `created_session` in `client.rs` checks
only the id. A raw or profile create sends no launch selection at all, yet a reply that includes one is still recorded.
By contrast, the repository history and the remembered profile are taken from the helm's own request, not the reply.

SPEC.md ("Remote input, session defaults, and availability") says remote supervisor metadata "must not override an
explicit user choice or indefinitely determine that default for other hosts". It also says the agent create exception
"does not authorize this influence over user-driven session creation". A hostile host can answer any create with
`permissions: Yolo` plus workspace trust. Every client's next New dialog, and "reset choices", will then preselect that
for every host, including the helm's own machine.

Suggested fix: carry the helm's own accepted selection (the compiled structured selection, or `None`) in
`CreateAcceptance`, as is already done for `github_repo`. Derive the remembered defaults and `launch_history` from that,
never from `entry.launch`. Optionally refuse, or log, a reply whose `launch` differs from what was requested.

User-visible consequence: creating one session on a compromised remote machine can quietly switch your remembered launch
defaults to "yolo" with workspace trust, so the next new session on any machine starts with approvals off unless you
notice.

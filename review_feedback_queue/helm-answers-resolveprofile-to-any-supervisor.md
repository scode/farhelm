# The helm answers ResolveProfile with raw command lines to any supervisor

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A compromised remote machine can quietly read every saved profile's full command line, including any API keys in it,
without a new session appearing.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F2 / COR-RESOLVE-PROFILE`, tagged **possible**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:322-336`, `farhelm-helm/src/agent_requests.rs:655-664`,
`farhelm-helm/src/agent_requests.rs:74-86`, `farhelm-helm/src/client.rs:1845-1851`,
`farhelm-supervisor/src/service/handlers.rs:3895-3897` — The helm answers `ResolveProfile` from any supervisor
connection with raw profile command lines; only the asking host's own supervisor refuses it

`ResolveProfile` is an internal upcall. A supervisor sends it when an agent runs `farhelm spawn --agent NAME` (or
`--profile-id ID`), because only the helm has the profile catalog. The helm answers with the profile's full launch
bundle: its `invocation` (the complete command line), `agent_kind`, `resume_template`, and profile id and name
(`agent_requests.rs:322-336`). This is the one agent verb whose answer holds exactly what SPEC says agents must never
see.

The only thing that keeps a session from asking for that bundle directly is its own supervisor. `validate_agent_verb`
refuses `ResolveProfile` from a session-credential peer (`handlers.rs:3895-3897`). The helm does not re-check:

- The helm-side client dispatches every incoming `AgentRequest` to the handler without looking at which verb it is
  (`client.rs:1845-1851`).
- The helm's own validation, `validate_authoritative_verb`, accepts `ResolveProfile` whenever exactly one of name or id
  is present (`agent_requests.rs:655-664`).
- The module docs spell out the trust model: "the trust boundary is the connection, not the message". A request that
  arrives on a supervisor connection is assumed to have passed that supervisor's checks (`agent_requests.rs:74-86`).

SPEC says otherwise ("Local authority and trust between hosts"): the helm must treat remote supervisor messages as
untrusted, and redaction promises hold even against senders that have local account authority. Suppose a remote host's
account is compromised. The attacker can replace what answers on the far end of the helm's ssh connection. They list
profile IDs with the permitted `Profiles` verb, then send `ResolveProfile` for each ID and read every saved profile's
full command line and resume template, including any API keys in them. No session is created, and the helm's
`ResolveProfile` arm writes no log line. By contrast, `create_for_agent` and `clone_for_agent` each write an `info!`
audit line.

The reviewers kept this as **possible** because of one open premise. The asking host already receives a resolved
invocation by design: a legitimate `farhelm spawn --agent X`, or the accepted `farhelm agent create --profile X` aimed
at its own host, puts the profile's command line into the child's argv, which the same account can read. The maintainer
may treat that as the same exposure. The relay path still adds three things the existing paths lack: it leaves no trace
(create is logged), it reveals resume templates, and it stays open after the planned guardrails on agent create/clone
land.

A decision is needed. There are two options:

- **Accept it.** Record in SPEC or SPEC_impl that attached supervisors may receive resolved profile bundles, so profiles
  are no place for secrets. Correct the trust-boundary docs in `agent_requests.rs` and `client.rs`. Log every helm-side
  `ResolveProfile` with origin host, asking session, and profile id.
- **Close it.** Stop sending raw bundles upward on request. The helm could perform the named-profile create itself or
  return an opaque single-use handle, and `validate_authoritative_verb` would refuse `ResolveProfile`.

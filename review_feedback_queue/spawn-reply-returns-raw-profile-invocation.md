# A spawn by profile name returns the profile's raw command line

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent can read credentials embedded in any profile's command line by spawning it.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F3 / COR-SPAWN-REPLY-RAW`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:503-520`, `farhelm-supervisor/src/service/handlers.rs:825` — A spawn by
profile name returns the profile's raw command line to the asking session

`farhelm spawn --agent NAME` (or `--profile-id ID`) creates a session on the asker's own host from a helm profile. The
supervisor resolves the name through the `ResolveProfile` upcall (`resolve_restricted_profile`, `handlers.rs:503-520`),
which yields the profile's full invocation and resume template. It creates the session and replies with
`ControlMsg::SessionCreated { session }` (`handlers.rs:825`). That `session` is the full internal `SessionInfo` record,
and its fields include `invocation` and `resume_template`. So any holder of a session credential can read any profile's
command line, including embedded credentials, by spawning that profile and decoding the reply. The shipped CLI prints
only the new id, but the bytes are on the socket. The helm-routed equivalent, `farhelm agent create`, replies with the
redacted `AgentSession` row, which has no command line.

SPEC lets agents see profile names and IDs only, and keeps redaction promises even against processes that already have
local account authority. The open premise is whether that promise covers the reply about a child the agent just launched
itself: the child's argv is already readable by the same Unix account. The suggested change is either to reply to a
session-authenticated spawn with a redacted row or with just the id, or to record the exemption in SPEC. This overlaps
with F2's open premise, and the maintainer will likely want to decide the two together.

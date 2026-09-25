# Clone hands the asker a raw source's full command line

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A credential typed into one session's command line can be copied to any other host by any agent.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F19 / SEC-CLONE-DISCLOSURE`, tagged **possible**. Anchors and title:
`farhelm-helm/src/agent_requests.rs:1176-1204`, `farhelm-proto/src/lib.rs:2155-2169` — Cloning a raw-invocation session
onto the asker's own host hands over the source's full command line

The session listing deliberately shows another session's agent only as a profile name or program basename (see F1). The
`AgentSession::agent` doc (`farhelm-proto/src/lib.rs:2155-2169`) justifies that by saying a caller who needs the real
command line "has the helm's own APIs, which are not reached by a session credential".

`farhelm agent clone` reaches it anyway. For a source session created from a raw invocation, `clone_for_agent` reads the
source's full `invocation` from its host's live session list (`agent_requests.rs:1176-1204`) and creates the copy with
that exact command line. SPEC requires this: a raw session "clones as that invocation". An agent on host A can therefore
run `farhelm agent clone --source-session <session on host B> --host <A>`. The copy starts on A, and its command line is
readable there by any process in the same Unix account (for example `/proc/<pid>/cmdline`, or the supervisor's own
state). A credential someone typed into one session's raw command line on host B can thus be copied to any other host by
any agent, while the listing gives the impression it is hidden. Before cloning, an agent on a different host had no
Farhelm path to that argv.

The open premise is whether the accepted temporary exception for agent create/clone ("arbitrary execution on the target
host") also covers disclosing the source's command-line secrets. That exception is about execution, and SPEC separately
says redaction promises still hold. Both resolutions need a SPEC-level decision:

- **Accept it.** Record in SPEC that clone discloses a raw source's invocation to wherever it is cloned, and correct the
  `AgentSession::agent` doc comment.
- **Restrict it.** Add a policy for cross-host raw clones. That needs a spec change, because SPEC currently promises a
  raw source clones as its invocation.

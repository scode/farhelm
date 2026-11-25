# Helm agent-create dispatch logs the asking session id unescaped

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

The helm's "an agent is creating a session" audit log line writes the asking session id exactly as the host's supervisor
reported it, while every sibling log site escapes it. A supervisor that reports a session id containing newlines or
invisible/direction-changing characters can forge extra helm log lines or disguise which session asked — the session-ID
logging rule exists to prevent exactly this. Diagnostic integrity only; no execution or data consequence.

## Details

**other.** Source inspection by the commissioner, following a boundary lead from the area-2 chunk-A reviewer (the site
sits just past that chunk's line boundary; chunk B did not report it). Same class as the queued supervisor-side
`trailing-pending-scope-cleanups-logs-invalid-session-id-unescaped.md`, different scope (helm agent-create dispatch).

`crates/farhelm-helm/src/agent_requests.rs:3088-3093` (`dispatch_agent_create`):

```rust
info!(
    asking = asking_session,
    host = host_name,
    verb = "create",
    "an agent is creating a session"
);
```

`asking_session` is the supervisor-reported session id, taken without re-verification at the ingress
(`crates/farhelm-helm/src/client.rs:2442-2448`, `ControlMsg::AgentRequest` — the trust comment scopes that acceptance to
"which of this host's own sessions is asking", not to log presentation). The adjacent comment defends only `host_name`
("the REGISTRY's own rendering … nothing attacker-chosen reaches this line") — it does not cover `asking_session`. Every
other tracing site in the file escapes it: lines 1469, 2007, 2436, 3397, 3469, 3620, 3706, 4678 all use `escape_for_log`
(defined at 1520, neutralizing `farhelm_proto::text::is_presentation_unsafe`). SPEC_impl.md "Session IDs in logs" (lines
152-161): session IDs must not inject log lines or use invisible/direction-changing characters to disguise the ID shown
in logs; call-site escaping or a safe formatter satisfies it — this codebase's convention is the helper, and this one
site skips it.

Verify: inspect line 3089 against the escaped siblings; or relay an AgentRequest whose session_id contains `\n` plus a
forged line and observe the helm log. Fix: `asking = escape_for_log(asking_session).as_str()`, matching the siblings.

Reviewer metadata: suggested bucket: other (diagnostic integrity; not a security-boundary or data-loss consequence);
confidence: confirmed; proposed_drop: none; possible_cover: none — the queued supervisor-side unescaped-session-id item
covers startup scope cleanups on the supervisor, not this helm site. Coverage searched (queue, TODO.md, BUGS.md,
TRIAGE_OUTCOMES.md, plans/queue) — no match.

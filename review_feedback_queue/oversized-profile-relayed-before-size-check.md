# Oversized spawn profile selector relayed before any size check

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A program running inside an agent session can push a multi-megabyte profile name onto the shared connection to the helm
and force a wasted round trip, for a request the supervisor's own size rules would refuse — the size check runs after
the send instead of before it.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (security-general p1).
Coordinator confirmed the ordering and the doorway-policy contrast; the restater corrected the cap's shape (combined
ceiling, not per-field) and the coordinator verified the correction against the code.

When a restricted (session-authenticated) create with `source_profile` arrives, the dispatcher resolves the profile
first (`resolve_restricted_profile`, `crates/farhelm-supervisor/src/service/handlers.rs:3054-3071`), serializing the
caller-supplied `profile_name`/`profile_id` into an `AgentRequest::ResolveProfile` verb relayed over the shared helm
connection (`relay_agent_request`, agent_relay.rs:289-298) — before the lifecycle claim (3093), the credential re-check
(3094-3128), and the create-path size cap that only runs later inside `handle_create_session` (3163). The cap is one
combined 64 KiB ceiling over the sum of parent + cwd + profile name + profile id + title (`CREATE_FIELD_CAP`,
handlers.rs:103, enforced at 520-541). This inverts the module's deliberate doorway policy: `validate_agent_verb`
enforces field-shape rules early precisely because the two supervisor↔helm queues are byte-unbounded ("the two
byte-unbounded queues this validation exists to protect", 5762-5768) — but the spawn path builds its verb internally and
never passes through it. Blast radius is bounded (reviewer-measured): the 64-slot helm queue admits the frame while the
refusal is awaited, and the ~8 MB incoming-frame cap bounds any single name — so multi-megabyte shared-memory pressure,
head-of-line blocking, and one helm round trip, not a kill.

Suggested fix: apply the same combined field-length cap to the selector in the restricted dispatcher before
`resolve_restricted_profile`, refusing `InvalidRequest` locally so an oversized selector never reaches the shared
connection. Follow-up (not a finding): a byte cap on the helm's count-only 64-slot queue would bound the shared queue
the way the doorway bounds the connection's.

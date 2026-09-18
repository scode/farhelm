# Clone audit log skips escaping for agent-chosen ids

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

The server's audit log can show a misleading session id for clone operations: one id can render as if it were another,
so an operator reading the log cannot trust which session a clone actually came from.

## Details

Source: pre-pr-review-swarm, area helm-agent-surface, 2026-09-17. Confidence: definite, low severity. Merge of two
lenses on the same lines (data-flow p1, sec-input-trust p1) with complementary analysis. Coordinator verified every
line, including that bidi/Cf/Zl/Zp pass the relay's control-character check.

The lifecycle verbs log through `resolve_target`, which passes `asking` and `target` through `escape_for_log` (Cc +
bidi/zero-width/invisible → visible `\u{...}}`, `crates/farhelm-helm/src/agent_requests.rs:722-754`, applied at
:688-696). The clone audit line logs `asking = asking_session` and `source = request.source_session_id` raw
(agent_requests.rs:1217-1223), and the create line logs `asking = asking_session` raw (:1066-1071). The comment above
the create line (:1060-1065, referenced again by the clone line at :1216) claims "nothing attacker-chosen reaches this
line" — false for the clone line: `source_session_id` is a verb field any agent holding any one session credential
chooses freely. The relay's `validate_agent_verb` refuses only `Cc` control characters (`char::is_control`,
handlers.rs:3495-3499; clone source checked at :3584 — bidi/Cf/Zl/Zp are NOT control and pass) plus length caps, so bidi
overrides/isolates and zero-width/invisible formatting reach the log: an id that renders as another id, or two different
ids rendering identically, in the helm operator's audit log. `asking` is the supervisor-authenticated session id
(handlers.rs:3358), so its raw logging is a defense-in-depth inconsistency — but note `resolve_target`'s docs (:685-687)
explicitly say escaping is "the only one at all for `asking`", directly contradicting the create comment's "the
supervisor validated `asking_session`" claim; the two comments cannot both be right. Routing itself uses exact string
equality — log readability/audit integrity only, hence low severity.

Suggested fix: pass `asking_session` and `request.source_session_id` through `escape_for_log` in both `info!` lines,
matching `resolve_target`; correct the comments to state what is actually trusted (registry-rendered `host_name`) vs.
escaped (supervisor/agent-supplied ids).

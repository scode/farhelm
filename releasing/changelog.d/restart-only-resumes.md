---
kind: breaking
---

Restart now always picks up the session's own conversation where it left off. When Farhelm cannot do that, Restart and
Restart with are greyed out, and hovering over them says why: no conversation was captured for the session, or Farhelm
cannot resume conversations for that agent at all. Use Replace to start such a session over. Before, Restart in that
case started a fresh conversation or ran the profile's fallback resume command, so the conversation was lost.

Sessions running Cursor, Muse, or OpenCode, and any session started from a typed command that Farhelm does not know as
an agent, can no longer be restarted; Replace starts them over. An interrupted session that cannot be resumed offers
only Replace.

For agents using the `farhelm agent` commands: `farhelm agent restart` no longer takes `--mode` and refuses it with a
message saying to drop the flag. In `farhelm agent sessions`, the offer is `resume` when a restart can work and
otherwise says why it cannot (`not-captured` or `no-reporting`, `not_captured` or `no_conversation_reporting` in
`--json`, whose schema version is now 3).

Downgrading to an earlier release after this one is not supported.

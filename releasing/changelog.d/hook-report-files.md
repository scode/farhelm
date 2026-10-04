---
kind: fixed
---

When an agent tells Farhelm which conversation it is in (at startup, or after `/clear` or `/new`), that report is no
longer lost when the host's Farhelm supervisor is not running. On the Mac that is whenever the Farhelm app is closed:
sessions keep running, and a conversation an agent switches to while the app is closed is now remembered when you reopen
the app, so Restart resumes the conversation the agent was actually in. Agents also no longer pause for a few seconds at
startup or between turns when the supervisor is not running.

While the supervisor is not running, an agent's `farhelm spawn` and `farhelm agent` commands still fail until it is
running again.

The per-session hook log now records two kinds of line: the hook's own (`written` when it saved a report) and the
supervisor's verdict on each report (`acked`, or `refused` with the reason). Reports are applied within about two seconds
rather than instantly.

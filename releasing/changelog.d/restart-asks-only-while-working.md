---
kind: changed
---

Restart (and Restart with) now asks for confirmation only when the agent is working. An agent that is idle, waiting for
your input, or whose status is unknown is stopped and restarted on the first click. Before, Restart asked whenever the
agent was running at all, which made the question easy to click through without reading. Agents whose activity Farhelm
can only guess from screen changes (every agent except Claude Code and Codex, and custom commands) may be restarted
without asking while they are busy but their screen is still. Replace still always asks. `farhelm agent restart` follows the same rule: it needs
`--stop-if-running` only for an agent that is working. This release changes the helm–supervisor protocol, so update the
supervisors on your hosts along with the helm.

---
kind: fixed
---

When an agent's request to start a child session is refused (an unknown launch profile, or launch fields over the size limit), the refusal no longer keeps the agent's own session locked until the agent reads the answer. An agent that stopped reading at that moment used to leave its session unable to be stopped, deleted, or archived, and blocked new sessions on that host, until its connection drained.

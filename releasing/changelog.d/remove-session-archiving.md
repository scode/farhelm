---
kind: breaking
pr: 834
---

Session archiving is removed. Archived sessions become visible again in the session list with their retained output and outcome, and nothing is launched on their behalf. Deleting a session still archives its owned GitHub checkout when the last reference to that checkout goes away.

The agent protocol and JSON schema versions advance because their archive vocabulary is gone; an agent built against an older schema that still sends archive verbs is refused, so update the helm and supervisor together as usual.

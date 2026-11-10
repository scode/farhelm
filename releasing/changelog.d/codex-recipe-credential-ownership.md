---
kind: fixed
---
Codex API endpoints selected in a Tensorlake recipe now discard old keys and extra headers from Farhelm's provider
settings, so they do not follow a changed endpoint. Existing hosts get this cleanup on the next credential delivery,
such as an endpoint or token change.

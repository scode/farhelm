---
kind: fixed
---
Claude Code API endpoints and subscription logins selected in a Tensorlake recipe now discard old extra headers,
API keys and credential helpers from its settings. Existing hosts get this cleanup on the next credential delivery,
such as an endpoint, token or login change.

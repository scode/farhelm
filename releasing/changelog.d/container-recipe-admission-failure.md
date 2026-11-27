---
kind: fixed
---

When Rebuild of a recipe on a container host that has never built it stops at the host's Podman check, Settings shows why without leaving an empty build record for that host, and the reason is gone after the next attempt or a restart of Farhelm.

A recipe build that starts just as the host's SSH destination or Tailscale setting is changed now fails instead of recording its work under settings that were never checked. The plan behind this change says it needs no release note; this fragment covers the PR for the full list only.

---
kind: fixed
---

On a host whose supervisor reports no identity, a session created right after the host connected now opens, stops, and renames normally. Before, it answered "no such session" until the host's session list had refreshed successfully, which could take a while if refreshes kept failing.

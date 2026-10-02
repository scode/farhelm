---
kind: fixed
---

Closing or reloading the page while Farhelm was checking a host you were adding could leave a helper process from that
check running on this machine, such as a `ProxyCommand` from your ssh configuration. The check now always finishes and
cleans up after itself. A side effect: if the check finds Farhelm already running on the host, the host is added even
when the page that asked has gone.

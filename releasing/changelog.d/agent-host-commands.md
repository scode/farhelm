---
kind: added
---

Agents can ask to change a host's alias, icon or color with `farhelm agent host edit`, or to remove a host with
`farhelm agent host remove`. Each request shows a card you must answer with Allow every time: the host's "run farhelm
commands from this host without asking" setting never covers it, and there is no Always allow. An agent cannot remove
this machine or the host its own session runs on.

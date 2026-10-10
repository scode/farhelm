---
kind: changed
---

Uninstalling a remote host from the hosts panel now ends its private tmux server as well as removing Farhelm. Removal still refuses while sessions or terminal tabs are live, but a session started on that host while removal runs may be ended too. The host's Farhelm data stays in place; local `farhelm uninstall` is unchanged.

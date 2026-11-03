---
kind: changed
---

Uninstalling Farhelm from a host in the hosts panel now refuses a host whose Farhelm data directory is not an absolute
path, before changing anything; only a host whose `HOME` is a relative path reports one. The local `farhelm uninstall`
preview now also says that sessions and their terminals keep running without Farhelm Desktop or the supervisor, and that
uninstall does not end them, which was already the case.

For the curator: the hosts panel's uninstall also ends the terminal hosts in the host's data directory, between stopping
the supervisor and ending its tmux server, and its step now reads "Stop the supervisor, its terminal hosts and its tmux
server". That part is hidden behind the not-yet-on switch: nothing starts terminal hosts until a later change turns new
terminals on, and that change carries its entry. The step's new wording is visible today.

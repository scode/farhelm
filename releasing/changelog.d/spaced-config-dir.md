---
kind: fixed
---

When the systemd user manager's `XDG_CONFIG_HOME` contained a space or another character systemd prints escaped (common
on systemd 255 and later), the hosts panel refused automatic setup with a false "relative XDG_CONFIG_HOME" reason,
`farhelm helm setup` refused with a wrong message, and `farhelm uninstall` could look for Farhelm's services in the wrong
place and leave them behind. Farhelm now reads that setting from `busctl`, which reports it exactly. On a host without a
user D-Bus session bus, where `busctl` cannot answer, such a value is refused with a message that says why instead.

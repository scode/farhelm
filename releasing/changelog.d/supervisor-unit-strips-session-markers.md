---
kind: fixed
---

The supervisor's systemd unit no longer lets it inherit a session's identity from the user manager. If a terminal tab's shell startup files ran `systemctl --user import-environment` (or `dbus-update-activation-environment --systemd --all`), the next supervisor restart picked up that tab's session markers, and deleting that session or closing that tab made the supervisor stop itself, and could end every session on the host. `farhelm helm setup` and host provisioning write the corrected unit; hosts pick it up the next time their unit is rewritten. A supervisor you start by hand is not covered.

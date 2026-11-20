---
kind: added
---

Farhelm can ask for SSH verification codes, passwords and key passphrases in the desktop app when connecting remote
sessions or setting up a host. The prompt appears inside Add host while checking a new destination. Other questions
wait in the sidebar while you work, and the chime has a switch under Notifications.

An amber highlight draws your eye to a new sign-in, setup failure, or identity problem in the sidebar. The row keeps
an amber edge while it needs attention. With reduced motion, the highlight flashes in place.

Adding a host can require two sign-ins, one to check and set up the host and another for sessions. If the setup
connection expires while the confirmation is open, starting setup can ask again.

For curation, not for the entry: the landing also fixed a case where Add host or Update could fail after a good
install when a Retry or Sign in on the same host arrived at the same moment as setup's last step.

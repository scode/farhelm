---
kind: fixed
---

Adding or updating a remote host from the hosts panel no longer replaces a supervisor service that `farhelm helm setup` installed on that host. The panel now refuses and says setup manages it there. Before, the replacement dropped setup's ownership mark, so `farhelm helm setup` and `farhelm uninstall` on that machine afterwards refused to manage the service.

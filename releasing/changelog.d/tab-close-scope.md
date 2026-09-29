---
kind: fixed
---

On a host with systemd, closing a terminal tab could leave its background processes running, with no error, if the
supervisor had once failed to reach the systemd user manager (for example briefly at startup). Closing a tab now checks
again before giving up on the tab's own cleanup, and reports a failure when it cannot confirm the tab's processes are
gone.

---
kind: fixed
---

Host and unavailable-session details now show which connection step is pending and distinguish SSH login problems, missing Farhelm, an absent or silent supervisor and unfinished setup after it replies. Failed attempts include the host's last SSH error lines; timeouts against older remotes remain explicit about uncertainty.

Context for curation: when the supervisor is absent, the details suggest the exact command to start it. If the configured
Farhelm program or state folder contains an apostrophe or a backslash, they say to run `farhelm supervisor run` with the
host's configured program and state folder instead of showing a command that could come out altered (#2567, a follow-up
to this unreleased change).

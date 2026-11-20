---
kind: fixed
---

The desktop app uses your login shell's PATH for ssh and other tools, so ssh can find programs your configuration runs
even when you start Farhelm from the Dock or Finder. If Farhelm cannot read that PATH, a notification explains the
fallback and offers **Send feedback**.

Context for curation: the plan's release note says programs such as a `ProxyCommand` in `/usr/local/bin` are now found,
where signing in to such a host failed with "Connection closed" before. That holds, with two details it does not say:
it applies on Linux as well as macOS, and sessions keep starting from the PATH the app was started with, since their own
login shell builds the PATH from there.

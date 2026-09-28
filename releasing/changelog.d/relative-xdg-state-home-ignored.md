---
kind: fixed
---

Farhelm now ignores a relative `XDG_STATE_HOME`, as the XDG spec asks, and uses `~/.local/state/farhelm` instead. Before, `farhelm helm setup` wrote the relative path into the service units, where it resolved against your home directory, while commands such as `farhelm helm token show` resolved it against the directory you ran them from, so they could read a different state directory and print a token the running helm did not accept. If you deliberately set a relative `XDG_STATE_HOME`, pass `--state-dir` instead.

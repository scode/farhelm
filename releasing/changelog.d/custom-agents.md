---
kind: added
---

You can register your own agent in **Settings → Custom agents** and launch it from the desktop app, the CLI, or a
template. Custom agents always run in YOLO mode and can resume a conversation on Restart if they support resumption
and report its ID.

A conversation ID you already know can be supplied under **More options** in the launcher or with `--conversation`
on a compatible CLI launch.

`farhelm custom-agent spec` prints the wrapper contract for your installed version, including its change history.

An agent that runs `farhelm agent create --agent` with a value that is neither a built-in agent type nor a custom agent
registered on that helm is refused with the list of built-in types, as before custom agents.

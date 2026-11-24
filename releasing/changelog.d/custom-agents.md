---
kind: added
---

You can register your own agent in **Settings → Custom agents** and launch it from the desktop app, the CLI, or a
template. Custom agents always run in YOLO mode and can resume a conversation on Restart if they support resumption
and report its ID.

A conversation ID you already know can be supplied under **More options** in the launcher or with `--conversation`
on a compatible CLI launch.

`farhelm agent agents` lists built-in agents and registered custom IDs with their available launch options. Use
`--json` for structured output. The `--json` output of every `farhelm agent` listing now reports schema version 8, so
scripts that check the version need to accept it; the existing fields keep their shape. `farhelm custom-agent spec` prints the wrapper contract for your installed version,
including its change history.

An unknown agent ID in a CLI launch or template write lists the built-in types and registered custom IDs. A refused
built-in effort also reports the choices that apply. A launch through a recipe or recipe template refuses
`--conversation` when it creates a new environment.

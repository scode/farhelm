---
kind: fixed
---

Starting Codex with `-a never -s danger-full-access`, its spelled-out form of `--yolo`, now counts as a YOLO launch in
any spelling of those two options (long or short, with or without `=`, in either order, or as `-c` config settings): a
host that asks before YOLO launches asks for it, and the sidebar shows its badge. Either option alone still does not
count, since `-a never` keeps Codex's sandbox and `-s danger-full-access` keeps its approval prompts.

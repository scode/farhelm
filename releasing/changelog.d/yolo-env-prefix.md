---
kind: fixed
---

A custom command line or profile that starts an agent behind an `env NAME=value` prefix, such as
`env CLAUDE_CONFIG_DIR=… claude --dangerously-skip-permissions`, is now recognized as a YOLO launch: a host that asks
before YOLO launches asks for it, and the sidebar shows its YOLO badge. Before, the prefix hid the agent and the launch
started without asking. Recognizing YOLO in custom command lines remains best effort; a launch wrapped in a script or
`sh -c` is not guaranteed to be caught.

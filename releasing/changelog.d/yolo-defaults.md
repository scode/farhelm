---
kind: changed
---
OpenCode, OMP and Goose now launch in YOLO mode by default, like Pi. Choose an approval mode in OMP or Goose to keep
approval prompts. Farhelm passes the YOLO setting explicitly, overriding a vendor configuration that asks; OpenCode
also stops asking about paths outside the project. On hosts that ask before YOLO launches, these default launches now
require confirmation. A harness's default YOLO no longer preselects YOLO when you switch to another harness.
A YOLO preference remembered before upgrading stays preselected in new dialogs until your next structured launch.

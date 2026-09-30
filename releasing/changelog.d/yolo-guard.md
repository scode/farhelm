---
kind: breaking
pr: 1273, 1288
---

Starting a YOLO session on a host that is not marked safe for YOLO launches now asks first. In the GUI, a create,
clone, replace, or restart with a YOLO permission (every Pi launch counts, as do `claude --dangerously-skip-permissions`
and `codex --yolo`) shows a confirmation naming the host, and nothing starts until you confirm. `farhelm agent create`,
`farhelm agent clone`, and `farhelm spawn` refuse such a launch unless you pass `--allow-yolo-on-sensitive-host`, so a
script that starts YOLO sessions keeps failing until you either add the flag or mark the host safe in its settings.
Every host starts out not marked safe, including this machine. A plain restart of an existing YOLO session does not
ask.

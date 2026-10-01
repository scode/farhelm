---
kind: changed
---

The command-line override for a refused YOLO launch is now `--confirm-yolo` on `farhelm agent create`,
`farhelm agent clone` and `farhelm spawn`; the old `--allow-yolo-on-sensitive-host` still works, so existing scripts keep
working, but it no longer appears in help. The matching checkbox in each host's settings now reads "start YOLO sessions
here without asking" (off by default, so Farhelm asks before each YOLO launch there), and the refusal and the
documentation use the same wording instead of calling hosts "sensitive" or "YOLO safe".

---
kind: fixed
---

Opening a session's terminal can no longer hang forever when two browser tabs or windows open the same session at the same moment and tmux stops responding while Farhelm cleans up the extra connection. The slower open now fails with an error you can retry, instead of waiting indefinitely.

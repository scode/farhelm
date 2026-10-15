---
kind: fixed
---

Settings no longer silently turns a carriage return in the post-clone command into a newline when you edit it; it refuses the save and explains why. Change such a command with `farhelm helm checkout-config set-post-clone`, or clear it in Settings.

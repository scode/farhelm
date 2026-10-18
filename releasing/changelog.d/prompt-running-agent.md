---
kind: added
---

Agents and scripts can now send a prompt to another running Claude or Codex session with `farhelm agent prompt`.
Farhelm asks first unless the host is trusted; busy agents queue the prompt, and agents waiting on a question refuse it.

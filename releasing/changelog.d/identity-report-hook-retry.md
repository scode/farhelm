---
kind: fixed
---

Resume now follows the conversation you switched to with `/clear` even when Farhelm's supervisor is briefly busy or restarting. If you configured Grok's Farhelm hooks with the earlier 3-second timeout, raise each timeout to 60 seconds; otherwise Grok may cut the hook off during a supervisor restart. Older Claude and Codex sessions keep their previous timer until they are relaunched.

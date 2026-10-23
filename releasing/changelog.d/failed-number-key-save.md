---
kind: fixed
---

If saving a number-key shortcut in the Templates dialog fails, the dialog now says the key may not have been saved and
re-reads the keys from Farhelm. A lost reply can mean the save went through. If the keys cannot be read either, the
dialog says so and shows the previous keys. It never quietly retries the failed save later.

Before, that later save could undo a key an agent or the command line had moved in the meantime, so a number key could
launch a template the agent had moved off it.

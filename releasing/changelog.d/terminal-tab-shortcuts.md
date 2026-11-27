---
kind: changed
---

Deleting a session that still has terminal tabs open now always asks first, even with **Skip session confirmations**
on, whether you delete it from its row, its Delete button or the Delete shortcut. Deleting several marked sessions at
once still follows the switch.

Context for curation: the Settings switch's own help still says it deletes on the first click; it does not mention
this exception (nor the sandbox and container Delete it never skipped).

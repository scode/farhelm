---
kind: fixed
---

"Stop and suspend" on a sandbox session no longer closes a terminal tab or stops an agent you did not see. If another
window opened a tab, or the agent was started again, after you clicked and before Farhelm finished checking the sandbox
for activity, the stop is now refused as it would have been without that delay.

The window was one quick request to the helm, so this was rare. It applies to sandbox and container sessions alike
(both use the same "Stop and suspend").

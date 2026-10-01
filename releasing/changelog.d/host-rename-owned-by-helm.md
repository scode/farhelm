---
kind: fixed
---

Renaming a host in its settings now always finishes on the helm even if the browser disconnects or the page reloads
mid-request, as adding, retargeting, removing and adopting a host already did. Before, the new name could be saved but
not shown anywhere: every window kept showing the old name until some other host change happened.

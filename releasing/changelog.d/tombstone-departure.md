---
kind: fixed
---

A terminal that was taken over by another window, or detached for not keeping up with its output, while it was
reconnecting could come back as a blank pane, with no Detached notice and no take-control action, after its host
stopped answering and then returned. Leaving the session and opening it again was the only way out. It now comes back
normally. Such terminals also no longer keep using memory after they leave the view, for example when their tab closes.

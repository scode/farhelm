---
kind: fixed
---

The New dialog's remembered permission mode, workspace-trust choice, and recent setups now come only from what you chose in it, both on the helm and in an open browser tab or desktop window. A remote host could previously answer a session creation claiming "yolo" with workspace trust, and that became the default the dialog preselected for every host, including the helm's own machine. Sessions an agent creates no longer appear among your recent setups either, and pressing Replace on a session no longer records that session's settings or profile as your choice.

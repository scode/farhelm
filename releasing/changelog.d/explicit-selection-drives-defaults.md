---
kind: fixed
---

The New dialog's remembered permission mode, workspace-trust choice, and recent setups now come only from what you chose in it. A remote host could previously answer a session creation claiming "yolo" with workspace trust, and that became the default the dialog preselected for every host, including the helm's own machine. Sessions an agent creates no longer appear among your recent setups either.

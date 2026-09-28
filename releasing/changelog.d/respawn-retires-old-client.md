---
kind: fixed
---

When a host's connection handler has crashed and the helm restarts it (on Retry or on a host change), the old connection to that host is now always closed. Before, in a short window after the crash, the old connection could stay open and keep answering agent requests for that host.

---
kind: fixed
---

When a terminal viewer detaches after its output stream ends, Farhelm cancels its queued input before admitting a
replacement viewer. Input already handed to the terminal keeps its place.

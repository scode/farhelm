---
kind: fixed
---

A host setup or update whose download of a Farhelm release breaks partway, such as when the network drops, no longer
leaves the unfinished download in the helm's cache until the next helm restart. Every failed download now removes its
partial file.

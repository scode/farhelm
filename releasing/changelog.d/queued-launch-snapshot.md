---
kind: fixed
---

A queued quick-launch sequence now uses the template names it showed when Enter was pressed, even if another client has
changed the shortcut bindings. A key shown as unbound still refuses instead of launching its new assignment.

Context for curation: a later unreleased change (#2598) pairs each sequence with its own snapshot, so after a sequence the app refused to start, a later sequence launches the templates its own indicator showed rather than an earlier sequence's or none. That trigger is a render race found by code reading and not reproduced.

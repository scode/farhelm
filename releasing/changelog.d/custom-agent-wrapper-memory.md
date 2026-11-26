---
kind: fixed
---

The session launcher now remembers a wrapper prefix separately for each custom agent, so one custom agent's wrapper no longer appears for another, and launching one without a wrapper no longer erases the others'. A wrapper remembered before this release for all custom agents together is not carried over.

Deleting a custom agent forgets its wrapper, and an agent registered again under the same ID starts without one. Built-in agent types keep the wrappers they remembered.

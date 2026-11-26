---
kind: fixed
---

The session launcher now remembers a wrapper prefix separately for each custom agent, and switching between two custom agents, by hand or with a template, loads the chosen agent's own wrapper in place of the previous one's. Before, one custom agent's wrapper could run in front of another, and launching one without a wrapper erased the others'. A wrapper remembered before this release for all custom agents together is not carried over.

Deleting a custom agent forgets its wrapper, and an agent registered again under the same ID starts without one. Built-in agent types keep the wrappers they remembered.

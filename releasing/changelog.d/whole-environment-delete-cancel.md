---
kind: fixed
---

The question that deletes a session's whole sandbox or container, which also removes every other session in it, now
starts with Cancel focused, including when it opens by itself after the helm says the session cannot be deleted on its
own, and in bulk Delete when the marked sessions include a whole environment. Before, Enter right after it opened
destroyed the environment.

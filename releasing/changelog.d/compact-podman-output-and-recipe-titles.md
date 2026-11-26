---
kind: fixed
---

Retiring an old image during a container recipe's Rebuild or Delete, building a recipe, and listing leftover
containers in Settings no longer fail with an output-limit error when Podman reports a lot of metadata. A new session's
title is checked before Farhelm starts a disposable environment for it.

Context for curation: Farhelm now asks Podman only for the fields it uses. How much metadata it took to hit the old
limit was not measured, so the first sentence rests on the review finding rather than a reproduction.

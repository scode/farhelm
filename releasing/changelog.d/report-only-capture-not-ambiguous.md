---
kind: fixed
---

Two sessions of Codex, Goose, Pi, OMP, or Grok started within about a minute of each other in the same directory no longer log a warning that neither conversation can be captured, and are no longer marked ambiguous. These agents report their conversation themselves, so the ambiguity rule, which exists for Claude's fallback of finding its conversation file on disk, never applied to them.

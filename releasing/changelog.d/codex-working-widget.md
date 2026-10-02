---
kind: fixed
---

Farhelm now recognizes Codex's on-screen "Working (… esc to interrupt)" line as a sign that the agent is busy. Before,
the check for it never matched what Codex actually draws, so a busy Codex session relied entirely on the spinner in its
terminal title and could show as idle where that title is not available.

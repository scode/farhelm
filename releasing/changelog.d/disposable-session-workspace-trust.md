---
kind: changed
---

Farhelm applies workspace trust automatically to supported agent launches in sandbox and container sessions. On hosts
you add yourself, workspace trust follows Settings, which now says so, and trust hints offer Settings only on hosts
where its switch applies.

Claude in a disposable host's home uses its existing home trust entry. A custom Claude config or an explicit refusal
can leave the question visible. When Codex's Custom permission flags set the project's trust themselves, Farhelm does
not add its own; Antigravity's own settings likewise stay in charge of its trust.

Context for curation: the Codex case covers trust overrides Farhelm recognizes in the flags, including a project path
with dots in it (that case was fixed at landing); Pi's Custom flags are not offered for new launches, so it is not
mentioned.

---
kind: changed
pr: 4797
---

A `written` line in a session's hook log now ends with a tab and `event=` and the moment the agent reported (`Stop` at
the end of a turn, Grok's `UserPromptSubmit`, or Antigravity's `SessionStart`) when the agent named it.

The field was added for the live drift tests, which need to tell two reports from the same conversation apart. The
hook log is a diagnostic file, documented on the "Agent hook injection" page; a script that reads its lines by position
sees one more field on those lines.

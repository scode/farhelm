---
kind: fixed
---

Approval cards now show a carriage return in an agent's command, resume command or wrapper as `<U+000D>`. Before, a
carriage return just before a line break was hidden, so a command that Bash runs as two separate commands could look
like one command continued onto the next line, and approving it could run the second command.

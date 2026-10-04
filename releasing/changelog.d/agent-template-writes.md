---
kind: added
---

Agents can now create, change and delete launch templates with `farhelm agent template create`, `edit` and `delete`,
using the same flags as `farhelm agent create`. Unless you have told Farhelm not to ask for the agent's host, each write
waits for your approval on a card that shows the whole template, including any command line it carries. An edit only sets the fields it is given, so a command line the agent
cannot see is never dropped by accident.

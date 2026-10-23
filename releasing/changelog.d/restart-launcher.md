---
kind: added
---

Restart with now uses the session launcher, including search and template shortcuts. Templates must leave the
agent/command choice, agent type, host, folder, session name and first message unset; setting one refuses the whole
template.

Enter in the model field now chooses the model without restarting; the Restart action applies the selected settings.

Templates saved by New or Clone include the agent/command choice, and the Templates editor requires that choice for
launch-setting fields, so those definitions cannot be used by Restart with as saved. Partial agent-setting templates
can be written through the existing agent template command without an agent or command choice. The editor cannot author
those partial definitions yet.

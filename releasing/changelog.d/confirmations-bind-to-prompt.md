---
kind: fixed
---

A delete or restart confirmation now authorizes only what it said. The sidebar's delete prompt rewords itself while it
stays open, and its confirm used to delete whatever was running at the click; so did a prompt that said nothing was
alive, or warned only about open terminal tabs, if the session had been restarted meanwhile (by the command line, an
agent or another window). Such a delete, or a Replace, is now refused, nothing is deleted, and the next attempt asks
again; a prompt that warned only about tabs still closes them, including one opened after it was shown. A restart
prompt that had come to say there was nothing to stop no longer stops an agent that started working again; the restart
is refused and the next one asks.

For curation: this change moves the helm–supervisor protocol to version 35 (a supervisor that ignored the new "only if
the agent has ended" precondition would delete unconditionally), so the release needs the remote-hosts update entry.

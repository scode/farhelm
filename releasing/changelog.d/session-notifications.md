---
kind: added
---

A session's row in the session list now shows a bell, just left of its age, when Farhelm notices something about that
session you can act on. For now these are problems with following the agent's conversation that would keep **restart**
from picking it up again: an agent that has been working for a while without telling Farhelm which conversation it is
in, a launch Farhelm could not set up to follow, or a saved conversation Farhelm stopped offering to resume. Until now
these were only written to a log. Each notification says what happened and what to do about it, such as starting the
session over with **replace with**, or, when nothing can be done, what you lose. The bell is red and filled while something is new; clicking it lists the session's
last 10 notifications, closing the list marks them read, and **clear all** removes them. Read and cleared notifications
are the same in every window connected to the same helm.

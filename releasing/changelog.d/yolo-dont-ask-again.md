---
kind: added
---

The confirmation Farhelm asks for before starting a YOLO session (an agent that runs without approval prompts) on a host
you have not marked safe can now also mark that host safe and start: "start, and don't ask again on this host". It then
stops asking for that host, as if you had ticked the box in the host's settings; if marking the host fails, nothing is
started and the confirmation stays up with the reason. The confirmation also explains itself now: what YOLO means, and
why this launch is one (you picked YOLO, the agent has no mode that asks, or its command line turns asking off). In the
session launcher it appears right under the Launch button and scrolls into view, instead of at the bottom where you had
to scroll to find it.

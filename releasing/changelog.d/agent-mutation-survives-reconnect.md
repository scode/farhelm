---
kind: fixed
---

A session action an agent started through Farhelm (creating, cloning, renaming, stopping or restarting a session) is no
longer cut off partway when the helm's connection to the agent's host drops or is replaced while the action is running,
for example by a reconnect or a host retarget. It used to stop at whatever step it had reached; a session created that
way could stay out of the session list until the next refresh. The action now finishes. The agent still gets an "outcome
unknown" answer in that case, because the reply had nowhere to go, and should check the session list before retrying;
the action may take a moment to finish.

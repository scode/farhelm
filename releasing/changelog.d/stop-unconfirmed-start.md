---
kind: fixed
---

Stop no longer reports success on a session whose latest launch was never confirmed, such as one whose creation was
interrupted. Farhelm cannot tell what such a session is running, so Stop now refuses, stops nothing (its open terminal
tabs included), and points you to Delete, which deals with whatever the launch left behind, or to Restart where the
session offers it. Before, Stop could say the session had stopped while its agent was still starting.

Two cases that Stop used to handle are now refused the same way: a session whose Restart failed without Farhelm knowing
whether its new agent started, and one whose creation failed after its terminal had already started. To keep the first
kind, Restart it again and then Stop it; otherwise, and for the second kind, Delete the session.

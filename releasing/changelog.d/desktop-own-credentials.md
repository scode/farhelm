---
kind: fixed
---

The desktop app no longer has to re-establish its connection to its own helm after `farhelm helm token rotate`, or after
many browser sign-ins pushed out its saved credentials. Before, that recovery happened behind the scenes, and when it
went wrong the window could open with terminals and the session list that could not connect, or replace itself with an
error page while an action such as a delete was still running. Rotating the token now only signs out browsers; the
desktop window and its open terminals carry on. The desktop app also no longer keeps credentials in its state file.

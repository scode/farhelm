---
kind: fixed
---

Updating or uninstalling Farhelm on the Mac no longer deletes a version of Farhelm that something is still using. Before,
an install that ran while Farhelm was starting, or while an agent's `farhelm` command was starting, could remove the
version it was about to run, so new sessions, Restart and agents' `farhelm` commands failed until Farhelm was quit and
reopened. Inside a session, a `farhelm` command whose version has been removed now runs the installed version instead of
failing.

Draft note for curation: this fragment covers the whole race-free installer change, which lands as a stack of PRs that
ship together; later PRs in the stack edit this fragment rather than adding their own. So far: version locks in `farhelm`
and the forwarder's claim; the desktop app holding its own version from startup (if that version was removed while the
app was starting, the app now says to open Farhelm again instead of failing later); new sessions holding their version while the
user's shell startup files run (a session started just before Farhelm quits can still lose its version to an install
in a short window, and then fails to start, visibly; starting it again works). Installs and uninstall start
honoring the locks in later PRs of the same release.

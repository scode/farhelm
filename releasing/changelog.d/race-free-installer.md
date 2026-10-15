---
kind: fixed
---

Updating or uninstalling Farhelm on the Mac no longer deletes a version of Farhelm that something is still using. Before,
an install that ran while Farhelm was starting, or while an agent's `farhelm` command was starting, could remove the
version it was about to run, so new sessions, Restart and agents' `farhelm` commands failed until Farhelm was quit and
reopened. Inside a session, a `farhelm` command whose version has been removed now runs the installed version instead of
failing.

Draft note for curation: this fragment covers the whole race-free installer change, which lands as a stack of PRs that
ship together; later PRs in the stack edit this fragment rather than adding their own. This first PR adds version locks
to `farhelm` and the forwarder's claim; installs and uninstall start honoring them in later PRs of the same release.

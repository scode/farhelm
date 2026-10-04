---
kind: breaking
---

On a Mac, you can now update Farhelm while it is open. The installer used to replace the program a running Farhelm keeps
starting, so until you restarted it, new sessions could fail to start, agents lost track of their conversations, and
`farhelm spawn` failed. Now the Farhelm that is open keeps working on the version it started with, and quitting and
reopening it finishes the update, for sessions started before the update too.

`~/Applications/Farhelm.app` is now the whole installation. `farhelm-desktop` is no longer installed in `~/.local/bin`,
and `~/.local/bin/farhelm` is a link into the app, for using `farhelm` from a terminal. Open Farhelm from Spotlight,
Launchpad or `~/Applications`. Quit Farhelm before the first update to this release, which converts the installation;
after that, updating while it is open is fine. Releases from before this change can no longer be installed with the
current installer.

---
title: Uninstall Farhelm
description: Remove Farhelm from your Mac, and from remote hosts.
sidebar:
  order: 5
---

## From your Mac

First stop your sessions on your Mac and close their terminal tabs, then quit Farhelm. Uninstalling does not stop
sessions or check for them, and a session left running afterwards loses the `farhelm` command and its connection to
Farhelm. Then run:

```sh
~/.local/bin/farhelm uninstall
```

It lists what it will remove and asks before removing anything. It removes the Farhelm app from `~/Applications`, with
every version kept inside it, and the `farhelm` command from `~/.local/bin`. It keeps your data under
`~/.local/state/farhelm`: your list of hosts, preferences, session history, logs, and the repository caches for managed
checkouts. Your projects, your agents, tmux, and your managed checkouts (including their trash and
[prepared checkouts](/docs/using/start-a-session/#prepared-checkouts) in `.farhelm-spares` inside the directory set as
**Checkout folder**) are not touched. To see the list without changing anything, add `--dry-run`. If Farhelm is still
open when you confirm, it refuses and asks you to quit it first, and nothing is removed. It also refuses while any other
program still runs a version of Farhelm, and names the `lsof` command that shows which one, so you can quit it and run
uninstall again. If an uninstall is interrupted, running it again finishes it.

If uninstall refuses because the installation's record is newer than it understands and asks you to rerun the installer,
it is an uninstall from an older Farhelm, typed in a terminal tab whose session still runs that older version. Quit
Farhelm and run `~/.local/bin/farhelm uninstall` in a terminal outside Farhelm instead, or, from a shell inside a
session, `env -u FARHELM_SUPERVISOR_SOCK ~/.local/bin/farhelm uninstall`. If an upgrade was interrupted, run the
installer first to finish it.

To remove your data as well, delete `~/.local/state/farhelm` once the uninstall is done.

## From a remote host

Uninstalling on your Mac does not touch your remote hosts, and their sessions keep running. To remove Farhelm from a
remote host, select **Uninstall** in that host's `⋯` menu in the host list; see
[Manage hosts](/docs/using/manage-hosts/#uninstall-farhelm-from-a-remote-host).

Farhelm needs to reach the host to do this, so it can check that no session is still running there. If the host shows as
unreachable or with a problem in the list, fix that first, then choose **Uninstall** again. Farhelm also refuses in a
few less common cases, such as a host where Farhelm was set up by running `farhelm helm setup` on the host itself, and
says why when it does. The one exception is finishing an uninstall that stopped partway: once it has removed Farhelm's
service and Farhelm's supervisor there has stopped, the host is expected to look unreachable, and choosing **Uninstall**
again still finishes the job.

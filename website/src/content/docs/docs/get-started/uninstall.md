---
title: Uninstall Farhelm
description: Remove Farhelm from your Mac, and from remote hosts.
sidebar:
  order: 5
---

## From your Mac

Quit Farhelm first, and stop any sessions you do not want left running: uninstalling stops none of them. Then run:

```sh
~/.local/bin/farhelm uninstall
```

It lists what it will remove and asks before removing anything. It removes the Farhelm app from `~/Applications`, with
every version kept inside it, and the `farhelm` command from `~/.local/bin`. It keeps your data under
`~/.local/state/farhelm`: your list of hosts, preferences, session history, and logs. Your projects, your agents, and
tmux are not touched. To see the list without changing anything, add `--dry-run`. If Farhelm is still open, it refuses
and asks you to quit it first.

To remove your data as well, delete `~/.local/state/farhelm` once the uninstall is done.

## From a remote host

Uninstalling on your Mac does not touch your remote hosts, and their sessions keep running. To remove Farhelm from a
remote host, select **uninstall** in that host's `⋯` menu in the host list; see
[Manage hosts](/docs/using/manage-hosts/#uninstall-farhelm-from-a-remote-host).

Farhelm needs to reach the host to do this, so it can check that nothing is still running there. If the host shows as
unreachable or with a problem in the list, fix that first, then choose **uninstall** again. The one exception is
finishing an uninstall that stopped partway: once it has removed Farhelm's service, the host is expected to look
unreachable, and choosing **uninstall** again still finishes the job.

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

It lists what it will remove and asks before removing anything. It removes the Farhelm app from `~/Applications` and the
`farhelm` and `farhelm-desktop` commands from `~/.local/bin`. It keeps your data under `~/.local/state/farhelm`: your
list of hosts, preferences, session history, and logs. Your projects, your agents, and tmux are not touched. To see the
list without changing anything, add `--dry-run`. If Farhelm is still open, it refuses and asks you to quit it first.

To remove your data as well, delete `~/.local/state/farhelm` once the uninstall is done.

## From a remote host

Uninstalling on your Mac does not touch your remote hosts, and their sessions keep running. There is no uninstall
command for a remote host yet. To remove Farhelm from one, first stop its sessions in Farhelm, then run this on the
host:

```sh
systemctl --user disable --now farhelm-supervisor.service
rm ~/.config/systemd/user/farhelm-supervisor.service
systemctl --user daemon-reload
rm -rf ~/.local/lib/farhelm
```

Delete `~/.local/state/farhelm` on the host as well to remove its data. Then remove the host from your list in Farhelm;
see [Manage hosts](/docs/using/manage-hosts/).

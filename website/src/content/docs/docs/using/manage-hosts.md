---
title: Manage hosts
description: Name, update, and remove the hosts your helm knows about.
sidebar:
  order: 5
---

Farhelm keeps your hosts in one shared list. A host is your Mac or a Linux machine that runs sessions. Select a host's
`⋯` menu to edit its SSH destination or display name, mark it safe for YOLO sessions, update Farhelm, or remove it from
the list.

Removing a host only makes Farhelm forget the entry. The supervisor and its sessions keep running, and adding the same
destination again finds them. Farhelm asks before removing a host in a dialog; choose **remove, and don't ask again** if
you want future removals from this helm to happen immediately. The choice is shared by every client after it reloads its
preferences. **Cancel** or press **Escape** to leave the host untouched.

Adding a remote host is described in [Add a remote host](/docs/get-started/add-a-remote-host/), including the SSH
prerequisite and setup details.

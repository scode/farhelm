---
title: Manage hosts
description: Name, update, and remove the hosts your helm knows about.
sidebar:
  order: 5
---

Farhelm keeps your hosts in one shared list. A host is your Mac or a Linux machine that runs sessions. Select a host's
`⋯` menu to edit its SSH destination or display name, mark it safe for YOLO sessions, update Farhelm, or remove it from
the list.

An older remote host also shows an **↑ update** button beside its name when an update is available. Amber means the
update is optional; red means it is required before the host can connect. Hover over the button to see the versions and
what clicking will do. Clicking updates the host to your [helm's](/docs/how-it-works/the-pieces/) version without
another confirmation, and progress takes the button's place. **Update** remains available in the host's menu too.

The Mac running Farhelm is the exception: its row is labeled **local (this machine)** unless you renamed it, and Farhelm
does not update it from the list. Its menu shows **update** greyed out, and you update it by running the installer
again, as described in [Install Farhelm](/docs/get-started/install/).

Removing a host only makes Farhelm forget the entry. The supervisor and its sessions keep running, and adding the same
destination again finds them. Farhelm asks before removing a host in a dialog; choose **remove, and don't ask again** if
you want future removals from this helm to happen immediately. The choice is shared by every client after it reloads its
preferences. **Cancel** or press **Escape** to leave the host untouched.

To bring the question back, select the gear beside the version number at the top of the sidebar. In **settings**, untick
**remove hosts without asking**. The other checkbox, **set up new hosts without asking**, controls the question shown
when a new host needs setup. Ticking either box skips its question. Your next action follows the change immediately;
other open clients pick it up when they reload. These choices apply to all hosts on your helm.

Adding a remote host is described in [Add a remote host](/docs/get-started/add-a-remote-host/), including the SSH
prerequisite and setup details.

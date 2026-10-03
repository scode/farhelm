---
title: Add a remote host
description: Add a Linux machine over SSH and start a session on it.
sidebar:
  order: 4
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

Add a Linux machine as a host and run a first session on it. The **add** button opens a dialog with the SSH destination
and optional Farhelm and state-directory overrides. Farhelm probes first; if a supervisor is already running, it
registers that supervisor without an installation question.

When setup is needed, the dialog lists the host changes Farhelm is about to make: the directories it will create, the
Farhelm executable (and bundled `tmux` when needed), the per-user systemd service, and its persistent and startup
behavior. Choose **yes**, **yes, and don't ask in the future**, or **cancel**. The permanent choice is shared by every
client after it reloads its helm preferences. To turn the question back on, select the gear beside the version at the
top of the sidebar and untick **set up new hosts without asking** in **settings**. See
[Manage hosts](/docs/using/manage-hosts/) for both confirmation choices. A failed setup stays available to retry from
the host row.

Passwordless SSH is the prerequisite on the host. Why SSH is all Farhelm needs is in the
[security model](/docs/how-it-works/security-model/); naming, updating and removing hosts afterwards is in
[Manage hosts](/docs/using/manage-hosts/).

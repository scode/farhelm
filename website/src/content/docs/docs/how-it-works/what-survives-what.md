---
title: What survives what
description: Which events a session, its terminal contents, and its conversation survive.
sidebar:
  order: 2
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

The product's core promise as one table: across closing the window, restarting the helm, restarting a supervisor, and
rebooting a host, what happens to the session, its terminal contents, and the agent's conversation. This is the
canonical statement of those guarantees; other pages link here rather than restating them. Getting a stopped agent back
is in [Stop, restart, and resume](/docs/using/stop-restart-resume/). It will also cover
[container session lifecycle](/docs/agents/containerized-sessions/#stopping-and-deleting). Suspend preserves processes
and memory. A host reboot stops the container and keeps its files. Farhelm attempts to start it again when its host is
connected. The interrupted agent waits for you to choose Restart.

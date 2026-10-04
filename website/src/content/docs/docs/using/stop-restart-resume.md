---
title: Stop, restart, and resume
description: What stopping, restarting, and resuming a session does to the agent and its conversation.
sidebar:
  order: 5
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

The lifecycle actions and what each one does to the agent process and its conversation: stop, restart, restart with
changed settings, replace, clone, and delete. Restart always picks up the session's own conversation where it left off;
there is no restart that starts a new conversation, so a session whose conversation Farhelm cannot resume cannot be
restarted; its restart button is greyed out and says why. Replace, or Replace with to change its settings, is how you
start such a session over. Which agents Farhelm can resume is in [Supported agents](/docs/agents/), a command you run
yourself needs a declared agent and a resume command for it (see [Custom commands](/docs/agents/custom-commands/)), and
[What survives what](/docs/how-it-works/what-survives-what/) covers what a reboot does on its own.

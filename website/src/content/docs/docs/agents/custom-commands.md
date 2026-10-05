---
title: Custom commands
description: Run any command as a session, say whether it runs without approval prompts, and opt into Resume.
sidebar:
  order: 50
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

Running any command as a session from the session launcher's **command** tab. The YOLO answer every command needs: you
say whether it runs without approval prompts, and Farhelm believes you and never reads the command to check (an agent's
answer is not taken; see
[Approve what agents do](/docs/using/approve-agent-requests/#yolo-sessions-and-custom-commands)). How to say which agent
the command runs, with `{farhelm_args}` marking where Farhelm adds its own arguments, and what that gives you: the
agent's status and conversation tracking. `{cwd}` for the session's directory. Opting into Resume with a resume command
containing `{conversation}` and `{farhelm_args}`, and why a command without one cannot be restarted. What you give up
compared with launching a [supported agent](/docs/agents/) directly. Saving a command to reuse is a
[launch template](/docs/using/launch-templates/). Agents started through a wrapper script have their own page:
[Agent wrappers](/docs/agents/agent-wrappers/).

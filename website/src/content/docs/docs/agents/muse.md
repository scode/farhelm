---
title: Muse
description: "What is specific to running Muse under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Muse. This page only adds what is
specific to Muse.

## Restarting a conversation

With Muse 1.4.14 or later, **Restart** becomes available after you submit your first prompt. Farhelm adds hooks for that
launch without installing a Muse plugin or changing Muse's saved configuration. They also let Muse learn how to handle
your `$farhelm` requests; see [Agent hook injection](/docs/how-it-works/agent-hook-injection/).

Muse reports its conversation again at the end of a turn. After `/clear` or `/new`, it reports the new conversation at
your next prompt, so restarting before then can bring back the previous conversation. Restart keeps your launch's
permission choices and resumes the last conversation Muse reported.

Muse works through [wrappers that stay running](/docs/agents/agent-wrappers/). A custom command must declare Muse and
pass `{farhelm_args}` to it; its resume command can use `muse resume {conversation} {farhelm_args}` with the same
permission flags as its start command.

Muse sessions created before this support was added keep their old launch. They gain the hooks at their next start, but
have no resume command for **Restart** to use; recreate them through **Clone** or **Replace with**. If a new session
never becomes restartable after a prompt, see
[When something goes wrong](/docs/how-it-works/agent-hook-injection/#when-something-goes-wrong).

## Gotchas and quirks

- Guarded selects Muse's auto-review profile with `--permission-profile :auto-review`, overriding an unrestricted
  default. Your approval rules, sandbox settings and later mode switches still matter; see
  [Permission modes](/docs/agents/#permission-modes). Farhelm also tries to notice **YOLO** on Muse's status line and
  show a YOLO shield; this is best effort, not a check of your approval or sandbox settings.
- Muse's switch in [agent trust settings](/docs/using/start-a-session/#trust-in-managed-checkouts) can skip its
  directory-trust question for directories Farhelm manages, on every host. It is off by default. Farhelm passes a trust
  argument for each new launch without changing Muse's saved configuration; trusting the directory allows the
  repository's own hooks, settings and commands to run as if you had approved them.
- YOLO also makes Muse trust the session's directory. Turning the setting off cannot take back trust granted by YOLO or
  Muse's own configuration. Directories outside managed checkouts keep Muse's own trust behavior.

For what survives closing Farhelm or rebooting a host, see [What survives what](/docs/how-it-works/what-survives-what/).

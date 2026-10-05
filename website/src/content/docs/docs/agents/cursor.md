---
title: Cursor
description: Launching Cursor's cursor-agent CLI; conversation tracking and restart are not supported.
sidebar:
  order: 10
---

Farhelm can launch Cursor's `cursor-agent` CLI, with optional model selection and a YOLO permission choice.
**Conversation tracking is not supported**, so a Cursor session cannot be restarted: replace it to start over. Clone
preserves launch choices, not the conversation. This page describes Cursor CLI `2026.09.18-9a7762b`.

## Launching

Install and authenticate [Cursor CLI](https://cursor.com/docs/cli/overview) on each host/account where you will run it.
Farhelm starts it as `cursor-agent`, so that command must be on the host's PATH. Cursor also installs it as `agent`, but
Farhelm does not use that name: other tools install a command called `agent` too, so the name says nothing about which
program will run.

Choose Cursor in the session launcher. The default permissions run `cursor-agent`, and YOLO runs `cursor-agent --force`.
Omitting a model leaves Cursor's default in effect. The small suggested model list contains `auto` and `composer-2.5`; a
custom Cursor model ID is passed as one literal `--model` argument. Your choices are kept in recent setups and carried
over by clone.

Default permissions add no flag and retain Cursor's configuration. YOLO adds `--force`, which allows commands except
those explicitly denied by Cursor's configuration. Other vendor modes can be used in a
[custom command](/docs/agents/custom-commands/). Farhelm does not provide a separate Cursor effort selector; use a
vendor model variant or bracket parameters in a custom model ID.

## Limits

Cursor uses Farhelm's generic integration. It has normal terminal interaction, activity status, stop and replace, but no
Cursor-specific waiting-state detection. Answer approval prompts in the terminal.

Farhelm installs no Cursor hooks, status wrappers, plugins or model instructions. It neither reads nor edits Cursor's
configuration or conversation stores. There is no tracking setup command to enable: session tracking is outside this
version's scope. Use Cursor's own conversation picker or CLI manually when you need to reopen saved history.

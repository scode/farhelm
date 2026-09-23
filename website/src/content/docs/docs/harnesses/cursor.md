---
title: Cursor
description: Launching Cursor's agent CLI; conversation tracking and Resume are not supported.
---

Farhelm can launch Cursor's `agent` CLI, with optional model selection and a YOLO permission choice. **Conversation
tracking and Resume are not supported.** Restart starts a new conversation; clone preserves launch choices, not the
conversation. This page describes Cursor CLI `2026.09.18-9a7762b`.

## Launching

Install and authenticate [Cursor CLI](https://cursor.com/docs/cli/overview) on each host/account where you will run it.
The `agent` command on that host's PATH must be Cursor's executable. If another program uses that name, use an explicit
path to Cursor's `cursor-agent` launcher through “other / command” or a custom Generic profile.

Choose Cursor in the launch composer, or use the `cursor` / `cursor-yolo` built-in profiles. These produce `agent` and
`agent --force`, respectively. Omitting a model leaves Cursor's default in effect. The small suggested model list
contains `auto` and `composer-2.5`; a custom Cursor model ID is passed as one literal `--model` argument. Saved
structured choices survive history, clone and fresh restart.

Default permissions add no flag and retain Cursor's configuration. YOLO adds `--force`, which allows commands except
those explicitly denied by Cursor's configuration. Other vendor modes can be used in raw commands. Farhelm does not
provide a separate Cursor effort selector; use a vendor model variant or bracket parameters in a custom model ID.

## Limits

Cursor uses Farhelm's Generic runtime integration. It has normal terminal interaction, activity status, stop and fresh
restart, but no Cursor-specific waiting-state detection. Answer approval prompts in the terminal.

Farhelm installs no Cursor hooks, status wrappers, plugins or model instructions. It neither reads nor edits Cursor's
configuration or conversation stores. There is no tracking setup command to enable: session tracking is outside this
version's scope. Use Cursor's own conversation picker or CLI manually when you need to reopen saved history.

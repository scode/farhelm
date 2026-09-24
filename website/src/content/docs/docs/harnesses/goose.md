---
title: Goose
description: Model choice, the conversation-reporting helper, and resuming Goose outside Farhelm.
---

## Model choice

You can leave the model unset in Farhelm. Goose then uses its configured provider and model; Farhelm does not add
`--provider` or `--model`. Choosing a model explicitly keeps Farhelm's OpenRouter launch arguments for that choice.

## Session tracking limitation

Farhelm's Goose reporter records the session Goose identifies as current. The reporter credential proves that the report
belongs to this Farhelm session, but the basic integration does not independently prove that the Goose session is the
foreground root. Goose native subagents can inherit the reporter and use their own session IDs, and a separately
launched Goose can be given the same reporter. A child report can therefore replace the saved restart target.

This is a known limitation of the basic integration. Ordinary foreground sessions and their `/new` transitions are
supported; reliable isolation of native or shelled-out child sessions is deferred. Do not treat a captured Goose
conversation as proof that no child session reported. The design research and the preserved stricter implementation are
recorded in `lore/2026-09-23-goose-session-tracking.md`.

## Resuming outside Farhelm still needs Farhelm installed

Farhelm starts a small helper alongside Goose to learn which conversation you are in. When you type `/new` in Goose to
start a new conversation, the helper tells Farhelm to resume that conversation instead of the old one. Goose treats the
helper as an extension and saves its startup command with the conversation, so it starts again on later resumes.

That means reopening the conversation directly in Goose, outside Farhelm, still requires the `farhelm` command to be
available in your shell (`PATH`). If you uninstall Farhelm or move the conversation to a machine without it, Goose can
report an extension-startup error. Resuming through Farhelm supplies the helper's location automatically.

The saved command contains no Farhelm credentials or machine-specific executable paths. Outside Farhelm, the helper
starts but does nothing: it sends no reports and gives the agent no tools or instructions. No global Goose settings are
changed.

Disabling [Farhelm's conversation reporting](/docs/concepts/agent-hook-injection/#turning-it-off) prevents the helper
from being added to new conversations but does not remove its startup command from conversations Goose has already
saved.

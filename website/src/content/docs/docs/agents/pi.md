---
title: Pi
description: Model choice, the YOLO-only permission mode, and conversation reporting for Pi.
sidebar:
  order: 10
---

## Model choice

You can leave the model unset in Farhelm. Pi then uses its configured provider and model; Farhelm does not add
`--provider` or `--model`. Choosing a model explicitly keeps Farhelm's OpenRouter launch arguments for that choice.

## YOLO is the only permission mode

Pi does not ask for permission before running shell commands or editing files, so Farhelm shows YOLO as its only
permission option. Pi's `--approve` flag instead controls whether Pi trusts settings and extensions supplied by the
project you opened; it does not enable approval prompts for the agent's actions. This was verified against Pi 0.85.1.

The structured launch composer offers workspace trust separately from YOLO. Choosing true adds `--approve` for that
launch; choosing false adds `--no-approve`. The last explicit choice from a successful user launch becomes the next new
dialog's default. Farhelm does not write Pi's persistent trust settings.

## Resume needs a saved conversation

Pi saves conversations automatically; there is no separate Save step. A new conversation's file is written after the
first assistant message. Send a prompt and let Pi respond; Farhelm can then offer Resume once Pi reports the saved
conversation.

Typing `/new` in Pi starts a new conversation. Until that conversation is saved, Farhelm stops offering Resume for the
old one: restarting should not take you back to a conversation you already left.

Farhelm checks the saved file before resuming. If it is missing or belongs to a different conversation, Farhelm refuses
Resume and asks you to choose a fresh launch instead. This matters because Pi itself can silently start a new
conversation when given a missing session file.

The helper Farhelm uses to track Pi's active conversation is supplied at launch and is not saved with the conversation.
Reopening a Pi conversation outside Farhelm does not require Farhelm to be installed.

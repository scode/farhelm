---
title: Muse
description: "What is specific to running Muse under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Muse. This page only adds what is
specific to Muse.

## Gotchas and quirks

- Guarded selects Muse's auto-review profile with `--permission-profile :auto-review`, overriding an unrestricted
  default. Your approval rules, sandbox settings and later mode switches still matter; see
  [Permission modes](/docs/agents/#permission-modes). Farhelm also tries to notice **YOLO** on Muse's status line and
  show a YOLO shield; this is best effort, not a check of your approval or sandbox settings.
- Muse's switch in [managed-checkout settings](/docs/using/start-a-session/#trust-in-managed-checkouts) can skip its
  directory-trust question for directories Farhelm manages, on every host. It is off by default. Farhelm passes a trust
  argument for each new launch without changing Muse's saved configuration; trusting the directory allows the
  repository's own hooks, settings and commands to run as if you had approved them.
- YOLO also makes Muse trust the session's directory. Turning the setting off cannot take back trust granted by YOLO or
  Muse's own configuration. Directories outside managed checkouts keep Muse's own trust behavior.

---
title: Shell sessions
description: Start Bash or Zsh in a Farhelm session.
sidebar:
  order: 40
---

To open a terminal in its own session, type `bash` or `zsh` in the [session launcher](/docs/using/start-a-session/) and
press Enter twice. You can also pick either shell on the **Agent** tab, after the agent chips. The selected shell must
be on the PATH where the session runs.

A shell session has no agent or permission mode. Its row shows a shell icon without a permission shield. Use **Stop** to
end it; Farhelm hangs up the shell, as closing a terminal window would, so it can save its history. Use
[**Replace**](/docs/using/stop-restart-resume/) to start a fresh shell in the same directory, or **Clone** to open
another with the same setup. A [wrapper prefix](/docs/agents/agent-wrappers/) works for shells too.

If you often open a shell in the same place, [save the setup as a template](/docs/using/launch-templates/).

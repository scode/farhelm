# Grok

Farhelm can launch the official `grok` CLI with a dedicated runtime and an explicit permissive mode. **This launch layer
does not yet capture Grok conversations or offer Resume.** It preserves the native resume command so capture can be
added without changing saved launch intent. This page describes behavior verified on Linux against Grok Build 1.0.40;
macOS runtime behavior has not been verified.

## Launching

Install and authenticate `grok` on every host where you will run it, then choose Grok in the launch composer. A normal
launch runs:

```console
grok --no-leader
```

The default permission choice adds no approval flag. Choosing YOLO adds `--always-approve`; use it only when you intend
Grok to approve tool use without prompting. Farhelm does not expose a Grok model or effort selector because those CLI
contracts have not been verified. Native Grok configuration, authentication, the working directory, and saved sessions
remain Grok's responsibility.

Every generated Grok command includes `--no-leader`, including the saved resume form:

```console
grok --no-leader --resume <conversation-id>
```

The flag keeps Grok's backend inside the process tree Farhelm owns. Grok's shared leader can outlive that tree and does
not carry the launch's Farhelm ownership proof. A private leader may use more resources, but native subagents still work
inside it.

## Current limits

Farhelm uses its generic running and idle status for Grok. It does not recognize Grok approval prompts as waiting, so
answer them in the terminal.

This launch layer installs no hooks, edits no Grok configuration, scans no Grok history, and does not inject Farhelm's
agent-instructions pointer. The saved resume template alone is not evidence of a conversation: without a verified
capture, restart starts fresh and Farhelm never guesses a target from Grok's history. Wrappers, package-manager launch
forms, shared leaders, and other custom command shapes remain ordinary generic launches rather than silently claiming
tracked Grok behavior.

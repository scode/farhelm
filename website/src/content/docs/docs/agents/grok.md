---
title: Grok
description: Tracking and resuming the native Grok Build CLI, and the three hooks it needs.
sidebar:
  order: 10
---

Farhelm can track and resume one top-level conversation in the native Grok Build CLI. Native subagents still work inside
that conversation. Multiple top-level conversations, `/fork`, and dashboard switching are outside the tracking promise:
use a separate Farhelm session for each top-level conversation you need to keep distinct.

NOTE: The runtime behavior on this page was verified on Linux with Grok Build 1.0.40. The macOS process shape has not
been verified. Wrappers, package-manager launchers, renamed executables, and shared-leader launches may run normally but
never capture a conversation, so they cannot be restarted, because Farhelm cannot prove which foreground runtime sent
their reports.

## Launching

Install and authenticate `grok` on every execution host, including remote hosts. A normal launch runs:

```console
grok --no-leader
```

The default permission choice adds no approval flag. Choosing YOLO adds `--always-approve`; use it only when you intend
Grok to approve tool use without prompting. Farhelm does not expose Grok model or effort controls because those CLI
contracts have not been verified.

`--no-leader` keeps Grok's backend in the process tree Farhelm owns. The shared leader can outlive that tree and its
hooks do not carry this launch's Farhelm credential, so it cannot establish the ownership proof capture requires. A
private leader gives up backend reuse and may consume more resources. Native subagents continue to work.

Choosing Grok in the session launcher is the supported path. A [custom command](/docs/agents/custom-commands/) that
declares Grok as its agent must keep the native `grok` executable and `--no-leader`, before any `--`, and its resume
command must resume with `--resume {conversation}`, for example
`grok --no-leader --resume {conversation} {farhelm_args}`. Farhelm runs the commands you write as written; it does not
move or delete arguments whose meaning belongs to Grok.

## Configure the three hooks

Farhelm does not edit Grok configuration. Create `$GROK_HOME/hooks/farhelm.json` on every execution host. When
`GROK_HOME` is unset, that path is `~/.grok/hooks/farhelm.json`. Replace `/absolute/path/to/farhelm` below with the
installed binary visible on that host:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/absolute/path/to/farhelm internal hook --vendor grok",
            "timeout": 60
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/absolute/path/to/farhelm internal hook --vendor grok",
            "timeout": 60
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "/absolute/path/to/farhelm internal hook --vendor grok",
            "timeout": 60
          }
        ]
      }
    ]
  }
}
```

Grok merges JSON files under its hooks directory, so leave other hook files alone. If you add these entries to an
existing JSON document instead, append the matcher groups to the existing arrays rather than replacing them. Do not put
Farhelm session IDs, tokens, or socket paths in the file; the tracked launch supplies those values in its environment.

Restart Grok after adding the file. Use Grok's `/hooks` view to confirm all three entries loaded. To remove the
integration, remove these three matcher groups (or delete `farhelm.json` when it contains nothing else) and restart
Grok. Farhelm does not remove them for you.

The reporter is silent and exits zero. It saves each report for Farhelm and is done in a moment, without waiting for
Farhelm, so it adds only a short call at each subscribed event, including `UserPromptSubmit` on the prompt path; a
report made while the host's supervisor is not running (on your Mac, while Farhelm is closed) is applied when the
supervisor starts again. The 60-second hook timeout above leaves room for the one wait it still has, reading the event
Grok hands it. It does no reporting when the Grok process was not launched by Farhelm. Grok does not receive Farhelm's
agent-instructions announcement from these manual hooks.

## What capture and Resume mean

`SessionStart` selects the top-level UUID. A new conversation usually cannot be restarted at first, because Grok has not
yet created `updates.jsonl`. `UserPromptSubmit` normally supplies that exact path after the first prompt, and `Stop` is
a later fallback. A loaded saved conversation may be ready immediately.

Farhelm offers Resume only after two files agree with the selected UUID: the reported absolute `updates.jsonl` begins
with the supported `_x.ai/session/update` record and matching `params.sessionId`, and its sibling `summary.json` is a
complete bounded JSON document with the same value at `info.id`. Resume substitutes the UUID into:

```console
grok --no-leader --resume <conversation-id>
```

The exact files are checked again while Farhelm builds the offer and immediately before Resume. Missing, moved,
malformed, unreadable, unsafe, oversized, or mismatched evidence withdraws or refuses Resume. Farhelm never scans Grok
history, chooses the latest saved session, substitutes a record path, or silently turns a Resume request into a fresh
conversation.

`/new` selects the replacement UUID as soon as its `SessionStart` report is accepted, withdrawing the old offer even
when the replacement is still pending. Selection timestamps form a fail-closed ordering fence: another UUID must carry a
strictly newer event timestamp, while an equal or backward timestamp is refused rather than allowed to restore an older
conversation. Farhelm keeps no history or clock-recovery protocol behind this check.

There is one accepted delivery race. If you run `/new` and Grok exits or crashes before its callback reaches Farhelm,
the previous conversation can remain the last known identity. There is no acknowledgement queue or history scan to
repair a callback that never arrived.

Native subagent callbacks carry a child marker and are always refused (the hook does not even save them), so they do not
replace the top-level UUID. Compaction does not change the selection. A separately launched nested Grok runtime is also
refused by process attribution.

## Diagnose capture

After the first prompt, the session should become restartable. If it does not, inspect
`<state dir>/hook-log/<session id>.log`, where `<state dir>` is `$XDG_STATE_HOME/farhelm` or `~/.local/state/farhelm`.
Each callback leaves a `written` line when the hook saved it, and the supervisor answers the reports it takes up with an
`acked` or a `refused` line with the reason; a later callback can replace a saved one before the supervisor looks, and
the replaced one gets no answer. `bad-payload` and `write-failed` mean the hook could not save the callback at all,
except `bad-payload subagent-report`, which is a subagent's callback being ignored, as intended.
[Agent hook injection](/docs/agents/agent-hook-injection/#when-something-goes-wrong) documents the complete log format.

No log usually means Grok did not load or run the hook, or the command path is wrong. An `acked` line on a session that
still cannot be restarted usually means the selected conversation does not yet have both exact files, or later
verification withdrew the offer. The supervisor log distinguishes process-attribution refusals from file mismatch and
ordering refusals.

Farhelm uses generic running and idle activity for Grok. It does not recognize Grok approval prompts as waiting and does
not take over Grok's status line. Continue handling approvals in Grok's terminal.

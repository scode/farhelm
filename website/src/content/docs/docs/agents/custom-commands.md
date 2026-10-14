---
title: Custom commands
description: Run any command as a session, say whether it runs without approval prompts, and opt into restart.
sidebar:
  order: 50
---

:::caution[Subject to change]

Custom commands are among the parts of Farhelm most likely to change from one release to the next: the details of how
they work, and what is and is not supported, may change.

:::

The session launcher's **command** tab runs any command line as a session. Use it for an agent Farhelm does not know, or
for one it does know when you want to start it your own way: through a wrapper, with flags the **agent** tab does not
offer, or from a different install. What Farhelm can then do for the session depends on what you tell it, and the
**Command** column of the [support matrix](/docs/agents/) sums that up.

## The fields

- **agent command**: the command line to run. Write `{cwd}` as a whole argument where the command wants the session's
  directory, and Farhelm fills it in at every launch and restart.
- **runs without approval prompts**: **yes (YOLO)** or **no**, and you have to pick one. Farhelm never reads the command
  to work out which; it takes your answer, and asks you to
  [confirm a YOLO launch](/docs/using/start-a-session/#confirm-a-yolo-launch) on hosts that ask. An agent cannot give
  this answer for you, so an agent cannot start a command at all on a host that asks before YOLO launches
  ([Approve what agents do](/docs/using/approve-agent-requests/#yolo-sessions-and-custom-commands)).
- **agent type**, optional: the agent the command ends up running. Leave it empty for anything that is not one of the
  agents Farhelm knows.
- **resume**, and the **resume command** it reveals: how to start the command again on the same conversation, so the
  session can be restarted.

## Declaring the agent

Declaring the agent type gives the session what Farhelm does for that agent, as far as the agent allows: Claude's and
Codex's reading of the screen, including showing when the agent waits for you, and conversation tracking for Claude,
Codex, Goose, OMP, Pi, and Grok (with its hooks installed). Declaring Cursor, Muse, or OpenCode gives no tracking, since
Farhelm has none for them.

A declared agent needs `{farhelm_args}` in the command, once, as a whole argument, where that agent's own arguments go.
Farhelm puts there whatever it needs to follow the conversation; for an agent that takes nothing, it becomes nothing.
Your command has to pass those arguments on to the agent: a wrapper that drops them loses tracking.
[Agent wrappers](/docs/agents/agent-wrappers/) shows how to write one that keeps them.

## Making it restartable

A command can be restarted only when it declares an agent Farhelm can track, ticks **resume**, and has a resume command
with `{conversation}` and `{farhelm_args}` in it, each once, as a whole argument. Farhelm puts the tracked conversation
where `{conversation}` stands. Without all of that, **restart** stays greyed out and replace is how you start over.

The resume command is usually the start command plus the agent's own way of picking a conversation:

| Agent     | Resume command, after your start command's options                        |
| --------- | ------------------------------------------------------------------------- |
| Claude    | `--resume {conversation} {farhelm_args}`                                  |
| Codex     | `resume {conversation} {farhelm_args}`                                    |
| Goose     | `--resume --session-id {conversation} {farhelm_args}`, on `goose session` |
| OMP, Grok | `--resume {conversation} {farhelm_args}`                                  |
| Pi        | `--session {conversation} {farhelm_args}`                                 |

So `claude --model sonnet {farhelm_args}` resumes as `claude --model sonnet --resume {conversation} {farhelm_args}`.
Your answer to **runs without approval prompts** covers the resume command too.

Each agent's own page notes anything its custom commands need beyond this, such as Grok's `--no-leader` and Goose's
`session`.

## Opening links

Commands in remote sessions can open links in your browser when they open them through the `BROWSER` environment
variable, as most programs do. Your own browser setting wins; see
[Open links from a remote session](/docs/using/approve-agent-requests/#open-links-from-a-remote-session) for approvals
and the limitations of remote login callbacks. A command can ask explicitly with `farhelm agent open-url -- <url>`.

## Reusing a command

Save a command and its fields as a [launch template](/docs/using/launch-templates/) to start it again without typing it,
or choose **clone** on a session started from it.

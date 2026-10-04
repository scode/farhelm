---
title: Agent wrappers
description: Running agents through a wrapper that changes directory and stays resident as the agent's parent.
sidebar:
  order: 60
---

Some environments do not start agents directly; they start a wrapper — `wrapper run <dir> <agent...>` — that `cd`s into
the directory, does whatever bookkeeping it manages there, and runs the agent as a child while staying resident as its
parent. Farhelm runs a wrapper like any other [custom command](/docs/agents/custom-commands/); there is no wrapper mode
to turn on. Write `{cwd}` where the wrapper wants the directory and Farhelm substitutes the session's own directory at
every launch and every restart, so one command (or one [launch template](/docs/using/launch-templates/)) serves every
directory. Declare the agent the wrapper ends up running as the command's agent, and put `{farhelm_args}` where that
agent's arguments go: without a declared agent, the session gets no agent-specific status, no conversation tracking, and
no restart. On stop the wrapper is killed along with the agent; being the agent's parent exempts it from nothing.

## A wrapper command, field by field

On the session launcher's **command** tab, four fields matter:

- **agent command**: `my-wrapper run {cwd} claude {farhelm_args}`
- **runs without approval prompts**: your answer, **yes (YOLO)** or **no**. Farhelm believes it and never reads the
  command to check it, and it covers the resume command too.
- **agent type**: Claude — the agent the wrapper ends up running, never the wrapper itself
- **resume command**, which appears once you tick **resume**:
  `my-wrapper run {cwd} claude --resume {conversation} {farhelm_args}`

`{cwd}` is a whole word or it is nothing. An argument either equals `{cwd}` exactly, in which case it is replaced, or it
is passed through as literal text: `--dir={cwd}` reaches the wrapper unchanged, and so does a `{cwd}` written inside a
`sh -c` script string, since that is part of one argument rather than an argument of its own. `{conversation}` and
`{farhelm_args}` follow the same rule, and both must appear exactly once, as whole arguments, where they are required.
Beyond that: every occurrence of `{cwd}` is filled, not just the first; it may not be the first word, because
substituting there would make the working directory the program this session runs, and the launcher refuses that
outright; and a directory whose name contains spaces arrives as a single argument, because the value goes into the
argument's own slot with no quoting for you to get right.

The value is the directory the session's terminal starts in, spelled the way you gave it after `~` expansion, symlinks
intact. On a restart — and on the retry of an interrupted create — Farhelm checks that the spelling still resolves to
the canonical identity it recorded when the session was created and hands over that resolved path instead. Either way
the wrapper is handed exactly the string the session's terminal is started in — one string, which is not quite one
directory: on a fresh create each side resolves that spelling for itself, so a symlink retargeted in between puts them
in different places. That race is yours rather than Farhelm's, and the recorded directory closes it from the first
restart onwards. Resume depends on the agent reporting its conversation; Farhelm does not use the working directory to
guess which one it is.

## A worked example with `sh`

`sh` is a real wrapper of the minimal kind, and worth running once before you point a command at the actual thing:

```
sh -c 'cd "$1" && shift && "$@"; exit $?' w {cwd} claude {farhelm_args}
```

`w` becomes `$0` for the inner shell, `{cwd}` lands in `$1` as a positional argument (not inside the script text, where
it would stay literal), and `"$@"` is the agent command line with Farhelm's arguments where `{farhelm_args}` stood. The
trailing `; exit $?` is load-bearing: when `/bin/sh` is bash, the last command of a script is tail-`exec`ed as an
optimization, so `... && "$@"` on its own replaces the shell with the agent and leaves no resident wrapper at all. With
a command after it, bash and dash both stay put as the agent's parent, which is what a real wrapper does.

## Why the agent must be declared

Farhelm never reads a command line to decide which agent it runs: a command whose first word happens to be `claude` runs
no Claude integration unless you declare Claude as its agent. That is deliberate, because the shapes it would have to be
clever about — wrappers, `env`, a command buried in a `bash -c` script string — cannot be recognized reliably, and
declaring the agent means nothing has to try.

A wrapper command with no declared agent launches and runs fine — no error, no warning. What it does not get is
conversation tracking, Farhelm's arguments, per-agent status, and the restart that follows from them: such a session
cannot be restarted at all, only replaced.

## What the wrapper must pass through

Farhelm puts its arguments exactly where `{farhelm_args}` stands, and only there; for an agent that takes none it stands
for nothing. They are what turns the agent's conversation reporting on, as
[Agent hook injection](/docs/agents/agent-hook-injection/) describes. A resume is no exception: the resume command is a
complete command line in its own right — Farhelm fills in its `{conversation}` and `{farhelm_args}` and runs THAT
instead of the start command, which is why the wrapper has to appear in it too.

So Farhelm's arguments are what a wrapper must forward, and one that treats everything after its own arguments as the
command to run, verbatim, does. A wrapper that parses trailing options as its own eats them first, and the symptom is
indirect: the agent never reports its conversation, and the session cannot be restarted. NOTE: that is the one thing
Farhelm cannot check for you. Its own tests stand in `sh -c` for the real wrapper, so whether YOUR wrapper stops at the
agent command and forwards the rest is something only you can verify — `ps -o args= -p <agent pid>` against a live
session shows what actually reached the agent.

For Claude there is one more condition on the shape: the wrapper must start Claude as its own direct child. Farhelm only
accepts a Claude report from the session's own process or that process's direct child, which is what keeps a `claude`
the session starts through its shell (a shelled-out sub-agent) from replacing the conversation you are in. A resident
wrapper that runs Claude itself is exactly one level, so it keeps reporting. A chain of two resident launchers — a
wrapper that runs a script, which in turn runs Claude without `exec` — puts Claude too far below: its reports are
refused, the hook log records a `refused conflict` line, and the session cannot be restarted. A launcher that `exec`s
(as `env` does, and as a script ending in `exec claude "$@"` does) replaces itself rather than staying in the chain, so
it adds no level.

Four variables travel in the environment rather than on the command line: `FARHELM_SESSION_ID` (which session this is —
no sweep will claim a process that does not carry it), `FARHELM_AGENT_ID` (the same session id again, under a name that
says this process belongs to the session's AGENT rather than to one of its terminal tabs; that is the marker a stop
selects on), `FARHELM_SESSION_TOKEN` (the bearer credential proving a spawn request came from this session), and
`FARHELM_SUPERVISOR_SOCK` (the supervisor socket to dial). A wrapper inherits all four and passes them to its child by
default, so this needs no thought unless your wrapper deliberately scrubs the environment.

## What happens on stop

There are fewer processes involved than the launch chain suggests. tmux starts the pane's login shell, the shell `exec`s
Farhelm's launch shim, and the shim `exec`s your command — so the WRAPPER is the pane's own process and the agent is its
child. Where farhelm's own probe finds a WORKING systemd user manager — it creates a throwaway scope, looks it up, kills
it, and confirms it went away, rather than trusting a `systemd-run` on `PATH` — the launch also runs inside a per-launch
cgroup scope, which is containment rather than a level of the tree (`systemd-run --scope` execs in place too). Each
launch records what it selected, so a session can outlive the manager that scoped it.

Stopping a session and restarting it reap the agent's tree and leave the session's terminal tabs running; deleting it
takes the tabs too. What a stop claims is the pane process's descendants plus every process carrying the session's
`FARHELM_AGENT_ID` — the value is the session's own id, set on every agent launch, so the marker says "an agent of this
session" rather than "this generation of it" — and, for sessions launched by builds predating that marker, anything
wearing the session marker with no other claim on it. All of those additionally require the session's
`FARHELM_SESSION_ID`: one session's stop can never reach another's processes however their other markers read. A tab's
shell wears its own tab marker, which is what keeps it out — and, when a tab or a whole session is torn down, what earns
it a SIGHUP alongside the SIGTERM: an interactive shell ignores SIGTERM and exits on the hangup its terminal closing
would have sent, so a tab close ends inside the grace instead of waiting for the SIGKILL. Agents never carry a tab
marker and are never hung up.

Where this launch recorded a cgroup scope, the manager is still there, and that unit still exists, the scope is killed
first: SIGTERM to everything in it, up to five seconds for the unit to retire on its own (the wait ends the moment it
does), then SIGKILL if it has not, then a bounded wait for the unit to be collected. The process-table sweep runs
afterwards regardless — as the backstop there, and as the whole mechanism on a host with no user manager. It SIGTERMs
everything the first enumeration found, waits up to the same five seconds for every one of those pids to be confirmed
gone (again ending as soon as they are), then RE-ENUMERATES and SIGSTOPs the refreshed set: the refresh comes first so
that a process forked inside a TERM handler is frozen along with everything else rather than escaping with the parent.
Stopped processes cannot fork, so what is left only shrinks — up to five further passes re-enumerate and SIGSTOP
whatever is newly there, then everything found gets SIGKILL and is polled until each pid is confirmed gone: exited,
replaced by an unrelated process on the same number, or a zombie nobody has reaped yet, all three of which mean it can
no longer run anything. Both bounds fail loudly rather than quietly: a fifth pass that still finds something new, or a
pid still alive when the poll gives up, makes the stop report a failure. A supervisor restart does not kill sessions,
and a host reboot takes everything down with the machine.

There is no ordering between parent and child: the wrapper and the agent are signalled in one pass over an unordered
set, and nothing waits for either before signalling the other. What is offered is up to five seconds between SIGTERM and
SIGKILL, and only to what the first enumeration found — something that first appears AFTER the grace period is picked up
by a later pass and gets SIGSTOP and SIGKILL without ever seeing a SIGTERM at all. The wait is not a fixed delay: it
ends as soon as everything signalled is gone, so exiting promptly is what makes a stop feel prompt.

For a wrapper author that means three things. A TERM handler has a budget of about five seconds of wall clock, and a
budget is not a guarantee: the timer starts when the signal goes out, not when your handler is scheduled, and a loaded
host or a slow disk spends it for you. Plan for less than you measure. Unlinking a marker or writing a small state file
fits; waiting on the child, syncing a large tree, or a network round trip does not. A handler still running when the
grace expires is killed mid-work — SIGSTOPped first on the sweep-only path, straight to SIGKILL under a scope — so
anything it writes must be crash-safe: write a temporary file and rename it, never update one in place. And an `flock`
is released by the kernel when its holder dies, whichever signal did it, so holding a lock on the directory for the
agent's lifetime needs no handler at all.

## Related shapes

`env FOO=1 claude {farhelm_args}` and `bash -c` are not wrappers in the sense above, and they need no `{cwd}` — the
session's terminal already starts in its directory and the command inherits it. `env` is for variables you do not mind
being public: the value is stored verbatim with the session and sits in the launch's argv, where `ps` shows it to anyone
who can read the process table for as long as the process runs. A secret belongs in a file the agent reads, not on a
command line. Declare the agent and write the resume command out as for any wrapper:
`env FOO=1 claude --resume {conversation} {farhelm_args}`.

`bash -c` needs one more step. `{farhelm_args}` written inside the script string is not a whole argument, so it does not
count, and a command whose only `{farhelm_args}` is there is refused. Pass it after the script as positional parameters
instead and forward them, with a dummy `$0` ahead of them exactly as the `sh` example does:
`bash -c 'claude "$@"' sh {farhelm_args}`. A script that names the agent and its arguments itself and does not forward
`"$@"` drops Farhelm's arguments, and the only sign is that the agent never reports its conversation.

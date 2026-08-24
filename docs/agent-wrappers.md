# Agent wrappers

Some environments do not start agents directly; they start a wrapper — `wrapper run <dir> <agent...>` — that `cd`s into
the directory, does whatever bookkeeping it manages there, and runs the agent as a child while staying resident as its
parent. Farhelm runs a profile like that like any other profile; there is no wrapper mode to turn on. Write `{cwd}`
where the wrapper wants the directory and farhelm substitutes the session's own directory at every launch and every
restart, so one profile serves every directory instead of one profile per directory. Set the profile's agent kind to the
agent the wrapper ends up running — the field says `generic` until you change it, and generic turns every integration
off. On stop the wrapper is killed along with the agent; being the agent's parent exempts it from nothing.

## A wrapper profile, field by field

Three fields, and all three matter:

- **invocation**: `my-wrapper run {cwd} claude`
- **agent kind**: `claude` — the agent the wrapper ends up running, never the wrapper itself
- **resume invocation**: `my-wrapper run {cwd} claude --resume {conversation}`

`{cwd}` is a whole word or it is nothing. An argument either equals `{cwd}` exactly, in which case it is replaced, or it
is passed through as literal text: `--dir={cwd}` reaches the wrapper unchanged, and so does a `{cwd}` written inside a
`sh -c` script string, since that is part of one argument rather than an argument of its own. The same rule
`{conversation}` has always had, for the same reason — substitution replaces a whole argument and never splices into
part of one. Beyond that: every occurrence is filled, not just the first; it may not be the first word, because
substituting there would make the working directory the program this session runs, and the profile editor refuses that
outright; and a directory whose name contains spaces arrives as a single argument, because the value goes into the
argument's own slot with no quoting for you to get right.

The value is the directory the session's terminal starts in, spelled the way you gave it after `~` expansion, symlinks
intact. On a restart — and on the retry of an interrupted create — farhelm checks that the spelling still resolves to
the canonical identity it recorded when the session was created and hands over that resolved path instead; a session old
enough to have no recorded identity has nothing to check against and gets the spelling again. Either way the wrapper is
handed exactly the string tmux is handed for the pane — one string, which is not quite one directory: on a fresh create
each side resolves that spelling for itself, so a symlink retargeted in between puts them in different places. That race
is yours rather than farhelm's, and the recorded identity closes it from the first restart onwards. It matters for
capture in one specific way: the record scan decides a conversation belongs to a session by comparing the `cwd` the
AGENT recorded against the session's canonical directory, so a wrapper that runs the agent somewhere else takes that
correlation with it. A hook-reported identity is authenticated by the session's credential and does not depend on the
directory at all.

## A worked example with `sh`

`sh` is a real wrapper of the minimal kind, and worth running once before you point a profile at the actual thing:

```
sh -c 'cd "$1" && shift && "$@"; exit $?' w {cwd} claude
```

`w` becomes `$0` for the inner shell, `{cwd}` lands in `$1` as a positional argument (not inside the script text, where
it would stay literal), and `"$@"` is the agent command line with whatever farhelm appended to it. The trailing
`; exit $?` is load-bearing: when `/bin/sh` is bash, the last command of a script is tail-`exec`ed as an optimization,
so `... && "$@"` on its own replaces the shell with the agent and leaves no resident wrapper at all. With a command
after it, bash and dash both stay put as the agent's parent, which is what a real wrapper does.

## Why the kind must be explicit

A profile always states its kind — the field is part of the profile, and `generic` is the spelling for "no kind".
Farhelm never second-guesses it: a generic profile whose invocation happens to start with `claude` stays generic,
because that is what you picked. Basename derivation is the RAW-create rule, for a session started from a command line
rather than a profile: the basename of the first word, exact equality, `claude` is Claude, `codex` is Codex, everything
else generic. It is deliberately dumb rather than clever (see [docs/agent-hook-injection.md](agent-hook-injection.md))
because the shapes it would have to be clever about — wrappers, `env`, a command buried in a `bash -c` script string —
cannot be recognized reliably, and the kind field is there so nothing has to try.

A wrapper profile left at generic launches and runs fine — no error, no warning. What it silently does not get is
conversation-identity capture, the hook flags, per-agent status sharpening, and the resume that follows from them.
Restart then offers a fresh launch in the same directory — unless the profile carries a resume invocation with no
`{conversation}` in it, in which case restart offers to run that command as written, `{cwd}` still filled. That is
SPEC.md's verbatim fallback, and it is the one thing a generic profile does still get.

## What the wrapper must pass through

Farhelm appends exactly one thing to the END of the agent command line: the hook flags described in
[docs/agent-hook-injection.md](agent-hook-injection.md), on a launch whose kind is integrated and whose argv qualifies —
that document's table lists the shapes that disqualify it (a bare `--` anywhere, an existing `--settings`, codex hook
configuration of your own), and a wrapper's own arguments are part of the argv those checks look at. A resume is not an
exception to any of that. The resume invocation is a complete command line in its own right — farhelm replaces its
`{conversation}` element and runs THAT instead of the launch invocation, which is why the wrapper has to appear in it
too — and the hook flags go on the end of it exactly as they go on the end of any other qualifying launch.

So the hook flags are what a wrapper must forward, and one that treats everything after its own arguments as the command
to run, verbatim, does. A wrapper that parses trailing options as its own eats them first, and the symptom is indirect:
the agent never reports its conversation, and the restart offer falls back to whatever the record scan can infer. NOTE:
that is the one thing farhelm cannot check for you. Its own tests stand in `sh -c` for the real wrapper, so whether YOUR
wrapper stops at the agent command and forwards the rest is something only you can verify — `ps -o args= -p <agent pid>`
against a live session shows what actually reached the agent.

Four variables travel in the environment rather than on the command line: `FARHELM_SESSION_ID` (which session this is —
no sweep will claim a process that does not carry it), `FARHELM_AGENT_ID` (the same session id again, under a name that
says this process belongs to the session's AGENT rather than to one of its terminal tabs; that is the marker a stop
selects on), `FARHELM_SESSION_TOKEN` (the bearer credential proving a spawn request came from this session), and
`FARHELM_SUPERVISOR_SOCK` (the supervisor socket to dial). A wrapper inherits all four and passes them to its child by
default, so this needs no thought unless your wrapper deliberately scrubs the environment.

Cosmetic, but worth knowing: a session created from a profile shows that profile's snapshotted NAME in the session list,
while a raw create shows the basename of the first word — the wrapper's. The agent-kind field, not the displayed name,
decides how farhelm treats the session.

## What happens on stop

There are fewer processes involved than the launch chain suggests. tmux starts the pane's login shell, the shell `exec`s
farhelm's launch shim, and the shim `exec`s your invocation — so the WRAPPER is the pane's own process and the agent is
its child. Where farhelm's own probe finds a WORKING systemd user manager — it creates a throwaway scope, looks it up,
kills it, and confirms it went away, rather than trusting a `systemd-run` on `PATH` — the launch also runs inside a
per-launch cgroup scope, which is containment rather than a level of the tree (`systemd-run --scope` execs in place
too). Each launch records what it selected, so a session can outlive the manager that scoped it.

Stopping a session and restarting it reap the agent's tree and leave the session's terminal tabs running; deleting or
archiving it takes the tabs too. What a stop claims is the pane process's descendants plus every process carrying the
session's `FARHELM_AGENT_ID` — the value is the session's own id, set on every agent launch, so the marker says "an
agent of this session" rather than "this generation of it" — and, for sessions launched by builds predating that marker,
anything wearing the session marker with no other claim on it. All of those additionally require the session's
`FARHELM_SESSION_ID`: one session's stop can never reach another's processes however their other markers read. A tab's
shell wears its own tab marker, which is what keeps it out.

Where this launch recorded a cgroup scope, the manager is still there, and that unit still exists, the scope is killed
first: SIGTERM to everything in it, about 500 ms, then SIGKILL, then a bounded wait for the unit to be collected. The
process-table sweep runs afterwards regardless — as the backstop there, and as the whole mechanism on a host with no
user manager. It SIGTERMs everything the first enumeration found, waits the same 500 ms, then RE-ENUMERATES and SIGSTOPs
the refreshed set: the refresh comes first so that a process forked inside a TERM handler is frozen along with
everything else rather than escaping with the parent. Stopped processes cannot fork, so what is left only shrinks — up
to five further passes re-enumerate and SIGSTOP whatever is newly there, then everything found gets SIGKILL and is
polled until each pid is confirmed gone: exited, replaced by an unrelated process on the same number, or a zombie nobody
has reaped yet, all three of which mean it can no longer run anything. Both bounds fail loudly rather than quietly: a
fifth pass that still finds something new, or a pid still alive when the poll gives up, makes the stop report a failure.
A supervisor restart does not kill sessions, and a host reboot takes everything down with the machine.

There is no ordering between parent and child: the wrapper and the agent are signalled in one pass over an unordered
set, and nothing waits for either before signalling the other. What is offered is about half a second between SIGTERM
and SIGKILL, and only to what the first enumeration found — something that first appears AFTER the grace period is
picked up by a later pass and gets SIGSTOP and SIGKILL without ever seeing a SIGTERM at all.

For a wrapper author that means three things. A TERM handler has a budget of about half a second of wall clock, and a
budget is not a guarantee: the timer starts when the signal goes out, not when your handler is scheduled, and a loaded
host or a slow disk spends it for you. Plan for less than you measure. Unlinking a marker or writing a small state file
fits; waiting on the child, syncing a large tree, or a network round trip does not. A handler still running when the
grace expires is killed mid-work — SIGSTOPped first on the sweep-only path, straight to SIGKILL under a scope — so
anything it writes must be crash-safe: write a temporary file and rename it, never update one in place. And an `flock`
is released by the kernel when its holder dies, whichever signal did it, so holding a lock on the directory for the
agent's lifetime needs no handler at all.

## Related shapes

`env FOO=1 claude …` and `bash -c 'claude …'` are not wrappers in the sense above, and they need no `{cwd}` — the pane
already starts in the session's directory and the command inherits it. `env` is for variables you do not mind being
public: the value is stored verbatim in the profile and sits in the launch's argv, where `ps` shows it to anyone who can
read the process table for as long as the process runs. A secret belongs in a file the agent reads, not on a command
line. What these two shapes share with a wrapper is the first-word problem, and for them one field does not fix it.
Setting the kind turns the integration on, but the DEFAULT resume invocation is derived from the invocation's first
word, so kind `claude` on `env FOO=1 claude` synthesizes `env --resume {conversation}` — a command that resumes nothing.
Write the resume invocation out yourself as well (`env FOO=1 claude --resume {conversation}`), or the first restart is
what finds out.

`bash -c` has a second problem, and the kind field does not reach it either. Appended flags become the shell's own
positional parameters — `$0` and the ones following it, two elements for Claude and five for Codex — rather than
arguments of the agent named inside the script string. Whether they reach the agent is up to the script: one ending in
`"$@"`, with a dummy `$0` ahead of the real arguments exactly as the `sh` example does, forwards them like any other
wrapper; `bash -c 'claude …'`, which names the agent and its arguments itself, drops them. Farhelm appends them and logs
that it did — it is looking at an argv, not at the contents of your script — so the only sign the agent never saw them
is the tripwire warning that no identity was reported. Capture falls back to the record scan and works as it did before
hooks existed. If you want the hook, either forward the positional parameters or make the agent the profile's own
command — `env`, or a real wrapper — rather than a string handed to `-c`.

# Agent hook injection

Farhelm injects a conversation reporter into Claude, Codex, Goose, and Pi launches so the agent tells farhelm which
conversation it is in. That is what makes "resume conversation" land in the conversation you were actually in after a
`/clear` or a `/new`, instead of the one you threw away. It writes nothing to your `~/.claude` or to Codex's
configuration home. For Claude and Codex, the flags ride on one command line and die with the process. On a Codex launch
that gets the flags, Codex prints one warning line about hook trust, and with that bypass in place any hook of your own
in that configuration home (`$CODEX_HOME` when it is set, `~/.codex` otherwise) that you have not trusted runs too. A
few Claude and Codex invocation shapes turn the injection off and fall back to the older record-scanning method.

Goose and Pi do not have a scanning fallback. Goose stores one credential-free named MCP reporter in conversation
metadata; a Farhelm resume supplies its current launch controls without declaring it again. A manual Goose resume needs
`farhelm` on `PATH` so that stored reporter can start, though it remains inert without Farhelm launch credentials. Pi
loads a versioned extension from Farhelm's private state. Its resume target includes the exact session file, which
Farhelm checks against the reported session ID immediately before restart; a missing, malformed, symlinked, or
mismatched file withdraws the stale resume offer instead of letting Pi silently start fresh.

## Why hooks at all

Farhelm's job on a restart is to bring back the conversation you were in, not just the agent. Until now it worked that
out from the outside, by watching which conversation file the agent created around the time you typed your first prompt.
That guess is right most of the time, but it is a guess, and it has one blind spot it cannot fix: when you run `/clear`
(Claude) or `/new` (Codex), the agent starts a brand-new conversation with a new id, and nothing on disk says "this
replaced that one". A restart would then resume the conversation you had just thrown away.

Claude and Codex offer a session-start hook — a command they run whenever a conversation begins — and the hook receives
the exact conversation id. Farhelm uses it purely as a messenger: the agent says "I am now in conversation X", and
farhelm stores that. Nothing else rides on the hook. No status, no control, no extra permissions.

Goose exposes the same fact as `AGENT_SESSION_ID` to its MCP extensions. Pi exposes it to extensions together with its
optional persisted session file. Their reporters feed the same authenticated supervisor message as the hooks, but they
never inspect or search the vendors' own state directories.

## The no-errors policy

The reporter must never be something you notice. The Claude and Codex hook and Pi's reporting subprocess print nothing
to your terminal, always exit successfully, give up after two seconds, and do no identity reporting outside a farhelm
session. Those two seconds cover reading the vendor's payload and reporting it — the part your agent is waiting on.
Writing the log line afterwards is best-effort and unbounded, but by then the agent already has its answer. Goose is
different because its credential-free reporter declaration persists in conversation metadata: outside Farhelm it still
starts as a valid empty MCP server, but without Farhelm launch credentials it reports nothing and exposes no tools.

If a reporter fails, the session itself is unaffected. Claude and Codex retain the older record-scan fallback; Goose and
Pi gain no new exact resume target. The one visible thing is the Codex warning line, and that is Codex talking, not the
reporter.

## Does my invocation get the hook?

| invocation shape                                                                              | kind farhelm derives | hook injected? | what you get instead                                                                                                                                                                                                                                                                  |
| --------------------------------------------------------------------------------------------- | -------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `claude <any flags>`                                                                          | Claude               | yes            | —                                                                                                                                                                                                                                                                                     |
| `claude --settings <x> …`                                                                     | Claude               | no             | the record scan — Claude honors only the LAST `--settings`, so injecting ours would silently drop yours                                                                                                                                                                               |
| `codex <any flags>`, `codex resume …`                                                         | Codex                | yes            | —                                                                                                                                                                                                                                                                                     |
| `codex --dangerously-bypass-hook-trust …`, `codex -c hooks.… …`, `codex -c features.hooks… …` | Codex                | no             | the record scan — you are already steering codex's hook configuration, and appending a second bypass flag could break the launch                                                                                                                                                      |
| `goose session …`                                                                             | Goose                | fresh only     | resumes use the reporter Goose already persisted; utility, help, ambiguous, and reporter-name-collision forms are left unchanged                                                                                                                                                      |
| `pi …`                                                                                        | Pi                   | yes            | the static extension reports the exact ID and optional persisted file; utility/help forms are left unchanged                                                                                                                                                                          |
| `claude … -- <prompt>`, `codex … -- <prompt>`                                                 | either               | no             | the record scan — after a bare `--`, our flags would become prompt text. This check runs ahead of the per-vendor ones, so it disqualifies both kinds alike                                                                                                                            |
| `/opt/bin/my-wrapper …`                                                                       | generic              | no             | no hook and no scan as written — write the directory as `{cwd}`, set the kind explicitly, and both come back; see [docs/agent-wrappers.md](agent-wrappers.md)                                                                                                                         |
| `env FOO=1 claude …`                                                                          | generic              | no             | no hook and no scan as written, and no `{cwd}` needed — set the kind, and write the resume invocation out by hand, since the derived default would be `env --resume …`                                                                                                                |
| `bash -c 'claude …'`                                                                          | generic              | no             | the record scan once you set the kind, and no hook as written: the flags are appended to the argv, so they land as the shell's `$0` and the following positional parameters rather than reaching the agent inside the script string — a script that forwards `"$@"` does pass them on |

One more case skips injection independent of invocation shape entirely: if farhelm's own absolute executable path (the
one every injected hook command would name) is not valid UTF-8, no kind gets the hook, for any invocation — see
"`farhelm executable path is not utf-8`" in the troubleshooting section below.

The wrapper path is absolute on purpose: farhelm does not expand `~` in an invocation. The fallback resume invocation
also has to be runnable as written — one carrying an unfilled `{conversation}` is refused rather than garbled, which
lands back on the fresh-launch offer. For the generic rows, setting the profile's agent kind is what turns the
integration back on; farhelm then appends the flags to the END of whatever argv the profile names, which only helps if
that argv's tail actually reaches the real agent. [docs/agent-wrappers.md](agent-wrappers.md) covers both halves.

## How the kind is decided

By the basename of the invocation's first word, compared for exact equality: `claude` is Claude, `codex` is Codex,
`goose` is Goose, `pi` is Pi, and everything else is generic. A path in front makes no difference (`/opt/bin/goose` is
still Goose); a decoration around it does (`goose-wrapper` and a raw `env FOO=1 goose` invocation are both generic).
That is deliberately dumb rather than clever, because a wrapper that silently inherited an integration would look
integrated and never capture anything. The profile's agent-kind field overrides the derivation, and is the supported way
to tell farhelm what your wrapper really launches. Once a structured launch identifies the kind, injection can preserve
a simple leading `env NAME=value …` prefix; option-bearing forms such as `env -i …` remain untouched.

## What the hook does

The farhelm binary itself is the hook, invoked as `farhelm internal hook --announce` by an absolute path — the flag is
present by default; see "Turning it off" below for the switch that removes it. It reads the agent's `SessionStart`
payload from stdin and forwards two things out of it: the conversation id, and the vendor's own `source` word —
`startup` on both vendors' first-launch payloads, whatever they choose to call the other events — which is carried for
diagnostics only: it shows up in the logs and nothing keys on it. Both go to the supervisor over the one socket the
supervisor listens on, `supervisor.sock` in its state directory, authenticated with the per-session credential already
in the session's environment. There is no per-session socket. It always exits 0 — including on a panic — and gives up
after two seconds, stdin read included. Outside a farhelm session there is no credential, so identity reporting exits
immediately and touches no socket — though if `--announce` was passed on the command line, the pointer line described
below still prints regardless, since it needs no credential at all.

It never prints a diagnostic, on either descriptor. It does print one deliberate line, on stdout, unless you have turned
that off: the pointer telling the agent that `$farhelm ...` in your message means the `farhelm agent` CLI and that
`farhelm agent instructions` explains it. Both vendors feed a `SessionStart` hook's plain-text stdout into the model's
context, which is the whole delivery mechanism — nothing is written to disk and nothing reaches your terminal. See the
"Talking to Farhelm from inside a session" in [old_readme.md](old_readme.md), and `FARHELM_AGENT_INSTRUCTIONS` below.

## What you will see

Claude: nothing new. The session row offers "resume conversation" within seconds of launch, before you have typed
anything, because Claude fires the hook at process start.

Codex: on the launches that get the flags, the `⚠ --dangerously-bypass-hook-trust is enabled` line above the composer,
and the resume offer only after your first prompt — Codex fires `SessionStart` at first prompt submission, not at
launch. After a `/new` the identity updates the same way, on your next prompt rather than immediately. This was verified
against Codex 0.149.0 and 0.149.1; hooks have shipped in Codex since well before that. On a version that accepts the
flags but does not fire the hook, nothing happens — the hook log stays absent, and the record scan carries the session
exactly as before hooks existed.

NOTE: with the bypass flag in place, any hook you have configured in Codex's active configuration home (`$CODEX_HOME`
when it is set, `~/.codex` otherwise) but have not trusted will also run during farhelm-launched Codex sessions. If that
is not what you want, turn injection off for Codex (below) and farhelm launches it without the bypass flag, falling back
to record scanning.

## Turning it off

`FARHELM_AGENT_HOOKS` in the supervisor's environment, read once when the supervisor starts:

- unset, empty, or `all` — every supported kind gets its reporter. The default.
- `none` — no kind gets its reporter.
- a comma-separated list of kinds, `claude`, `codex`, `goose`, and/or `pi` — only those kinds get it. Whitespace around
  each name is trimmed and case does not matter.

An unrecognized value is not honored in part: the supervisor warns, names the token it did not recognize, and behaves as
if the variable were unset. An opt-out with a typo in it must not quietly become "opt out of everything".

The variable only shapes command lines the supervisor builds after it has read it, so it changes nothing about agents
that are already running. A Codex session launched before you switched injection off keeps its bypass flag and keeps
reporting across every `/new` until that session is restarted.

Turning injection off also silences the instructions pointer for those launches, because every kind delivers the pointer
through the same integration that reports identity. To keep identity capture and drop only the pointer, use
`FARHELM_AGENT_INSTRUCTIONS` instead: `on` (the default, and what unset or empty means) or `off`, read once when the
supervisor starts, same as above. Anything else warns, names what you wrote, and behaves as if it were unset — a switch
whose off position removes a feature must not be flipped by a typo.

What you lose by turning `FARHELM_AGENT_HOOKS` off: Claude and Codex resume after `/clear` or `/new` goes back to the
old, scan-only behavior. Where the scan captured nothing or found an ambiguity, restart offers a fresh launch. Goose and
Pi have no scan, so disabling their reporter prevents new exact targets from being captured. It does not erase a target
already stored for the current launch.

## When something goes wrong

The symptom is a session that should offer "resume conversation" and does not, or that offers a stale one. Look in this
order.

**1. The per-session hook log**, `<state dir>/hook-log/<session id>.log`, where `<state dir>` is the supervisor's state
directory (`$XDG_STATE_HOME/farhelm`, or `~/.local/state/farhelm` by default). One line per hook run, shaped
`<unix-seconds> <outcome> [<detail> ]<conversation-id> <source>`; the trailing id and source appear only once the
payload has parsed — before that there is no id to name — and `<source>` is `-` when the vendor sent none or sent
something that is not a string. The outcome word is the whole diagnosis:

- `acked` — the report landed and the supervisor accepted it. The healthy case.
- `refused` — the supervisor said no; the detail carries its error kind and message. Some refusals appear nowhere else,
  so read this detail before the supervisor log.
- `no-credential` — the agent was not launched by farhelm: the flags were there, the session environment was not.
- `bad-payload` — nothing usable came out of stdin. The detail says where it went wrong: `oversized` or `unreadable` for
  the read itself, `no-reader: <error>` when the reader thread could not even be started, and `unparsable`,
  `missing-session-id`, `session-id-not-a-string`, `empty-session-id`, or `oversized-session-id` for the JSON. A vendor
  renaming the field shows up as `missing-session-id`.
- `connect-failed` — the round trip never completed; the detail is `<phase>: <error>`. `connect:` is the common one and
  usually means a supervisor that is not running; `handshake:` covers a protocol-version mismatch between the hook
  binary and the supervisor; `runtime:` is this process failing to build its own async runtime.
- `timeout` — the two-second budget ran out in the named phase (`stdin`, `connect`, `handshake`, `send`, `reply`). That
  report is lost and nothing retries it, but the next `SessionStart` for the session — a resume, another `/clear` —
  fires normally and can report again.
- `panic` — a bug in the hook. Worth reporting, and harmless to the session.

No file at all usually means the vendor never ran the hook: check that the flags reached the process (step 3), and for
Codex check whether you have typed a prompt yet. Two other things also leave no file. The path is derived from the
session id and the socket's directory, so a run missing either of those from its environment has nowhere to write and
cannot even leave its `no-credential` line. (A run missing only the token still writes one, which is deliberate: that is
exactly the half-configured case someone comes to this file for.) And logging is best-effort by contract — an
uncreatable directory, an unwritable path, or a full disk is ignored rather than turned into a failure.

**2. The supervisor log.** Every line here carries the session id.

- `conversation hook flags injected` — at launch, naming the kind and carrying `announce=true` or `announce=false` for
  whether `--announce` was included (`FARHELM_AGENT_INSTRUCTIONS`'s only visible effect on this log).
- `conversation hook flags not injected` — the skip and its reason: `invocation already passes --settings`,
  `invocation already configures codex hooks`, `invocation contains a bare --`, `disabled by FARHELM_AGENT_HOOKS`, or
  `farhelm executable path is not utf-8`. A generic session logs nothing — no integration means there was never a hook
  to skip. Every one of these launches still runs. Claude and Codex use the record scan for identity; Goose and Pi keep
  running without gaining a new exact target from that launch.
- `recorded the conversation identity this session's agent reported` — an accepted report, with the conversation and the
  vendor's `source` word. When it displaced a claim naming a DIFFERENT id, a second line says so:
  `this session's
  agent reported a conversation identity that replaces the one previously claimed for it`. A report
  that beats its own session's in-memory entry into existence is accepted too, and says so differently:
  `this session's agent reported a
  conversation identity before its entry was published; the durable row carries it until the entry appears`.
- Refusals are warn-level, and only some of them reach this log. Three shapes do: one starting
  `refused a reported conversation identity` (an id this build will not store), one starting
  `could not record the conversation identity` (the durable write failed, and nothing retries it), and two ending
  `the report is discarded` — one for a report about a launch that has since been replaced, one for a session row that
  could not be read at all. The rest are answered on the wire and logged nowhere here: a credential the store rejects or
  cannot validate, and a supervisor that is no longer recording. For those, the hook log's `refused` detail is the only
  record there is.
- `this session was launched with a conversation hook but none has reported` — the tripwire, once per launch, some way
  past the session's first input.

**3. `ps -o args= -p <agent pid>`** to confirm the flags reached the process. Claude's are `--settings` followed by a
JSON blob naming the farhelm binary; Codex's are `--dangerously-bypass-hook-trust` plus two `-c` overrides, one for
`features.hooks=true` and one for `hooks.SessionStart`.

In every one of these failure cases the session keeps working. The only thing at stake is which conversation the restart
offer points at: Claude and Codex fall back to the record scan, while Goose and Pi have no scan and gain no new exact
target from the failed or skipped reporter.

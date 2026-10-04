---
title: Agent hook injection
description: How Farhelm's conversation reporters get into each agent's launch, and how to turn them off.
sidebar:
  order: 70
---

Farhelm uses conversation reporters for Claude, Codex, Goose, Pi, OMP, and Grok so Resume lands in the conversation you
were actually in after a `/clear` or `/new`, instead of the one you threw away. Claude, Codex, Goose, Pi, and OMP get
their reporter from the launch Farhelm builds. Grok requires three user-installed hook entries because its supported
hook surface is configuration-based. Farhelm never edits that configuration for you.

For Claude and Codex, the injected flags ride on one command line and die with the process. On a Codex launch that gets
the flags, Codex prints one warning line about hook trust, and with that bypass in place any hook of your own in that
configuration home (`$CODEX_HOME` when it is set, `~/.codex` otherwise) that you have not trusted runs too. Farhelm puts
these flags where the launch's `{farhelm_args}` stands and never reads the rest of the command to decide whether they
fit (see [Which launches get a reporter](#which-launches-get-a-reporter)). Without a report, Farhelm cannot offer to
resume that new conversation.

Farhelm does not guess which conversation to resume from files on disk. See the harness notes for
[Goose's saved reporter and manual-resume dependency](/docs/agents/goose/),
[Pi's saved-file requirement and permission behavior](/docs/agents/pi/), and
[OMP's report-only contract and limits](/docs/agents/omp/).

[Cursor has basic launch support only](/docs/agents/cursor/): it uses no hook or status wrapper and has no conversation
tracking or automatic Resume.

[Grok's integration](/docs/agents/grok/) documents the required `SessionStart`, `UserPromptSubmit`, and `Stop` entries,
the `--no-leader` ownership requirement, and exact two-file verification. It has no scanning fallback.

## Why hooks at all

Farhelm's job on a restart is to bring back the conversation you were in, not just the agent. Until now it worked that
out from the outside, by watching which conversation file the agent created around the time you typed your first prompt.
That guess is right most of the time, but it is a guess, and it has one blind spot it cannot fix: when you run `/clear`
(Claude) or `/new` (Codex), the agent starts a brand-new conversation with a new id, and nothing on disk says "this
replaced that one". A restart would then resume the conversation you had just thrown away.

Claude and Codex offer a session-start hook — a command they run whenever a conversation begins — and the hook receives
the conversation id. Grok's three configured lifecycle hooks split the job: `SessionStart` selects the UUID, while
`UserPromptSubmit` and `Stop` can supply or refresh the exact saved-record path for that same selection. For Codex and
Grok, Farhelm also requires foreground-process attribution and vendor-owned record evidence: another process can inherit
the credential without becoming the conversation in your terminal. No status, control operation, or extra permission
rides on the identity report.

Goose exposes the same fact as `AGENT_SESSION_ID` to its MCP extensions. Pi exposes it to extensions together with its
optional persisted session file. OMP exposes it to extensions through separate session events — `session_start`,
`session_switch` (which carries the reason: a new, resumed, or forked conversation), `session_branch`, and `agent_end` —
and reports each conversation's exact id and, when one exists, its session file. Their reporters feed the same
authenticated supervisor message as the hooks, but they never inspect or search the vendors' own state directories.

OMP's reporter is loaded the way Pi's is: Farhelm materializes a private extension under its own state directory
(`integrations/omp/`, written with exact-bytes verification and private permissions) and loads it with `-e <path>` where
the launch's `{farhelm_args}` stands. Farhelm does not inspect the command, so a command declared as OMP that cannot
take the extension (a utility subcommand, or `--trusted-extension`, which OMP refuses to combine with `-e`) is yours to
fix. The same goes for an `--append-system-prompt` of your own: OMP keeps only the last one, and Farhelm's instructions
pointer arrives as one where `{farhelm_args}` stands, so put yours after `{farhelm_args}`, or turn the pointer off with
`FARHELM_AGENT_INSTRUCTIONS=off` (see [Turning it off](#turning-it-off)). The reporter executable is named by the
`FARHELM_OMP_REPORTER_EXE` environment variable, and the extension never reads or writes OMP's own state: what it knows
comes from the session events it subscribes to.

## The no-errors policy

The reporter must never be something you notice. The Claude, Codex, and manually configured Grok hooks, Goose's
reporter, and the Pi and OMP reporting subprocesses print nothing to your terminal, always exit successfully, and do no
identity reporting outside a Farhelm session. The hooks and Goose's reporter allow up to 30 seconds for reading the
vendor's payload and reporting it. If no supervisor is running, the hook keeps trying for about four seconds; if one is
running but busy, the hook waits up to the full 30 seconds. Pi and OMP keep their published 2-second child timers, so
their reporters can still be cut short while the hook is retrying; with no supervisor running, each report can now run
until that timer ends it. If the vendor kills that child, its final hook-log line may be absent; writing the line
afterwards is best-effort and unbounded, but by then the agent already has its answer. Goose is different because its
credential-free reporter declaration persists in conversation metadata: outside Farhelm it still starts as a valid empty
MCP server, but without Farhelm launch credentials it reports nothing and exposes no tools.

If a reporter fails, the session itself is unaffected. Farhelm gains no new conversation to resume. A new session whose
agent has not reported cannot be restarted until it does. The one visible thing is the Codex warning line, and that is
Codex talking, not the reporter.

## Which launches get a reporter

Codex has additional process-chain restrictions even when its flags are in place; see
[Codex launchers and wrappers](/docs/agents/codex/#launchers-and-wrappers). Grok has its own native process and
`--no-leader` requirements in [the Grok guide](/docs/agents/grok/#launching).

- **Claude, Codex, Goose, Pi or OMP picked in the session launcher** always gets its reporter, unless
  [you turned it off](#turning-it-off): Farhelm builds that command itself and knows where the reporter goes. Cursor,
  Muse and OpenCode have none.
- **A [custom command](/docs/agents/custom-commands/) that declares its agent** gets the declared agent's reporter where
  `{farhelm_args}` stands, which the command must contain exactly once. Farhelm does not read the rest of the command,
  so it does not notice a `--settings` of your own for Claude (Claude honors only the last one), Codex hook
  configuration of your own, an `--append-system-prompt` of your own before `{farhelm_args}` for OMP, or a bare `--`
  before `{farhelm_args}` (after which the flags would become prompt text); those are yours to avoid. A wrapper has to
  forward the arguments to the agent, as [Agent wrappers](/docs/agents/agent-wrappers/) describes.
- **A custom command with no declared agent** gets no reporter at all, whatever its first word is: Farhelm never decides
  from a command line which agent it runs.
- **Grok** reports through the three hooks you configure yourself in
  [the Grok guide](/docs/agents/grok/#configure-the-three-hooks); Farhelm injects no Grok hook.

A session started before this release from a typed command line or a profile keeps the rules it was created under:
Farhelm still adds its arguments where it used to, at the end of the command, and still leaves them off a Claude command
that passes `--settings`, a Codex command that configures hooks itself, and a command with a bare `--`.

## What the hook does

The farhelm binary itself is the hook, invoked as `farhelm internal hook --vendor <adapter> --announce` by an absolute
path for injected hooks — the announce flag is present by default; see "Turning it off" below for the switch that
removes it. Grok's manual command is `farhelm internal hook --vendor grok` without `--announce`. The `--vendor` flag
names which adapter this hook invocation is (Claude, Codex, Pi, or OMP from an injected command and Grok from its manual
configuration; the Goose helper supplies its own internally), so the supervisor can refuse a report addressed to a
session of another kind before consulting any vendor state. It reads one callback payload from stdin and forwards the
conversation id, the vendor's `source`, and any transcript path, event name, and subagent identity. Claude uses the
source for diagnostics. Codex requires `SessionStart` with source `startup`, `resume`, `clear`, or `compact`, plus
foreground attribution and exact-record validation. Grok requires `SessionStart`, `UserPromptSubmit`, or `Stop`; its
selecting event also carries source `new` or `load` and a timestamp used to reject delayed replacement reports. These
fields go over `supervisor.sock` in the supervisor's state directory, authenticated with the per-session credential
already in the launch environment. There is no per-session socket. The hook always exits 0 — including on a panic — and
uses one 30-second budget for stdin and the round trip. A live connection can use that budget while the supervisor is
briefly busy; a restart gap gets a separate short reconnect window before the hook gives up quietly. Outside a farhelm
session there is no credential, so identity reporting exits immediately and touches no socket — though if `--announce`
was passed on the command line, the pointer line described below still prints regardless, since it needs no credential
at all.

It never prints a diagnostic, on either descriptor. It does print one deliberate line, on stdout, unless you have turned
that off: the pointer telling the agent that `$farhelm ...` in your message means the `farhelm agent` CLI and that
`farhelm agent instructions` explains it. Claude and Codex feed a `SessionStart` hook's plain-text stdout into the
model's context, which is the whole delivery mechanism — nothing is written to disk and nothing reaches your terminal.
See the "Talking to Farhelm from inside a session" in
[old_readme.md](https://github.com/scode/farhelm/blob/main/docs/old_readme.md), and `FARHELM_AGENT_INSTRUCTIONS` below.

## What you will see

Claude: nothing new. The session row offers "resume conversation" within seconds of launch, before you have typed
anything, because Claude fires the hook at process start. Only a hook run by the session's own foreground Claude counts:
the pane process, or its direct child under a one-level wrapper. A `claude` that the session starts through its shell (a
shelled-out sub-agent) inherits the session's credential, but if it reports a conversation Farhelm refuses it, and the
hook log records a `refused conflict` line. A nested invocation cannot replace its parent's target.

Codex: on the launches that get the flags, the `⚠ --dangerously-bypass-hook-trust is enabled` line above the composer,
and the resume offer only after your first prompt — Codex fires `SessionStart` at first prompt submission, not at
launch. After a `/new` the identity updates the same way, on your next prompt rather than immediately. This was verified
against Codex 0.149.0, 0.149.1, and 0.155.1. On 0.155.1, `/clear` also switches the conversation at the next prompt,
while compaction retains it. On a version that accepts the flags but does not fire the hook, the hook log stays absent
and Farhelm gains no exact Codex resume target.

Codex's native executable must be named `codex`; a wrapper may launch it, but renaming the native executable makes
foreground attribution unavailable. Farhelm verifies the exact root transcript named by the hook, including when
`CODEX_HOME` points somewhere else. A nested persistent or ephemeral invocation cannot replace its parent's target.
After an attributed `/clear`, an unwritten transcript means no resume offer yet, not permission to resume the discarded
conversation. Farhelm waits for that exact file rather than searching for another one. Historical bare captured IDs
remain stored but are not treated as verified resume targets; no transcript is deleted or automatically selected
instead.

NOTE: with the bypass flag in place, any hook you have configured in Codex's active configuration home (`$CODEX_HOME`
when it is set, `~/.codex` otherwise) but have not trusted will also run during farhelm-launched Codex sessions. If that
is not what you want, turn injection off for Codex (below). Farhelm launches it without the bypass flag, but cannot
capture new exact resume targets without an attributable report.

OMP: the resume offer follows the extension's session-event reports. Typing `/new` starts a conversation that OMP 18.2.4
persists at once, so the fresh conversation can be resumable immediately, not only after a first assistant message.
Farhelm does not scan OMP's own state directory to guess at anything; what it knows about your conversations comes from
those reports alone — with one bounded exception: before a resume, Farhelm reads a bounded prefix of the exact session
file the report named, wherever it lives (usually inside OMP's state directory), to verify it still belongs to that
conversation. The sidebar shows no waiting status for OMP — an approval prompt is the ordinary running/idle
classification.

Grok: the manually configured `SessionStart` selects the UUID, normally leaving a fresh conversation pending until
`UserPromptSubmit` or `Stop` supplies its exact `updates.jsonl`. Resume appears only while that file and its sibling
`summary.json` both identify the selected UUID. See [the Grok guide](/docs/agents/grok/) for setup, the timestamp
ordering rule, and the accepted `/new` delivery race.

## Turning it off

`FARHELM_AGENT_HOOKS` in the supervisor's environment, read once when the supervisor starts:

- unset, empty, or `all` — every automatically configured kind gets its reporter. The default.
- `none` — no automatically configured kind gets its reporter.
- a comma-separated list of kinds, `claude`, `codex`, `goose`, `pi`, and/or `omp` — only those kinds get it. Whitespace
  around each name is trimmed and case does not matter. Grok is absent because this switch does not own its manual
  configuration.

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

Turning `FARHELM_AGENT_HOOKS` off prevents new conversations from being saved for Resume by Claude, Codex, Goose, Pi,
and OMP. A session with no saved conversation cannot be restarted. Turning the switch off does not erase a conversation
already saved for the current launch. Grok's manual hooks are independent of this switch; remove or disable those
entries in Grok itself when you want them off.

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
  renaming the field shows up as `missing-session-id`. Grok adds precise reasons for conflicting dual spellings and
  malformed event, source, timestamp, path, or child fields.
- `connect-failed` — the round trip never completed; the detail is `<phase>: <error>`. A missing or refused socket is
  retried for about four seconds before this line; a long-lived connection that drops gets a fresh short reconnect
  window, while repeated immediate drops remain within the current window. `connect:` is the common one and usually
  means a supervisor that stayed down; `handshake:` covers a protocol-version mismatch between the hook binary and the
  supervisor; `runtime:` is this process failing to build its own async runtime.
- `timeout` — the 30-second budget ran out in the named phase (`stdin`, `connect`, `handshake`, `send`, `reply`).
  Claude's, Goose's, and Pi's report may still be applied after the hook gives up; Codex's, Grok's, and OMP's is lost,
  but a later subscribed event may report again. For Claude and Codex that is a later `SessionStart`; Grok may also
  retry the selected UUID through `UserPromptSubmit` or `Stop`.
- `refused` — the supervisor answered and deliberately rejected the report. The admission failure is final and is not
  queued for a later write, but a later subscribed event may report again.
- `panic` — a bug in the hook. Worth reporting, and harmless to the session.

No file at all usually means the vendor never ran the hook: check that injected flags reached the process (step 3), or
that Grok loaded all three manual entries, and for Codex check whether you have typed a prompt yet. Two other things
also leave no file. The path is derived from the session id and the socket's directory, so a run missing either of those
from its environment has nowhere to write and cannot even leave its `no-credential` line. (A run missing only the token
still writes one, which is deliberate: that is exactly the half-configured case someone comes to this file for.) And
logging is best-effort by contract — an uncreatable directory, an unwritable path, or a full disk is ignored rather than
turned into a failure. Pi and OMP can also lose the line when the supervisor is down: their published two-second child
timer can kill the reporter while the hook is still retrying.

**2. The supervisor log.** Every line here carries the session id.

- `conversation hook flags injected` — at launch, naming the kind and carrying `announce=true` or `announce=false` for
  whether `--announce` was included (`FARHELM_AGENT_INSTRUCTIONS`'s only visible effect on this log).
- `conversation hook flags not injected` — the skip and its reason, usually `disabled by FARHELM_AGENT_HOOKS`. A session
  started before this release from a typed command line or a profile can also log
  `invocation already passes --settings`, `invocation already configures codex hooks`, or
  `invocation contains a bare --`. A session with no declared agent logs nothing — no integration means there was never
  a hook to skip. Every one of these launches still runs. Sessions keep running without gaining a new conversation to
  resume from that launch.
- `recorded the conversation identity this session's agent reported` — an accepted report, with the conversation and the
  vendor's `source` word. When it displaced a claim naming a DIFFERENT id, a second line says so:
  `this session's
  agent reported a conversation identity that replaces the one previously claimed for it`. A report
  that beats its own session's in-memory entry into existence is accepted too, and says so differently:
  `this session's agent reported a
  conversation identity before its entry was published; the durable row carries it until the entry appears`.
- Refusals are warn-level, and only some of them reach this log. Three shapes do: one starting
  `refused a reported conversation identity` (an id this build will not store), one starting
  `could not record the conversation identity` (the durable write failed, and the supervisor queues no speculative
  retry), and two ending `the report is discarded` — one for a report about a launch that has since been replaced, one
  for a session row that could not be read at all. The rest are answered on the wire and logged nowhere here: a
  credential the store rejects or cannot validate, and a supervisor that is no longer recording. For those, the hook
  log's `refused` detail is the only record there is.
- `this session was launched with a conversation hook but holds no conversation identity` — the tripwire, once per
  launch, 65 seconds after the first input if no conversation is saved. A resumed session with a saved conversation does
  not warn.

**3. Confirm the reporter is active.** Use `ps -o args= -p <agent pid>` for injected reporters. Claude's flags are
`--settings` followed by a JSON blob naming the farhelm binary; Codex's are `--dangerously-bypass-hook-trust` plus two
`-c` overrides, one for `features.hooks=true` and one for `hooks.SessionStart`. For Grok, use `/hooks` inside Grok and
confirm that the `SessionStart`, `UserPromptSubmit`, and `Stop` entries name the absolute Farhelm binary.

In every one of these failure cases the session keeps working. The only thing at stake is which conversation the restart
offer points at. Without an accepted report, a new session cannot be restarted. Previously saved conversations remain
available under the agent's usual Resume rules; a failed or skipped reporter supplies no new conversation to resume.

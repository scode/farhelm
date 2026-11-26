---
title: Custom agent contract
description: "Technical specification for implementing a Farhelm-compatible custom agent binary or wrapper script."
slug: docs/agents/custom-agents/contract
sidebar:
  order: 46
---

This specification defines the exact interface between Farhelm and a user-registered custom agent binary or wrapper
script. It is written for both engineers and coding agents implementing or updating a custom agent adapter.

:::caution[Stability notice]

This contract is at an early, minimal version and has **no stability guarantee yet**. Future Farhelm releases may add,
rename, or change flags and reporting rules, though needless churn will be avoided. When updating an existing script
after a Farhelm upgrade, read the [Contract changelog](#contract-changelog) below first.

:::

## Contract changelog

- **`v1` (initial release):** Executable path (`foo`, `/path/to/foo`, `~/path/to/foo`); optional `--model <value>` and
  `--effort <value>` (`[a-z0-9_-]+`); optional `--resume <id>` (used for both fresh named starts and Restart);
  conversation reporting via `farhelm custom-agent report-conversation --id <id>` on `PATH`; all custom agents treated
  as YOLO; no initial prompt flag; no `--clear` flag.
- **Hook log acknowledgments (2026-11-26):** the flags and the report command are unchanged. An `acked` line in the hook
  log now ends with a tab and `committed gen=<number>` when the supervisor saved the report, or `ignored` when it left
  the saved conversation unchanged. A check that matched the whole line `acked <id> -` should match its start instead
  and look for `committed`.

## 1. How Farhelm invokes your executable

The user configures one command string in **Settings → Custom agents**: a bare executable name resolved via `PATH` (for
example `acme-code`), an absolute path (`/opt/acme/bin/agent`), or a home-relative path (`~/bin/acme-code`, expanded
against the host supervisor's `HOME` at launch).

- **No shell parsing on the command itself:** Farhelm executes your program directly with an argument vector (`argv`),
  unless the user adds a wrapper prefix or launches it from the Command tab. Do not rely on shell word splitting inside
  the command setting; put any fixed arguments inside your wrapper script.
- **Working directory and environment:** Your executable starts in the session's working directory with the user's
  login-shell environment (including `PATH`). Farhelm prepends the directory containing its own `farhelm` binary to
  `PATH`, so invoking `farhelm` without an absolute path resolves to the build managing the session.
- **TTY and lifecycle:** Your executable runs attached to an interactive PTY. Keep the underlying agent interactive in
  the foreground. Prefer `exec` when handing off from a wrapper script so signals reach the agent cleanly. On **Stop**
  or **Restart**, Farhelm sends `SIGTERM`, waits up to 5 seconds, and then sends `SIGKILL`; exit promptly on `SIGTERM`.
- **Preserve `FARHELM_*` environment variables:** Do not strip `FARHELM_SESSION_ID`, `FARHELM_AGENT_ID`,
  `FARHELM_SESSION_TOKEN`, or `FARHELM_SUPERVISOR_SOCK` when launching the underlying agent or its hooks.

## 2. Command-line flags Farhelm may pass

Every flag below is passed as two separate `argv` elements (`["--flag", "value"]`). Only flags that apply to the launch
are passed; omitted values are never passed as empty strings. Parse flags in any order rather than assuming a fixed
position.

- **`--model <value>`** — Passed when the user, template, or CLI specifies a model; omitted when blank. Value is a
  non-empty string matching `^[a-z0-9_-]+$` (max 256 bytes). Pass the model selection to the underlying agent (or map it
  to the agent's own flag or environment variable).
- **`--effort <value>`** — Passed when the user, template, or CLI specifies an effort; omitted when blank. Value is a
  non-empty string matching `^[a-z0-9_-]+$` (max 256 bytes). Pass the reasoning-effort selection to the underlying agent
  (or map it to the agent's own flag or environment variable).
- **`--resume <id>`** — Passed only when **Supports session resumption** is ticked in Settings, and either (a) a new
  session is started with a known conversation ID, or (b) the user or an agent runs **Restart** on a session with a
  reported conversation ID. Value is an opaque non-empty printable ASCII string (max 128 bytes; see ID rules in Section
  3). Resume the named conversation interactively. **Important:** `--resume <id>` can arrive on a brand-new Farhelm
  session start, not only on Restart.

In **Restart with**, **Conversation ID** can supply `--resume <id>` directly when the agent supports resumption. This
also works when the session has yet to report a conversation.

**What Farhelm does NOT pass in v1:**

- No permission flags (no `--yolo` or `--guarded`). Farhelm treats every custom agent session as YOLO.
- No initial prompt argument.
- No injected hook arguments. On the Command tab, `{farhelm_args}` for a custom agent expands to zero arguments.

## 3. Reporting the active conversation

When **Supports session reporting** is ticked in Settings, your agent (or a hook/callback it invokes) reports its active
conversation ID to Farhelm by running:

```sh
farhelm custom-agent report-conversation --id <ID>
```

Call this command with plain `argv` (`["farhelm", "custom-agent", "report-conversation", "--id", id]`). No shell is
needed, and no quoting gymnastics are needed when invoked directly via `exec`/`spawn`. Calling it through a hook shell
command (`sh -c 'farhelm custom-agent report-conversation --id "$1"' _ "$id"`) is also supported.

### When to call it

- Call it once your agent has a conversation ID that can actually be resumed with `--resume <ID>`.
- Call it again whenever the active conversation changes inside the same session (for example, after the user starts a
  new conversation or clears the current one). The latest accepted report wins.
- Repeating the same ID (for example, at the end of every turn) is harmless and idempotent.
- There is no withdrawal or `--clear` command in v1. Until a new valid ID is reported, Farhelm keeps the last accepted
  ID.

### Conversation ID constraints

Farhelm rejects IDs that fail any of these rules:

- Must be non-empty printable ASCII, at most 128 bytes.
- Must not start with `-` (so it can never be mistaken for a CLI flag when passed back to `--resume`).
- Must not contain whitespace, quotes (`'`, `"`), backslashes (`\`), or control characters.
- Must not start with Farhelm's internal locator prefixes (`pi:`, `omp:`, `codex:`, `grok:`) or equal a Farhelm
  placeholder (such as `{conversation}` or `{cwd}`).

### Guarantees from Farhelm

- **Always exits 0 and stays silent:** `farhelm custom-agent report-conversation` writes nothing to stdout or stderr and
  exits 0 even on invalid input or internal error, so it never pollutes your agent's terminal or triggers a vendor hook
  error.
- **Works offline / during supervisor restarts:** It writes a small file into Farhelm's local state directory and
  returns immediately without waiting for the supervisor or desktop app. The supervisor picks up saved reports within
  about 2 seconds (or when it next starts).
- **Safe outside Farhelm:** If the session's `FARHELM_*` environment variables are absent (for example, when the user
  runs your wrapper outside Farhelm), the command exits 0 immediately without writing anything.

### Process-tree and nested-agent rules

Farhelm verifies that every report came from a process running inside this session's terminal under the current launch:

- The reporter must run as a descendant of the session's terminal process and inherit `FARHELM_SESSION_ID`,
  `FARHELM_SESSION_TOKEN`, and `FARHELM_SUPERVISOR_SOCK`. Do not run the reporter from an unrelated background daemon
  outside the session's process tree.
- **Nested-agent protection:** Farhelm pins the first process that successfully reports (skipping a narrow `sh -c` hook
  trampoline if present). A nested child agent spawned inside the session cannot overwrite the pinned parent's
  conversation ID. If a child reports before the parent does, a later report from an ancestor process in the same
  terminal corrects the pin back to the parent.
- If the underlying agent does not run hooks as children of its own main process (for instance, if it delegates hooks to
  a shared daemon outside the session), reports from that daemon will be refused because their process ancestry does not
  reach the session's terminal.

## 4. Reference wrapper script

Suppose your internal CLI `my-internal-cli` takes `-m <model>`, `--thinking <effort>`, resumes with
`my-internal-cli continue <id>`, and accepts a hook script via `--on-session-start <script>` that receives the session
ID as `$1`. A minimal compliant wrapper at `~/bin/acme-code` looks like this:

```bash
#!/usr/bin/env bash
set -euo pipefail

model=""
effort=""
resume_id=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model)
      model="${2:?missing value for --model}"
      shift 2
      ;;
    --effort)
      effort="${2:?missing value for --effort}"
      shift 2
      ;;
    --resume)
      resume_id="${2:?missing value for --resume}"
      shift 2
      ;;
    *)
      echo "acme-code: unexpected argument: $1" >&2
      exit 2
      ;;
  esac
done

# Hook helper invoked by my-internal-cli with the conversation ID as $1.
# In practice, point --on-session-start at a small static script beside this wrapper
# that runs: exec farhelm custom-agent report-conversation --id "$1"
hook_script="$(dirname "$0")/acme-code-report-hook"

args=()
[[ -n "$model" ]] && args+=(-m "$model")
[[ -n "$effort" ]] && args+=(--thinking "$effort")
args+=(--on-session-start "$hook_script")

if [[ -n "$resume_id" ]]; then
  exec my-internal-cli continue "$resume_id" "${args[@]}"
else
  exec my-internal-cli "${args[@]}"
fi
```

And the companion hook script at `~/bin/acme-code-report-hook` (`chmod +x`):

```sh
#!/bin/sh
# Safe outside Farhelm too: exits 0 silently when FARHELM_* env vars are absent.
exec farhelm custom-agent report-conversation --id "$1"
```

If your underlying agent already prints or exposes its conversation ID to the wrapper without needing a hook, the
wrapper itself can call `farhelm custom-agent report-conversation --id "$id"` directly before or while running the agent
(keeping in mind that the reporting process must stay the parent or a child inside the session's terminal).

## 5. Contract test cases and troubleshooting

Use the cases below to write unit tests for your wrapper script (for example, by stubbing the underlying CLI and
asserting the translated `argv`) or to check contract adherence by hand. Replace `<binary>` with your configured
command.

### Key test cases

- **Plain fresh start:** Command: `<binary>` Expectation: Starts a new interactive session in the current working
  directory using the agent's own default model and effort, with `FARHELM_*` environment variables preserved.
- **Model only:** Command: `<binary> --model fast_v2` Expectation: Starts a new interactive session with model `fast_v2`
  and default effort.
- **Effort only:** Command: `<binary> --effort high` Expectation: Starts a new interactive session with effort `high`
  and default model.
- **Model and effort in either order:** Command: `<binary> --model fast-v2 --effort high` and
  `<binary> --effort high --model fast-v2` Expectation: Both start a new interactive session with model `fast-v2` and
  effort `high` regardless of flag order.
- **Fresh start on a known conversation ID** (if _Supports session resumption_ is enabled): Command:
  `<binary> --resume conv-123` Expectation: Opens and resumes `conv-123` interactively even on a brand-new Farhelm
  session start, not only on Restart.
- **Resume combined with model and effort** (if _Supports session resumption_ is enabled): Command:
  `<binary> --model fast-v2 --effort high --resume conv-123` (and with `--resume conv-123` first) Expectation: Resumes
  `conv-123` with model `fast-v2` and effort `high`.
- **Conversation report inside Farhelm** (if _Supports session reporting_ is enabled): Action: Invoke
  `farhelm custom-agent report-conversation --id conv-123` from the agent or its hook with `FARHELM_*` intact.
  Expectation: Exits `0` with no stdout or stderr; the session's hook log records `written conv-123 -` followed within
  ~2 seconds by `acked conv-123 -`, then a tab and `committed gen=<number>`.
- **Conversation change within one session** (if _Supports session reporting_ is enabled): Action: Start a new
  conversation inside the same session (such as `/new` or `/clear`) and invoke
  `farhelm custom-agent report-conversation --id conv-456`. Expectation: Hook log records `written conv-456 -` and
  `acked conv-456 -` with `committed gen=<number>` after the tab; a later **Restart** invokes
  `<binary> ... --resume conv-456`.
- **Safe outside Farhelm** (if _Supports session reporting_ is enabled): Action: Run `<binary>` and its reporting hook
  with `FARHELM_SESSION_ID`, `FARHELM_SESSION_TOKEN`, and `FARHELM_SUPERVISOR_SOCK` unset. Expectation:
  `farhelm custom-agent report-conversation --id conv-123` exits `0` silently without writing files or showing errors in
  the agent UI.
- **Clean stop on `SIGTERM`:** Action: Send `SIGTERM` to the launched process. Expectation: The wrapper and underlying
  agent exit promptly (well within Farhelm's 5-second grace period before `SIGKILL`).

For the Restart step in the conversation-change test, enable both resumption and reporting. The supervisor can record
`acked` before the reporter records `written`, so match the two lines by conversation ID.

### End-to-end check and troubleshooting

1. **Register and launch in Farhelm:** add the agent under **Settings → Custom agents**, start a session from the
   launcher, and trigger whatever causes your agent to report its conversation ID.
2. **Inspect the hook log on the host:** read `~/.local/state/farhelm/hook-log/$FARHELM_SESSION_ID.log` (or under
   `$XDG_STATE_HOME/farhelm/hook-log/`):
   - `<ts> written <id> -` followed by `<ts> acked <id> -` with `committed gen=<number>` after a tab: report accepted;
     **Restart** should now be enabled (if **Supports session resumption** is ticked). `ignored` after the tab means the
     supervisor left the saved conversation unchanged.
   - `<ts> refused <kind> <reason> <id> -`: report rejected (for example, invalid ID format or process ancestry did not
     reach the session's terminal).
   - `<ts> bad-payload <reason>`: the reporter rejected its arguments or the ID before sending a report to the
     supervisor.
   - `<ts> no-credential`: the reporter ran without Farhelm's session environment variables (usually because a wrapper
     scrubbed `FARHELM_*` or the agent ran outside Farhelm).
   - No log file at all: the reporter command was never executed, or `FARHELM_SESSION_ID` / `FARHELM_SUPERVISOR_SOCK`
     were missing from its environment.
3. **Verify both resume paths:** click **Restart** on the running session and confirm it resumes the reported
   conversation, then open **+ (New session) → More options → Conversation ID** (or run
   `farhelm agent create --agent <id> --conversation <conv-id>`) and confirm a fresh session starts directly on that
   conversation ID.

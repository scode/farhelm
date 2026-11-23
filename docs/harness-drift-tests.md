# Real harness drift tests

Use this suite to check that the latest release of each agent CLI still accepts Farhelm's launch commands and that the
agents with conversation tracking still report a conversation Farhelm can restart. Run it deliberately on Linux. The
runner first upgrades each selected agent (see "Upgrading the agents first"). Each tracked agent then sends two small
prompts using your existing vendor login or provider credentials. A smoke check of Cursor or OpenCode opens the
interactive CLI without submitting a prompt.

```sh
python3 -B scripts/run-harness-drift.py --list
python3 -B scripts/run-harness-drift.py --harness codex --harness claude
python3 -B scripts/run-harness-drift.py
python3 -B scripts/run-harness-drift.py --no-upgrade --harness grok
python3 -B scripts/run-harness-drift.py --upgrade-only
```

With no selection the runner upgrades and checks every discovered harness. `--list` builds the e2e test binary and lists
cases without starting agents. These cases are ignored in normal nextest, CI, the release gate and the deflake sweep.

Each case starts the shipped launch command in its own supervisor, terminal host and empty working directory. A tracked
case asks the agent to remember a random code word and waits for its reply and an accepted conversation report. The case
then stops the session, records how many hook reports were accepted, and uses Restart with the saved resume arguments.
The resumed agent must recall the code word in a second reply. Farhelm must also accept a new hook report that leaves
the original resume arguments intact. The runner allows up to 720 seconds per case for startup, both turns and cleanup.
The launch-only cases check that the interactive screen or workspace trust prompt opens.

An agent may save a trust choice for the temporary directory. The Muse launcher and OMP can update themselves during
startup, so these cases disable their updaters for the test invocations using a launcher variable and a temporary OMP
configuration overlay.

## Upgrading the agents first

A drift pass only means something for the release users get when they install today, so a request to run the drift tests
includes upgrading the agents, and the runner does it by default. Before any case starts it runs each selected agent's
own updater (`claude update`, `codex update`, `opencode upgrade` and so on; the recipes are in
`scripts/harness_upgrades.py`) with stdin closed, and prints one line per agent: upgraded with both versions, already
latest, not installed, or UPGRADE FAILED with the updater's last line. Where the vendor's updater is known to break, the
recipe falls back to the vendor's documented installer. An agent with no recipe is an UPGRADE FAILED, so a new drift
case also needs one. The summary names the version each case ran against, and a failed upgrade makes the run exit
nonzero even when its case passed, because that pass is for a release users may no longer get.

`--no-upgrade` probes the installed versions without changing them. Use it to reproduce a failure on the version that
produced it, or when the agents were upgraded moments ago. `--upgrade-only` upgrades the selected agents and runs no
case, without building the test binary. An agent session that is already running keeps its old version until it is
restarted.

## Prerequisites

The test uses the vendor's existing credentials. Antigravity and Grok also need the conversation hooks described in the
[Antigravity setup guide](../website/src/content/docs/docs/agents/antigravity.md) and
[Grok setup guide](../website/src/content/docs/docs/agents/grok.md). Sign in and start a session with setup enabled
before retrying a case that needs those hooks. Tests use private Farhelm state and leave hook files alone. The
maintainer handles vendor sign-in and hook setup; the runner handles updates. An agent running this suite reports unmet
prerequisites as COULD NOT RUN and reports an UPGRADE FAILED line with the agent and the updater's reason. The rules in
`.agents/real-agent-tui.md` apply.

## Reading the result

- **PASSED** means all of that case's live checks ran and succeeded.
- **COULD NOT RUN** means a prerequisite was missing. The table gives the reason, such as a missing binary, a login
  screen or an absent hook file.
- **UPGRADE FAILED** means the agent's updater (and its fallback installer, if it has one) failed. The case still ran,
  against the version shown.
- **FAILED** means the CLI or test failed after starting. The retained run contains its output, source identity and
  `drift-verdict.json`. Keep that evidence before retrying.

The runner continues across these outcomes. It exits nonzero on any failure and when an explicitly selected harness
could not run. Unavailable harnesses in an all-harness run are reported without making a runnable harness's verdict
ambiguous. Directly running a matching ignored Rust test without the runner fails on missing prerequisites so libtest
cannot misreport them as a pass.

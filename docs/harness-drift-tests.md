# Real harness drift tests

Use this suite to check that the installed agent CLIs still accept Farhelm's launch commands and that the agents with
conversation tracking still report a conversation Farhelm can restart. Run it deliberately on Linux. Each tracked agent
sends one small prompt using your existing vendor login or provider credentials. A smoke check of Cursor or OpenCode
opens the interactive CLI without submitting a prompt.

```sh
python3 -B scripts/run-harness-drift.py --list
python3 -B scripts/run-harness-drift.py --harness codex --harness claude
python3 -B scripts/run-harness-drift.py
```

With no selection the runner checks every discovered harness. `--list` builds the e2e test binary and lists cases
without starting agents. These cases are ignored in normal nextest, CI, the release gate and the deflake sweep.

Each case starts the shipped launch command in its own supervisor, terminal host and empty working directory. A tracked
case checks for a real response to the prompt and a conversation report that Farhelm accepts for Restart. The case then
stops the session and uses Restart to start it again with the saved resume arguments. A successful Restart accepts fresh
input in the composer without submitting another turn. Checking the conversation identity shown by each resumed CLI is a
follow-up. The launch-only cases check that the interactive screen or workspace trust prompt opens.

An agent may save a trust choice for the temporary directory. The Muse launcher and OMP can update themselves during
startup, so these cases disable their updaters for the test invocations using a launcher variable and a temporary OMP
configuration overlay.

The test uses the vendor's existing credentials. Antigravity and Grok also need the manual hooks described in the
[Antigravity setup guide](../website/src/content/docs/docs/agents/antigravity.md) and
[Grok setup guide](../website/src/content/docs/docs/agents/grok.md). Sign in and install or update the hooks yourself
before retrying cases that need them. Tests use private Farhelm state and leave hook files alone. An agent running this
suite never signs in or accepts a vendor update (`.agents/real-agent-tui.md`) and never installs or edits hooks; it
reports the case as COULD NOT RUN and leaves those steps to the maintainer.

## Reading the result

- **PASSED** means all of that case's live checks ran and succeeded.
- **COULD NOT RUN** means a prerequisite was missing. The table gives the reason, such as a missing binary, a login
  screen or an absent manual hook.
- **FAILED** means the CLI or test failed after starting. The retained run contains its output, source identity and
  `drift-verdict.json`. Keep that evidence before retrying.

The runner continues across these outcomes. It exits nonzero on any failure and when an explicitly selected harness
could not run. Unavailable harnesses in an all-harness run are reported without making a runnable harness's verdict
ambiguous. Directly running a matching ignored Rust test without the runner fails on missing prerequisites so libtest
cannot misreport them as a pass.

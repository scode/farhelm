# Real harness drift tests

Use this suite to check that the latest release of each agent CLI still accepts Farhelm's launch commands and that the
agents with conversation tracking still report a conversation Farhelm can restart. Run it deliberately on Linux. The
runner first upgrades each selected agent (see "Upgrading the agents first"). Each tracked agent then sends two small
prompts using your existing vendor login or provider credentials. A smoke check of Cursor or OpenCode opens the
interactive CLI without submitting a prompt.

```sh
python3 -B scripts/run-harness-drift.py --list
python3 -B scripts/run-harness-drift.py --list --category base --harness codex
python3 -B scripts/run-harness-drift.py --harness codex --harness claude
python3 -B scripts/run-harness-drift.py
python3 -B scripts/run-harness-drift.py --no-upgrade --harness grok
python3 -B scripts/run-harness-drift.py --upgrade-only
```

With no selection the runner upgrades and checks every harness in the `base` category. Use `--category <name>` to select
a category, repeat it to select several, or pass `--category all` to run every discovered category. Combine it with
`--harness <name>` to narrow the selection. Each selected harness upgrades once, even when it runs in several
categories.

`--list` builds the e2e binary and prints the discovered `category/harness` pairs without starting agents or upgrading
them. It lists all categories by default and accepts the same filters. These cases are ignored in normal nextest, CI,
the release gate and the deflake sweep. The Rust entry points live in the category modules under
`crates/farhelm/tests/e2e/real_agent_capture/drift/`.

Each case starts the shipped launch command in its own supervisor, terminal host and empty working directory. A tracked
case asks the agent to remember a random code word and waits for its reply and an accepted conversation report. The case
then stops the session, records how many hook reports were accepted, and uses Restart with the saved resume arguments.
The resumed agent must recall the code word in a second reply. Farhelm must also accept a new hook report that leaves
the original resume arguments intact. The launch-only base cases check that the interactive screen or workspace trust
prompt opens. Every category has up to 720 seconds per harness, including startup, checks and cleanup.

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
produced it, or when the agents were upgraded moments ago. `--upgrade-only` upgrades the selected harnesses without
building the test binary. It accepts `--harness` and rejects `--list` and `--category`. An agent session that is already
running keeps its old version until it is restarted.

## Hook setup in a private copy of the vendor's configuration

Grok and Antigravity find Farhelm's conversation hooks in files in their own configuration (`~/.grok/hooks/farhelm.json`
and `~/.gemini/config/plugins/farhelm/`), which Farhelm writes when a launch has the user's consent to set them up. A
drift case runs that same setup, because it is part of what users get, but never against your real files: they name the
`farhelm` that wrote them, which in a drift run is the build under test, and every session of that agent on the machine
runs them, inside your installed Farhelm or not.

So each such case builds a private copy of the vendor's directory (`GROK_HOME` for Grok, the whole `HOME` for
Antigravity). Every entry in it is a symlink to your real one, except the files Farhelm's setup writes and the
directories above them. The agent keeps its login, settings and sessions. Farhelm's real setup writes its files into the
copy, and the agent is started with the copy's variable set, also on Restart. The case fails if any of your real hook
files changed. A vendor that saves a file by renaming a new copy over it (a refreshed login, say) replaces the copy's
symlink rather than writing your file; the case copies such a file back afterwards when your real one is unchanged, and
prints a `drift:` note naming any it could not.

Which files to keep private comes from Farhelm's setup itself, so an agent that gains persistent hooks is covered
without listing its paths again, as long as they lie under the home directory or Grok's. A guard test
(`real_agent_capture::drift::private_config::tests`, part of the ordinary e2e run) fails for one whose paths do not, or
that has no drift profile.

## Prerequisites

The test uses the vendor's existing credentials. You do not need to set up any agent's hooks first; the runner handles
updates and each case handles hook setup as described above. An agent running this suite reports a missing login as
COULD NOT RUN and reports an UPGRADE FAILED line with the agent and the updater's reason. The rules in
`.agents/real-agent-tui.md` apply.

## Reading the result

- **PASSED** means all of that case's live checks ran and succeeded.
- **COULD NOT RUN** means a prerequisite was missing. The table gives the reason, such as a missing binary or a login
  screen.
- **UPGRADE FAILED** means the agent's updater (and its fallback installer, if it has one) failed. The case still ran,
  against the version shown.
- **FAILED** means the CLI or test failed after starting. The retained run contains its output, source identity and
  `drift-verdict.json`. Keep that evidence before retrying.

The runner continues across these outcomes. It exits nonzero on any failure and when an explicitly selected harness
could not run. Unavailable harnesses in an all-harness run are reported without making a runnable harness's verdict
ambiguous. Directly running a matching ignored Rust test without the runner fails on missing prerequisites so libtest
cannot misreport them as a pass.

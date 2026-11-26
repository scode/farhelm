# Real harness drift tests

Use this suite to check that the latest release of each agent CLI still accepts Farhelm's launch commands and that the
agents with conversation tracking still report a conversation Farhelm can restart. Run it deliberately on Linux. The
runner first upgrades each selected agent (see "Upgrading the agents first"). In the default category each tracked agent
then sends two small prompts using your existing vendor login or provider credentials, and Cursor and OpenCode open
their interactive CLI without submitting a prompt.

```sh
python3 -B scripts/run-harness-drift.py --list
python3 -B scripts/run-harness-drift.py --list --category base --harness codex
python3 -B scripts/run-harness-drift.py --harness codex --harness claude
python3 -B scripts/run-harness-drift.py
python3 -B scripts/run-harness-drift.py --no-upgrade --harness grok
python3 -B scripts/run-harness-drift.py --no-upgrade --category model_selection
python3 -B scripts/run-harness-drift.py --no-upgrade --category named_conversation
python3 -B scripts/run-harness-drift.py --no-upgrade --category instructions_delivery
python3 -B scripts/run-harness-drift.py --upgrade-only
```

With no selection the runner upgrades and checks every harness in the `base` category. Use `--category <name>` to select
a category, repeat it to select several, or pass `--category all` to run every discovered category. Combine it with
`--harness <name>` to narrow the selection. Each named harness must have a case in at least one selected category. Each
selected harness upgrades once, even when it runs in several categories.

`--list` builds the e2e binary and prints the discovered `category/harness` pairs without starting agents or upgrading
them. It lists all categories by default and accepts the same filters. These cases are ignored in normal nextest, CI,
the release gate and the deflake sweep. The Rust entry points live in the category modules under
`crates/farhelm/tests/e2e/real_agent_capture/drift/`.

Each case starts the shipped launch command in its own supervisor, terminal host and empty working directory. A tracked
base case asks the agent to remember a random code word and waits for its reply and an accepted conversation report. It
then stops the session, records how many hook reports were accepted, and uses Restart with the saved resume arguments.
The resumed agent must recall the code word in a second reply. Farhelm must also accept a new hook report that leaves
the original resume arguments intact. The Codex, Antigravity and Grok checks require a committed report for the resumed
generation and its conversation ID. The launch-only base cases check that the interactive screen or workspace trust
prompt opens. Every category has up to 720 seconds per harness, including startup, checks and cleanup.

The `model_selection` category launches the current catalog models and checks which model the agent shows as active.
Goose, Pi and OMP also start without provider or model flags and check that the agent shows an active model. Their
explicit OpenRouter models need `OPENROUTER_API_KEY`. The no-flag check still runs without it, but the case is then
reported as COULD NOT RUN.

A Codex retirement prompt can appear before the composer opens. The affected model is reported as unavailable when the
prompt asks the user to choose between keeping it and switching to its replacement.

The `reasoning_effort` category launches each harness at the boundary efforts its current catalog models accept (Off,
Minimal, Xhigh and Max, or High where none of those is offered) and checks the effort the agent shows as active. One
enabled effort per harness also completes a real turn. Goose shows no effort, so its case checks only that Farhelm
passes the effort to Goose and that Goose reaches its prompt and answers; a pass does not confirm the effort the
provider applied. Goose, Pi and OMP need `OPENROUTER_API_KEY` for the whole case and report COULD NOT RUN without it.

The `permission_modes` category opens the CLI with the launcher's approval choices and checks Farhelm's live YOLO
indicator where the integration reads one. Tracked agents complete one text turn with Custom permission flags. The test
checks that Restart keeps those arguments in order and that the new process reaches its composer.

The `named_conversation` category seeds a conversation from each test workspace, kept in the vendor's own store like any
other, and checks that a launch with an explicit ID or file recalls the seed. Pi and OMP exercise both absolute and
relative file paths. Goose reopens a conversation with a reporter Farhelm saved when it started that conversation. Both
named Create and named Restart must recall the seed, capture the selected conversation and make plain Restart available.
The named Restart starts from a different captured conversation so the test can prove the previous capture is gone.

The `initial_prompt` category starts each agent that accepts a launch prompt with a two-line prompt whose first line
begins with `--`, checks that the process received it as one argument, and checks the reply, which needs both lines. For
the agents whose conversations Farhelm tracks, Restart must then reopen the conversation without the prompt in its
arguments, so the prompt is not sent again. Where the agent also accepts a launch prompt when it opens a named
conversation, another launch prompt must recall the seed turn's word. Cursor's fresh launch still runs without a
selector. The case allocates a separate chat ID with `cursor-agent create-chat` and starts a seed turn before testing
named recall on that chat.

The `capture_timing_hooks` category checks whether Restart becomes available before or after the first text turn. It
looks for the installed vendor's turn callbacks in the hook log and checks that they stay off the screen. A harness that
reports at first prompt must show its matching prompt or start report before its first matching Stop. Each harness also
gets a fresh supervisor with hooks disabled. Codex still needs its daemon-ownership option, and Goose's Restart must
keep its reporter disabled even when the session already has a saved conversation.

The `workspace_trust` category launches each agent that offers a trust choice in a fresh directory with that choice and
never answers a trust question: a trusted launch has to reach the composer on its own. Codex and Pi also get an explicit
untrusted launch in a second directory. The case then checks the effect where the agent shows it: Codex's process
carries the trust argument, Pi loads a project extension only when trusted, and Farhelm records the trust in Claude's
configuration, which the case reads from a private copy (see below).

The `instructions_delivery` category asks an agent in two fresh sessions which command its startup context gives for
`$farhelm`. The enabled run must name the command, and the disabled run must answer that it received no command. Both
sessions must report a conversation Farhelm can reopen.

An agent may save a trust choice for the temporary directory. Claude, the Muse launcher and OMP can update themselves
during startup, so these cases disable their updaters for the test invocations using an environment variable for Claude
and Muse and a temporary OMP configuration overlay.

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
symlink. The case copies that file back only after it has confirmed each session's deletion and terminal-host shutdown,
and only if your original is unchanged. A create that failed to return a session keeps cleanup unconfirmed. This proof
covers the processes Farhelm can settle during Delete. If cleanup is unconfirmed or the original changed, the case
reports the dropped replacement. A failed copy-back makes the case fail after it has tried the remaining files and
checked the real hook files. Files the vendor creates inside the copy's private directories are dropped with it.

Claude's workspace trust case uses a private copy too, for a different reason: Farhelm records the trust in Claude's
configuration file, `~/.claude.json` or the one under `CLAUDE_CONFIG_DIR`, so the case starts Claude with a private home
holding its own copies of Claude's possible configuration files, with an empty place for their `backups` directory. The
fixture clears workspace and ancestor trust from the private copies so the launch must add its own workspace entry.
Those copies and any backups Claude writes are discarded with the case. A new trust decision in your real config for the
test workspace fails the case. Other concurrent changes to your real config are reported in a `drift:` note.

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
- **COULD NOT RUN** means a prerequisite was missing, or a vendor prompt the test leaves for the vendor's user to answer
  stood in the way, such as Codex's model retirement notice. The table gives the reason, such as a missing binary or a
  login screen. A model selection case reports it when any of its models could not run, even when the others were
  confirmed; the retained run names each model's outcome.
- **UPGRADE FAILED** means the agent's updater (and its fallback installer, if it has one) failed. The case still ran,
  against the version shown.
- **FAILED** means the CLI or test failed after starting. The retained run contains its output, source identity and
  `drift-verdict.json`. Keep that evidence before retrying.

The runner continues across these outcomes. It exits nonzero on any failure and when an explicitly selected harness
could not run. Unavailable harnesses in an all-harness run are reported without making a runnable harness's verdict
ambiguous. Directly running a matching ignored Rust test without the runner fails on missing prerequisites so libtest
cannot misreport them as a pass.

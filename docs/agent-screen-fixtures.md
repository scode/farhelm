# Agent screen fixtures

NOTE: This is a manual development tool. Nothing in CI runs it, and it spends real turns on the vendor logins of the
host it runs on.

The supervisor tells whether a Claude Code or Codex session is working, waiting on the user, or idle by recognizing what
the agent draws in its terminal. That recognition is only as good as the screens it was written against, and vendors
change their TUIs often. `crates/farhelm-supervisor/tests/fixtures/screens/` holds real screens captured from real
agents, each named for the state the agent was actually in, and the supervisor's tests hold the screen readers to them.

## Re-capturing after an agent upgrade

Run this on a development host after `claude` or `codex` was upgraded:

- `cargo build -p farhelm` once, if the checkout has no built `farhelm` yet (the tool reads `target/debug/farhelm`, or
  `$CARGO_TARGET_DIR/debug/farhelm`; `--farhelm` names another).
- `python3 scripts/capture-agent-screens.py` (add `--harness claude` or `--harness codex` for just one).

The tool launches each installed agent in a real Farhelm terminal host the size of a production terminal, as a
supervisor starts an agent in a new terminal: the supervisor's own emulator answers the agent's startup queries, and the
agent gets the environment a supervisor gives it (`TERM_PROGRAM=farhelm` and the rest). It drives each agent through its
trust dialog, an idle prompt, a turn that thinks and then runs a slow shell command behind a real permission prompt, the
finished turn, the agent's own idle-time decorations (Claude's `/clear` hint, a Codex recap), and a question form.
Claude then runs `/compact`; its in-progress spinner is saved as `working-compacting.{txt,title}`. The wait accepts
timers past one minute, though the saved frame may be earlier. It writes every screen, the visible screen as text as the
status sampler reads it, to `<harness>/<version>/<expected>-<scenario>.txt`, with the terminal's title beside it in a
`.title` file, and then runs the fixture tests through `scripts/record-test-run.py`.

A new version directory whose tests pass means nothing needs shipping. A failing test names the harness, version, and
scenario of the screen that broke it, which is the screen whose rules need updating. Commit the new version directory
either way, so the next vendor change is compared against current screens.

Versions captured before November 2026 were captured in a private tmux pane (`capture-pane -p`), with the environment an
old terminal gives its agent (Claude, for one, shows a hint about scrolling in tmux there). Keep those directories:
sessions in old terminals, and agents started in a new terminal before the upgrade that changed its environment, still
draw those screens until they are restarted.

Each scenario is best effort. If the agent cannot be driven into a state on this host (Codex reports its question tool
unavailable in its default mode, or an agent whose configuration already trusts `/tmp` shows no trust dialog for the
scratch directory), the tool reports it as not captured and keeps going. Hand-made screens that the tool cannot reach,
such as a rate-limit footer that only appears with a different login, are named `*-derived-*` and survive re-capture,
and so does an earlier capture of any scenario a run missed.

## What a run touches

- A few short turns per agent. Claude runs with `--model sonnet` in its default permission mode; Codex runs with its
  configured model in a read-only sandbox asking on request.
- Each vendor records a trust decision for the run's scratch directory in its own configuration (`~/.claude.json`,
  `~/.codex/config.toml`), exactly as the `#[ignore]`d real-agent tests do.
- Nothing of a live Farhelm's. The terminal host is started from the built `farhelm` into a fresh random directory under
  `/tmp`, and the agent gets no `FARHELM_*`, `TMUX` or `TMUX_PANE` variable of the caller's, nor the markers of an agent
  session the tool runs in (`OUTER_SESSION_MARKERS` in `scripts/agent_screen_host.py`).

## Personal data

The fixtures are committed to a public repository. The tool replaces the home directory, user name, host name, Git
author name, email addresses, and URLs that point into the capture machine or its private network (a local proxy an
agent warns about, say) before writing a screen, and refuses a screen that still contains any of them.
`screen_fixtures_carry_no_personal_data` checks the committed files again independently. Still read the diff before
committing: a custom status line or an account banner can carry something neither check knows about.

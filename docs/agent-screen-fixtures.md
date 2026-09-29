# Agent screen fixtures

NOTE: This is a manual development tool. Nothing in CI runs it, and it spends real turns on the vendor logins of the
host it runs on.

The supervisor tells whether a Claude Code or Codex session is working, waiting on the user, or idle by recognizing what
the agent draws in its pane. That recognition is only as good as the screens it was written against, and vendors change
their TUIs often. `crates/farhelm-supervisor/tests/fixtures/screens/` holds real screens captured from real agents, each
named for the state the agent was actually in, and the supervisor's tests hold the screen readers to them.

## Re-capturing after an agent upgrade

Run this on a development host after `claude` or `codex` was upgraded:

- `scripts/build-pinned-tmux-ci.sh` once, if `.ci-tmux/tmux` does not exist yet.
- `python3 scripts/capture-agent-screens.py` (add `--harness claude` or `--harness codex` for just one).

The tool launches each installed agent in a private tmux server the size of a production pane, drives it through its
trust dialog, an idle prompt, a turn that thinks and then runs a slow shell command behind a real permission prompt, the
finished turn, the agent's own idle-time decorations (Claude's `/clear` hint, a Codex recap), and a question form. It
writes every screen to `<harness>/<version>/<expected>-<scenario>.txt`, with the pane title beside it in a `.title`
file, and then runs the fixture tests through `scripts/record-test-run.py`.

A new version directory whose tests pass means nothing needs shipping. A failing test names the harness, version, and
scenario of the screen that broke it, which is the screen whose rules need updating. Commit the new version directory
either way, so the next vendor change is compared against current screens.

Each scenario is best effort. If the agent cannot be driven into a state on this host (Codex reports its question tool
unavailable in its default mode), the tool reports it as not captured and keeps going. Hand-made screens that the tool
cannot reach, such as a rate-limit footer that only appears with a different login, are named `*-derived-*` and survive
re-capture, and so does an earlier capture of any scenario a run missed.

## What a run touches

- A few short turns per agent. Claude runs with `--model sonnet` in its default permission mode; Codex runs with its
  configured model in a read-only sandbox asking on request.
- Each vendor records a trust decision for the run's scratch directory in its own configuration (`~/.claude.json`,
  `~/.codex/config.toml`), exactly as the `#[ignore]`d real-agent tests do.
- Nothing of Farhelm's. The tmux server is private, on a random socket under `/tmp`, and uses the pinned tmux.

## Personal data

The fixtures are committed to a public repository. The tool replaces the home directory, user name, host name, Git
author name, and email addresses before writing a screen, and refuses a screen that still contains any of them.
`screen_fixtures_carry_no_personal_data` checks the committed files again independently. Still read the diff before
committing: a custom status line or an account banner can carry something neither check knows about.

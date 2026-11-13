# Driving real agent TUIs

How to find out what a real, installed agent (`claude`, `codex`, `cursor-agent`, `goose`, `grok`, `muse`, `omp`,
`opencode`, `pi`) actually does in a terminal: which prompts it shows at start, what a screen looks like, how it reacts
to a key. Reach for this whenever a claim about an agent's behavior matters and nothing in the repo already proves it,
rather than trusting SPEC.md, a docs page, or memory of how the agent used to behave. Vendors change their TUIs often.

NOTE: This is not a test suite and nothing in CI runs it. It drives real programs on this machine's vendor logins, so
submitting a prompt spends a real turn. Recapturing the supervisor's screen-reader fixtures is a separate, dedicated
job: `scripts/capture-agent-screens.py`, described in `docs/agent-screen-fixtures.md`.

## The tool

`scripts/agent-tui.py` runs one program in a real Farhelm terminal host, as a supervisor runs an agent (the supervisor's
emulator answering the program's startup queries, the environment a supervisor gives an agent), from a fresh random
directory under `/tmp`, and lets you read and type at it. It needs the built `farhelm` (`cargo build -p farhelm`;
`--farhelm` on `start` names another). It never touches the live install (see "The live install is off-limits" in
AGENTS.md): no helm, no supervisor, no session of the maintainer's is involved, only the agent binaries and the built
`farhelm` itself. It also starts the program without `TMUX`, `TMUX_PANE`, any `FARHELM_*` variable, or the markers an
enclosing agent session sets for its children (`CLAUDECODE` and the rest of `OUTER_SESSION_MARKERS` in
`scripts/agent_screen_host.py`), which matters because an agent session usually runs inside Farhelm itself: an agent
under test that inherited those would talk to the live helm, or believe it runs below the agent that started the tool.

```sh
run=$(scripts/agent-tui.py start --cwd "$scratch" -- cursor-agent) || exit 1
scripts/agent-tui.py settle "$run"                 # wait until the screen stops changing; prints it
scripts/agent-tui.py wait "$run" 'Trust' --timeout 30   # or wait for specific text; prints the screen
scripts/agent-tui.py type "$run" 'hello'           # literal text, not submitted
scripts/agent-tui.py keys "$run" Enter             # named keys: Enter, Escape, Down, C-c
scripts/agent-tui.py screen "$run"                 # the visible grid, as Farhelm's sampler sees it
scripts/agent-tui.py stop "$run"                   # always: ends the program, removes the run directory
```

`scripts/agent-tui.py self-test` checks the tool itself against a small shell program, the environment scrub included.
Run it after changing the script.

## Doing it right

- Start the agent the way Farhelm would. The command line Farhelm builds for each agent and choice is pinned in the
  tests at the bottom of `crates/farhelm-helm/src/launches.rs` (`cursor-agent --force` for YOLO,
  `env GOOSE_MODE=auto goose session`, `grok --no-leader`, `opencode --auto`, and so on). An agent started with
  different flags can behave differently, and that is the whole point of looking.
- Use a fresh, empty scratch directory under your `/tmp/farhelm-tmp/<id>/` for every run that cares about first-run
  behavior. Agents remember per-directory decisions such as workspace trust, so a reused directory hides the prompt you
  were looking for.
- Answering an agent's prompt has side effects outside the run: a trust answer lands in the vendor's own configuration
  (`~/.claude.json`, `~/.codex/config.toml`, and the like) for that directory. Observe without answering when the
  question is only "does it ask?"; answer only when the test needs what comes after.
- Wait on the screen, never on the clock. `wait` for text you expect, or `settle` when you want whatever the agent shows
  once it is done drawing. A fixed sleep is either too short on a loaded machine or wastes time on an idle one.
- To submit a prompt, `type` it, `wait` until the text is visible, then send `keys Enter` as a separate step. Text and
  Enter in one burst can read as a paste (Codex inserts the newline instead of submitting).
- An agent that stops at a sign-in or update screen is not testable as it stands. Do not sign in or update on the
  maintainer's behalf; report which agent and what it showed.
- Run several agents at once if you like: each `start` is its own terminal host. `stop` every run when done, including
  after a failure, and check that no `/tmp/fh-tui-*` directory of yours is left behind.

## Reporting what you saw

Quote the relevant lines of the screen, name the agent's version (`--version`) and the exact command line, and say which
flags and directory state the observation holds for. "`cursor-agent` 2026.09.18 asks for workspace trust in a fresh
directory, also with `--force`" is a finding; "Cursor asks for trust" is not.

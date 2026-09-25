# Session create lets tmux format-expand the working directory

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Creating a session in a folder like `C#Samples` silently runs the agent in the home folder; a crafted folder name runs a
hidden command when the session is created.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F1 / COR-CWD-CREATE`,
tagged **definite**. Anchors and title: `tmux.rs:2607-2608`, `service/core.rs:12395`, `service/core.rs:3420` — Session
create passes the working directory to tmux `-c` unescaped: `#` paths start the agent in `$HOME`, and `#(cmd)` in a
directory name runs `cmd`

Farhelm runs every session's agent inside a private tmux server that the per-host supervisor daemon owns. When a session
is created, `TmuxDriver::create_session` runs
`tmux new-session … -c <cwd> -- <shell> -l -i -c 'exec farhelm internal launch …'`, and `-c` is the only thing that
decides the agent's working directory. Nothing later in the launch chain changes directory: the login shell, the
`farhelm internal launch` shim, and the agent all inherit whatever directory tmux started the pane in. The code comment
says the values are "literal argv elements … so no quoting applies". That is true for the shell, but tmux does not treat
the `-c` value as a literal path. It runs it through its own format language first. In that language `##` means a
literal `#`, `#{…}` is a variable, `#S`, `#H`, `#D`, `#W` and similar are short aliases (session name, host name, pane
id, window name), and `#(cmd)` runs `cmd` through `/bin/sh`. If the rewritten path does not exist, tmux silently starts
the pane in `$HOME` and reports success. Several reviewers checked this on a throwaway server running the pinned tmux
3.7c. A directory named `C#Samples` or `proj#Sx` started the agent in `$HOME`. `x##y` started it in the sibling
directory `x#y`. A directory whose name contained `#(touch …)` ran the `touch` about a second later.

The supervisor's own validation does not catch this. `ensure_cwd_usable` (core.rs:3420) checks only that the literal
path is absolute, exists, and is a directory. The request passes that check, the session record and the UI show the
chosen path, and the agent then works somewhere else. Agents are often run with permission checks disabled, so it can
read and edit files in the home directory with no error anywhere. The second consequence is command execution. A
directory name containing `#(…)` runs its contents as the user when the session is created, before the agent starts and
before any workspace-trust prompt the agent has. `#` is legal in directory names and does occur ("C#", "issue#12"). The
directory browser (core.rs:4693) passes folder names through unchanged, and names often come from content the user did
not write, such as cloned repositories or unpacked archives. GitHub fresh-checkout basenames cannot contain `#`, but a
configured checkout root could.

The fix is to escape every `#` as `##` before handing the path to tmux. Reviewers verified that tmux then uses the
literal path. Put the escaping in one helper and use it at all three `-c` call sites (this one, F2 and F3). Add
real-tmux regression tests with directories named with `#(touch marker)`, `#S` and `##`, asserting that the pane's
directory is the literal path and that the marker file never appears. Only `-c` needs this: reviewers confirmed that
`-e` environment values and the command after `--` are not format-expanded.

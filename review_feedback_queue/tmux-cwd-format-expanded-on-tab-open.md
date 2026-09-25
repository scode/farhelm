# Opening a tab lets tmux format-expand the session directory

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

New tabs open in the home folder with no error, and a crafted folder name runs a hidden command whenever a tab opens.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F3 / COR-CWD-TAB`, tagged
**definite**. Anchors and title: `tmux.rs:2709-2710`, `service/core.rs:11474`, `service/core.rs:11364` — Opening a
terminal tab passes the session directory to `new-window -c` unescaped

A terminal tab is an extra shell next to the agent, implemented as another tmux window in the same tmux session.
`open_tab_window` (core.rs:11474) creates it with `TmuxDriver::new_window`, which runs
`tmux new-window -d … -c <session cwd> -- env -u FARHELM_AGENT_ID <shell> -l -i`. A moment earlier, `open_tab` checked
the literal path with `ensure_cwd_usable` (core.rs:11364). tmux format-expands this `-c` too. Reviewers verified that
`#(touch …)` in the path ran on every tab open, and that directories named `repo#S`, `dir#W`, `a#,b` and `a#{b` all
opened the shell in `$HOME`.

SPEC.md's "Terminal-tab working directory" decision says a tab opens at the session's path under normal filesystem
resolution, so a repointed symlink is accepted. It also says that "missing or unusable paths still fail clearly". tmux
rewriting the text of the path is not filesystem resolution, and a silent fallback to `$HOME` is not a clear failure. A
user who runs `git reset`, `rm`, or a build in what looks like the project tab does it in the wrong directory. Because
every tab open re-reads the stored path, a crafted `#(…)` directory name runs its command each time a tab is opened. The
fix is the same shared `#` → `##` helper as F1, with a test that a `#(touch marker)` directory never creates the marker.

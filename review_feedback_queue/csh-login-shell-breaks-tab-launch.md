# The tab launch form may fail under csh/tcsh

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Users with a csh/tcsh login shell may be unable to open tabs.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F20 / COR-CSH-TAB`,
tagged **definite** (retagged after the restater confirmed tcsh behavior from source). Anchors and title:
`launch.rs:646-654` — The tab launch form `<shell> -l -i` has the same csh/tcsh problem

Terminal tabs run the login shell directly, with no launch shim: `tab_window_command` (launch.rs:646-654) builds
`[<scope prefix…>, env, -u, FARHELM_AGENT_ID, <shell>, -l, -i]`. Under tcsh, `-l` combined with another flag hits the
same usage error as F19, so the tab's shell exits at once. `OpenTab` then refuses with "the terminal tab's shell (…) was
already dead when the tab…", which names the shell but not the cause. This is a separate call site from F19 and needs
its own fix; the premise and the suggested per-shell-family handling are the same as F19.

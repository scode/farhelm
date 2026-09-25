# The agent launch form may fail under csh/tcsh

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Users with a csh/tcsh login shell may see agents fail to launch.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F19 / COR-CSH-AGENT`,
tagged **definite** (retagged after the restater confirmed tcsh behavior from source). Anchors and title:
`launch.rs:601-607` — The agent launch form `$SHELL -l -i -c …` may fail under csh/tcsh login shells

Farhelm launches every agent through the user's login shell, so the agent sees the same environment as an interactive
terminal. `window_command` (launch.rs:601-607) always builds
`[shell, "-l", "-i", "-c", "exec … farhelm internal launch <spec>"]`. The shell comes from `$SHELL` or the passwd entry,
and any shell is accepted. SPEC_impl.md prescribes this exact form, so a fix changes that document too. csh-family
shells do not accept it. The tcsh manual says `-l` is "applicable only if -l is the only flag specified". The reviewer
could not run csh or tcsh on the review host. I checked tcsh's source (`sh.c` on tcsh-org master): `-l` is recognized
only when it is the single argument (`argc == 2`), and the option loop has no `case 'l'`, so `-l -i -c …` reaches the
unknown-option branch, prints a usage error and exits. Classic BSD csh was not checked.

For a user whose login shell is tcsh or csh (on macOS and BSDs `/bin/csh` is typically tcsh), every agent launch and
restart would therefore die immediately. The launch shim never runs, so no launch sentinel explains the failure; the
only trace is tcsh's usage message left in the dead pane. The suggested change is to build the flags per shell family
and use a form csh-family shells accept, such as `-c` with an explicit login-file source or running through `sh -lc`
with a documented limitation. At minimum, detect a csh-family shell and refuse with a clear message. Update SPEC_impl.md
in the same change.

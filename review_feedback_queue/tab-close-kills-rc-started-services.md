# Tab close kills services the tab shell's rc files started

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Exiting or closing a Farhelm tab can kill the user's ssh-agent, gpg-agent, SSH connection sharing, or a personal tmux
server that other terminals rely on.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F14 / COR-TAB-RC`, tagged **possible**. Anchors and title: `service/core.rs:11403-11431`, `launch.rs:625-655`,
`service/sweep.rs:251-260`, `service/ticker.rs:1486` — Tab shells start with Farhelm markers and inside the tab scope,
so services their rc files start are killed on tab close, auto-reap and Delete

A tab runs `env -u FARHELM_AGENT_ID $SHELL -l -i`, a login interactive shell. `FARHELM_SESSION_ID` and `FARHELM_TAB_ID`
are already set through `new-window -e` (core.rs:11403-11431), and on hosts with a user manager the tab's `systemd-run`
scope wraps the login shell itself (launch.rs:625-655). Everything the user's shell startup files launch therefore
carries both markers and sits in the tab's cgroup. Typical examples are keychain or `ssh-agent`, `gpg-agent`,
`emacs --daemon`, a first `tmux` that becomes the user's personal tmux server, and an ssh ControlPersist master.

The tab-close target and the whole-session Delete target both claim those processes by marker (sweep.rs:251-260), and
the scope kill reaches them regardless of markers. If a claimed process is the user's personal tmux server, the
parent-pid walk from it reaches every pane of that server, including windows unrelated to Farhelm. The ticker reaps a
tab within one tick of its shell exiting (ticker.rs:1486), so simply typing `exit` in a tab triggers all of this.

The agent launch deliberately does the opposite. Its markers and scope are applied after login-shell initialization, and
SPEC and SPEC_impl exempt "detached services started by shell initialization … [that] may serve the user's login
environment beyond this session". A shared `ssh-agent` or `gpg-agent` that other terminals depend on, or a personal tmux
server with all its unrelated windows, therefore dies with whichever Farhelm tab happened to start it.

This needs a maintainer decision rather than a straightforward fix. SPEC's shell-initialization exemption is worded
about the agent launch, while SPEC also says closing a tab "kills that shell and its processes". If rc-started services
in tabs should survive, apply the tab markers and scope after shell initialization, as the agent path does. Otherwise,
state explicitly in SPEC that tab close, auto-reap and Delete kill them.

For the user, exiting or closing a Farhelm tab can kill their ssh-agent, gpg-agent, SSH connection sharing, or a
personal tmux server that other terminals rely on.

Restater note: The behavior is a documented choice in code, not an oversight. `tab_window_command`'s docs
(launch.rs:625-655) say the scope wraps the tab shell so that "the shell must be in the cgroup, rc-file subprocesses and
all". SPEC_impl's exemption says startup-file services "need not be reaped", which permits killing them rather than
forbidding it. So this is a product decision to confirm or reverse, not a SPEC violation.

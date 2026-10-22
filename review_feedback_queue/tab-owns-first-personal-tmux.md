# The tab-startup-services decision rests on a premise the code contradicts for tmux

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

NOTE: This is not a code defect against the current spec. The behavior is specified and accepted. This item challenges
the premise behind that acceptance, for the maintainer to re-decide or to correct the spec.

## TLDR

If your shell startup file starts or attaches your own tmux when you are not already inside tmux (a very common setup),
and no personal tmux server is running on the host (after a reboot, say), the first Farhelm terminal tab you open starts
that server. Farhelm then treats the server as part of the tab: closing the tab or deleting its session kills it, along
with every window in it, including windows you later opened from ordinary SSH logins. With an `exec tmux ...` startup
line, an ordinary detach does the same, because the tab's shell exits and the tab is cleaned up at once. When this was
accepted, the reasoning was that the agent's launch normally starts such a service first, so a tab rarely owns one. For
tmux started this way that reasoning does not hold, so the tab routinely becomes the owner.

## Details

Found by the 2026-10-22 prioritized persistent review, group `035-kill-sweep-1of3` (finding f1). The reviewer proposed
dropping it as covered; an independent check disagreed and asked for this narrower item, framed as a challenge to the
decision's premise.

Suggested bucket: highest (it ends user processes and work done outside Farhelm, in tmux windows unrelated to any
session).

Confidence: likely. The sweep and marker behavior and the order in which `TMUX` is removed are confirmed by code
reading, rechecked at recording against the reviewed commit. Not run: that tmux copies its start environment (with the
tab's markers) into every later session's panes, and the auto-reap path for an exec'd tmux client. Also unverified:
whether the agent launch's login shell reaches a `.bashrc` tmux line at all. If it does not, the premise fails for a
different reason, because the agent launch then never ran that startup logic.

### What the spec accepts

SPEC.md "Session view", lines 1364-1368: "A tab's processes include whatever its shell's startup files start:
containment begins before those files run, so a shared service (an `ssh-agent`, an editor daemon, a detached personal
tmux server) that a tab's startup files are the first to start belongs to that tab and dies when the tab is closed,
reaped, or deleted with its session".

SPEC.md "Ownership during cleanup and provisioning", lines 3394-3395, anticipates the trigger: "A tab's shell starts
without them, so a startup file that launches tmux when `TMUX` is unset does so in a tab as it would in an SSH login."

TODO.md's `Doc todo` (lines 243-245) has the user-documentation entry the decision asked for.

### The premise

SPEC.md "Session view", lines 1371-1372: "since the agent's launch has normally already run the same startup files, a
tab usually finds such a service running and only reuses it."

`TRIAGE_OUTCOMES.md`, "tab-close-kills-rc-started-services.md", lines 1158-1160: "The realistic trigger is narrower than
the finding implies: a tab can only open on a live agent, whose launch already ran the same rc files outside
containment, so guarded rc logic normally reuses that instance and the tab never owns it." The user then chose
`fix spec` to specify the current behavior.

### What the code does instead

- Agent launch: `crates/farhelm-supervisor/src/launch.rs:1286-1288` documents the removal of the private server's
  `TMUX`/`TMUX_PANE` "here rather than earlier so the login shell's startup files still ran with them"; the removal
  itself is at `launch.rs:1326-1329` ("Removed after login initialization, matching the visible launch contract"). So
  when the agent's startup files run, `TMUX` names Farhelm's private server, and a `[ -z "$TMUX" ] && tmux ...` guard
  does not fire. The agent launch never starts the user's tmux.
- Tab: `launch.rs:1207-1209` (`exec_tab_shell`) removes the same variables before the tab's shell starts, as the
  `tab_window_command` docs at `launch.rs:733-738` intend, so the guard fires in the tab.
- Tab markers `FARHELM_SESSION_ID` and `FARHELM_TAB_ID` are set through `new-window -e`
  (`crates/farhelm-supervisor/src/service/core.rs:13286-13293`) before the shell's startup files run. A tmux server
  forked from the tab keeps them, and per the reviewer tmux copies its start environment into the global environment of
  later sessions, so all its panes carry them.
- `SweepTarget::Tab` and `WholeSession` claim on those markers
  (`crates/farhelm-supervisor/src/service/sweep.rs:252-261`), `snapshot_proc` (`sweep.rs:422`) admits the server and its
  panes as roots, and the parent-PID closure in `enumerate_tree` (`sweep.rs:588-643`) follows every pane's children. On
  systemd hosts the tab's scope kill catches the server as well.
- With `exec tmux new -A -s main`, the tab's pane process is the tmux client. Detaching exits it, the tab is reaped
  automatically (SPEC.md "Session view", lines 1372-1373), and the reap sweep kills the server the user just detached
  from.

So on a host with no personal tmux server running, the first tab opened routinely becomes that server's owner. That is
the common case the decision assumed was rare.

How to verify: on a host with no tmux server, put `[ -z "$TMUX" ] && exec tmux new -A -s main` in `~/.bashrc`, open a
Farhelm tab, open a second window from an SSH login, then detach in the tab. The server and the SSH login's window die.

### Options for the maintainer

- Keep the behavior and correct the spec sentence at 1371-1372 (and the user documentation) so it does not promise that
  a tab usually reuses an existing service; say plainly that a TMUX-guarded tmux autostart makes the first tab the
  owner.
- Change the behavior: exclude tmux (or any process whose tmux socket differs from Farhelm's) from tab markers, or start
  tab containment after the startup files, as the agent launch does. That reverses the principle the decision recorded
  ("tab containment deliberately starts before the shell's startup files, because the tab shell is itself what close
  promises to kill").

Related: `agent-stop-kills-shared-personal-tmux.md` covers the agent-side case, where an agent rather than a tab's
startup file starts the user's first tmux server.

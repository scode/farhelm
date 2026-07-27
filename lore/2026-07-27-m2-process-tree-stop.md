# M2 stop: process-tree kill via /proc walk plus env-marker sweep

SPEC.md's stop contract is "the agent and its entire process tree — MCP servers, dev servers, and other descendants
included." SPEC_impl.md's original plan was `systemd-run --user --scope` cgroups on Linux with a process-group kill
plus marker-environment sweep as the fallback. What M2 actually ships, and why:

- **cgroups (systemd-run scopes)** are the only mechanism that captures every descendant including a daemon that
  double-forked AND exec'd with a scrubbed environment. Deferred to M3, not dropped: wiring the launch shim through
  `systemd-run` adds a runtime dependency on a per-user systemd manager that CI containers and some real hosts lack,
  so the fallback has to exist and be trustworthy first — building the fallback well IS the M2 work either way.
- **Process-group kill (`kill(-pgid)`) alone** was rejected: it only reaches the foreground group; MCP servers and dev
  servers that made their own groups escape.
- **A bare /proc PPID walk** (the first cut of this PR) was rejected by review as insufficient: a daemon that
  reparented to init before the walk is invisible to it, and that is exactly the shape of process the contract names.
- **Chosen: union of the PPID closure and an environment-marker sweep.** Launch injects `FARHELM_SESSION_ID` into the
  session's environment (a name SPEC.md already reserves); stop scans same-user /proc environ files for it, which
  finds reparented daemons the walk cannot. Kill sequence: SIGTERM the union, short grace, then SIGSTOP-quiesce +
  re-enumerate to a fixpoint + SIGKILL — the quiesce closes the fork-during-teardown race (a TERM handler that forks
  and exits), and every signal validates the pid's /proc start time first so a recycled pid is never touched. Residual
  gap, accepted until M3's cgroups: a descendant that exec'd with a cleaned environment after reparenting.

Delete orders processes-then-terminal-then-row deliberately: a crash mid-delete leaves a listed-but-dead session
(visible, re-deletable) rather than an unlisted-but-running agent (invisible, unreapable). The same "never leave an
agent nothing can see" reasoning as the create path's tmux-then-DB ordering, applied in reverse. Review pushed this
further than the first cut: teardown failures now fail the delete loudly and keep the row, because removing the last
handle to a possibly-running agent is the one unrecoverable outcome.

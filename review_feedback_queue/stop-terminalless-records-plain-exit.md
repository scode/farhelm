# Stopping a half-launched session records a plain exit and loses the restart consent check

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Stopping a session whose launch never confirmed (it has no terminal but may have a running agent) records it as an
unexplained exit instead of "stopped by user". If the stop's process sweep then fails, a later Restart no longer asks
before killing whatever survived.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F17 / COR-STOP-TERMINALLESS`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/handlers.rs:1251` — stopping a never-confirmed, terminal-less launch records a
plain exit before sweeping a possibly live agent.

A session can end up with no recorded terminal while its last outcome is still "launching". The usual cause is an
ambiguous create, where tmux reported an error but the session and the agent in it may exist anyway. The supervisor has
a helper, `terminal_less_launch_may_be_live` (`crates/farhelm-supervisor/src/service/core.rs:4363`), whose job is to
treat such a session as possibly running. Restart (around line 10120) refuses to touch it without the user's consent to
stop it, and Delete (around line 11506) counts it as still alive.

Stop does not use that helper. With no pane to check, the Stop handler
(`crates/farhelm-supervisor/src/service/handlers.rs`, from line 1251) takes the "already dead or absent" branch. It
looks for a launch-failure record, finds none, and durably records a plain exit with no "stopped by user" note (around
lines 1341-1369). Only after that does it run the sweep that kills any process tagged with the session's marker (around
line 1409), which is what would actually stop a live agent. Two things follow:

- If the agent was really running, the user's stop is recorded as an unexplained exit. SPEC.md says a user-initiated
  stop yields "exited" with an annotation.
- If the sweep fails (the stop refuses when it cannot confirm the agent's systemd scope is gone), Stop reports an error,
  but the session already shows as exited. Because the outcome is no longer "launching", the helper now answers false,
  and a later Restart no longer asks for consent before reaping whatever survived.

This is marked possible because it depends on the terminal-less, still-launching state arising in practice. The code
comments say it can, but the reviewer did not reproduce it. The suggested fix is to give such a session the same stop
lifecycle as a live pane when the helper says it may be live: record the stop intent, sweep, then record the annotated
outcome. At minimum, run the sweep before recording anything.

# Review feedback filters

NOTE: This file is NOT a product specification and NOT a coding standard. Nothing in it makes a behavior acceptable, and
it must never be cited to justify a design, to skip handling a case while writing or changing code, or to wave off a
finding about a change that is still under review. Code being written is expected to get these cases right. The filters
exist for one situation only: an automated review of code that has already landed turns up a flaw of this kind, and
recording and triaging it would cost a human more than the flaw is worth. SPEC.md, SPEC_impl.md and the root agent
instructions are where acceptable behavior is defined.

## Where the filters apply

- Recording feedback: a finding from an automated review of committed code that fully matches a filter is not written to
  the queue.
- Triage: an already queued item that fully matches a filter is handled like one covered by the specification: remove
  its file and index entry immediately, record outcome `other` in the root `TRIAGE_OUTCOMES.md` naming the filter as the
  reason, and mark its execution complete. If only part of a finding matches, bring the remaining part to the user.

A filter never applies to findings from reviewing a change before it merges, to a problem a person reports or asks to
have fixed, or to a finding that also carries a consequence outside the filter's boundary. When it is unclear whether a
finding matches, it does not match.

## Filters

### Rare, self-correcting glitches and imprecise diagnostics

Added 2026-09-28.

A finding matches when both of these hold:

- The trigger is rare: a narrow timing window, a crash or interruption at a specific point, or an input the shipped
  clients never send.
- The whole consequence is one of: a display that is briefly wrong and corrects itself on the next refresh, retry or
  reconnect, or when the user leaves the view; an operation that fails safely and succeeds when retried; or an error
  message, log line or other diagnostic that is imprecise, misleading, missing detail, or lost.

It does not match when the consequence includes any of the following, however rare the trigger:

- loss of user data, credentials, processes, or other user-owned work;
- a destructive or state-changing action applied to the wrong target;
- a wrong state that persists: recorded durably, or needing a restart or manual repair to clear;
- success reported for an operation that failed;
- a security or trust-boundary consequence.

### A hung private tmux server or systemd user manager

Added 2026-09-28. This is the review-time counterpart of SPEC.md's "Healthy local filesystems" decision, applied to two
other local dependencies a supervisor drives: Farhelm's private tmux server and the host's systemd user manager. Unlike
that decision, it does not make the behavior acceptable; it only keeps such findings out of triage.

A finding matches when both of these hold:

- The trigger is that one of those dependencies stops answering while still running, or that the configured tmux program
  hangs or never exits instead of behaving like tmux.
- The whole consequence stays on the affected host: its supervisor, or some of its requests, stall or wait forever; work
  there queues behind the stuck operation; or resources leak until that supervisor restarts.

It does not match when the consequence includes any of the following:

- the helm becomes unresponsive or unusable, or hosts other than the affected one are disrupted;
- loss of user data, credentials, processes, or other user-owned work;
- a destructive or state-changing action applied to the wrong target;
- a wrong durable record that remains after the dependency recovers or the supervisor restarts;
- success reported for an operation that failed;
- a security or trust-boundary consequence.

The helm machine's own supervisor is an affected host like any other: a hang there may stall that host's sessions, but
the helm's UI and its other hosts must keep working.

### Session status and history after a crash or a partly failed operation

Added 2026-09-28. Code should still keep a session's status and history accurate wherever that is easy, and should not
drop useful diagnostics; this filter only keeps such findings out of triage.

A finding matches when both of these hold:

- The trigger is that the supervisor stops, crashes, or loses its ability to record in the middle of a session
  operation; that an operation fails partway; or that startup recovery has to reconstruct what happened from incomplete
  evidence.
- The whole consequence is that what Farhelm shows or records about the session is inaccurate or imprecise: how a run
  ended (stopped by the user or exited on its own, the exit code, an error or a plain exit), whether a relaunch
  happened, whether the agent is still running, or which error explains the state. It may also include a Stop, Restart
  or Delete that was cut short leaving that session's processes part-way through being stopped (for example still frozen
  by the kill sweep). Either holds even when the inaccurate record is durable, as long as the user can still recover by
  acting on the session again through the ordinary Stop, Restart and Delete operations.

It does not match when the consequence includes any of the following:

- the session can no longer be opened, stopped, restarted, or deleted through ordinary operations;
- a destructive or state-changing action without the confirmation the specification requires, or applied to the wrong
  target;
- loss of user data, credentials, processes, or other user-owned work, including losing or replacing a valid Resume
  offer;
- the helm becoming unresponsive or unusable, or other sessions or hosts being disrupted;
- a security or trust-boundary consequence.

### Rare edge cases in harnesses without first-class support

Added 2026-09-28. The first-class harnesses are currently Claude Code and Codex; every other harness integration (Goose,
Pi, OMP, Grok, and any added later) is supported with less rigor.

A finding matches when all of these hold:

- It concerns the integration with a harness that is not first-class, and does not also affect a first-class one.
- The trigger is rare or unconfirmed: a vendor behavior nobody has observed, an unusual payload or configuration, or a
  narrow timing window.
- The whole consequence stays within that harness's integration features for the affected sessions: a missing or delayed
  Resume offer, imprecise status, an extra or missing diagnostic, or a harness-side warning.

It does not match when the consequence includes any of the following:

- sessions of that harness failing to launch or run in ordinary use;
- loss of user data, credentials, processes, or other user-owned work, including resuming the wrong conversation;
- the helm, other sessions, or other hosts being affected;
- a security or trust-boundary consequence.

### Races a person would have to win inside a sub-second window

Added 2026-10-01; widened the same day to cover convenience history.

A finding matches when both of these hold:

- The trigger needs a person to act (click, confirm, submit) inside a window of about a second or less that opens and
  closes on its own, for example between the helm noticing a change and the next step it takes about that change.
- The whole consequence is recoverable through ordinary use: the wrong state is replaced, or asked about again, on the
  next connection, refresh or prompt, and anything it clears is either a cache Farhelm refills through ordinary use or
  convenience history. Convenience history here means remembered suggestions that only pre-fill choices in the
  new-session dialog, such as a host's recent setups and used folders, which build up again as the host is used. For
  this filter it counts as recoverable; it is not user-owned work.

It does not match when the consequence includes any of the following:

- loss of user data, credentials, processes, or other user-owned work;
- connecting to, sending an operation to, or acting on the wrong machine or session;
- a wrong state that persists with no ordinary way back;
- a security or trust-boundary consequence.

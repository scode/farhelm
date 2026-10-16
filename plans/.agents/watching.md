# Waiting for new work

How a monitoring executor waits between drains with `scripts/plans-watch.sh`: starting the watcher, running it so its
exit wakes the session in Claude Code or Codex, and what each exit means. Drainers read it before their first action
when the request says "and keep monitoring"; the landing monitor reads it before its first round, because
`plans/.agents/landing.md`, "Waiting between rounds", runs the same watcher the same way.

## Drain and keep monitoring

"Drain the plans and keep monitoring" adds a loop around the drain (`plans/.agents/draining.md`, "Draining: "drain the
plans""): when a drain finishes, wait until there may be new work, drain again, and keep going until the maintainer says
stop. The waiting is done by `scripts/plans-watch.sh`, not by the model. A model wake-up re-reads the agent's whole
conversation, often uncached after a long wait, so waking hourly just to find nothing would cost a full model round each
time. The watcher polls GitHub cheaply and exits only when `plans/` on main changes in a way that can give an idle
executor work: with `--wake-check`, only a plan becoming pickable (new, answered, followed up, given back, revised,
reordered, or freed by a dropped dependency), a plan leaving the queue, or an in-flight plan blocking (which can end a
conflict a pick skipped for) counts, and claims, deliveries, landings in progress and report reviews do not. The watcher
has no maximum wait unless one is passed, so an idle executor does not wake at all. It used to stop every 110 minutes,
and each of those wake-ups only restarted the watcher while re-reading the session's whole context: 0.4 to 1.4 million
tokens per wake-up for the Codex executors, mostly cached input, measured on 2026-10-15. Run it like this:

- Start
  `scripts/plans-watch.sh --repo <OWNER/NAME> --baseline-from <commit> --wake-check <checkout>/scripts/plans-queue.py`
  from the checkout, where `<commit>` is the main commit id `queue status` printed for the drain's last pick and
  `<checkout>` is the checkout's absolute path. Use that commit, not a fresh fetch, so a plan that landed in between
  still counts as a change. If that pick's `status` failed, do not start a watcher at all: notify the maintainer and
  stop monitoring.
- Run it as a background command whose completion wakes the session:
  - In Claude Code, `run_in_background`. Claude Code does not apply the command's `timeout` to a background command
    (checked on 2026-10-15), so it needs none. Whether Claude Code limits how long a background command may live in some
    other way is not known; a watcher killed like that is covered by the exit rules in this list. Record its task handle
    and the baseline commit in the executor log.
  - In Codex, arm it through the maintainer's `skillette-codex-wake-on-exit` skill (trigger: "use Codex wake-on-exit"),
    from the checkout, with a new run directory inside the agent's scratch directory, and follow that skill for the
    mechanics: check the phase it returns before relying on it, end the turn once it is armed, and acknowledge the
    callback before acting on the watcher's exit. Record its run id, run directory and baseline commit in the executor
    log, with the continuation: the exit rules in this list. A `queue-failed` or `runner-failed` phase, or a session the
    skill cannot arm (no `CODEX_THREAD_ID`, no `codex queue`), means waking does not work here: fall back to the
    foreground rule at the end of this list rather than polling, and record why.

  After a compaction or a resume, if the recorded watcher is no longer running (in Codex, the skill's one-time `status`
  check settles it), start a new one with the recorded commit.
- Act only on the exit of the watcher the executor log records (in Codex, a callback whose run id and directory match
  the recorded ones). Exits of a watcher the agent stopped itself, or of any other, are ignored.
- Its exit decides what happens next:
  - `changed` (status 0): drain again.
  - `idle` (status 10): nothing changed. Only a watcher restarted after an error (the `error:` exit in this list) has a
    `--max-wait`, so only that one exits this way. Start it again with the same commit and the plain command in the
    first item of this list, without fetching or draining.
  - `error:` (status 3): if the line mentions HTTP 401 or 403, the GitHub login needs the maintainer; notify and stop
    monitoring. Otherwise keep restarting it with `--interval 1800 --max-wait 3600` until a watcher exits `changed` or
    `idle`. The cap is there so that the slow interval ends on its own once GitHub answers again: the `idle` restart
    goes back to the plain command. An outage that is still being retried does not need the maintainer, so it is
    recorded in the executor log, not notified.
  - No output line and a status of 128 or more: something other than the agent killed the watcher (a harness limit on
    background commands, say). Start it again once with the same commit; if that one is killed too, notify and stop
    monitoring.
  - `usage:` (status 2) or anything else: the invocation is wrong, so restarting cannot help. Notify and stop
    monitoring.
- A harness that cannot wake a session when a background command exits runs the watcher in the foreground instead, with
  a `--max-wait` shorter than its own command timeout. In Codex that is a run without a connected client to receive the
  callback: a one-shot `codex exec`, or a native spawned subagent, which Codex will not wake with queued input.

A batch of new plans wakes every monitoring executor at once, and they will race for the same first pick. That is fine:
the losers get exit 10 and move to their next candidate.

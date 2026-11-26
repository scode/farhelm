# Plan records

Where executors and the landing monitor keep their own state, and where each plan's working log lives: in records,
reached through the records tool. Executors, drainers, the landing monitor and the planner read this before their first
action, and the plan reviewer reads it for the monitor's status line (`plans/AGENTS.md`, "Flows and reading lists").

## Files outside the repository

Executors and the landing monitor keep their logs and notes as records in a separate git repository, `agent-records`,
and reach them only through the records tool, `cargo xtask records`, run from the root of their own checkout. The tool's
job is to keep these instructions stable: they name records, never a repository, a URL, a token or a git command.
`.agents/agent-records.md` describes the repository, the host files and the tool for anyone who needs more.

Each executor checkout, and the landing monitor's, is a slot listed in `~/.fh-records/config`. The tool finds its slot
from the checkout it runs in, keeps one clone of the repository per slot, and refuses to push a record the slot may not
write. A checkout that is not a slot cannot run an executor or the monitor: stop and tell the maintainer.

- `cargo xtask records path <name>` brings the record up to date with its last push and prints its directory. Write in
  that directory as you work, the way you would write any local file; that copy is the live one, and anything that needs
  to see what an agent is doing right now looks there.
- `cargo xtask records push <name>... -m <message>` commits the named records together and pushes them. It never
  force-pushes, and when another slot pushed first it retries on its own. `-m` is one line saying what is about to
  happen (`claim <slug>`, `push plan/<slug>/02-retry-loop`), never report text.
- When `path` or `push` fails (exits non-zero), do not take the step it came before. Notify (`plans/AGENTS.md`,
  "Notifications": one line naming the slug, or the monitor), keep any claim where it is, and stop the flow. A failed
  push leaves the clone as it was, so once the cause is fixed the flow carries on from the same step.
- When `path` or `push` prints `records: set aside local edits to <path> ...`, another slot changed a file this slot had
  unpushed edits to (normally leftovers from a claim this slot no longer holds). Record the line in the executor log;
  the saved copy is where the line says, for the maintainer.

The records:

- `executor`: the slot's own executor log, `log.md` in the directory `path executor` prints. Read it first after any
  start, compaction or resume. It records the flow in progress; the current claim's slug and claim id; the verb it is
  about to run or last ran on that claim (`claiming`, `blocking`, `delivering`, `unclaiming`); the watcher's task handle
  and baseline commit while monitoring; the reasoning behind each pick; and each plan's closing entry. Only this slot
  writes it.
- `plan/<slug>`: the plan's working log, `log.md`, per the goal file's resume protocol, and a `notes/` directory beside
  it. Run `path plan/<slug>` right after winning the claim; that is how a new holder gets the earlier holders' log and
  notes. Whoever holds the plan's claim writes it, a later claimer reads all of it, and nothing writes it once the claim
  has ended. Each holder opens its claim with an entry naming its slot and claim id, and records after each push the
  commit pushed for each bookmark. `notes/` holds anything a later claimer needs that is not log text (resume-check
  findings, drafts of a Blocked question or report): the previous holder's scratch directory is never available to the
  next. Slugs are never reused, so an old record can never be mistaken for a new plan's.
- `landing`: the monitor's own log, `log.md`, read first after any start, compaction or resume
  (`plans/.agents/landing.md`, "Starting and resuming"). There is one, because one monitor runs at a time. It starts
  with a `## Status` section holding one line, `running, last checked <UTC time>` or `stopped <UTC time>: <reason>`,
  which the monitor rewrites on every wake and which a review session reads. Below that it records the landing claim in
  progress (slug, claim id, and the verb it is about to run or last ran: `starting`, `blocking`, `stopping`,
  `recording`), the PRs of that plan merged so far, the watcher's task handle and baseline commit, whether the
  maintainer chose to run without notifications, and the claim ids and plans it has already notified about. Only the
  monitor writes it.
- `round/<n>`: one landing round's text files (`plans/.agents/landing.md`, "A landing round"), written by the monitor.

Plans written before records existed tell the executing session to keep their log as `farhelm-plan-<slug>-log.md` in the
parent directory of the checkout. That location is gone: the `plan/<slug>` record is the plan's working log whatever its
goal file says, and no log is ever created beside the checkout. The resource watchdog's status file is not a record and
stays beside the checkout as `farhelm-plan-<slug>-watchdog.json` (the parent directory, derived the way
`scode-build-goal`'s "Placing the files" section derives it), never inside a record's directory: it carries a pid and is
rewritten every minute, and its fixed place lets a resumed session find an earlier session's sampler.

## When records are pushed

Push the records right before anything that changes GitHub, so the pushed copy never lags behind what GitHub shows. If
the GitHub step then fails, the record holds an intent with no effect, which the next resume reconciles against GitHub,
the same way the executor log's verb is written before the verb runs. An executor pushes:

- before `queue claim`: `executor` alone, since there is no plan record to write yet;
- before every other queue verb on its claim (`unclaim`, `block`, `deliver`) and before every push of a plan bookmark:
  `plan/<slug>` and `executor` together, except for an `unclaim` at the re-check before the plan's record was opened
  (`plans/.agents/executing.md`, "Executing one plan", step 4), which pushes `executor` alone. The push before `block`,
  `deliver` or `unclaim` carries the plan log's last entry from this claim: whatever happens after it (the verb's
  outcome, the close) goes to the executor log only;
- once more after closing a plan (`plans/.agents/executing.md`, "Executing one plan", step 12): `executor` alone, which
  carries the closing entry and the cleared claim.

The landing monitor's pushes are in `plans/.agents/landing.md`. Records hold free-form logs and notes only: plan state
lives on GitHub main and nowhere else. Absolute paths in records are fine, since they never reach the public repository;
the rules in `plans/AGENTS.md`, "Public-repo hygiene", apply to anything copied from a record into a Blocked question, a
report or landing notes.

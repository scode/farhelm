# Plan logs outside the repository

Where executors and the landing monitor keep their own state, and where each plan's working log lives. Executors,
drainers, the landing monitor and the planner read this before their first action, and the plan reviewer reads it for
the monitor's status line (`plans/AGENTS.md`, "Flows and reading lists").

## Files outside the repository

Executors keep their state in the parent directory of the checkout, next to it and out of the repository, the way
`scode-build-goal` places goal logs (derive the parent the way its Placing the files section does). If that derivation
does not yield a usable directory, stop and tell the maintainer rather than picking a temporary location. Every executor
runs in its own sibling checkout or jj workspace on the same host, so they all share this directory; that is how a plan
started by one executor can be resumed by another. Separate clones are simplest. Workspaces of one jj repository also
work, since plan bookmarks never collide across plans, but they share bookmark state.

- `farhelm-plans-log-<checkout>.md`, where `<checkout>` is the checkout directory's name: the executor's own state, read
  first after any start, compaction or resume. It records the flow in progress; the current claim's slug and claim id;
  the verb it is about to run or last ran on that claim (`claiming`, `blocking`, `delivering`, `unclaiming`); the
  watcher's task handle and baseline commit while monitoring; and the reasoning behind each pick. Only that checkout's
  executor writes it.
- `farhelm-plan-<slug>-log.md`: the plan's working log, per the goal file's resume protocol. Whoever holds the plan's
  claim writes it; a later claimer reads all of it. Besides the goal's own entries it records the executing checkout's
  absolute path and, after each push, the commit pushed for each bookmark. Slugs are never reused, so an old log can
  never be mistaken for a new plan's.
- `farhelm-plans-monitor-log.md`: the monitor's own state, read first after any start, compaction or resume
  (`plans/.agents/landing.md`, "Starting and resuming"). There is one, because one monitor runs at a time. It starts
  with a `## Status` section holding one line, `running, last checked <UTC time>` or `stopped <UTC time>: <reason>`,
  which the monitor rewrites on every wake and which a review session reads. Below that it records the landing claim in
  progress (slug, claim id, and the verb it is about to run or last ran: `starting`, `blocking`, `stopping`,
  `recording`), the PRs of that plan merged so far, the watcher's task handle and baseline commit, whether the
  maintainer chose to run without notifications, and the claim ids and plans it has already notified about. Only the
  monitor writes it.

These files are never committed, so absolute paths in them are fine. Nothing else about a plan lives outside the
repository.

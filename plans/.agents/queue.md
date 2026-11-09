# The queue: states and the queue script

The plan states, the line grammars of `INDEX.md` and `REPORTS.md`, and the verbs and exit codes of
`scripts/plans-queue.py`. Every role that runs a `queue` command reads this before its first one: the planner, the todo
planning and triage scheduling flows, executors, drainers, the landing monitor, the plan reviewer, and release plan X
(`plans/AGENTS.md`, "Flows and reading lists"). The editing rules that follow from the states are in `plans/AGENTS.md`,
"Editing rules".

## States, and the script that moves them

`INDEX.md` lines have this exact grammar:

```
- [<state>] `<slug>.md` — <summary>[ (after `<other>.md`[, `<other2>.md`])]
```

| State               | Meaning                                                                       | Held by               |
| ------------------- | ----------------------------------------------------------------------------- | --------------------- |
| `pending`           | waiting for an executor: new, answered, or sent back for a follow-up          | nobody                |
| `held`              | planned, but the maintainer does not want it built yet; never picked          | nobody                |
| `in-flight <claim>` | an executor is working on it                                                  | that claim's executor |
| `blocked`           | waiting on the maintainer; the plan file has a `## Blocked` section           | nobody                |
| `complete`          | every PR built and gated; `reports/<slug>.report.md` delivered; ready to land | nobody                |
| `landing <claim>`   | the monitor is landing it                                                     | that claim's monitor  |

A plan that has landed is not in `INDEX.md` at all. Landing moves its line to `REPORTS.md`, which has this grammar:

```
- [`<slug>`](reports/<slug>.report.md) landed <YYYY-MM-DD> in #<n>[, #<n>...]: <summary>
```

It stays there until the maintainer approves it, which ends its tracking, or asks for a follow-up, which puts it back at
the top of `INDEX.md` as `[pending]`.

A held plan is planned work parked until the maintainer wants it built. Holding decouples planning from scheduling: the
maintainer can plan whenever they have the time, and decide later when the work runs. No executor picks it, a plan that
runs after it waits as long as it stays held (its work is not on main), and only the maintainer moves it back to
`pending`.

`(after x.md)` means the plan may only start once `x.md`'s work is on main. That is the case once `x.md` is no longer
listed in `INDEX.md`: only landing (`landed`) and abandoning take a line out, and abandoning settles its dependents
explicitly. A plan can therefore start as soon as what it waits for has landed, whether or not the maintainer has read
that plan's report yet. A follow-up does not take that back: when it puts `x.md` back into `INDEX.md`, the script drops
`x.md` from every `(after ...)` clause, because its first round is already on main. The maintainer chose that changes
keep flowing by default; if a follow-up would break a dependent, they deal with it by hand.

Every state change is made by `scripts/plans-queue.py`, which commits it directly to main. In `plans/AGENTS.md` and
`plans/.agents/`, `queue <verb>` is short for `scripts/plans-queue.py --repo <OWNER/NAME> <verb>`, where `<OWNER/NAME>`
is the GitHub repository of the `origin` remote (the script needs it spelled out, because a jj workspace has no `.git`
for `gh` to infer it from). The script builds its commit on the main it just read and publishes it with a non-forced
update that GitHub refuses if main moved in between, so of two executors claiming the same plan exactly one wins and the
other gets exit 10. Its header documents the rest; in short: exit 0 done, 1 `check` found violations (or `check-slug`
found the slug), 10 the queue is not in the state the verb needs (look again and re-decide), 2 a usage mistake, 3 an
error whose outcome may be unknown.

Exit 3 from a verb that moves a state does not mean nothing happened: the commit may have landed with its response lost.
Before doing anything else, run `queue status`. If the line already shows the verb's target state (for a claim holder,
with your own claim id where the state has one; `queue history <slug>` lists the plan's recent commits and the claim id
each carried), the verb landed. If it still shows the starting state, run the verb again. Anything else is someone
else's change: handle it as exit 10.

| Verb                                                      | From → to                                                    | Used by                                    |
| --------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------ |
| `claim <slug> --claim ID`                                 | pending → in-flight                                          | an executor that picked the plan           |
| `unclaim <slug> --claim ID`                               | in-flight → pending                                          | the claim holder, giving the plan back     |
| `block <slug> --claim ID --question FILE`                 | in-flight or landing → blocked                               | the claim holder                           |
| `deliver <slug> --claim ID --report FILE`                 | in-flight → complete                                         | the claim holder                           |
| `start-landing <slug> --claim ID`                         | complete → landing                                           | the monitor                                |
| `stop-landing <slug> --claim ID`                          | landing → complete                                           | the claim holder, giving the landing back  |
| `landed <slug> --claim ID --merged N[,N...] --notes FILE` | landing → `REPORTS.md`                                       | the claim holder, once every PR merged     |
| `release <slug> [--hold]`                                 | in-flight → pending (held with `--hold`), landing → complete | only when the maintainer asks              |
| `hold <slug>`                                             | pending → held                                               | only when the maintainer asks              |
| `unhold <slug>`                                           | held → pending                                               | only when the maintainer asks              |
| `answer <slug> --decision FILE [--hold]`                  | blocked → pending, or held (see below)                       | a review session, or the landing monitor   |
| `follow-up <slug> --decision FILE [--hold]`               | `REPORTS.md` → pending (held with `--hold`), first in line   | a review session                           |
| `approve <slug>`                                          | `REPORTS.md` → gone                                          | a review session                           |
| `abandon <slug> [--dependents block\|drop-dep]`           | pending, held, blocked or complete → gone                    | a review session, on the maintainer's word |

`--hold` on `release`, `answer` or `follow-up` puts the plan on hold in the same commit. Doing the verb first and
holding after would publish the plan as `[pending]` in between, and an idle executor could claim it before the hold
landed. A plan that blocked while on hold (a held plan whose prerequisite was abandoned with `--dependents block`; its
Blocked section opens "Blocked while on hold") goes back to `[held]` when answered, with or without `--hold`, so an
answer the landing monitor gives on the maintainer's behalf never schedules work the maintainer parked.

The read-only verbs: `status` (the queue with claims, claim times, open PRs, waiting dependencies, and flags for what a
person or the monitor should look at, then the landed plans awaiting review; it prints main's commit id first), `check`
(the invariants of a local tree, or of a remote ref with `--ref`; with `--base <branch>`, also what a planning PR may
not change), `check-slug <slug>`, `history <slug>` (the plan's recent commits under `plans/`, each with the claim id it
carried), and `wake-check` (used by the watcher, with `--for executor`, `--for lander`, or `--for lander-blocks` for the
monitor's watcher during a round).

# Landing monitor tooling

The mechanical steps of a landing round as commands, so the landing monitor runs the same guarded jj and gh sequences
every round instead of retyping them, and a session that resumes after a compaction or a reboot finds the tooling in the
checkout rather than in a scratch directory that may be gone.

NOTE: This is not the landing flow. `plans/.agents/landing.md` and `plans/.agents/reviewing.md` hold the rules (which
plans a round holds, what to review, when to block, what the notes say, when to notify), and they win over anything
here. Nothing in this directory makes a decision: conflict resolution, the cross-plan review, the choice of checks, the
notes' text and their cold read stay with the monitor.

- `landing.py`: one command per step, below. Python 3.11+, standard library only. `landing.py <command> --help` shows
  each command's arguments.
- `idle-watch.ts`: the watcher between rounds, and with `--blocks-only` the one that runs during a round (Deno).
- `notify.sh`: the plans notification send.
- `test-landing.py`: tests of `landing.py`'s log edits, round files, records pushes, claim check, notes rules and page
  escaping.

## Arguments every command shares

`--checkout` defaults to the checkout holding the script, and `--repo` to that checkout's `origin` remote, so from the
monitor's own checkout neither is needed. The monitor log is `log.md` in the agent records' `landing` record, the
directory `cargo xtask records path landing` prints; it is never an argument. The commands that edit it expect the two
lines the monitor keeps there: `running, last checked <time>` in `## Status`, and `- Landing claim in progress: <text>`.
History entries go at the end of the file.

Round commands take `--round <N>`, the round's number. The round's text files (`plans.txt`, `base.txt`, `prs.txt`,
`tested_commits.txt` and `notes/`) live directly in the `round/<N>` record, the directory
`cargo xtask records path round/<N>` prints (`round-start` prints it too), so a resumed session reads them back from
there, and a replacement monitor finds them after a fetch, with nothing to copy or restore. Bulk never goes into a
record: `diffs` writes to `<scratch>/diffs/`, where `<scratch>` is a directory of the round's own inside the monitor's
scratch directory (for example `/tmp/farhelm-tmp/<id>/r<N>`, per `AGENTS.md`, "Agent scratch space"). `landing.py`'s
module docstring describes each file.

The commands push the records (`landing` and the round's `round/<N>`, or `landing` alone where no round applies) right
before each GitHub-changing step they take, and a push that fails stops the command with exit 1 before that step:
`claim` before `queue start-landing`, `land` at the start of each PR's turn, `record` before `queue landed`,
`land-docs-prs` before each PR, and `round-end` once at the end. `log` and every other command only write locally. A
GitHub change the monitor makes by hand (`queue block`, `queue answer`, `queue stop-landing`) and a notification it
sends need their own push first (`plans/.agents/landing.md`).

A failed records push, inside `land` or any other command, leaves any landing claim held and is handled per
`plans/.agents/landing.md`, "Pushing the monitor's records": notify, leave the claim where it is, and stop. It is not a
reason to block the plan or run `queue stop-landing`; both are GitHub changes, which a failed push rules out.

NOTE: Every command that reads or edits the monitor log, `log` included, first runs `cargo xtask records path landing`,
which brings the record up to date from the records remote and so fails while that remote is unreachable. During a
records outage, write the Status line (`stopped <time>: <reason>`) by hand in `log.md`, in the directory that
`path landing` printed earlier in the session.

`round-start` and `round-end` write the round's History entries themselves, `r<n> started, head <short sha>` and
`r<n> ended`. They are the source of the round number: the next round is one more than the newest `r<n> started` entry
in the log.

## A landing round, in order

Commands run from the checkout root, and `landing.py` is short for `scripts/plans-landing/landing.py`. Placeholders:
`<n>` is the round's number, `<record>` the directory `round-start` prints for it, `<scratch>` the round's scratch
directory, `<slug>` a plan, `<claim>` the id `claim` prints.

- Apply the clean-main boundary by hand (`plans/AGENTS.md`), then
  `scripts/plans-landing/landing.py round-start --round <n>`. It refuses a round whose record already holds `plans.txt`
  (that round was started already: resume it from its record) and a dirty working copy, runs `queue status`, appends
  `r<n> started, head <short sha>` to the log, and records the `[complete]` plans (in queue order, each with its
  bookmarks, now tracked) and the status head in the round's record. The round's plans are fixed from here on.
- `landing.py bases --round <n>`: each plan's pushed base, main's non-bookkeeping commits since it, and any `plans/`
  path its stack touches (step 3).
- `landing.py stack --round <n>`: stacks the plans in queue order on `main@origin` and lists each conflicted commit with
  its files. Resolve conflicts by hand as a careful rebase.
- `landing.py diffs --round <n> --scratch <scratch> --oldest-base <commit>`: writes each plan's diff and main's landed
  work since the oldest base to `<scratch>/diffs/`, the cross-plan review's input for the monitor and its sub-agent.
- `landing.py pr-map --round <n>`: writes `prs.txt`, refusing a bookmark without exactly one open PR.
- `landing.py tested --round <n>` when the combined checks start, then run the checks.
- For each plan, in queue order:
  - `landing.py claim --round <n> <slug>`: step 1. Writes `starting` to the log, pushes the records, runs
    `queue start-landing` (exit 10: no longer complete, skip it) and prints the claim id.
  - `landing.py land --round <n> <slug> <claim>`: step 5. For each PR bottom-up it pushes the records, rebases onto
    `main@origin`, refuses a conflict, retargets the PR to main, confirms the claim, pushes, waits until GitHub shows
    the pushed head, marks it ready, and squash-merges with `--match-head-commit` after confirming the claim again. Ends
    by comparing main with the tested tree. A failure leaves the claim held, for the monitor to block or stop the
    landing per `landing.md`, except a failed records push, which is handled as described above.
  - `landing.py claim-land --round <n> <slug>` runs the two back to back.
  - Write `<record>/notes/<slug>.md` with a `CHECKS_PLACEHOLDER` line, and the round's shared checks in
    `<record>/notes/_checks.md`, then `landing.py assemble-notes --round <n> <slug>`, which writes `<slug>.final.md`
    rewrapped by dprint. Have it cold-read.
  - `landing.py record --round <n> <slug> <claim> <pr,pr,...>`: step 7. Writes `recording`, pushes the records, and runs
    `queue landed` with the final notes.
- `landing.py round-end --round <n>`: back to a clean `main@origin`, abandons the local copies of the round's merged PR
  heads (`--skip-unmerged` keeps a blocked plan's), appends `r<n> ended` to the log, pushes the records, and prints
  `jj status`, the checkout's size and `queue status`. Prune if the size is over 50 GB, then start the watcher with that
  status's head.

`landing.py verify --round <n> <top-bookmark>` repeats the comparison `land` ends with.

## Between rounds and around them

- Waiting: run `scripts/plans-landing/idle-watch.ts --baseline-from <commit> --wake-on-any-blocked` as a background
  command whose exit wakes the session (`plans/.agents/watching.md`), adding `--ignore-blocked <slug>` for each block
  left for the maintainer. It runs `scripts/plans-watch.sh --wake-for lander --max-wait 6600` and handles the `idle`
  exit itself as `landing.md`, "Waiting between rounds", says (a `queue status` check, then a restart unless a round is
  due), so the model wakes only for a change, a round the watcher missed, a flag, a block, or an error that needs it.
  After a transient watcher error it restarts with `--interval 1800 --max-wait 3600` until a watcher exits `changed` or
  `idle`. It never writes the monitor log; retries are reported on its own stderr. It prints one `wake <reason>` line.
- Waiting for blocks during a round: run `scripts/plans-landing/idle-watch.ts --blocks-only --baseline-from <commit>`
  with the round's status head, plus the same `--ignore-blocked` list, in the background once `round-start` has recorded
  plans, and stop it before `round-end`'s watcher starts (`landing.md`, "A landing round"). It runs the watcher with
  `--wake-for lander-blocks`, and its idle checks wake only for a `[blocked]` plan, never for a `[complete]` one or a
  flag.
- Logging: `echo '<entry>' | scripts/plans-landing/landing.py log` appends a History entry and refreshes the Status line
  with the same real time. It writes the log locally and pushes nothing.
- Notifying: `scripts/plans-landing/notify.sh '<one line naming the slug>'`, or with `--urgent` first when the
  maintainer asked for true blockers to stand out. It never reads the topic file other than inside the URL.
- Answering blocks, step 4: commit the TODO entries on a bookmark, write the message to a file, and run
  `landing.py land-docs-prs --jjstack <jjstack skill>/scripts/jjstack <bookmark>=<message-file>`, which opens and lands
  each PR without waiting for checks.

## Reviewing

From a plain git clone of your own (never an executor's or the monitor's checkout), on a fresh `origin/main`:

- `landing.py --checkout <clone> review-page --out <dir>/page.html --monitor-status '<status line>' --total <n> <summaries.json>...`
  builds the review page's body fragment for the `interactive-artifact` skill from the readers' summaries (JSON arrays
  of `{slug, prs, summary, done, leftovers: [{text, decision_needed}]}`) and the reports in the clone.
- `landing.py --checkout <clone> approve <slug>...` runs `queue approve` for each, from a fresh `origin/main` each time,
  and stops at the first failure.

## Checking changes to these files

`python3 -B scripts/plans-landing/test-landing.py`, `deno check` and `deno test -A` on `idle-watch.ts`, and
`shellcheck scripts/plans-landing/notify.sh` (`.agents/validation.md`).

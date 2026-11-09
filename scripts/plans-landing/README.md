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
- `test-landing.py`: tests of `landing.py`'s log edits, round files, claim check, notes rules and page escaping.

## Arguments every command shares

`--checkout` defaults to the checkout holding the script, and `--repo` to that checkout's `origin` remote, so from the
monitor's own checkout neither is needed. The monitor log is `farhelm-plans-monitor-log.md` in the checkout's parent
directory (`plans/.agents/outside-files.md`), never an argument. The commands that edit it expect the two lines the
monitor keeps there: `running, last checked <time>` in `## Status`, and `- Landing claim in progress: <text>`. History
entries go at the end of the file.

Round commands take `--round-dir`, a directory of the round's own inside the monitor's scratch directory (for example
`/tmp/farhelm-tmp/<id>/r<N>`, per `AGENTS.md`, "Agent scratch space"). It holds plain text files a resumed session can
read back: `plans.txt`, `base.txt`, `prs.txt`, `tested_commits.txt`, `diffs/` and `notes/`. `landing.py`'s module
docstring describes each.

## A landing round, in order

Commands run from the checkout root, and `landing.py` is short for `scripts/plans-landing/landing.py`. Placeholders:
`<round>` is the round directory, `<slug>` a plan, `<claim>` the id `claim` prints.

- Apply the clean-main boundary by hand (`plans/AGENTS.md`), then
  `scripts/plans-landing/landing.py round-start --round-dir <round>`. It refuses a dirty working copy, runs
  `queue status`, and records the `[complete]` plans (in queue order, each with its bookmarks, now tracked) and the
  status head. The round's plans are fixed from here on.
- `landing.py bases --round-dir <round>`: each plan's pushed base, main's non-bookkeeping commits since it, and any
  `plans/` path its stack touches (step 3).
- `landing.py stack --round-dir <round>`: stacks the plans in queue order on `main@origin` and lists each conflicted
  commit with its files. Resolve conflicts by hand as a careful rebase.
- `landing.py diffs --round-dir <round> --oldest-base <commit>`: writes each plan's diff and main's landed work since
  the oldest base to `<round>/diffs/`, the cross-plan review's input for the monitor and its sub-agent.
- `landing.py pr-map --round-dir <round>`: writes `prs.txt`, refusing a bookmark without exactly one open PR.
- `landing.py tested --round-dir <round>` when the combined checks start, then run the checks.
- For each plan, in queue order:
  - `landing.py claim --round-dir <round> <slug>`: step 1. Writes `starting` to the log, runs `queue start-landing`
    (exit 10: no longer complete, skip it) and prints the claim id.
  - `landing.py land --round-dir <round> <slug> <claim>`: step 5. For each PR bottom-up it rebases onto `main@origin`,
    refuses a conflict, retargets the PR to main, confirms the claim, pushes, waits until GitHub shows the pushed head,
    marks it ready, and squash-merges with `--match-head-commit` after confirming the claim again. Ends by comparing
    main with the tested tree. A failure leaves the claim held, for the monitor to block or stop the landing per
    `landing.md`.
  - `landing.py claim-land --round-dir <round> <slug>` runs the two back to back.
  - Write `<round>/notes/<slug>.md` with a `CHECKS_PLACEHOLDER` line, and the round's shared checks in
    `<round>/notes/_checks.md`, then `landing.py assemble-notes --round-dir <round> <slug>`, which writes
    `<slug>.final.md` rewrapped by dprint. Have it cold-read.
  - `landing.py record --round-dir <round> <slug> <claim> <pr,pr,...>`: step 7, `queue landed` with the final notes.
- `landing.py round-end --round-dir <round>`: back to a clean `main@origin`, abandons the local copies of the round's
  merged PR heads (`--skip-unmerged` keeps a blocked plan's), and prints `jj status`, the checkout's size and
  `queue status`. Prune if the size is over 50 GB, then start the watcher with that status's head.

`landing.py verify --round-dir <round> <top-bookmark>` repeats the comparison `land` ends with.

## Between rounds and around them

- Waiting: run `scripts/plans-landing/idle-watch.ts --baseline-from <commit> --wake-on-any-blocked` as a background
  command whose exit wakes the session (`plans/.agents/watching.md`), adding `--ignore-blocked <slug>` for each block
  left for the maintainer. It runs `scripts/plans-watch.sh --wake-for lander --max-wait 6600` and handles the `idle`
  exit itself as `landing.md`, "Waiting between rounds", says (status check, Status line, restart), so the model wakes
  only for a change, a flag, a block, or an error that needs it. It prints one `wake <reason>` line.
- Waiting for blocks during a round: run `scripts/plans-landing/idle-watch.ts --blocks-only --baseline-from <commit>`
  with the round's status head, plus the same `--ignore-blocked` list, in the background once `round-start` has recorded
  plans, and stop it before `round-end`'s watcher starts (`landing.md`, "A landing round"). It runs the watcher with
  `--wake-for lander-blocks`, and its idle checks wake only for a `[blocked]` plan, never for a `[complete]` one or a
  flag. Its idle checks only append to the monitor log, which the round is editing at the same time, and leave the
  Status line alone.
- Logging: `echo '<entry>' | scripts/plans-landing/landing.py log` appends a History entry and refreshes the Status line
  with the same real time.
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

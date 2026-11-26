# records backup reports 'rewritten history' for any local ref-update failure

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

When the backup cannot update its own copy (another backup run holds the ref lock, an earlier killed run left a stale
lock file, or the disk is full), it tells the maintainer that the records repo's history was rewritten and that the
backup refused it. The remote is fine and nothing was rewritten, but the alarm points at a possible destructive
force-push by an agent. If a stale lock persists, every later run repeats it.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t15a-records-tool` `f3` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept (maintainer tooling). No coverage source covers it.

### Reviewer record

Location: `xtask/src/records/backup.rs:112`

xtask/src/records/backup.rs:112-121 (reviewed commit) sets
`rewritten = stdout.lines().any(|line| line.starts_with('!'))` on the `git fetch --porcelain` output. That picks
Cause::Rewritten, whose notification is "agent-records backup refused a rewritten history in the records repo; the
backup kept the old history" (backup.rs:47-49), and whose message is "the records repo's history was rewritten; the
backup refused the non-fast-forward update and is unchanged". git-fetch(1), OUTPUT (installed man page, line 1060-1062),
defines the `!` flag as "for a ref that was rejected or failed to update", so the flag also marks local update failures
such as `cannot lock ref` / "unable to update local ref" that have nothing to do with fast-forwarding. Routes: two
schedules (or a manual run during the nightly one) running backup into the same --dest at once; a run killed mid-update
leaving `refs/heads/main.lock`, which then fails every later run the same way; ENOSPC or a permission problem on the
backup disk. The module doc and `.agents/agent-records.md`, "Backup", promise that a rewrite is what triggers this
refusal. Fix: classify Rewritten only when the porcelain line's summary/reason or the stderr
`! [rejected] ... (non-fast-forward)` line says non-fast-forward (LC_ALL=C is already forced), or confirm by checking
`git merge-base --is-ancestor <old> <new>` for each `!` line's old and new ids. Anything else falls under Fetch or a new
"backup repo could not be updated" cause. Verify by reading the git-fetch(1) OUTPUT section; no run was made.

Suggested bucket: other. A misleading alarm in maintainer tooling with no data loss: the backup does keep its history.
It would move up if the maintainer's response to a "rewritten history" alert were destructive (for example restoring the
remote from the backup over newer records), which no doc instructs.

Confidence: confirmed from source plus git's documented porcelain flag meaning. That `cannot lock ref` specifically
produces a `!` line was not observed in a run.

proposed_drop: none

possible_cover: none

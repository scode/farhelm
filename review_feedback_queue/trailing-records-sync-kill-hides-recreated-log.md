# A records tool killed mid-sync can leave a re-created log file hidden under .git

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

If the records tool is killed (timeout, SIGKILL, host reboot) during a narrow window of a sync, a record file the agent
had re-created after its own earlier push deleted it disappears from the slot's working copy without any message. It
sits under the clone's .git directory, but no later run puts it back or mentions it, so the agent continues as if it
never wrote it and the next push does not carry it.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t15b-records-tool` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept; harm is confined to agent tooling and the file is kept, not deleted. A route showing
these logs count as user-owned work lost would argue for `high`. No coverage source covers it.

### Reviewer record

Location: `xtask/src/records/clone.rs:270`

At 7f703a44e0 (unchanged at the tip), `SlotClone::sync` (clone.rs:262-273) collects 'own' paths that the new remote tree
lacks but the working tree has, then `hold` (clone.rs:307-336) renames each file to `.git/records-hold-<pid>/<i>`, runs
the two-tree `read-tree -m -u` and `update-ref` (`merge`, clone.rs:281-303), and only then `release` (clone.rs:339-366)
renames them back. Nothing records the hold anywhere durable, and no code path looks for leftover `records-hold-*`
directories. If the process dies after `hold` and before `release`, the next call sees: HEAD old or new, index without
the path (stage_remote_entry force-removed it), working tree without the file. If the merge had not run,
`matches(new, path)` finds it absent in both and treats it as already-remote; if it had, the path is not even a changed
path. Either way the agent's re-created file stays under `.git/records-hold-<pid>/<i>` silently. A second hold dir reuse
with the same pid (pid reuse after a reboot) would `fs::rename` over `<i>` and destroy the older copy. The module docs
(clone.rs:45-53) state that interrupted operations heal through rule 2 and enumerate crash points, but this one does not
heal and differs from the set-aside path, which at least prints a stderr line. The window is a few git invocations
(milliseconds), so this is rare. Fix: on `open`, look for `records-hold-*` directories and either restore their files
(if the working tree path is absent) or report them on stderr and in an error, the way set-aside copies are reported;
also name the held file by its path (like set-aside does) rather than by index so a recovery knows where it belongs.
Verify by reading the code: no reference to HOLD_DIR outside hold/release (grep `HOLD_DIR`).

Suggested bucket: other; harm is confined to agent tooling, the window is milliseconds and the file is kept under .git
rather than deleted. A route showing these logs count as user-owned work lost would argue for high.

Confidence: confirmed (source-established that nothing recovers the held files; the crash timing itself is not
observed).

proposed_drop: none

possible_cover: none

# `records path` follows a symlinked record directory out of the clone

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

If any writer of the shared records repository replaces a record's directory (a plan's directory, say) with a symbolic
link, every other host that later opens that record is told to write its log wherever the link points: another checkout,
a scratch directory, or any directory the agent's user can write. The next plan holder's log and notes then land in, and
overwrite same-named files in, that outside directory instead of the records clone, and are not pushed.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t15c-records-tool` `f6` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept. Considered for a higher bucket as a cross-host write redirection; kept in `other`
because it is maintainer tooling, the route needs a writer who already holds a write token and controls log text other
agents act on, and the redirected writes are the agent's own log files. No coverage source covers it.

### Reviewer record

Location: `xtask/src/records/mod.rs:157-162`

mod.rs:157-162 (reviewed commit; unchanged at tip) `path_with` computes `clone.root().join(&directory)`, runs
`fs::create_dir_all(&absolute)`, and prints that path; it never checks that the record directory or any of its parents
(`farhelm/`, `farhelm/plans/`) is a real directory rather than a symlink, and `create_dir_all` succeeds on a symlink
that resolves to a directory. The clone is an ordinary git checkout and git.rs sets no `core.symlinks=false` (its config
pairs at git.rs:33-34 are hooksPath and excludesFile only), so a tree entry of mode 120000 at `farhelm/plans/<slug>` is
checked out as a real symlink by the sync. Such an entry gets onto the remote two ways: (a) through the tool itself,
since clone.rs `snapshot_tree` (clone.rs:459-470 at tip) runs `ls-files --others -- farhelm/plans/<slug>`, which lists a
symlink at that path as one untracked file, and `update-index --add` stores it as a 120000 entry, and clone.rs:659-670
treats symlinks as ordinary content; so an agent that replaces its plan directory with a link (to its scratch notes,
say) and pushes propagates it; (b) any holder of a write token using plain git, which every host has. The next claimer
on another host runs `path plan/<slug>` as plans/.agents/outside-files.md instructs, receives a lexically in-clone path,
and writes `log.md` and `notes/` into the link's target on its own host. `push plan/<slug>` from that holder then
snapshots only the link again, so the holder's log never reaches the remote while the tool reports success ('nothing to
push' or a pushed commit of the unchanged link). Same for `executor`, `landing` and `round/<n>`. Verify: read
mod.rs:157-162, clone.rs:459-470 and 659-670, git.rs:33-34; source-established, not executed. Fix: in `path_with` (and
before `push` snapshots), walk the record directory's components from the clone root with `symlink_metadata` and refuse
(exit 1) when any is a symlink or a non-directory; optionally refuse to snapshot a record path whose top is a symlink,
and/or set `core.symlinks=false` for the slot clones so links check out as plain files.

Suggested bucket: other; the route needs a writer to push a symlink, and every writer already holds a write token and
controls log text that other unattended agents read and act on, which is a stronger capability than choosing where a
fixed-name log file goes. It would move to high or highest if hosts with weaker trust than the maintainer's own (a
disposable sandbox running an executor) are meant to hold write tokens and the records repo is meant to be a confinement
boundary, or if an accidental symlinked plan directory is shown to happen in practice and lose a holder's log.

Confidence: confirmed for the mod.rs behavior and for git checking out 120000 entries as symlinks with the shown config;
likely for route (a), resting on git `ls-files --others -- <symlinked path>` listing the link itself (standard git
behavior, not executed here).

proposed_drop: none

possible_cover: none

# The records tool's pattern-name test cannot see a sibling being hit

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

The records tool must treat note file names such as `[a]*.md` or `?.md` as plain names, or setting aside one stale note
could silently overwrite an agent's unpushed edit to a different note whose name the pattern happens to match. The
end-to-end test written to guard this gives its sibling files no local edits, so a pattern hitting them changes nothing
it checks; it would keep passing if the protection were removed. Only a unit test that inspects the environment variable
list still guards it.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t15e-records-tool` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept; a unit test asserting GIT_LITERAL_PATHSPECS=1 still guards the behavior. No coverage
source covers it.

### Reviewer record

Location: `xtask/tests/records.rs:1410`

Revision (synthesis, turn 2): finding preserved unchanged in substance. Additional check: the other pathspec-taking
calls on the set-aside route would not expose glob matching either. `matches` and `stage_remote_entry`
(clone.rs:628-735) run `git ls-tree -z <commit> -- <path>` and take the first entry; even if ls-tree matched the sibling
too, the odd name sorts first (`[` 0x5B before `a`, `?` 0x3F before `b`), so the chosen entry is unchanged.
`hash-object -- <path>` and `update-index --force-remove -- <path>` take file paths, not pathspecs. `only_own_changes`
(`git log -- <path>`) sees no sibling commits in the range. So the conclusion holds: with GIT_LITERAL_PATHSPECS removed,
every assertion in the test still passes, and only the env-list unit test in xtask/src/records/git.rs catches it. No
later commit in the range touches xtask/.

as in the original f1 event (set_aside at xtask/src/records/clone.rs:759-830; siblings in the test carry no local edits;
`:x.md` is never leading because paths start with `farhelm/`). Fix: give each sibling an unpushed local edit and assert
it survives with exactly three set-aside lines; optionally change a sibling from another slot in the range; drop the
`:x.md` claim or test a path where a leading colon is possible.

Suggested bucket: other — test-strength gap in maintainer tooling with the behavior still guarded by a unit test; it
would move up only if that unit test were removed.

Confidence: confirmed (source-established; git pathspec semantics reasoned, not executed).

proposed_drop: none

possible_cover: the unit test in xtask/src/records/git.rs (isolation_settings_are_always_present) asserting
GIT_LITERAL_PATHSPECS=1 partially covers the regression.

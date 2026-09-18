# Folder-history rename fails the whole refinement on duplicate display spellings

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Browsing a folder can leave its history permanently unmerged — duplicate folder suggestions that never get cleaned up —
with a warning logged on every later visit to that folder.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: possible — the two-alias precondition is unobserved in
the wild, not because the mechanism is uncertain. The reviewer verified empirically that SQLite raises
`UNIQUE constraint failed` in this shape.

`refine_folder_history`'s third statement (`crates/farhelm-helm/src/store.rs:4989-5000`) renames EVERY unproven alias
row carrying the browsed display spelling to the canonical key. `display_cwd` is not unique (the PK is on
`canonical_cwd`), so two alias rows can share one display spelling (e.g. the supervisor's canonical answer for one
submitted path changed between creates). The `NOT EXISTS` guard is uncorrelated, so it does not see the first row's
rename: the second rename violates the unique constraint and fails the whole refinement transaction. `browse_directory`
still serves the browse, but the refinement is lost and EVERY later browse of that folder fails identically — warning
spam plus permanently unmerged duplicate suggestions.

Suggested fix: rename a single newest alias (`LIMIT 1` by ordering key) and delete the remaining same-display aliases,
or merge all matches into one survivor first.

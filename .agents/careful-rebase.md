# Careful rebase

The root `AGENTS.md` section "Careful rebase", moved here with its references rewritten. Read it when asked for a
careful rebase, and wherever another procedure says to rebase "as a careful rebase" (plan execution and landing do).

> NOTE: In an unattended run (plan executor, landing monitor, delegate), an instruction here to ask, confirm with, or
> propose to the maintainer follows `AGENTS.md`, "Unattended runs".

## Careful rebase

"Careful rebase" means changes may have landed on main since the work in progress was based, and it should be rebased
onto the latest main with more care than resolving whatever textual conflicts come up. A rebase that applies cleanly can
still be wrong. A change on main can rename or repurpose something the work relies on, change a contract a new caller
assumes, add a caller of code the work changed, alter an invariant, a lock, or an ordering the work depends on, or amend
SPEC.md or SPEC_impl.md in a way the work now contradicts, all without touching the same lines.

- Fetch, then read every change that landed on main between the current base and the new one: the diffs, not just the
  titles, with the work's own diff in mind. Look specifically for functional conflicts that would not show up as textual
  ones.
- Rebase, and resolve textual conflicts as usual. For a stack, rebase every PR in it and check each against what landed.
- When main gained a database schema version since the work was based (the helm's or the supervisor's `SCHEMA_VERSION`
  in its `store.rs`) and the work adds one too, renumber the work's step onto the new end of the ladder: its migration
  branch and the version it stamps, the fresh-schema statements and their `PRAGMA user_version` literal (a bare SQL
  number the compiler cannot check against the constant), the constant itself, and any historical-schema fixtures or
  tests that name the version. The fresh schema must end up with both changes. This usually shows up as a textual
  conflict at the end of the ladder, but check it on purpose: two steps that claim the same version, or a fresh-schema
  literal that disagrees with the constant, can survive a rebase that applied cleanly, and an existing database then
  skips a migration.
- Fix obvious problems found either way as part of the rebase. A conflict that needs a design decision is the user's:
  describe it and ask rather than picking an answer.
- Make a judgment call about re-running tests, following `AGENTS.md`, "Tests and validation", and
  `.agents/validation.md`: a targeted subset covering the areas where the landed changes and the work interact, if any
  interaction carries real risk. Do not mechanically run the whole test suite. When nothing that landed touches what the
  work depends on, no runtime tests is a valid answer, and existing evidence still applies.
- Report what landed, the interactions examined and what was concluded about each, what was fixed, and which checks ran
  and why (or why none were needed).

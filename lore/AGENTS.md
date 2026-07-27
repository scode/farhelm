# lore/ rules

This directory holds historical artifacts: records of notable design and
implementation decisions, written at the time the decision was made, with
the motivation and the alternatives as they looked then.

Rules — these are strict:

- Contents here are NOT part of the codebase. Nothing in the code may
  depend on them, and no tooling should process them.
- Entries are never "maintained": when the code evolves past a recorded
  decision, the entry stays as written. A stale lore entry is working as
  intended — it documents what was believed and decided at that point in
  time.
- Entries are only added, changed, or removed on explicit user request.
  An agent recording a decision it was asked to record counts; an agent
  "tidying up" or "correcting" old entries does not.
- Naming: `YYYY-MM-DD-foo-bar-baz.md` — date of the decision, then a
  short kebab-case slug.

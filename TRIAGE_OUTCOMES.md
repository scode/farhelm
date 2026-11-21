# Triage outcomes

## attention-update-accessible-name-omits-target.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Half of this is fixed. The
  attention list's Update button now carries the host's version and whether the update is required, as its hover help,
  which doubles as the accessible description (`crates/gui/farhelm-gui/src/session_list/attention.rs:885-893`;
  `hosts/update.rs:321-357`; `components/mod.rs:122`). The accessible name is still the bare label "Update"
  (`attention.rs:895-905`), with no host. The Hosts panel's inline Update is named "Update <host>"
  (`hosts/update.rs:371-385`). With several update rows, a screen-reader user tabbing through the buttons hears
  identical "Update" names.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Attention list Update buttons don't
  say which host they update"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs,
  not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any
  of these spiral into something very complicated, the executor should make a near term todo item to follow up on it
  instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback
  file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/hosts-small-fixes.md`.

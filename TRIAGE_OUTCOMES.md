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

## trailing-ordinary-host-remove-bypasses-sealed-attempt-outcome.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): `forget_host_locked`
  (`crates/farhelm-helm/src/hosts.rs:1339-1408`) returns provider and local-cleanup errors without writing the seal
  record. Its comment at :1372-1375 says the caller writes `sealed_outcome`. But `remove_host_for` (:1211-1219, used by
  `DELETE /api/hosts/{id}`) and `remove_host_checked` (:1240-1256, used by agent removal and whole-environment session
  Delete) never do; only `finish_seal_locked` (`sessions/sandbox.rs:309-322`) records outcomes. The desktop app offers
  Delete sandbox / Delete container again on a host whose deletion failed
  (`crates/gui/farhelm-gui/src/hosts/offers.rs:297-300`), and that route takes the unrecorded path.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Deleting a host whose earlier
  deletion failed keeps showing the old failure"), with a test where practical. Size gate: about 100 lines of diff
  (code, tests and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a
  complexity gate: "if any of these spiral into something very complicated, the executor should make a near term todo
  item to follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket),
  remove the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/hosts-small-fixes.md`.

## trailing-ui-host-lifecycle-provider-and-start-wording.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): Most of this is fixed. The
  row's phase words say "Starting…" when a stopped container is starting (`hosts/identity/phase.rs:63`), "asked …" uses
  the provider's words (`phase.rs:91-107`, `environment_ui.rs:176-181`), and the Hosts Suspend tooltips are
  provider-aware (`hosts/lifecycle.rs:198,206`). One part remains: the detail line `host_evidence` (`phase.rs:406-433`)
  ignores the `starting` flag and says "Asked the container host to resume at …" during a container start (the helm sets
  `starting: verb == "start"` at `crates/farhelm-helm/src/environment/podman.rs:706`). That line shows in the Hosts
  row's details and on the session's unavailable card (`session_pane/unavailable.rs:263`).
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("A starting container's detail line
  says it is resuming"), with a test where practical. Size gate: about 100 lines of diff (code, tests and docs, not
  counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate: "if any of
  these spiral into something very complicated, the executor should make a near term todo item to follow up on it
  instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the feedback
  file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/hosts-small-fixes.md`.

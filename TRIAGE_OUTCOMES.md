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

## exit-notice-hides-oldest-scrollback-row.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop app ported the
  same layout. `crates/gui/farhelm-gui/src/terminal/attached.rs:1814-1827` (`placement`) and `lift` at 1988-2003 lift
  the shown rows by a whole row whenever the program has exited and the fitted pane has less than one spare row.
  `screen` at 2114-2149 draws them at `mt(-lift)` inside the `overflow_hidden` grid area at 2392-2398. None of this
  looks at the scroll position, and the viewport's top is clamped to the oldest retained line
  (`crates/gui/farhelm-grid-model/src/viewport.rs:60-74,103-107,115-121`). Scrolled all the way back after an exit, the
  oldest line (or the "Some earlier output could not be recovered" notice line) is clipped and no earlier scroll
  position can bring it into view.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("After an agent exits, its oldest
  output line can't be scrolled into view"), with a test where practical. Size gate: about 100 lines of diff (code,
  tests and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a
  complexity gate: "if any of these spiral into something very complicated, the executor should make a near term todo
  item to follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket),
  remove the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/terminal-exit-and-placeholder-fixes.md`.

## feedback-discord-link-omits-external-indicator.md

- Outcome: `fix code`.
- Assessment: unresolved by a delegated reader against main 28f177284 (code reading, not run): The desktop feedback
  dialog's link is still labelled just "Chat on Discord" (`crates/gui/farhelm-gui/src/feedback.rs:596-605`). However,
  the rule the finding cited ("A link that opens a page outside the app ends in `↗`") was part of SPEC_impl.md's Dioxus
  GUI section and was deleted by the old app's removal commit `1eb4e3751` (#3407). Current SPEC.md (4777-4778) names
  only the words. The desktop app is mixed: Welcome, Announcements and Checkouts links carry `↗`, while the Discord,
  Tensorlake pricing (`hosts/setup.rs:470-480`) and prompt links do not. Whether `↗` is still a convention to enforce is
  a product decision nothing current settles.
- Decision: the maintainer picked "fix code for the ones I name" for the minor cosmetic group (2026-11-21 six-question
  triage page for findings that cited removed UI code, Q6), naming "the link missing the arrow".
- Completion criteria: The feedback dialog's Chat on Discord link ends in the external-link ↗, matching the desktop
  links that already carry it. Size gate: about 100 lines of diff (code, tests and docs, not counting this ledger, the
  queue removal or a changelog fragment). Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/gui-launcher-feedback-fixes.md`.

## light-appearance-darkens-terminal-catchup.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop terminal's
  "Connecting" placeholder (`crates/gui/farhelm-gui/src/terminal/attached.rs:2170-2189`) draws its text in the interface
  palette's `fg_dim`. That is `#414b5f` in the light appearance (`theme.rs:292`). The text sits on the pane's ground
  (2447), which comes from `style::defaults(None)` and is the fixed terminal background `#282c34` in both appearances
  (`theme.rs:379-381`). That pair is roughly 1.6:1 contrast.
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes (""Connecting" text in a terminal is
  near-unreadable in light appearance"), with a test where practical. Size gate: about 100 lines of diff (code, tests
  and docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity
  gate: "if any of these spiral into something very complicated, the executor should make a near term todo item to
  follow up on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove
  the feedback file, and say so in the execution record. Remove the feedback file and its index entry.
- Execution: planned in `plans/queue/terminal-exit-and-placeholder-fixes.md`.

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

## trailing-tensorlake-add-host-inline-container-recipe-compatibility.md

- Outcome: `fix code`.
- Assessment: confirmed by a delegated reader against main 28f177284 (code reading, not run): The desktop Add host
  Tensorlake form lists every recipe in the catalog as an ordinary enabled option, with no provider compatibility check
  (`crates/gui/farhelm-gui/src/hosts/add/tensorlake.rs:479-545`). Continue needs only a name and a recipe (`:456-458`).
  The embedded recipe editor offers the "Container image" base ("Runs on Podman hosts",
  `settings/recipes/editor.rs:94-115`), and a finished Save selects the new recipe (`tensorlake.rs:24-28, 312`). The
  helm then refuses the plan with "this recipe's base cannot run on Tensorlake"
  (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs:804`), which contradicts SPEC.md:474 ("Unsupported recipes
  remain visible but disabled with a reason").
- Decision: the maintainer picked "Fix all" for the batch of small, clearly wrong things (2026-11-21 six-question triage
  page for findings that cited removed UI code, Q1), adding: "with a complexity gate, if any of these spiral into
  something very complicated, the executor should make a near term todo item to follow up on it instead of adding tons
  of complexity."
- Completion criteria: Fix within the current spec what the Assessment describes ("Add host offers container-only
  recipes for a Tensorlake sandbox"), with a test where practical. Size gate: about 100 lines of diff (code, tests and
  docs, not counting this ledger, the queue removal or a changelog fragment). The maintainer added a complexity gate:
  "if any of these spiral into something very complicated, the executor should make a near term todo item to follow up
  on it instead of adding tons of complexity"; in that case add that TODO.md entry (Near term bucket), remove the
  feedback file, and say so in the execution record. Remove the feedback file and its index entry.
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

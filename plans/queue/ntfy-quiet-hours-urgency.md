# Scheduled ntfy urgency (default priority and quiet hours)

## Goal

Allow controlling ntfy notification urgency by schedule, starting with (a) the existing default priority and (b) one
optional local-time quiet hours setting (a whole-hour range `00:00`–`23:00` supporting overnight wrap-around) that
lowers priority to a chosen level or silences ntfy alerts (`Off`) during quiet hours.

Covers and removes this `TODO.md` entry in the last code PR:

- Immediate: **Control ntfy urgency by schedule.**

### Acceptance criteria

- Settings → Notifications → ntfy alerts keeps the existing 5-level **Default priority** (`Min`, `Low`, `Default`,
  `High`, `Urgent`) and adds one optional **Quiet hours** setting: enable toggle, start hour and end hour pickers
  (`00:00`–`23:00` in this machine's local time), and a 6-way **Quiet hours priority** control offering `Off` plus the 5
  ntfy priorities (defaulting to `Low`).
- Quiet hours uses the half-open local-time interval `[start_hour, end_hour)`: overnight ranges like `22:00` to `07:00`
  match `hour >= 22 || hour < 7`; same-day ranges like `09:00` to `17:00` match `hour >= 9 && hour < 17`. Saving rejects
  invalid hours (`>= 24`) or equal start and end (`start_hour == end_hour`) with fixed error strings (never echoing
  secrets).
- Stored inside the existing `ntfy_alerts` JSON config with `#[serde(default)]` (zero SQLite schema migration; older
  saved configs decode cleanly with quiet hours disabled; older clients omitting `quiet_hours` preserve stored values).
- At send time inside `farhelm-helm`'s ntfy sender task (evaluated _before_ `RateWindow::admit` so muted alerts do not
  consume rate-limit slots), active quiet hours override the `Priority` header to the configured quiet-hours priority,
  or drop the alert when set to `Off`.
- **Send test** during quiet hours uses the effective priority right now and reports it clearly in the confirmation
  (e.g., `"Test alert sent (Low — quiet hours)"`). When quiet hours are currently active AND set to `Off`, `Send test`
  still sends at the normal saved priority so users can verify server/topic/token connectivity per `SPEC.md:2887`, and
  reports clearly in the confirmation that quiet hours are currently set to `Off` so real alerts are silenced (e.g.,
  `"Test alert sent (High — quiet hours active and set to Off; real alerts are silenced)"`).
- Local time evaluation uses POSIX `tzset` + `localtime_r` (no new timezone crate, no `TZ` env var mutation in tests)
  behind a minimal injectable local-hour source for deterministic unit tests.
- The 24-option hour pickers reuse `components/select.rs` with height capping and scrolling so the popup never overflows
  the window.
- `SPEC.md`, `SPEC_impl.md`, Settings search index, unit/GUI tests, affected goldens, and changelog fragment are
  updated.

## Decisions already made

- **Initial schedule shape (maintainer decision):** (a) default urgency, (b) one quiet hours setting (whole-hour range
  in local time).
- **Quiet hours priority options (maintainer decision):** Offer all 5 ntfy priorities (`Min`, `Low`, `Default`, `High`,
  `Urgent`, defaulting to `Low`) PLUS `"Off"` to suppress ntfy during quiet hours.
- **Send test behavior (maintainer decision):** Use the effective priority right now and say so in the confirmation
  (e.g., `"Test alert sent (Low — quiet hours)"`); see Acceptance criteria above for the `"Off"` connectivity-test
  resolution grounded in `SPEC.md:2887`.
- **Review gate (maintainer decision):** Single fresh-context agent on **Opus 5.5** at **high** reasoning effort with
  the general review charter below.

## Implementation outline and requirement sources

### Requirement sources

- **User request and decisions:** Default urgency + one local-time quiet hours setting (hour range, 5 priorities +
  `Off`, Send test showing effective priority / quiet hours) + settled release note below.
- **Binding repository constraints:** `SPEC.md` ("Settings → Notifications → ntfy alerts", including line 2887 on
  `Send test` verifying connectivity even when alerts are off, and "ntfy alerts" around lines 3417–3443), `SPEC_impl.md`
  ("Clock times: local time", "Ntfy waiting alerts"), `scripts/check-crate-layering.py` (`farhelm-proto` stays
  IO/`libc`-free), `crates/gui/clippy.toml` (reuse Farhelm's `components/select.rs`), user global rule (never mutate
  `TZ` or any env var in tests), `AGENTS.md` ("Changelog fragments", "Tests and validation"),
  `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- **Planner-proposed implementation choices (reviewed by fresh-context planning review):**
  - **Proto types (`crates/farhelm-proto/src/ntfy.rs`):** Add one nested
    `QuietHours { enabled: bool, start_hour: u8, end_hour: u8, priority: Option<Priority> }` (default
    `enabled: false, start_hour: 22, end_hour: 7, priority: Some(Priority::Low)`), reusing `Option<Priority>` (`None` =
    `"Off"`) rather than duplicating the 5-variant enum. Add pure `QuietHours::contains_hour(hour: u8) -> bool` and
    `Settings::effective_priority(local_hour: u8) -> Option<Priority>`, plus a small `TestOutcome` response struct for
    `POST /api/ntfy-alerts/test` (`200 OK` JSON; `operations::send` already handles both `204` and `200` JSON cleanly).
  - **Validation:** Validate `start_hour <= 23 && end_hour <= 23 && start_hour != end_hour` unconditionally on save in
    `Config::saved` with fixed error strings (never echoing secrets). Do not add client-side blocking of `start == end`
    in the GUI pickers (let Save surface the helm error so swapping `22:00–07:00` to `07:00–22:00` is never awkward).
  - **Local-time lookup:** Do NOT put POSIX FFI in `farhelm-proto` and do NOT enable `chrono`'s `clock` feature. Export
    two tiny panic-free functions from `farhelm-helm` (`refresh_local_time_zone` + `local_utc_offset_at`) for
    `crates/farhelm-desktop/src/time_zone.rs` to delegate to (avoiding two copies of
    `unsafe { tzset(); localtime_r(...) }`), or keep a ~15-line private helper in `crates/farhelm-helm/src/ntfy.rs` (log
    choice as `DECISION`). Inject a single `Arc<dyn Fn() -> u8 + Send + Sync>` local-hour source in `Alerts`'s test
    constructor.
  - **Sender task & GUI:** Evaluate quiet hours only in `Alerts`'s sender task (before `RateWindow::admit`) and in
    `Alerts::test`, leaving `Policy`, `ntfy_waiting_cache`, and `manager.rs` untouched and keeping the Settings GUI
    clock-free. Generalize `crates/gui/farhelm-gui/src/components/select.rs::SelectList` to cap its height to the window
    content area and scroll so 24 hour options fit cleanly. Place the 6-way Quiet hours priority
    `RadioGroup<Option<Priority>>` on a full-width row rather than inside the half-width `grid_cols(2)` cell. Add
    `quiet_hours` to `Form` in `crates/gui/farhelm-gui/src/settings/ntfy.rs` (automatic dirty tracking/discard) and
    index the new controls in `settings/search.rs`.

### PR shape (2 PRs)

1. **`plan/ntfy-quiet-hours-urgency/01-proto-and-helm-quiet-hours`:** Add `QuietHours`, `contains_hour`,
   `effective_priority`, and `TestOutcome` to `farhelm-proto::ntfy`; add validation, send-time quiet-hours resolution
   (before `RateWindow::admit`), injectable local-hour source + POSIX local-time helper, and `200 OK` `TestOutcome`
   response in `farhelm-helm/src/ntfy.rs` (delegating `farhelm-desktop/src/time_zone.rs` if shared); add deterministic
   unit tests covering overnight wrap-around, same-day range, `Off` suppression without burning rate-limit slots,
   `start == end` refusal, and `Send test` outcomes.
2. **`plan/ntfy-quiet-hours-urgency/02-settings-ui-and-docs`:** Add height-capped scrolling to
   `components/select.rs::SelectList`, add Quiet hours toggle, `00:00`–`23:00` start/end pickers, full-width 6-way
   priority `RadioGroup`, and `TestOutcome` feedback copy to `settings/ntfy.rs` + `settings/search.rs`; update
   `SPEC.md`, `SPEC_impl.md`, GUI tests/goldens, changelog fragment, and remove the `TODO.md` entry.

## Scope reassessment

Before implementing a substantial departure from the outline above, run a fresh-context review at the executing
session's own model and effort with the following charter verbatim. Supply the original request, settled decisions,
outline, current diff, and proposed departure (what changed, why it is necessary, and which simpler alternative was
ruled out). A close reuse-versus-duplication call not foreseen during planning is a routine fork when every option stays
within the agreed scope: decide after reassessment and log as `DECISION`. Repeated corrective reviews of the same
component also require reassessing necessity before another repair round. Give any same-model sub-agent bounded units
within the outline; unapproved new mechanisms are outside its task.

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

## The checkout

This goal runs inside the checkout the executing session runs in, identified as the nearest Git worktree or jj workspace
containing the session's working directory whose remotes include `scode/farhelm` (comparing host, owner, and name,
ignoring `.git` suffix and SSH vs. HTTPS). Every repository path in this file is relative to that checkout's root.
Confirm this first; if the nearest checkout is not `scode/farhelm`, stop before the resume protocol, write nothing
(including the log), and report that the goal must be run from a checkout of `scode/farhelm`.

## Resume protocol

Right after confirming the checkout, resolve the plan's working log directory with
`cargo xtask records path plan/ntfy-quiet-hours-urgency` (`plans/.agents/outside-files.md`, "Files outside the
repository"). Its `log.md` is the working log. Invoke the `agent-resumeable` skill with that resolved path: if `log.md`
already exists, read it in full and resume where the previous holder left off (cross-checking against VCS state and open
`plan/ntfy-quiet-hours-urgency/*` PRs on GitHub); if it does not exist, this is a fresh start.

## Execution mode

Run in **no-workhorse** mode. Do not send any unit of the executing agent's own decomposition to a weaker model or lower
reasoning effort than the executing session's own. Do not invoke, load, or follow `scode-galaxy-brain` or
`scode-model-routing` at any point; if `scode-galaxy-brain` is already active, this goal file is the maintainer's
express instruction to stop it.

Same-model sub-agents at the executing session's own model and effort are allowed when they are likely to finish work
sooner without costing quality; concurrent writers may use isolated worktrees or jj workspaces that the executing agent
merges back, while the executing agent owns the `jjstack` stack and every PR. The fresh-context read-only sub-agents
required by `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked questions and
reports), the scope reassessment review (at the executing session's own model and effort), and the per-PR review gate
below are explicitly required and exempt from any no-delegation restriction. Launch any sub-agent whose demanded model
is not available natively through `$scode-harness-shellout`.

## Resource watchdog

Mandatory before starting implementation or delegations. Resolve the `scode-build-goal` skill's `scripts/watchdog` path
via `realpath` at run time. Put the watchdog status file at `farhelm-plan-ntfy-quiet-hours-urgency-watchdog.json` in the
parent directory of the executing checkout (derived per `scode-build-goal`, "Placing the files") — never inside the
`plan/ntfy-quiet-hours-urgency` record directory.

- **Watched paths (`--path`):** the executing checkout root, `$HOME`, `/tmp`, the directory holding the status file, the
  executing session's scratch directory, and any worktree/build directory on another filesystem (restart the sampler if
  added). Interval: 60 seconds; default alert levels.
- **Harness setup & verification:** Before starting the real monitors, verify the delivery mechanism once per session on
  throwaway status files in the scratch directory with `--interval 5` (`--synthetic-alert`, killing via status `pid`,
  and `--stall-after 2` watched by an observer armed first). On Claude Code, run `observe` and `sample` under separate
  Monitors (30-minute timeout, observer first); on Codex, arm `observe --exit-on-event` then `sample` through
  `skillette-codex-wake-on-exit`; otherwise log the fallback, run the sampler in the background, and run
  `watchdog check --status <file>` before every launch and at natural boundaries.
- **Acting on shortages/failures & lifecycle:** On `ALERT`/`WORSE` or failing `check`, pause launches, clean run-owned
  build leftovers/worktrees, and resume only once `check` passes (if nothing run-owned can free enough, block/unclaim
  per `plans/.agents/executing.md`). Record monitor handles, watched paths, and delivery mechanism in the working log;
  on resume, stop any still-live prior monitors before starting new ones; stop owned monitors when the plan closes.

## Stack and PR discipline

Build a linear stack of draft PRs using the `jjstack` skill on top of `main@origin` (or this plan's own open
`plan/ntfy-quiet-hours-urgency/*` PRs when resuming, never another plan's PRs), set up per `plans/.agents/executing.md`,
"Executing one plan". Use bookmark names `plan/ntfy-quiet-hours-urgency/<nn>-<short-name>`. Keep every PR a draft —
never mark a PR ready and never merge. Restructure existing PRs in the stack instead of stacking fixup commits on top,
including PRs created by an earlier run. Never edit anything under `plans/`.

Follow `AGENTS.md`, "Conventional Commits", "Changelog fragments", and "Tests and validation", plus
`.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md`. The last code PR removes the
covered `TODO.md` entry.

## Review gate

Before finishing any PR in this stack, delegate an independent fresh-context review to a single agent on **Opus 5.5** at
**high** reasoning effort (explicit model and effort demand; launch natively if supported or via
`$scode-harness-shellout` without hardcoding a CLI command here). Give the reviewer no conversation context from the
executing session; supply the repo root of the executing checkout, the commit range or bookmark to review, the private
findings file path in the executing session's scratch directory, `.agents/test-authoring.md` verbatim when tests or
fixtures changed, and this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved. Report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

Address all valid findings and re-verify before moving on.

## Decision logging

Log major design decisions in the plan's working log as they happen with a scannable `DECISION` label, noting the
alternatives considered.

## Unattended fallback

This plan runs unattended. Never ask the maintainer anything in conversation — not a question, not an approval prompt,
not an optional offer, and not a harness question tool prompt (`plans/AGENTS.md`, "Unattended means no questions").
Resolve routine implementation forks within the agreed scope, log consequential choices as `DECISION`, and carry on. If
a required decision goes beyond the agreed scope, weakens a guarantee, or hits an unresolvable conflict, block per
`plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put non-blocking notes and possible follow-ups
in the closing report.

## Done criterion

Every PR in the stack exists on GitHub as a draft, has passed its Opus 5.5 high review gate and applicable validation,
removes the covered `TODO.md` entry in the last code PR, and satisfies the latest `## Decisions` entry (if any).
Delivering the report is the executor's closing step (`plans/.agents/executing.md`), not part of the goal.

## Release note

**You can now set quiet hours for ntfy alerts.** Choose a local-time hour range in Settings to automatically lower or
silence ntfy alert priority during off-hours.

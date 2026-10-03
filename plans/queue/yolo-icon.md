# Honest permission marks: a slashed shield for YOLO, a shield for known-safe, a question mark otherwise

Written against main at cd8bd112 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

## The goal

Every session row in the sidebar carries a permission mark that tells the truth about whether the agent can act without
asking, and never claims "not YOLO" on a guess:

- an amber shield with a slash through it for a YOLO session;
- a green plain shield for a session launched from the standard launch composer with a permission mode that asks for
  approval;
- an amber question mark for every other session (launched from a profile or a custom command line, Farhelm cannot
  know), unless its command line is recognized as YOLO, which gets the slashed shield.

Getting there takes four changes, each its own PR: the internal "sensitive host" vocabulary renamed to the user-facing
one; one shared YOLO classifier used by both the sidebar and the YOLO confirmation; the launcher treating OpenCode, OMP
and Goose the way it treats Pi (their stock default does not ask, so their default is YOLO); and the new marks.

Acceptance criteria:

- Marks (PR 4). Structured (composer) launch whose effective permission is YOLO, or any launch the shared classifier
  calls YOLO: slashed shield, `--warn` amber. Structured launch whose effective permission is not YOLO: plain shield,
  `--ok` green. Everything else, including plain shells and `codex --full-auto`: question mark, `--warn` amber. Every
  row carries exactly one mark. Hover and screen-reader text still name the specific mode (default, approve, smart
  approve, chat, YOLO) or say the mode is unknown because the session came from a profile or a custom command. The blue
  full-auto mark and the padlock geometry are gone.
- Classifier (PR 2). Exactly one implementation in `crates/farhelm-proto/src/yolo.rs` answers "is this launch YOLO" for
  structured selections and for raw command lines and profile invocations. The helm's YOLO confirmation guard and the
  sidebar row both call it; nothing else re-derives the answer. SPEC_impl.md states that the row and the guard must
  agree and do so by calling the same code.
- Launcher (PR 3). OpenCode, OMP and Goose have no "default" permission button. OpenCode offers only yolo (like Pi), OMP
  yolo and approve, Goose yolo, approve, smart approve and chat; YOLO is preselected. An omitted permission on these
  harnesses means YOLO everywhere Pi's does today (helm normalization and compilation, browser display and submit, the
  YOLO confirmation, saved selections and older sessions). A harness's own forced or default YOLO never carries over as
  a preselected YOLO into the next launch of a harness that has a real non-YOLO default (this also fixes Pi's existing
  spill); an explicit YOLO choice on such a harness is remembered as today.
- Rename (PR 1). The internal "sensitive host" names match the user-facing vocabulary (a host "asks before YOLO
  launches" unless it is set to "start YOLO sessions without asking"; the override is "confirm YOLO"), including REST
  fields and route, the response-header value, the helm↔supervisor protocol field (with a protocol version bump) and the
  database column. The CLI's hidden `--allow-yolo-on-sensitive-host` alias keeps working (SPEC.md promises it).
- SPEC.md, SPEC_impl.md and any docs page that describes these behaviors are updated with each PR; the TODO.md entry is
  removed by PR 4.

## Requirement sources

**The user's request:** "let's plan the following, each in its own plan: YOLO icon, ..." for the TODO.md `Near term`
entry, verbatim as of cd8bd112: "**Replace the YOLO session icon.** The sidebar marks a YOLO session with an open
padlock, but the other permission modes use a closed one, and the difference is hard to see. That makes it easy to read
as "this session is locked down", which is the opposite of what it means. Replace it with a mark that cannot be mistaken
for a lock; the replacement is TBD. The marks are drawn by `PermissionIcon` in `crates/farhelm-ui/src/icons.rs`."

**The user's decisions (2026-10-02), in their words where quoted:**

- U1. Shape and color: a shield with a slash through it for YOLO, amber; "lets do plain shield" for the safe case, "then
  use green for shielded".
- U2. Who gets the green shield: "i have never in my life seen the closed padlock. didn't even know this was a thing. so
  current behavior is broken. we need it to be all that that are NOT yolo and were spawned with standard launch settings
  and not a custom command line." Then: "and then use a question mark in same amber color when we don't know". (Today a
  structured launch with the harness default records no permission and shows no mark at all; the closed padlock only
  appears for raw `codex --full-auto` and Goose's explicit modes.)
- U3. Inference only ever points one way: "if we infer likely yolo show it as yolo. but the INVERSE does not happen,
  becaus claiming it's "not yolo" cannot have false positives." And: "if its custom, then NEVER green shield. i only
  meant it if it was standard. if it's inferred from a custom cmdline, then it's a question mark since it's not believed
  to be yolo and we cannot reliably say green shield for anything inferred from cmdline". And: "we cannot reliably tell
  agent vs. non-agent. so it's all question mark" (plain shells get the question mark too).
- U4. One classifier: "Yes we should have a _single_ way this is classified. If this is implemented in multiple places
  in different ways we need to re-factor so there is precisely one way the information is inferred." And: "that should
  be SPEC_impl - that it has to match, AND actually implemented using same logic".
- U5. Harnesses whose default does not ask: told that every harness except Pi shows a "default" permission button and
  that the stock defaults of OpenCode ("Most permissions default to allow"), OMP (`yolo` is its default approval mode)
  and Goose ("Autonomous Mode is applied by default", the `auto` mode Farhelm writes for a Goose YOLO launch) do not
  ask, the user said: "yes slashed then and the launcher must be changed to work like Pi then since that seems to be the
  behavior." They accepted, once told: a default launch of these on a host that asks before YOLO launches now needs YOLO
  confirmation, and OpenCode now launches with Farhelm's OpenCode YOLO setting (`--auto`), so it stops asking for the
  two things its stock config still asks about (paths outside the project, repeated identical tool calls).
- U6. Remembered permission: a harness's forced YOLO must not carry over as a preselection to harnesses with a real
  choice, fixing Pi too (the user chose this option).
- U7. Rename: "i think the internal code probably still has the "sensitive" wording, but we changed the naming schemere
  here for the user, and we should try to keep theimplementation more aligned with the user facing wording." Then, on
  the proposal: "rename but including protocol change, we do protocol bumps all the time it's not a big deal." Then,
  told the database column rename makes the release that carries it impossible to downgrade in place: "rename".
  Placement: "in same plan with correct order, plan execution is no longer linear due to recent changes. we have
  concurrent agents executing these things."
- U8. The README hero screenshot is refreshed by the maintainer separately; do not run `scripts/readme-screenshot.sh` or
  `scripts/publish-readme-hero.sh`.
- U9. Review gate: a fresh-context Opus 5.5 reviewer at high effort. No-workhorse mode (see How to run).

**Binding repository constraints:**

- Root `AGENTS.md` "Harness-specific code": per-harness facts are exhaustive functions in the places the map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` names; no `kind == X`, `matches!` over harnesses, or `_` arms in
  shared code. Update the map where it names `sole_permission`.
- `crates/farhelm-proto/src/launch.rs`: `offers_permission` is the one statement of the per-harness permission sets;
  `sole_permission` is the one statement of Pi's rule. Keep one table per fact.
- SPEC.md passages that change: the permission vocabulary and Pi/OMP/OpenCode/Goose launch paragraphs (~195-216,
  ~386-389), the composer search's `perms:default` rule (~408; `perms:` is the launch composer's search-box action, not
  a CLI flag), the YOLO launch paragraph (~455-480, including that clone/replace of an older default OMP, Goose or
  OpenCode session now counts as YOLO for the confirmation), and the session row's "permission mark" (~805).
  SPEC_impl.md: the glyph passages (~223-235, including OMP's "honest absence"), ~513, ~2074-2093, the "sensitive-host
  guard" wording (~2079). Each PR updates the passages its behavior touches.
- Protocol bumps: `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` goes 35 → 36 and the pin test
  `protocol_version_is_pinned_at_35` is renamed, as its doc requires at every bump. Precedent #970: `refactor:` with a
  `kind: changed` changelog fragment telling users to update their hosts.
- Database: `crates/farhelm-helm/src/store.rs` schema history and migrations; a new migration bumps `SCHEMA_VERSION` (31
  today) and updates the schema-history doc and the downgrade fixtures (store.rs and helm `lib.rs`).
  `releasing/AGENTS.md` "Downgrade check" describes what a schema bump means for a release; the changelog fragment must
  say the release cannot be downgraded in place.
- `docs/harness-marks.md` covers harness marks only and its sizing rule targets vendored marks; the permission marks are
  hand-drawn in the 12-unit viewBox like the padlock. Fix its passing "permission lock" wording.
- History files (CHANGELOG.md, TRIAGE_OUTCOMES.md, lore/) are not renamed.

**Planner choices (from two planning reviews):**

- P1. One new exhaustive per-harness fact on `LaunchHarness`, e.g. `omitted_permission() -> Option<LaunchPermission>`
  ("what an omitted permission means"): `Some(Yolo)` for Pi, OpenCode, OMP and Goose, `None` otherwise. Replace
  `sole_permission` with it, or derive "offers nothing but YOLO" from it plus `offers_permission`; do not keep two
  hand-maintained tables. The effective permission of a structured selection is one proto helper
  (`permissions.filter(offers_permission).or(omitted_permission())` or equivalent) used by the browser and the
  classifier; the helm keeps its stricter form (fill only when omitted, refuse an unsupported explicit value).
- P2. The classifier is a deletion: `invocation_marker`, `invocation_switches`, the precedence/rank logic,
  `InvocationMarker` (and its `FullAuto` row), and the guard-only split collapse into flag tables read by
  `argv_is_yolo`. Move `yolo_guard::invocation_is_yolo` (shell-split then `argv_is_yolo`) into the proto yolo module;
  the row and the guard call the same `selection_is_yolo` / `invocation_is_yolo` pair. Keep a test that `--full-auto` is
  not YOLO. Nothing outside the row consumes the marker labels (the row renders no marker text; "skip-perms" survives
  only in a stale doc comment).
- P3. Raw `omp`, `goose` or `opencode` without a YOLO flag stay "not recognized as YOLO" for the classifier and get the
  question mark: teaching the argv classifier vendor defaults would be a new mechanism, and U3's question mark already
  covers the case honestly. SPEC.md says so in one sentence.
- P4. Consumers of P1 that follow automatically: helm `normalize_selection`, `selection_is_yolo`, browser
  `normalized_permissions` (row detail text, recents, reset, submit), the remembered-permission seed, and the composer
  search's `perms:default` (offered only when the normalized omitted permission is `None`). Consumers that need edits:
  the permission buttons in `launch_controls.rs` (show "default" only when `omitted_permission()` is `None`; drop the Pi
  branch), the YOLO confirmation's "only mode" sentence in `yolo_confirm.rs` (derive it from "offers nothing but YOLO"
  and reword so it is true for OpenCode, e.g. "Farhelm offers {name} only in YOLO mode, so every {name} session runs
  this way"), and the recents dedup in `launch_composer.rs` (compare normalized selections, so an old omitted row and a
  new yolo row are one recent).
- P5. U6's remembered permission: the helm (`sessions.rs` records the normalized selection; `store.rs`) and the browser
  mirror (`list/view.rs`) must not record a harness's omitted-means-YOLO as an explicit YOLO choice. The cheap shape is
  to remember the submitted permission before normalization, or to record nothing when the chosen permission equals the
  harness's `omitted_permission()`; pick the smaller and log it. An explicit YOLO on a harness whose default is not YOLO
  is remembered as today.
- P6. Rename map (U7): `allow_yolo_on_sensitive_host` → `confirm_yolo`; `yolo_safe` / `set_yolo_safe` →
  `yolo_without_asking` / `set_yolo_without_asking` (Rust fields, store API, REST JSON, UI, e2e helpers including
  `yoloSafe` / `setLocalYoloSafe`); REST route `/api/hosts/{id}/yolo-safe` → `/api/hosts/{id}/yolo-without-asking`;
  `YoloOnSensitiveHost` → `YoloNeedsConfirmation`; the `x-farhelm-yolo-confirmation` header value `"sensitive-host"` and
  its constant `YOLO_CONFIRMATION_SENSITIVE_HOST` → `"confirmation-required"` / `YOLO_CONFIRMATION_REQUIRED` (it is a
  response-header value, not an error code; pick final spellings and log them); the DB column via
  `ALTER TABLE hosts RENAME COLUMN`; the log line "host yolo-safe setting changed"; comments, docs and test names. The
  browser and the helm always ship together, so REST spellings need no compatibility alias.
- P7. Marks: hand-drawn stroke-only SVG in the existing 12-unit viewBox, fitting the 10×12 `.sidebar-glyph` slot; keep
  `data-glyph` tokens with `yolo` unchanged (tests and the README hero stage find YOLO rows by it) and add `shielded`
  and `unknown` (final names logged). The badge's permission mark becomes non-optional; the two-column grid stays. Do
  not use a triangle or "!" (that is the session-ended error mark). The green is the same `--ok` the running status
  uses; that overlap is accepted.

## Implementation outline

Four PRs, in this order. Line numbers drift; find code by name.

1. `refactor:` rename the "sensitive host" vocabulary (P6, U7). Mechanical across proto (`AgentVerb::Create`, `Clone`,
   `ResolveProfile`, the supervisor-local request, `http.rs`), helm (`sessions.rs`, `agent_requests.rs`, `client.rs`,
   `yolo_guard.rs`, `hosts.rs`, `store.rs` with the column migration, `lib.rs` route), supervisor (`service/core.rs`,
   `service/handlers.rs`), the CLI (`crates/farhelm/src/main.rs`, `agent_client.rs`, `agent_instructions.rs`; keep the
   hidden alias), UI (`api.rs`, `list/create_form.rs`, `hosts.rs`, `hosts/settings_dialog.rs`, `desktop.rs`,
   `list/shared.rs`, `yolo_confirm.rs`, `github_checkout.rs`, `restart_with.rs`), `http-contract/host-list.json`, e2e
   specs and helpers, Rust e2e tests, SPEC_impl.md. Protocol 35 → 36. Changelog fragment `kind: changed`: update your
   hosts, and this release cannot be downgraded in place.
2. `fix:` one YOLO classifier (P2, U4). Delete the badge parser; the row calls `selection_is_yolo` /
   `invocation_is_yolo`; `PermissionGlyph::FullAuto`, its CSS rule and token go (raw `codex --full-auto` rows show no
   mark until PR 4). SPEC_impl.md states the single-source rule. Changelog fragment (`kind: fixed`): the sidebar now
   marks every launch the YOLO confirmation treats as YOLO (raw `pi`, `omp --approval-mode yolo`, Grok's YOLO flag,
   Claude's `--permission-mode bypassPermissions`, flags after other options), and no longer marks `codex --full-auto`.
3. `fix:` (or `feat:`; log the choice) OpenCode, OMP and Goose default to YOLO like Pi (P1, P4, P5, U5, U6). The default
   launches now compile to `opencode … --auto`, `omp … --approval-mode yolo`, `env GOOSE_MODE=auto goose
   session`.
   Update the browser specs that click "default" or expect an omitted permission for these harnesses
   (`e2e/tests/opencode.spec.ts`, `omp-composer.spec.ts`, `model-defaults.spec.ts`), the Rust structured-launch e2e
   tests, and the launches.rs / launch_composer.rs unit tests on omitted OMP and OpenCode permissions; hosts start out
   asking before YOLO launches, so those default launches now meet the confirmation. Changelog fragment
   (`kind: changed`): these harnesses now launch in YOLO unless approve is picked, a user's own vendor config that asks
   is overridden by the explicit flag, OpenCode stops asking about paths outside the project, and default launches on a
   host that asks before YOLO launches now ask.
4. `feat:` the three marks (P7, U1-U3) in `icons.rs` (`PermissionIcon`, `PermissionGlyph`), `list/row.rs`
   (`agent_badge`, `permission_description`, the hover wrapper text), `app.css` (colors; rewrite the lock comments),
   unit tests for the tier mapping, e2e updates (`sidebar.spec.ts`, `terminal.spec.ts` including the `sleep 300` no-mark
   case and the `/permission bypass|full-auto/` title pattern, `omp-composer.spec.ts`), the
   `docs/readme-hero/scenario.json5` comment, `docs/harness-marks.md` wording, and the stale lock wording in `icons.rs`,
   `app.css` and `yolo.rs`. SPEC.md states the three-tier rule; SPEC_impl.md replaces the padlock passages. Changelog
   fragment `kind: changed`. Remove the TODO entry.

### What not to build

No per-harness "default asks" table beyond P1's one fact; no argv parsing of vendor defaults or env-var modes (P3); no
second classifier for display; no REST compatibility aliases; no screenshot refresh; no change to the guard's policy
beyond what the shared classifier and P1 imply.

## Plan-specific notes

### Validation

Likely relevant, not a checklist: `cargo fmt --all -- --check`; both clippy runs; the proto yolo and launch unit tests,
helm `launches`, `yolo_guard`, `sessions`, `store` (migration and downgrade fixtures) and `hosts` tests, farhelm-ui
`row`, `launch_composer` and `yolo_confirm` tests; the Rust e2e tests that carry the renamed field or launch OMP,
OpenCode or Goose structurally; `cargo check -p farhelm-ui --features desktop`; the browser specs named in the outline
plus `yolo-guard.spec.ts`, `restart-with.spec.ts`, `terminal-restart.spec.ts`, `clone.spec.ts`, on Chromium and WebKit
through the recorder; `python3 releasing/check-changelog.py format`.

### Decisions to log

The final names in the rename (route, header value, constant); the shape of P1 (replace or derive `sole_permission`);
how P5 avoids the spill; the PR 3 type; the mark geometry and `data-glyph` tokens; the hover wording for the unknown
mark; and anything you found that the planning reviews did not anticipate.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-yolo-icon-log.md` in the
parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/yolo-icon/<nn>-<short-name>`.
- One commit, bookmark and draft PR per PR in the implementation outline, as a linear stack in the order given. Err on
  the side of bite-sized PRs, but do not create churn: code added in one PR and deleted in a later PR of the same stack
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to all of this plan's open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits with the types the outline names. A `feat`, `fix`, `perf`,
  `style` or `revert` PR, or one whose type carries `!`, adds its changelog fragment under `releasing/changelog.d/` in
  the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`). Never edit `plans/` in a PR; this plan's state moves only through
  `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.

### Validation

Follow root `AGENTS.md` "Finishing work" for each PR: the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution, with the narrow-tests recipe in `.agents/narrow-tests.md` for
reproductions. When tests or their fixtures change, apply `.agents/test-authoring.md` and run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`. `dprint check` on changed Markdown. The
plan-specific notes above list the checks this work most likely needs; they are suggestions, not a checklist. Report
checks run, reused (with the covered revision) and skipped (with the reason) in the report.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot reach that
model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: this file's goal, the user's request and decisions, and the
planner choices. When the PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on, and log the DECISION where you decline a
finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (new persistent state, a new endpoint, protocol
message or subsystem, a compatibility layer, or anything the "What not to build" list names; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the ones the plan-specific notes above name, and every reviewer finding you
declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: push what is consistent, record the concrete tradeoff, continue independent authorized work, and
block per `plans/AGENTS.md` (Executing one plan, step 10) for what depends on it.

## Done criterion

The plan is complete when every PR in the implementation outline exists as an open draft, the stack satisfies the
acceptance criteria, every PR has passed the review gate, and the last code PR has removed the TODO.md entry this plan
covers. Open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

# Launch kinds and templates: replace profiles, stop parsing command lines, restart only resumes

Written against main at adb2bf0c on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

The product behavior is already written down. The plan's first PR, `plan/launch-representation/01-spec`
(https://github.com/scode/farhelm/pull/1537, a draft the maintainer reviewed and approved during planning), rewrites
SPEC.md for this change; the rest of the stack implements it. Read that PR's SPEC.md in full before anything else: it is
the acceptance specification, and where this file and that SPEC.md disagree, that SPEC.md wins. A review page made
during planning, https://snippets.scode.org/s/farhelm-launch-kinds-spec/, shows its changed paragraphs beside the
previous wording; it is background, not a source of requirements.

This plan runs after `plans/queue/identity-report-wait-retry.md` and `plans/queue/remove-identity-heuristics.md` (its
`INDEX.md` line says so). Both rework conversation reporting, in code this plan rewrites. The first changes hook budgets
and timeouts, and how hook reports are admitted. The second removes Claude's conversation capture by record scan from
`agent_kind/` and `service/capture.rs`. Read what they landed before starting. Names in this file come from main at
adb2bf0c and may have moved. `desktop-internal-helm.md` was also in flight; it changes only the desktop app's helm and
is not a dependency, so do a careful rebase (root `AGENTS.md`) if it lands under the stack.

## The goal

Today a session is launched from one of four representations: structured choices in the New dialog, a built-in profile,
a user profile, or a raw command line, and a profile or raw create can carry a separate resume command. Each path
decides YOLO, resume and restart its own way, and command lines are parsed to guess what they mean: which agent they run
(program basename), whether they skip approval prompts (flag tables in `crates/farhelm-proto/src/yolo.rs`), and how to
resume them (appending a per-agent selector to the stored argv, with per-agent refusals and stripping in
`crates/farhelm-supervisor/src/agent_kind/mod.rs`). That produced a long run of bugs, recorded in `TRIAGE_OUTCOMES.md`
under headings such as `yolo-guard-skips-resume-template.md`, `claude-resume-template-selector-collision.md`,
`codex-resume-template-duplicates-selector.md`, `yolo-guard-misses-env-prefix.md` and
`create-reply-sets-remembered-yolo.md`.

After this plan, the system matches the 01-spec SPEC.md:

1. Profiles no longer exist, built-in or stored, and nothing converts them.
2. Every launch has a launch kind. An agent launch is an agent type (harness) plus its structured choices and no custom
   arguments; Farhelm composes its start and resume commands. A command launch is a user-written command with a required
   YOLO assertion, optionally a declared agent type (which requires `{farhelm_args}` exactly once as a whole argument),
   and, with a declared type, an optional resume command containing `{conversation}` and `{farhelm_args}`.
3. Farhelm never parses a command line to decide YOLO, agent type, or resume. The YOLO guard and the sidebar mark use
   the agent launch's permission or the command launch's assertion.
4. Restart, plain or with changes, always resumes the session's own conversation, and is offered only when the agent
   type is known and a resumable conversation is captured. There is no fresh restart and no fallback-template restart.
5. Named launch templates are partial launcher edits, applied in the GUI (`tl:name`) and on the CLI (`--template`,
   repeatable), managed in a Templates panel, never recorded on a session, and not writable by agents.
6. The New dialog shows the two launch kinds as tabs.
7. `farhelm spawn` and `farhelm agent create` take the launcher's fields as flags. `farhelm agent templates` replaces
   `farhelm agent profiles`, and `farhelm agent restart` has no mode.
8. Existing sessions are handled as the spec's upgrade paragraph says. Structured rows become agent launches with
   `{farhelm_args}` placed exactly where the old release appended Farhelm's arguments. Profile and raw rows stay legacy
   sessions with their old argument placement and the restricted operations the spec lists. Downgrading is unsupported.

Acceptance criteria:

- Every behavior the 01-spec SPEC.md states for launches, templates, YOLO, Restart, Restart with, Clone, Replace,
  Replace with, the agent CLI, the permission mark, and legacy sessions is implemented and covered by tests at the level
  the repository already tests the surrounding behavior.
- No production code reads a command line to decide YOLO, agent type, or how to resume. Remove the argv YOLO classifier,
  basename kind derivation, and derived-resume logic (the derivations, refusals and selector stripping) everywhere
  except where legacy sessions still need their old argument placement, and say in the report exactly what legacy code
  remains and why.
- No production code, API, wire message, storage table, or UI refers to profiles, except the upgrade migration that
  removes their data and the refusals that name what replaced the removed CLI selectors.
- SPEC_impl.md describes what was built (stored launch shape, protocol version, migration, placeholder expansion,
  argument versus environment policy, template storage and resolution), and the obsolete sections are gone.
- The docs website no longer documents profiles, and documents launch kinds, templates, and the Restart rule.
- Each user-visible PR carries a changelog fragment; the stack as a whole is breaking (`feat!`).
- The TODO.md entries this plan covers are removed (see PR discipline).

## Requirement sources

Kept apart so a reviewer can challenge the planner's reading rather than merely check compliance with it.

### The maintainer's request and decisions (planning session, 2026-10-03)

- M1. The TODO.md Near term entry "Re-examine and simplify how launches are represented" asked for the launch paths to
  be interrogated end to end with simplification in mind. The maintainer then gave the model, which the 01-spec SPEC.md
  encodes. In their words, condensed:
  - "We nuke profiles. They are gone entirely as a concept."
  - Structured launches launch "an agent we understand and there are no custom arguments".
  - Templates are "template structured launches, that are named … applying the template is identical to manually
    clicking/typing to make the equivalent series of edits in the launcher dialog".
  - Templates stack in the order applied; a template "only specifies what it contains", and the resulting friction for
    required fields is accepted.
  - For an arbitrary command, "instead of picking yolo/default, you _assert_ whether it's yolo". Optionally "assume this
    command runs an agent of type X", with a required `{farhelm_args}`, and opt-in resume with a resume command.
  - The launcher has "one 'agent' tab and one 'command' tab". Terminology is "launch kind" (agent or command) and "agent
    type" (Claude, Codex, …), never "tab" in the spec.
- M2. Resume placeholder keeps the existing name `{conversation}` (not `{session_id}`, since "session" is Farhelm's own
  term).
- M3. `{farhelm_args}` is required whenever an agent type is declared, and expands to nothing for agent types that take
  no arguments. Not per-type, because per-type rules "cause complications like if the agent changes behavior over time".
- M4. YOLO for command launches is the assertion alone, for users and agents, with "NO command line parsing". An agent
  can therefore lie on a host that asks before YOLO launches; this is accepted until the permission prompts for CLI
  actions land. The TODO.md entry for that work was amended in the planning PR to require that an agent's command launch
  on such a host asks, whatever its assertion.
- M5. Agents may apply templates but not create, edit or delete them; writes wait for the same permission-prompt work.
- M6. Sessions record only the resulting launch, never the templates. Clone copies the stored launch into the launcher.
  Plain Replace and `farhelm agent clone` copy it exactly. `spawn --inherit-agent` is unchanged. Recent setups are
  unchanged and stay agent-launch only.
- M7. Profiles are removed with no conversion ("i'm the only user. i don't care. nuke profiles.").
- M8. "restart always means restart with conversation preserved". When that is known not to be possible, Restart and
  Restart with are unavailable and the user uses Replace or Replace with. Today's fresh restart and verbatim
  fallback-template restart are a bug. Restart with also covers command launches with a declared agent type and resume:
  command, resume command and YOLO assertion change; agent type, host and folder stay fixed.
- M9. Environment variables are avoided: Farhelm passes whatever turns its integration on as arguments, because every
  process the agent starts inherits the environment, including nested agents of the same type, so a hook carried there
  would reach runs that are not the session's. The environment carries only settings Farhelm's own reporter reads once
  an argument has turned it on. This reverses an earlier preference for environment variables; the spec text is final.
- M10. Pre-existing sessions:
  - Structured rows become agent launches (converted).
  - Profile and raw rows stay legacy; the maintainer chose "keep as legacy" over converting them.
  - Downgrading across the change is unsupported, stated in the spec and the release notes.
- M11. Accepted as drafted:
  - Cursor, Muse, OpenCode, and Grok without its hooks can never Restart.
  - A resume command needs `{farhelm_args}`.
  - Templates live in a Templates panel whose form offers every launcher field, each optional; names are unique.
  - Template application is all-or-nothing.
  - The permission marks.
  - `spawn` accepts `--command`.
  - The CLI flag names.
  - `farhelm agent restart` loses its mode.
- M12. The Maybe later entry "Separate an agent profile's common invocation, initial launch arguments, and resume
  arguments" is superseded by this plan and removed by it.
- M13. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter (below).
- M14. No-workhorse mode (below).
- M15. The spec was reviewed adversarially by Opus 5.5 and gpt-6-astra (high) during planning. The resulting fixes are
  in the 01-spec PR:
  - the order of template edits;
  - fixed fields refused;
  - template host matched by install identity;
  - templates counting as explicit selection for remembered defaults;
  - Restart with validating before it stops anything;
  - the CLI's required-flag and template interplay;
  - idempotency bound to the resolved launch;
  - the placeholder rules;
  - legacy sessions;
  - downgrade.

### Binding repository rules

- Root `AGENTS.md` (also loaded as `CLAUDE.md`). Its sections that apply here:
  - "Harness-specific code": per-agent behavior goes where `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module
    docs say. No `kind == X`, no `matches!` over harnesses, no `_` arms over harnesses in shared code.
  - "Finishing work": validation judgment.
  - "Releases and the changelog": a fragment for every `feat`, `fix` or `!` PR.
  - "TODO.md": remove the entries addressed.
  - "Docs website", plus `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
  - "The live install is off-limits".
  - "Sharing the machine with other agents".
  - "Agent scratch space".
  - `.agents/test-authoring.md`, for any test change.
- SPEC_impl.md, wire compatibility: the helm and supervisors use an exact-version handshake, so any wire reshape bumps
  the protocol version (see its protocol-version section). Persisted reservations and session rows need a migration.
  SPEC.md's Durability section requires surfacing data-loss and downgrade risks; the 01-spec SPEC.md does that.
- Idempotency fingerprints: the existing encodings are frozen byte-for-byte (comment at `create_fingerprint`,
  `crates/farhelm-supervisor/src/service/core.rs`), and keyed retries are bound to the resolved bundle. New launches
  need a fingerprint that binds the new resolved launch. Existing reservations must keep replaying, or be retired by the
  migration with the loss stated in SPEC_impl.md.
- The global user rule: no test may modify the environment variables of the process running it; use dependency
  injection.

### Planner proposals (change them if the code says otherwise, and log a DECISION)

- P1. One resolved launch type end to end, replacing the current dozen. The planning map is in this file's outline and
  in SPEC_impl.md's sections on create modes, profile resolution, restart-with persistence and replace composition.
  Validated once, carried on the wire, stored in the session row, used by restart, clone, replace, inherit and the YOLO
  guard. It has three variants:
  - agent: selection plus composed start and resume argv;
  - command: command string, assertion, optional agent type, optional resume argv;
  - legacy: the old stored fields, untouched.
- P2. Placeholder expansion is the only argument surgery for new launches. Hook injection today also reads command lines
  (`inject_hooks`, `inject_hook_argv_tail` and the Goose, Pi and OMP variants in `agent_kind/`):
  - shape checks;
  - a bare-`--` skip;
  - "yield to the user's own flag" rules;
  - OMP's flag grammar;
  - Goose rewriting a resume even with hooks off.

  It also writes reporter settings as an `env NAME=value` prefix (`with_launch_environment`), which the 01-spec SPEC.md
  forbids for new launches.

  For new launches:
  - Each per-type function becomes "return this agent type's arguments for a start or for a resume". The caller says
    which; nothing inspects the command.
  - `{cwd}` and `{farhelm_args}` are filled at spawn. Expansion puts those arguments at `{farhelm_args}`.
  - Reporter settings go into a new environment field on the per-launch `LaunchSpec`
    (`crates/farhelm-supervisor/src/launch.rs`), which the shim applies to the process it execs.
  - Integration is turned on by arguments, per M9. If a harness can only be turned on through the environment, the "keep
    the environment" fallback below applies to it. Goose is the likely case, so check it first.

  Legacy sessions keep today's injection code path unchanged, behind one clearly named legacy function. That includes
  its shape checks and `effective_program_index`/`is_env_program`, which therefore stay; their only callers are legacy.
- P3. The helm's compiler (`crates/farhelm-helm/src/launches.rs`) composes the start argv for every agent type, and the
  resume argv for every agent type that has conversation reporting, with `{farhelm_args}` in both. Muse, Cursor and
  OpenCode get no resume command. All resume derivation is deleted, legacy included:
  - the supervisor's `default_resume_template` derivations;
  - the `ambiguous_derived_resume` refusals;
  - `strip_*_selectors`;
  - `derive_kind` from the argv basename.

  Legacy rows store a concrete agent type and resume command, and their Restart runs that stored resume argv.
- P4. YOLO:
  - Delete `invocation_is_yolo`, `argv_is_yolo` and their tables from `crates/farhelm-proto/src/yolo.rs`, keep
    `selection_is_yolo`, and add the assertion.
  - The helm guard (`crates/farhelm-helm/src/yolo_guard.rs`), the UI mark (`crates/farhelm-ui/src/list/row.rs`, which
    calls `invocation_is_yolo` today) and the confirmation's reason (`crates/farhelm-ui/src/yolo_confirm.rs`) all use
    the launch's own verdict.
  - Legacy rows carry no assertion; the UI marks them unclassified. Every operation the spec allows on a legacy row
    either refuses it or is a Restart, which the guard never asks about.
  - The UI's other readers of command lines go too: the sidebar's harness icon for old sessions taken from the program
    name (`row.rs`), and the Cursor notice keyed on built-in profile ids (`crates/farhelm-ui/src/list/create_form.rs`).
    Icons come from the stored agent type; legacy rows without one get the generic icon.
- P5. Restart offer becomes "resume available" or "not available, with a reason". `RestartOffer::FreshOnly` and
  `FallbackTemplate`, and `RestartMode::Fresh` and `FallbackTemplate`, go away, along with the `relaunch_argv` arms that
  serve them. "Evidence after resumability is withdrawn" in SPEC.md still lets capture evidence thin out once Resume is
  unavailable.
- P6. Templates are stored by the helm in `helm.db` as rows of optional fields (a small JSON document per template is
  fine), with unique names and last-write-wins edits. On write the helm checks each field's shape and size only; full
  validity depends on the launcher state at the moment of application.

  Application is one pure function in `farhelm-proto`: launcher state plus template gives a new launcher state, or a
  refusal naming the field. It is compiled into both the helm and the UI. Do not add an HTTP apply endpoint. Move the
  rule that picking an agent type clears incompatible choices (`reconcile_harness_selection` in
  `crates/farhelm-ui/src/launch_composer.rs`), and the catalog row type it needs, into `farhelm-proto`. The UI's own
  hand edits and template application then both call it, and the helm uses it to fold `--template` and flags for the
  CLI.

  In the GUI, the result goes through the launcher's existing setters. That makes three spec requirements hold with no
  separate mechanism; do not build provenance tracking for templates:
  - remembered defaults treat the result as the user's explicit selection;
  - recent setups record it;
  - a pending fresh-checkout preview is invalidated.

  On the CLI, idempotency binds the resolved launch in the same way. The Templates panel and the helm's template API can
  follow the shape of the profiles code that PR 03 deletes (read it from history), and spawn's template resolution can
  reuse the shape of the deleted `ResolveProfile` relay.
- P7. Remove profiles in their own PR, before the launch-kind PR. Raw command creates keep working there in their
  current shape. That PR removes:
  - the helm `profiles` table and its API;
  - built-ins, the remembered default profile, and the `source_profile` snapshot;
  - the `ResolveProfile` relay;
  - the GUI profile picker and Profiles panel;
  - `farhelm agent profiles`, and the `--profile`, `--profile-id` and `spawn --agent <name>` selectors.

  Until PR 04, agents create with `--inherit-agent` or `agent create --invocation`. From PR 04 on, see the outline.
- P8. Idempotency reservations across the upgrade. Keep the existing reservation rows and add no compatibility for them.
  A key is matched by plain string equality (`crates/farhelm-supervisor/src/service/core.rs`, around the reservation
  lookup), so a retry that crosses the upgrade with an old key is refused, never duplicated. Deleting the rows would
  allow two sessions for one intended create, which SPEC.md forbids.

  Delete the frozen old fingerprint encoders once nothing produces them. A fresh-checkout reservation stores a
  serialized create mode; if one fails to decode under the new types, refuse that retry explicitly rather than treating
  the key as unknown. State this in SPEC_impl.md.
- P9. The helm caches each host's sessions in `helm.db`, and skips cached rows it cannot decode. PRs 02 and 04 change
  that row shape (restart-offer values, launch fields), so without a migration, sessions on hosts that are down would
  vanish from the list after the upgrade. Migrate the cached rows in the helm schema migration of the PR that changes
  their shape, as helm.db schema 4 did for an earlier change.

## Implementation outline

This is the biggest change in the queue. It crosses the proto, supervisor, helm, UI and CLI crates, storage on both the
helm and the supervisor, the wire protocol, and the docs site. Estimates here are about mechanisms, not lines.

Suggested stack, bottom to top. Reshape it if the code argues for another order, without churn: no code added in one PR
and deleted in a later one.

1. **`docs: …` (exists).** `plan/launch-representation/01-spec`, the SPEC.md rewrite. Do not change its product
   behavior. If implementation shows a spec sentence is wrong or self-contradictory, block with the passage and the
   question (Unattended fallback). A pure wording fix that changes no behavior may be made in that PR; log it as a
   DECISION and list it in the report. SPEC_impl.md changes ride with the code PRs, not this one.
2. **`feat!: restart only resumes the conversation`.**
   - Supervisor: the restart offer and its per-kind computation; the `relaunch_argv` arms for Fresh and
     FallbackTemplate; the "verification failed so the offer becomes fresh" paths now make Restart unavailable.
   - Proto: `RestartOffer`, `RestartMode`, and a protocol bump.
   - Helm: restart handlers and the cached-session migration (P9).
   - `farhelm agent restart`: `--mode` is refused with a message saying there is one kind of restart. The restart column
     in `farhelm agent sessions` changes too, and so does its `--json` output: bump that envelope's `schema_version`.
     Update `farhelm agent instructions` (`crates/farhelm/src/agent_instructions.rs`).
   - UI: Restart and Restart with greyed out with the specific reason; the interrupted card offers Restart only when the
     session can resume.
   - Tests, and the e2e suites that exercised fresh or fallback restart.
   - Self-contained, and the fix for what the maintainer called a bug today.
3. **`feat!: remove agent profiles`** (P7), with the helm and supervisor storage migrations for the removed data, the
   cached-session migration, and the agent CLI changes (listing, instructions, JSON schema version).
4. **`feat!: launch kinds`**, the core. It must leave every surface able to make each launch kind, because from here on
   a command launch needs a YOLO assertion that the old surfaces cannot supply.
   - the resolved launch type (P1) in proto, the wire messages and the supervisor `sessions` row, with its migration:
     structured rows to agent, everything else to legacy, plus P8 and P9;
   - the 64 KiB create bound now counting the command, the resume command and the rest of the launch;
   - `{farhelm_args}` expansion, the per-type argument functions and the `LaunchSpec` environment field (P2, M9);
   - the compiler composing start and resume (P3), and deleting derivation and basename kind (P3);
   - YOLO by assertion (P4), including the UI mark and the confirmation's reason;
   - create, clone, replace, inherit and Restart with on the new type, including the legacy refusals, the interrupted
     legacy card offering Replace with, and Restart with for command launches;
   - the minimum surfaces:
     - the launcher's existing command mode gains the YOLO assertion as a required choice, the declared agent type, and
       the resume opt-in;
     - `farhelm agent create` replaces `--invocation` with `--command` plus `--yolo`/`--no-yolo`, `--agent` as the
       declared type, and `--resume-command`, and refuses `--invocation` naming `--command`;
   - the protocol bump.

   This is large. Split it where a reviewer is genuinely helped, for example storage and migration, then placeholders
   and hooks, then helm paths and YOLO with the minimum surfaces. Each split must build, pass tests, and not churn.
5. **`feat: launch-kind tabs in the launcher`.** The New dialog, Clone and Replace with:
   - agent and command tabs keeping separate drafts;
   - the command tab's layout around the fields PR 04 added;
   - search behavior on each tab;
   - the Restart with dialog for command launches;
   - legacy sessions' Clone and Replace with seeding.
6. **`feat: launch templates`** (P6). In the GUI:
   - the shared application function, with its fixed order, all-or-nothing refusal, fixed-field refusal and host matched
     by install identity;
   - helm storage and API;
   - the Templates panel beside New;
   - `tl:name` in the launcher search, and no templates in Restart with.

   On the CLI:
   - `--template` (repeatable) on `farhelm agent create`, plus spawn's template resolution;
   - required flags satisfiable by templates, with explicit flags winning;
   - `spawn` refusing a template that sets a host;
   - the CLI refusing fresh-checkout templates;
   - `farhelm agent templates` and its `--json` form, whose listing hides command and resume-command text.
7. **`feat!: agent launches from the agent CLI`:**
   - `--agent`, `--model`, `--effort`, `--permissions` and `--trust` on `farhelm agent create` and `farhelm spawn`;
   - `--command` and the rest on `spawn`;
   - `--inherit-agent` exclusivity;
   - repeated or contradicting flags refused;
   - `--agent` given something that is not an agent type refused with the list of agent types;
   - `farhelm agent instructions` regenerated.
8. **`docs: …`.**
   - The docs website: rewrite `website/src/content/docs/docs/using/custom-commands.md`, `using/agent-wrappers.md`,
     `using/agent-hook-injection.md`, and the Cursor, Grok, Goose, Pi and OMP agent pages (there is no profiles page).
     Find every other page that mentions profiles, typed commands, Restart or the agent CLI flags.
   - The final SPEC_impl.md sweep for anything earlier PRs left stale.
   - The TODO.md removals, if not already done in the PR that finished each entry.

Where each piece of today's code lives is mapped in SPEC_impl.md, and the `agent_kind/mod.rs` module docs list the
per-harness places. Start by reading those, then `crates/farhelm-helm/src/sessions.rs` (`CreateMode`, `CreateSpec`,
`mode_from_source`, `do_create_session`, `do_replace_session`, `do_restart_session`),
`crates/farhelm-helm/src/agent_requests.rs`, `profiles.rs`, `store.rs`, `preferences.rs`, `client.rs`, `launches.rs`,
`yolo_guard.rs`; `crates/farhelm-supervisor/src/service/handlers.rs` (`create_mode`, `CreateSelector`),
`service/core.rs` (`CreateMode`, `validate_create`, `relaunch_argv`, restart with, reservations), `store.rs` (sessions
row), `launch.rs` (`LaunchSpec`), `agent_kind/mod.rs` and `agent_kind/{goose,pi,omp}.rs`;
`crates/farhelm-proto/src/lib.rs`, `launch.rs`, `yolo.rs`; `crates/farhelm-ui/src/list/create_form.rs`,
`launch_composer.rs`, `launch_controls.rs`, `profiles.rs`, `list/row.rs`, `restart_with.rs`, `yolo_confirm.rs`,
`api.rs`; and `crates/farhelm/src/main.rs`, `render.rs`, `agent_instructions.rs`. Test migrations against SQL fixtures
of the old schemas, as the existing supervisor migration tests do (for example the `supervisor-v17.sql` fixture), not
against old binaries.

### What not to build

- No conversion of profiles into templates, and no converting legacy sessions beyond what M10 says.
- No command-line parsing for any decision. Validating the placeholders is the only reading of a command: present
  exactly once, as a whole argument.
- No built-in templates.
- No template writes from agents, and no `farhelm agent template create`/`edit`/`delete`.
- No "save the current launcher as a template" action.
- No template provenance tracking: sessions, remembered defaults and recent setups never learn that a template was
  involved (P6).
- No downgrade compatibility layer.
- No permission-prompt machinery (that is the separate TODO entry).

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository), this plan's working log is
`farhelm-plan-launch-representation-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. The plan
already has an open PR (01-spec), so `plans/AGENTS.md` (Executing one plan, steps 6 and 7) applies from the first run:
track and rebase that bookmark, and run the resume check, before any work.

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
running for the whole run.

What it does:

- Every 60 seconds it samples free space on the filesystems holding the checkout, the agent scratch directory and
  `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on
  macOS).
- It writes each sample to a private heartbeat/status file in the scratch directory.
- It emits a notification when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%,
  whichever comes first; again when the number keeps falling; on recovery; and if the monitor itself fails. In Claude
  Code, a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's equivalent.
  A file update alone is not a notification.

Before relying on it:

- Verify delivery with a harmless synthetic alert.
- Verify failure detection by killing a throwaway monitor and confirming you are told.
- If you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals; a monitor cannot detect its own sampling loop hanging.
- If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once
  a minute.

While running:

- Check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a heartbeat stale for
  two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.
- An alert is an instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the
  job most likely responsible, and resume only when the watchdog reports headroom.
- Record the watchdog's handle, watched paths, status path and delivery mechanism in the log, and include its state in
  every handoff note so a resumed session reconciles or restarts it.
- Stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- **jjstack.** Use the `jjstack` skill. The stack's base is `main@origin` beneath this plan's own open PRs, starting
  with `plan/launch-representation/01-spec`. Set it up per `plans/AGENTS.md` (Executing one plan, step 6); never build
  on another plan's PRs. Bookmarks are `plan/launch-representation/<nn>-<short-name>`, numbered after `01-spec`.
- **Shaping.** Bite-sized PRs, without churn. Within this run, a PR that needs correcting is restructured rather than
  corrected on top.
- **Commit format.** Conventional Commits. Every `feat`, `fix` or `!` PR carries a changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`. The fragments must tell users:
  - profiles are gone with no conversion;
  - Restart now only resumes;
  - Cursor, Muse and OpenCode sessions can no longer restart;
  - downgrading is unsupported;
  - the removed CLI selectors and their replacements.
- **Messages.** Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read. Leave a PR description empty when the diff and title say everything.
- **No landing.** PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed
  this plan's report (`plans/AGENTS.md`).
- **TODO.md removals.** The PR that finishes the work removes these entries; at the latest, the last PR does:
  - Near term: "Re-examine and simplify how launches are represented";
  - Maybe later: "Separate an agent profile's common invocation, initial launch arguments, and resume arguments";
  - Doc todo: the entry about telling users that YOLO recognition of custom command lines is best effort, now moot
    because nothing is recognized from command lines.

  If any other TODO.md or `review_feedback_queue/` entry is made moot by this work, or now mentions profiles in a way
  that needs rewording, list it in the report rather than changing it unasked.

### Validation

Follow root `AGENTS.md` "Finishing work". Run Rust tests through `scripts/record-test-run.py`, with the pinned nextest
and tmux per `docs/test-run-evidence.md`.

This change is cross-cutting: it touches lifecycle, storage migrations, the wire protocol, and the launcher. Per PR,
choose the narrow set that covers what the PR touched. Typical choices, not a checklist:

- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets -- -D warnings`, and `cargo clippy -p farhelm --bins -- -D warnings`;
- nextest selections of the affected crates and e2e modules;
- `cargo check -p farhelm-ui --features desktop`;
- the UI JS harness when asset JS changes;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check`;
- the website build for docs changes.

At the tip, two broader runs are justified, and the report should name the risk each covers:

- The full workspace nextest run and the workspace doctests: the migration and protocol bump touch every create and
  restart path.
- Browser specs on Chromium and WebKit for the launcher, Clone, Replace with, Restart with and the Templates panel: the
  launcher's UI changes substantially.

Run the migration against a supervisor database and a `helm.db` produced by the pre-change code, in a test. Do not touch
the live install.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to a fresh-context
Opus 5.5 agent at high effort, as the user demands (M13). No review swarm. The prompt carries the full charter, because
the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- its findings file, in the scratch directory;
- the acceptance criteria: The goal, Requirement sources and the outline above, plus a pointer to the 01-spec SPEC.md as
  the behavioral contract.

For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim. Address what the
reviewer finds before moving on, and log a DECISION where you decline a finding. Do not write a launch command here or
in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above, run a fresh-context review through galaxy-brain.
Examples of such a departure, not a blacklist:

- a second launch representation kept alongside P1;
- a compatibility layer for old clients or downgrades;
- a template versioning or history mechanism;
- a command-line parser of any kind;
- a new persistent subsystem beyond the template table.

Do the same whenever the same component has needed repeated corrective review rounds. Supply the request, the
requirement sources above, this outline, the current diff, and the proposed departure: what changed, why it is
necessary, and which simpler alternative was ruled out. The charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- the final stack shape;
- the resolved launch type's shape and its wire and storage encoding;
- the migration and the fate of existing idempotency reservations;
- which hooks moved between arguments and the environment;
- where template application lives (helm, UI, or shared);
- legacy code kept and why;
- any 01-spec wording fix;
- every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization.

Agreed fallbacks:

- If the 01-spec SPEC.md is silent on a detail the implementation must decide, choose the reading most consistent with
  the maintainer's "applying a template is identical to making the edits by hand" and "never parse command lines", log
  it as a DECISION, and add a sentence to SPEC_impl.md, or to SPEC.md when it is product behavior. Surface each such
  SPEC.md addition in the report.
- If a harness cannot take its integration as an argument at all, keep the environment for it. Say so in SPEC_impl.md
  and in the report, naming the harness, since M9's nested-run concern then applies to it.

Anything else that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan,
step 10). A spec sentence that is wrong or contradictory is one of those.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, starting with `01-spec`, satisfies the acceptance
criteria, has passed the review gate PR by PR, and has removed the TODO.md entries listed under PR discipline. The PRs
are open, not merged: merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

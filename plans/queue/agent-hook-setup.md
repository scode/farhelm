# Per-agent Settings sections and opt-in automatic Restart hooks

Written against main at 470e9e6c on 2026-11-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers two TODO.md entries (bucket "Near term"), merged into one plan with the maintainer's agreement because
the second builds on the first. Verbatim:

> **One Settings section per agent.** Settings gets a dedicated Agents group with one section per agent type, every
> agent type included, holding that agent's settings. An agent with nothing to configure gets a section that says so
> plainly: not finding your agent in Settings is what confuses people. The current agent-specific settings (the trust
> switches, their hint switches and Claude's caution box) move into their agents' sections, and the Agent trust page
> goes away. Credentials moves to the General group. Plan: `plans/queue/agent-hook-setup.md`.

> **Install Grok and Antigravity conversation hooks automatically, opt-in.** Restart needs Farhelm's hooks in Grok and
> Antigravity, which users install by hand today and forget (the maintainer did, and took a greyed-out Restart for a
> bug). Add an all-hosts opt-in per agent. While it is on, every start of that agent installs or updates Farhelm's own
> hook file on the machine it runs on. Ask in the launcher the first time, show a band in sessions launched without it,
> and make the greyed Restart tooltip say to turn the setting on. The opt-in wording must say that Farhelm changes the
> agent's own configuration, which affects the agent outside Farhelm too. Drop the manual setup instructions. Plan:
> `plans/queue/agent-hook-setup.md`.

The maintainer's own words while planning, which the wording requirements below come from: the manual install existed
because "I didn't feel comfortable with the helm modifying the user's configuration, but this is just too fucking
annoying"; it "needs to be opt-in, but also very obvious to the user without them having to find some obscure setting
somewhere"; and "the point is to make it clear to the user that the thing they're agreeing to is that we are modifying
something that affects the behavior when they run the harness outside of Farhelm."

This plan runs after `launcher-command-field.md`, whose redesign of the session launcher must be on main first: the
launcher prompt goes into that launcher.

### Where it stands (verified during planning, on Linux, 2026-11-21)

- **Antigravity CLI 1.5.14** discovers a plugin directory placed by hand at `~/.gemini/config/plugins/farhelm/`, holding
  `plugin.json` (`name`, `displayName`, `version`, `description`) and a `hooks.json` at the plugin root in the same
  named-group shape as the global file (`{"farhelm": {"SessionStart": [...], "Stop": [...]}}`), without
  `agy plugin install`. `agy plugin list --json` shows it `enabled` with `hooks: ["farhelm"]`, the CLI log says
  `loaded 1 named hooks from 1 hooks.json file(s)`, and in a real signed-in turn both `SessionStart` and `Stop` fired
  with `conversationId` and `transcriptPath` among their fields. So Farhelm never edits the shared
  `~/.gemini/config/hooks.json`; the fallback of merging a key into it, which the maintainer allowed, is not needed. The
  first record of 1.5.14 main-conversation transcripts is still `source: USER_EXPLICIT`, `type: USER_INPUT`, and a
  subagent's still `SYSTEM`/`SYSTEM_MESSAGE`, so the existing admission check
  (`crates/farhelm-supervisor/src/agent_kind/antigravity.rs`) holds.
- **Grok 1.0.77** merges a separate file in `~/.grok/hooks/` (or `$GROK_HOME/hooks/`) with any others, and all three
  hooks (`SessionStart`, `UserPromptSubmit`, `Stop`) fired in a real turn. The manual instructions already name that
  file `farhelm.json` (`website/src/content/docs/docs/agents/grok.md`).
- Both probes were removed afterwards. The hook commands Farhelm's manual instructions use today are in
  `website/src/content/docs/docs/agents/antigravity.md` and `grok.md`.
- Today Grok and Antigravity launches add no reporter arguments, start no silent-hook timer, and their Restart readiness
  is `RestartReadiness::FirstPromptWithHooks` (`crates/farhelm-proto/src/lib.rs`), whose clause sends the user to the
  docs page. The Restart tooltip is built in `crates/gui/farhelm-gui/src/session_list/restart.rs`
  (`not_captured_clause`), the launcher repeats it (`crates/gui/farhelm-gui/src/launcher/entry.rs`), and the agent
  instructions use the neutral wording (`crates/farhelm/src/agent_instructions.rs`).
- Settings' navigation groups are `General`, `Where sessions run` and `Agents`
  (`crates/gui/farhelm-gui/src/settings/ frame.rs`); `Agents` holds Agent trust and Credentials. The Agent trust page is
  `crates/gui/farhelm-gui/src/settings/agent_trust.rs`, and the search catalog is
  `crates/gui/farhelm-gui/src/settings/search.rs`.

### Acceptance criteria

Settings:

- The Agents group has one section per launch harness that is not a shell (`!LaunchHarness::is_shell()` in
  `crates/farhelm-proto/src/launch.rs`, which today means Claude, Codex, Muse, Pi, Antigravity, Grok, Goose, OMP, Cursor
  and OpenCode), derived from that enum so a new agent cannot be left out. Bash and Zsh get no section. Each holds
  exactly that agent's settings. A section with no settings says plainly that there is nothing to configure for that
  agent.
- Every current agent-specific setting moves to its agent's section with its behavior, storage and help unchanged: the
  trust switches (Claude, Codex, Muse, Pi, Antigravity), the hint switches, and Claude's caution box. The Agent trust
  page no longer exists. Credentials moves to the General group, keeping its "This helm" scope.
- Search finds every moved setting under its new section, and anything that opened Agent trust (the trust hint band's
  "Settings" action, for one) opens the right agent's section at the right switch.
- SPEC.md states the rule that every agent type has a section, and why.

Automatic hooks:

- Grok's and Antigravity's sections each have an all-hosts switch, off by default, whose label and help say that Farhelm
  changes that agent's own configuration on each machine where Farhelm starts it, that the change stays, and that it
  runs in that agent's sessions outside Farhelm too (where it does nothing but answer the agent). The help names the
  exact location and says that turning the switch off leaves already-installed hooks in place, naming the path to delete
  to remove them.
- While a switch is on, every start of that agent kind (new, clone, Restart, Replace, Restart with and Replace with,
  templates, quick launches, agent-created sessions, environment launches, and command launches that declare that agent
  type) makes the supervisor on the machine the agent runs on (host, container or sandbox) install or update Farhelm's
  file before the agent starts: Antigravity's plugin directory `~/.gemini/config/plugins/farhelm/`, Grok's
  `farhelm.json` in its hooks directory. The hook command is the same absolute `farhelm internal hook --vendor <agent>`
  command the manual instructions documented, never with `--announce`: the installed file runs in sessions outside
  Farhelm too, and with `--announce` the hook prints Farhelm's pointer line into the model's context even without a
  session credential (`crates/farhelm/src/hook.rs` module docs). The executable is the version-independent forwarder
  path the per-launch hooks use (`HookPolicy::exe`, from `Supervisor::session_farhelm_exe_str`), never the versioned
  binary, which an upgrade would leave stale in a persistent file.
- Each file is written atomically (temporary file in the same directory, then rename) and left alone when it already
  holds exactly the right bytes; the Antigravity plugin is two files, so atomicity and the exact-bytes check are per
  file. Missing parent directories (`~/.grok/hooks`, `~/.gemini/config/plugins/farhelm`) are created: the switch is the
  user's explicit consent. Grok's `farhelm.json` is overwritten unconditionally (it carries Farhelm's name and the
  manual instructions told users to create exactly that file). Antigravity's plugin directory is Farhelm's own; nothing
  touches the shared `hooks.json`, and an old manual entry there is left alone.
- The install uses the supervisor's own `HOME` and `GROK_HOME`. The session's login-shell environment is not visible to
  the supervisor before spawn, so a `GROK_HOME` set only in a shell rc file is missed; say so in the Grok docs. When
  `FARHELM_AGENT_HOOKS` turns hooks off for that agent on the host, the install is skipped and logged, never notified,
  as the Pi and OMP reporter materialization already behaves.
- A failed install never blocks the launch. Its reason reaches the user through the existing "reporter could not be set
  up" session notification and the Restart tooltip (below), and the supervisor log has it.
- After an install, Restart becomes available after the first prompt exactly as it does with the manual hooks today, end
  to end, under the existing admission checks.
- The launcher shows a prompt the first time the user picks Grok or Antigravity while that agent's switch is off and the
  prompt has not been dismissed. Its buttons are **Set up and launch** (turns the switch on and launches), **Launch
  without** (launches; asks again next time), and **Don't ask again** (stores a dismissal, reversible in that agent's
  section). Its words say the same thing as the switch's help: Farhelm changes the agent itself, on every machine where
  Farhelm starts it, and it affects the agent outside Farhelm too.
- Sessions of those agents whose launch did not set up the hooks (agent-created, templates, quick launches, Launch
  without, or started before the switch was turned on) show a nonmodal band while the conversation is not captured, with
  the trust hint band's look: words to the effect of "Farhelm didn't set up its hooks for this session, so Restart isn't
  available", an action that opens the agent's section at its switch, **Don't show again**, and an occurrence-only ×.
  The band hides once a conversation is captured (an old manual setup, or files left after the switch was turned off,
  can still report). It covers "not set up" only; a failed install is the notification's and tooltip's.
- Whether a session's launch set up the hooks is one per-session fact the supervisor reports with the session's facts
  (not requested, installed, or failed with its reason). The band and the tooltip choose their wording from it, without
  reading the switch.
- When Restart is unavailable for a Grok or Antigravity session, the tooltip names the actual reason: hooks not
  requested tells the user to turn on the switch in Settings → Agents → <agent> and start a new session; a failed
  install says so with the reason; hooks set up keeps "after your first prompt". Every other surface that uses the same
  Restart readiness words changes with it and is covered by tests: the launcher, `farhelm agent instructions` (neutral
  wording), the supervisor's Restart refusal (`RestartOffer::unavailable_reason` in `crates/farhelm-proto/src/lib.rs`),
  and the silent-hook warning (`hook_silent_text`, whose `{farhelm_args}` advice does not fit these two agents). None of
  them points at a docs page for manual setup any more.
- The docs drop the manual setup instructions for both agents and describe the setting instead. SPEC.md and SPEC_impl.md
  replace "users install the documented Grok and Antigravity hooks themselves" and "capture never writes to the agent's
  configuration" with a narrow, opt-in exception for these two agents, next to the existing Claude trust edit. Two more
  SPEC.md sentences change with "an installed launch counts as hooked": that the manual Grok and Antigravity hooks start
  no silent-hook timer, and that their callbacks are independent of `FARHELM_AGENT_HOOKS`.
- No conversation tracking is added for any agent that does not already have it.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page and in the conversation before it:

- **Scope:** automatic installation only for the agents whose conversation tracking already exists through manual hooks,
  Grok and Antigravity. No tracking for Cursor, OpenCode or anything else.
- **One all-hosts setting per agent**, not per host: "either you have opted into automatically installing the hooks for
  a particular harness or you haven't, and if you have opted into it, it applies to all hosts". Once on, any session
  started from then on checks and installs the hooks if needed.
- **Where the opt-in shows up:** the launcher prompt, the session band and the Restart tooltip, as mocked up on the
  planning page (the mockups' words are in the acceptance criteria above).
- **Prompt buttons:** Set up and launch / Launch without (asks again next time) / Don't ask again.
- **Wording:** the consent text must make clear that what the user agrees to is Farhelm modifying the agent's own
  configuration, which changes how the agent behaves outside Farhelm too; not merely enabling something for one session.
- **Turning a switch off** leaves installed hooks in place; the help names the file to delete.
- **Existing manual setups:** no special handling. Grok's `farhelm.json` is overwritten; Antigravity's plugin is
  installed whether or not the old `hooks.json` entry exists. The maintainer: "we JUST added anti gravity support and
  have very few users, after this plan the instructions will be gone and the new plugin mechanism is the only one, thats
  fine". A changelog caveat that the old entry can be deleted is fine; no detection, no migration.
- **Every agent type gets a Settings section**, including those with nothing to set, and this goes into SPEC.md with the
  reason.
- **Credentials moves to General.**
- **Docs drop the manual setup instructions.** The setting is the only documented way.
- **One plan, Settings sections first.**
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at high effort with the general charter.
- **No-workhorse mode**, stated on the planning page.
- **The Antigravity fallback** (merging into the shared `hooks.json`) was allowed but is not needed: the plugin
  directory works (see "Where it stands").

## Requirement sources

**The maintainer's request and decisions:** the two TODO entries, the quoted words, and the decisions above.

**Binding repository constraints:**

- Root `AGENTS.md`, "Code rules": everything Farhelm does differently per agent goes in the named per-harness places,
  never `kind == X`, `matches!` over harnesses or a `_` arm over harnesses in shared code. Read the module-doc map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` first and add the new capability where it says.
- SPEC.md and SPEC_impl.md, in particular Status (Farhelm-supplied integration must not make vendor configuration a
  condition of launching), the conversation capture sections (what capture may write), the Agent trust paragraphs, and
  protocol and schema versioning.
- Root `AGENTS.md`, "The live install is off-limits", and the maintainer's global rule against tests that modify the
  test process's environment variables: tests inject the home and hooks directories, never set `HOME` or `GROK_HOME`.
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`, `docs/test-sleep-check.md`, and the
  Settings golden pictures' procedure in `.agents/validation.md`.
- `website/AGENTS.md` (an unattended run starts no preview server and names the changed pages).

**Planner proposals** (reasons given; challenge them through the scope reassessment if they turn out wrong):

- **The opt-in rides on each launch.** The helm owns the setting and freezes it onto each start of that agent kind, the
  way it already carries `keep_spare` and `claude_trust` from global switches; the supervisor that spawns the agent does
  the install. This matches "any session started from that point on", needs no host-pushed configuration, and puts the
  write on the machine whose configuration it is, containers and sandboxes included. It probably needs a protocol bump;
  bump from whatever `PROTOCOL_VERSION` is on `main@origin`.
- **One shared fact says which agents have hook setup.** Add one exhaustive predicate in `farhelm-proto` (on
  `LaunchHarness` like `offers_workspace_trust`, or on `AgentKind`) that the helm and the GUI both ask, rather than
  separate lists in each crate. The supervisor's per-agent answer of which files to write stays exhaustive in the
  supervisor.
- **The per-agent files live with each agent's integration.** The files to install, relative to the directory the agent
  reads them from, and their exact bytes, are answered per kind in the places the `agent_kind/mod.rs` map names, Grok
  and Antigravity in their own modules and every other kind stating it has none. A trait method or an exhaustive free
  function beside `pi_extension::reporter_asset(kind)` both fit the map; pick one and log it. Build the command with
  `hook_command(exe, AgentInstructions::Off, vendor)` or an equivalent that never adds `--announce`.
- **Write with the existing file helpers, not the Pi asset type.** Use `crate::files::overwrite_private_file_sync`
  (temporary file, fsync, replacing rename) and `agent_kind::read_bounded_regular_file` (bounded, no symlink following)
  for the exact-bytes check. Do not generalize `pi_extension::VendorAsset` or `materialize_asset`: they hold fixed
  bytes, publish into Farhelm's own state and refuse a differing file, the opposite of overwriting dynamic bytes in the
  vendor's directory.
- **Install in `prepare_launch` and reuse its accounting.** `prepare_launch` in
  `crates/farhelm-supervisor/src/service/core.rs` already materializes the Pi and OMP reporter assets before the pure
  `AgentIntegration::farhelm_args`, passing the result in through `HookPolicy`. Run the install there, gated by the
  launch's mark and the `FARHELM_AGENT_HOOKS` policy, and pass its outcome in through one new `HookPolicy` input; Grok's
  and Antigravity's `farhelm_args` then return `hooked: true` with no arguments, or a skip whose reason feeds the
  existing `HookSkip::ReporterUnavailable` notification (which may need to carry a dynamic reason). The silent-hook
  warning then covers installed launches with no new accounting.
- **Restart readiness stays a per-kind fact; the session carries the per-launch one.** Keep `RestartReadiness` per kind
  (it also gates the silent-hook warning and lists kinds in the agent instructions) and change its clause to name the
  Settings switch instead of the docs page. Add the one per-session field from the acceptance criteria (not requested,
  installed, failed with reason) to the session's facts, alongside the stored per-generation `launch_hooked`; the
  tooltip and the band read only that.
- **The switch and the dismissal are stored like their neighbours.** The switch is helm configuration, stored and served
  like the per-agent trust switches (`crates/farhelm-helm/src/checkout_config.rs`, `/api/checkout-agent-trust/<agent>`).
  Default to one dismissal per agent, a preference field like the trust hint dismissals, covering both the launcher
  prompt's "Don't ask again" and the band's "Don't show again", with one switch in the agent's section to undo it; split
  them only if you find a reason the two must differ, and log it.
- **The band shares the trust hint band's look, not its machinery.** Reuse the look, buttons, ×, Don't show again and
  refused-save line of `crates/gui/farhelm-gui/src/session_pane/trust_band.rs`. Its show rule is its own and comes only
  from pushed data (the agent has hook setup, the per-session field says not requested, the conversation is not
  captured, not dismissed, not hidden by ×). Do not generalize the trust band's settings read and refresh, which serve a
  different contract (a live on-screen question checked against a stored setting). How much of the look to factor out is
  your call.
- **Don't write real vendor configuration while validating.** Tests and e2e runs use injected temporary directories. The
  real-agent drift test (`crates/farhelm/tests/e2e/real_agent_capture/drift.rs`, `manual_hook_configured`) only reads
  existing configuration; teach it to recognize Farhelm's installed files as well as the old manual layout, and never
  make it write. Do not install hooks into the executing host's real `~/.gemini` or `~/.grok`.
- **The launcher prompt fits the launcher as it is on `main` when you build it.** This plan runs after
  `launcher-command-field.md`, which redesigns the launcher; match its structure rather than the mockup's layout.

## Outline

Five PRs, in this order. The numbers are a starting point; restructure if the code shows a better seam, without building
anything in one PR that a later one removes.

1. `plan/agent-hook-setup/01-agent-settings-sections` (`feat:`): the Agents group with one section per agent type, the
   trust and hint switches and Claude's caution moved in, "nothing to configure" sections, Agent trust removed,
   Credentials moved to General, search and every link that opened Agent trust updated, goldens per
   `.agents/validation.md`, SPEC.md's new rule, and `website/src/content/docs/docs/using/settings.md`. Changelog
   fragment.
2. `plan/agent-hook-setup/02-supervisor-hook-install` (`feat:`): the proto field and protocol bump, the shared
   predicate, the per-kind files for Grok and Antigravity, the install in `prepare_launch`, hooked-launch accounting,
   failure reporting and the per-session field. Unit tests of the exact file bytes (no `--announce`, the forwarder path)
   and a supervisor-level test on injected directories: files written before spawn, the launch recorded as hooked, an
   unchanged file left alone, a failure becoming a skip with its reason, the `FARHELM_AGENT_HOOKS` opt-out skipping the
   install. Report-to-Restart is already covered end to end
   (`antigravity_manual_hook_captures_and_restart_passes_the_conversation` in
   `crates/farhelm/tests/e2e/hook_identity.rs`); add an e2e test only if the fake agent fixture
   (`crates/farhelm-fixtures/src/fake_agent.rs`) can run the installed file with a small change.
3. `plan/agent-hook-setup/03-helm-opt-in` (`feat:`): the two all-hosts switches and the dismissal preference (schema
   migration from whatever `SCHEMA_VERSION` is on `main@origin`), their routes, and marking every start of those agent
   kinds while the switch is on, with tests covering each kind of start, including Restart and Replace of sessions
   created before the switch was turned on.
4. `plan/agent-hook-setup/04-gui-opt-in` (`feat:`): the switches in the Grok and Antigravity sections, the launcher
   prompt, the session band, the Restart tooltip and launcher wording, and the agent instructions wording, with GUI
   tests and goldens.
5. `plan/agent-hook-setup/05-docs` (`docs:`): SPEC.md and SPEC_impl.md for the hook exception and the new Restart
   wording, `website/src/content/docs/docs/agents/antigravity.md` and `grok.md` with the manual instructions replaced by
   the setting, `docs/how-it-works/agent-hook-injection.md`, `docs/using/stop-restart-resume.mdx`, and the removal of
   both TODO entries from TODO.md. Keep SPEC text that describes earlier PRs' behavior in those PRs when it is the
   contract a reviewer checks against; this PR carries what is left.

One changelog fragment covers PRs 2 to 4 (later PRs amend it). It says the hooks can now be set up from Settings or the
launcher, what the setting changes outside Farhelm, and that an old manual Antigravity entry in `hooks.json` can be
deleted.

## Fallbacks

- If Antigravity or Grok on the executing host behaves differently from "Where it stands" (a newer CLI ignores
  hand-placed plugin directories, say), do not fall back to editing the shared `hooks.json` and do not probe by writing
  into the host's real configuration. Build against the verified behavior and record the difference in the report's
  "things you should know".
- If `PROTOCOL_VERSION` or the helm's `SCHEMA_VERSION` moved on `main@origin` by execution time, bump from the current
  values and renumber cleanly on rebase per `.agents/careful-rebase.md`.
- If marking Restart and Replace starts would mean changing a stored launch rather than adding a field at spawn time,
  prefer the spawn-time field and log the choice. Decisions to log: the exact shape of the per-kind file answer and its
  file contents, how the per-launch field and the per-session field are spelled and versioned, whether the prompt and
  the band share one dismissal, the final wording of the switch, prompt, band and tooltip, and every review finding
  declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-hook-setup-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-agent-hook-setup-watchdog.json` beside it. Resolve both to absolute
paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change. Run
`python3 releasing/check-changelog.py format` on the changelog fragment, `dprint check` on changed Markdown files, and
`python3 scripts/check-agents-md.py` when a change renames or removes a Markdown heading.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/agent-hook-setup/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words what was built.
  Where what got built departs from what this file describes, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's two TODO.md entries.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at high effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

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

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question. One such
item to cover in the report: what each agent does in a session outside Farhelm if Farhelm is uninstalled and the hook
command no longer exists, if you can tell without writing the host's real configuration.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (as you
restructured it, if you did), together meeting the acceptance criteria above, each having passed its review gate, and
the last PR removing this plan's TODO.md entries. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Farhelm can now enable conversation tracking (e.g. so 'restart' works and allows you to resume a conversation) for
Antigravity and Grok. Previously this was a manual installation step because it requires modifying the agent
configuration globally - they don't support enabling the necessary hooks just for the purpose of a single agent
invocation. Now, it will prompt you to enable this when you launch a session unless you've turned it off (and you can
shut it up forever with one click if you don't want it).

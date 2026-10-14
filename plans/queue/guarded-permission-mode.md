# Non-YOLO launches are "guarded", forced by explicit flags and explained per agent

Written against main at 66af18cd on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Farhelm shows whether a session runs YOLO from the choice it launched with, but for most agents the non-YOLO launch
passes no permission flag at all, so the agent's own configuration decides, and a configuration that defaults to a
bypass mode silently makes a "not YOLO" session YOLO (observed with Grok on 2026-10-11, and reproduced for Claude,
Codex, Muse and Cursor during planning). Fix the claim at its source and make the interface honest about it:

1. **Define the line.** A launch is **YOLO** when the agent acts with no check at all: no person, no classifier or AI
   reviewer, no sandbox can stop an action. A launch is **guarded** (the non-YOLO kind) when the agent may approve
   actions on its own, but only through a safety mechanism that can still refuse or escalate: a classifier or AI
   reviewer, a sandbox, or asking the user about anything outside those. Guarded does not mean "asks about everything".
2. **Pick each agent's guarded mode**: the most automatic guarded mode the agent offers; asking about every action only
   when an agent has no guarded automatic mode. Explicit choices Farhelm already offers (OMP's approve; Goose's approve,
   smart approve and chat) stay available unchanged.
3. **Force it with explicit flags** on every guarded launch and its resume, verified on the real agents to win over a
   configuration that defaults to YOLO, so the agent's own config can never silently turn a guarded launch into YOLO.
4. **Where no flag can guarantee it, don't claim it**: label the session as running on the agent's own setting, and have
   the host setting that asks before YOLO launches treat it as YOLO.
5. **Rename "default" to "guarded" in the interface and explain it per agent**, with hover text that names the agent's
   mode, what it means, the flags Farhelm passed, and a closing paragraph that Farhelm cannot guarantee the agent's
   behavior. The maintainer approved this mockup as is: https://snippets.scode.org/s/farhelm-guarded-permission/
   (session rows and shield hover, the launcher's permission choices with a **?** on the chip, the templates dialog
   field). Treat its wording as the starting point and correct it where verification on the real agents shows it is
   wrong.

### What planning verified on this machine (2026-10-13)

All observations used `scripts/agent-tui.py` in fresh directories, without changing user-level vendor configuration
(versions as observed; Grok and Cursor updated themselves during testing).

| Agent             | Today's non-YOLO launch                                                                                                                                 | Guarded mode to force                                                                                          | Flag verified to win over config?                                                                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Claude 2.1.296    | `claude`; starts in auto mode (Claude's built-in default since 2.1.283) unless config says otherwise; a `--settings` or user config setting bypass wins | auto mode: `--permission-mode auto`                                                                            | `--permission-mode default` was verified to override every configured mode, including user-level bypass and `--settings`; `auto` uses the same flag but was not itself tested against a bypass config: verify     |
| Codex 0.162.1     | `codex` (plus trust `-c` flags); YOLO under a config with `approval_policy=never` and `sandbox_mode=danger-full-access`                                 | the old `--full-auto`: `-a on-request -s workspace-write` (runs freely in the sandbox, asks before leaving it) | yes: `-a` overrides the approval policy and `-s` the sandbox, over `-c`, project config and `default_permissions`; both are needed (`-a on-request` alone with a full-access sandbox ran unasked and unsandboxed) |
| Grok 1.0.52       | `grok --no-leader`; "always-approve" from this host's own config                                                                                        | auto-review: `--permission-mode auto`                                                                          | `--permission-mode default` verified to override the always-approve config; `auto` showed "auto-review" on its own; verify it overrides always-approve too                                                        |
| Muse 1.4.7        | `muse`; starts in Auto-review; YOLO when config's `permissions.default_profile` is `:unrestricted`                                                      | auto-review: `--permission-profile :auto-review`                                                               | `--permission-profile :ask-me` verified to override an unrestricted default; `:auto-review` not tested: verify. `--permission-profile` is not saved to Muse's settings (unlike `--reasoning-effort`, see below)   |
| Cursor 2026.10.12 | `cursor-agent`; "Run Everything" when its config's `approvalMode` is `unrestricted`                                                                     | auto-review: `--auto-review`, only if verified to win                                                          | not tested. The bundled code decides the start mode with `force \|\| approvalMode === "unrestricted"`, so the flag may lose; if it does, Cursor takes the "agent's own setting" fallback                          |
| OMP 18.10.4       | Approve = `omp --approval-mode always-ask`; omitted = YOLO                                                                                              | write mode: `--approval-mode write` (approves workspace edits, asks before anything else)                      | `--approval-mode` is a runtime override that beats configured `tools.approvalMode`; verify `write` specifically                                                                                                   |
| Goose 1.50.1      | `env GOOSE_MODE=approve\|smart_approve\|chat goose session`; env beats config                                                                           | unchanged (its explicit choices stay; smart approve is its auto-approve style choice)                          | yes, env beats config                                                                                                                                                                                             |
| OpenCode, Pi      | YOLO-only; plain OpenCode without `--auto` also ran unasked                                                                                             | unchanged                                                                                                      | n/a, Farhelm already calls them YOLO, which is accurate                                                                                                                                                           |

What no flag covers, for every agent: the user's own allow rules (Claude, Grok, which also imports Claude's rules, and
OMP accept them from project files, so a cloned repository can ship one), approval hooks and plugins, "always allow"
answers, sandbox auto-allow settings, managed/enterprise policy, and switching modes inside a session (Shift+Tab,
`/permissions`, `/always-approve`, `/run-everything`, `/mode`). The hover's closing paragraph says so; this plan does
not try to detect them.

### Acceptance criteria

- Every guarded launch, and its resume command, passes the flags in the table above for Claude, Codex, Grok, Muse and
  OMP, and for Cursor only if `--auto-review` is verified to win over an `unrestricted` config. Each flag's win over a
  YOLO-defaulting configuration is verified on the real agent during the run (see Verification on real agents) and
  recorded with the agent's version.
- **The stored value (settled in planning).** Add one new permission value, `LaunchPermission::Guarded`, offered for
  Claude, Codex, Grok, Muse and OMP (and Cursor only if its flag verifies). The helm turns a request with no permission
  into `Guarded` for those agents; OMP's omitted permission stays YOLO, as stored OMP launches already mean it (today
  `normalize_selection` turns it into an explicit YOLO). When classifying a stored launch, "no permission" on those
  agents means **agent's own setting**. That one rule covers sessions launched before this change, their copies, and the
  Cursor fallback. Split today's `effective_permission` into one rule for new requests and one for stored launches. Make
  Goose's permission list explicit (`offers_permission` currently accepts every value for Goose, so `Guarded` would
  silently become a Goose choice).
- **Protocol bump (maintainer agreed).** `LaunchPermission` is a closed list and `LaunchSelection` rejects unknown
  fields, so older supervisors cannot read `Guarded`. Bump the helm–supervisor protocol version, as the earlier
  launch-choice additions did. After upgrading, remote hosts must be updated before launching on them, and older
  releases cannot read launches stored as guarded; the maintainer accepted the missing downgrade path for this.
- **Older sessions (maintainer agreed).** Restart and resume reuse the stored command (SPEC.md, Creation: "A plain
  restart relaunches the session's own stored launch"), and so do plain Replace, agent clone and
  `farhelm spawn --inherit-agent`, which copy it into new sessions. Those sessions and copies ran without the forcing
  flags, so they show the grey "agent's own setting" shield and count as YOLO for the host guard; on a host that asks
  before YOLO launches, a plain Replace of one asks for confirmation (agent clones and inherits are already refused
  there by `check_agent`'s exact-match rebuild). Clone, Replace with and Restart with build a new command from the
  stored choices and get the guarded flags.
- **Cursor without a verified flag** takes the same "agent's own setting" treatment (grey dashed shield, "Not
  classified" heading), YOLO for the helm's guard (`crates/farhelm-helm/src/yolo_guard.rs`); the YOLO confirmation
  (`crates/farhelm-ui/src/yolo_confirm.rs`) gains a fourth reason, "the agent's own setting, which Farhelm cannot
  override", and SPEC.md's list of reasons with it.
- **Wording.** The interface says **guarded**, or **agent's own setting** where no flag is verified, wherever it says
  "default" for the permission today. Known surfaces, each to be checked: the session row's shield hover
  (`permission_description` / `agent_badge` in `crates/farhelm-ui/src/list/row.rs`); the launcher's permission button
  and its tooltip ("use the agent's default permission mode", drawn in `crates/farhelm-ui/src/launch_controls.rs`); the
  launcher's search word "default" and `perms:default` history labels and `permission_tooltip`
  (`crates/farhelm-ui/src/launch_composer.rs`); the templates dialog's "default (asks)" (`list/templates.rs`,
  `save_template.rs`); `restart_with.rs` and `create_form.rs`; the approval card for agent requests (`approvals.rs`:
  "agent's default", and a YOLO yes/no line that needs an "own setting" answer); the command line
  (`farhelm agent create` / `spawn --permissions default` and its "…, or default" error text in
  `crates/farhelm/src/main.rs`, keeping `default` accepted as an alias if removing it would break existing callers,
  logged); and the agent instructions (`agent_instructions.rs`). Each shows the agent's guarded mode by name and its
  one-line meaning. Screen-reader text carries the same meaning.
- **Hover text.** The row's shield hover stays plain text (the existing tooltip, `assets/tooltip.js`, disappears when
  the pointer leaves, so a link inside it could never be clicked): it names the mode, its meaning, "Launched with" and
  the closing paragraph from the mockup, and names the docs section on permission modes. "Launched with" shows the
  session's actual stored command (`display_command()`, which the approval card already uses), which is truthful for old
  and new sessions alike. The launcher's **?** is a button with the same hover text that opens the docs section through
  the existing `open_external_page` (as the app menu's docs entry does in `app_bar.rs`). The per-agent flag list the
  launcher describes before launch must not drift from what the helm passes: share one definition (for example in
  `farhelm-proto`) or pin agreement with a test, and log the placement against the map in `agent_kind/mod.rs`. This
  departs from the mockup's clickable link only in where the link lives.
- The permission's colour and shapes keep their split: green plain shield for guarded, amber slashed for YOLO, plus the
  new grey dashed shield for "agent's own setting". The YOLO confirmation and the host's YOLO setting keep keying off
  the YOLO class.
- The docs website explains permission modes (`website/AGENTS.md` and `website/EDITORIAL_RULES.md` govern): the
  guarded/YOLO line, the per-agent table of modes and flags, what Farhelm cannot guarantee, the known ways each agent
  can be loosened, and the older-sessions caveat. A section with an anchor in the agents overview
  (`website/src/content/docs/docs/agents/index.mdx`, which already has an "Approval prompts or YOLO" column and says
  "Run with the agent's own approval prompts", which must change anyway) is enough; a separate page only if the section
  grows too large (log it). The per-agent pages are updated, including the caveat in `grok.md` that the flag now fixes.
- SPEC.md: the definition of guarded and YOLO, every launch line and permission table that names the non-YOLO argv
  (around the agent descriptions, the permission tables, the YOLO classification, the shield description in the session
  list, and the YOLO guard), and the "agent's own setting" case. SPEC_impl.md: where the flags are compiled and why.
- Per-agent behavior goes where root `AGENTS.md` "Harness-specific code" and the module map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` say it goes; no `kind == X` or catch-all arms over harnesses in
  shared code.
- Tests pin the new argv for every agent and choice, fresh and resume (`crates/farhelm-helm/src/launches.rs` tests), the
  classification (guarded, YOLO, own setting, including a stored pre-upgrade launch and its plain Replace, clone and
  inherit copies) and its guard behavior, OMP's new choice, Goose's explicit list, the protocol version, and the
  interface wording per agent.
- Changelog fragment(s): user-facing (`kind: changed`), naming that non-YOLO sessions are now called guarded, that
  Farhelm now forces each agent's guarded mode instead of leaving it to the agent's configuration, and the Cursor case.
  The maintainer asked that the upgrade caveats reach the release notes, so both the fragment and the PR description of
  the PR that introduces them state, in user terms: remote hosts must be updated after upgrading before launching on
  them (the protocol bump); sessions started before the upgrade show the grey "agent's own setting" shield and count as
  YOLO on hosts that ask before YOLO launches until relaunched with Clone, Replace with or Restart with, and a plain
  Replace of one asks for confirmation there; and there is no downgrade path for launches stored as guarded.
- The TODO.md entry is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: detecting mode switches inside a session from the agent's status line (the maintainer put that in its own
TODO entry), reading vendor configuration files, overriding users' allow rules, hooks or sandbox settings, offering
Claude's other modes as choices, a "custom" permission mode next to guarded and YOLO (its own Near term TODO entry,
added by the maintainer while this plan was written; do not build it, but do not shape the data model so it cannot be
added later), and changes to Goose, OpenCode or Pi.

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text:

> **Check that "not YOLO" is true in practice.** Farhelm decides and shows whether a session runs with approval prompts
> from the flags it passed, but an agent's own configuration can make it run without them anyway. Observed 2026-10-11:
> `grok` 1.0.50, started as Farhelm starts it with no YOLO choice (`grok --no-leader`), showed "always-approve" in its
> status line because the host's Grok configuration defaults to it. The same can happen to any agent whose settings set
> a default approval mode. Two parts, across all agents. First, assess the non-YOLO launch on real agents with
> `scripts/agent-tui.py` (`.agents/real-agent-tui.md`): what each shows at start when it really is in its approval mode
> (workspace trust, approval prompts), since for Grok only its always-approve mode has been seen. Second, decide how
> Farhelm avoids claiming a session asks for approval when it does not: pass an explicit non-YOLO flag where the agent
> has one, detect the effective mode from what the agent reports or from its configuration, or show the permission as
> the agent's own setting when Farhelm cannot know. The plain shield on a row, the YOLO confirmation, and the host
> setting that stops agents from starting YOLO sessions all rely on that claim being true.

**The user's decisions (2026-10-13):**

- Research inline during planning rather than leaving it to the executor: "figure out what's up with each. the goal is:
  never 'promise' not yolo if it's actually yolo effectively". The table above is that research.
- The definition, in the maintainer's words: "the default 'auto' mode is intended to be non-yolo, that is not a bug.
  Similar to codex behaving like --full-auto. non-yolo doesn't mean super-duper-constrained. so codex for example should
  be the auto-approve thing that used to be called --full-auto. equivalent for muse. But we should be clear on this in
  the GUI, more so than now. for omp, always-ask is fine if there isn't an auto-approve style mode. but with those
  corrections, we should be passing the arguments that we believe will force what we consider non-yolo."
- The maintainer then confirmed the planner's statement of the behavior (the goal's points 1 to 5 and the per-agent
  mapping, including OMP's `write` mode and leaving Codex's approval reviewer to the user's configuration, since an AI
  reviewer is still guarded).
- The name and the interface: "'default' doesn't make sense and is arguably a UX bug. Maybe we call it 'guarded' - but
  also surface specifically what it means for the agent in question", with "some kind of on-hover help that also makes
  the point that we cannot guarantee anything here. We promise we pass those flags, but harnesses might change behavior
  over time". The maintainer approved the mockup as is.
- Live status-line detection of mid-session switches is not in this plan; it is its own Near term TODO entry.
- After the planning review: a new stored `Guarded` value with a protocol bump, remote hosts needing an update, and no
  downgrade path for guarded launches ("Yes, bump the protocol"); and older sessions shown as "agent's own setting"
  until relaunched with Clone, Replace with or Restart with ("Yes, grey for old sessions").
- "make sure the caveat around the upgrade case here and greyed out / custom bla bla shows up in the PR description so
  it'll be picked up in release notes".
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md's agent descriptions, permission tables, YOLO classification ("classified exactly"), the session list's shield
  description, the YOLO guard, and "Upgrade compatibility and client scale"; SPEC.md's statement that Farhelm-supplied
  integration must not make vendor configuration a condition.
- Root `AGENTS.md`: Harness-specific code; Harness marks in the sidebar (if glyph geometry changes); Driving real agent
  TUIs and `.agents/real-agent-tui.md`; The live install is off-limits; Docs website; Conventional Commits; Finishing
  work; Releases and the changelog; Talking to the user.
- Global instructions: tests never modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below, the exact wording in the mockup, and the docs page's shape.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- The launch argv is compiled in `crates/farhelm-helm/src/launches.rs` (`compile()`, `program()`), with tests at the
  bottom pinning every agent and choice; the permission tables are in `crates/farhelm-proto/src/launch.rs`
  (`offers_permission`, `omitted_permission`, `offers_only_yolo`, `effective_permission`) and the classifier in
  `crates/farhelm-proto/src/yolo.rs` (`selection_is_yolo`) and `session_launch.rs` (`SessionLaunch::yolo()`).
- The interface: `agent_badge()` / `permission_description()` in `list/row.rs`; `PermissionGlyph` and `PermissionIcon`
  in `icons.rs`; the launcher's permission results and `permission_tooltip` in `launch_composer.rs`; templates in
  `list/templates.rs`; the YOLO confirmation in `yolo_confirm.rs`; the helm guard in `yolo_guard.rs`; the host setting
  `yolo_without_asking` in `hosts.rs` / `store.rs` and its dialog help in `hosts/settings_dialog.rs`.

Here's the work, as a proposal:

1. **Verify the flags on the real agents** first (below) and record the results; adjust the table where reality differs.
   Anything that changes the mapping beyond the table (a flag that does not exist or loses) follows the fallback rules
   under Unattended fallback.
2. **Launch compilation and classification**: the per-agent guarded flags for fresh and resume launches, OMP's guarded
   choice, the "agent's own setting" classification for Cursor (if needed) and its YOLO-guard treatment, with tests.
3. **Interface**: the guarded wording, per-agent descriptions, the hover text with flags and closing paragraph, the grey
   dashed shield, the launcher's **?**, templates, and screen-reader text.
4. **Docs page, per-agent docs, SPEC.md and SPEC_impl.md, changelog, TODO removal.**

### Verification on real agents

Follow `.agents/real-agent-tui.md`. For each of Claude, Codex, Grok, Muse, Cursor and OMP, start the agent exactly as
the new guarded launch will, and in its resume form for every agent that has one (Claude, Codex, Grok, OMP; Codex
matters most, since `-a`/`-s` sit before its `resume` subcommand), under a configuration that defaults to its YOLO mode,
and show from the screen (status line or `/status`) that the guarded mode wins. Simulate the YOLO-defaulting
configuration without touching user-level vendor configuration: command-line config overrides (Codex `-c`, Claude
`--settings`), a project-level file in a scratch directory where the agent reads one, or a scratch config directory the
agent supports (Cursor's `CURSOR_CONFIG_DIR`; Grok's `GROK_CONFIG` overlay; Muse `-c`). Observe without submitting
prompts where the screen settles the question; at most a couple of harmless prompts per agent (`touch probe.txt` in the
scratch directory) where it does not, declining any approval prompt. Record versions and exact commands in the working
log and the report. Also check that `--permission-mode auto` does not refuse to start (or silently fall back) when auto
mode is unavailable for a model or account; refusing to start would be a regression, so if it does, record it and decide
per Unattended fallback. Watch for side effects planning hit: agents updating themselves, Muse saving launch flags into
its settings (check after each Muse run that `~/.config/muse/settings.json` is unchanged, and restore it if a run
changed it, recording that in the report), and trust entries written for scratch directories.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-proto, farhelm-helm launch and YOLO-guard tests, and the farhelm-ui tests for the
  row, launcher, templates and confirmation (by filter while iterating, then the affected packages once);
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if tests change;
- Playwright on Chromium and WebKit for the specs that cover the launcher's permission choices, the row tooltips and the
  YOLO confirmation (`e2e/tests/`), since this changes their visible wording;
- the website build (`cd website && bun install --frozen-lockfile && bun run build`);
- `dprint check` on changed Markdown and `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-guarded-permission-mode-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/guarded-permission-mode/<nn>-<short-name>` (for example `plan/guarded-permission-mode/01-guarded-flags`).
- Expected shape: a few PRs along the outline, each leaving main correct on its own. The PR that introduces the protocol
  bump and the older-sessions behavior carries the upgrade caveats in its description and fragment (see the acceptance
  criteria), not only in the commit message. The flags and the wording that describes them should not be far apart: a PR
  that changes what a guarded launch does should not leave the interface describing the old behavior on main for long;
  either land them together or keep the interface PR immediately next. Avoid churn: code added in one PR and deleted in
  a later one means the stack should have been shaped differently. Within this run, a PR that needs correcting is
  restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is user-visible behavior, so `feat` or `fix` as fits each
  PR. Each such PR adds its changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md`
  (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the verified table, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that no guarded launch or resume can omit its flags; that stored launches are
not reinterpreted; that the "agent's own setting" case can never pass the YOLO guard as guarded; and that the wording
claims only what Farhelm controls.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example reading vendor configuration,
  status-line detection, overriding allow rules or sandbox settings, or a permission data model beyond the one `Guarded`
  value settled above (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- each agent's verification result, version and exact command;
- Cursor's outcome (flag or fallback);
- the request and stored-launch rules after splitting `effective_permission`;
- the CLI's handling of `--permissions default`;
- where the shared per-agent flag list lives;
- wording changes from the mockup;
- the stack's shape;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- An agent whose guarded-mode flag does not exist or loses to a YOLO-defaulting configuration takes the "agent's own
  setting" treatment (grey dashed shield, YOLO for the host guard), like Cursor. If an asking-mode flag for that agent
  does win (as Claude's `default`, Grok's `default` and Muse's `:ask-me` did), using it instead is NOT agreed: the
  maintainer wants the auto-approve style mode where one exists. Record it as an open question in the report and take
  the own-setting treatment.
- Wording in the mockup that verification shows is inaccurate is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: reading vendor configuration, live detection,
changing Goose, OpenCode or Pi, or overriding users' allow rules.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

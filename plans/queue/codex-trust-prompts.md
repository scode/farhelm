# Workspace trust for Codex, Muse and Pi moves from the launcher to Settings

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file states a decision the user made to change them; those changes go into SPEC.md in the same
PR as the behavior.

This plan has no dependency on another plan.

## The goal

Make workspace trust consistent across agents. Today Claude's folder-trust question is answered by an opt-in Settings
switch for managed checkouts (SPEC.md, "Settings also offers an off-by-default, all-hosts choice to answer
Claude's..."), while Codex, Muse and Pi instead offer a per-launch trust choice in the launcher (SPEC.md, the launcher
paragraph around "an explicit workspace-trust choice on Codex, Muse, or Pi is remembered"). The TODO entry was written
when the maintainer saw Codex ask "trust this folder?" and did not remember that Codex's trust is a launcher choice; the
maintainer has since said there is no evidence of a bug, only this inconsistency. This plan is a product change, not a
regression fix.

The user decided:

- Remove the per-launch workspace-trust choice for Codex, Muse and Pi everywhere it exists.
- Add one Settings switch per agent (Codex, Muse, Pi) in the managed-checkouts section beside Claude's, each off by
  default, each covering only managed checkouts (folders Farhelm clones, including agent-requested checkouts), applied
  per launch through the agent's own command-line mechanism where that works, never writing vendor configuration unless
  the Codex fallback below is needed.
- While an agent's switch is off and its trust question is on screen in a managed checkout, show a hint band like
  Claude's, with its own dismissal: for Codex, and for Muse and Pi where their trust question can be recognized.

Before building, verify that Farhelm's per-launch trusted override still suppresses the trust question on the installed
Codex (0.163.0 on the planning machine; the override was last verified on 0.155, and 0.159 redesigned the dialog,
`crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`). The result decides whether the
Codex switch can use the per-launch override or needs the agreed fallback (Fallbacks).

TODO.md entry this plan covers (Near term): "**Stop Codex's folder-trust questions from returning.** Codex appears to be
asking "trust this folder" again in Farhelm sessions. The maintainer recalls an earlier fix; investigate what changed
and restore the intended behavior."

### Acceptance criteria

- **Override verified on current Codex.** On the installed Codex (record `codex --version`), check whether a launch
  carrying Farhelm's filled trusted override (`fill_cwd` in `crates/farhelm-supervisor/src/agent_kind/mod.rs` turns
  `{codex:trusted-cwd}` into `projects={"<path>"={trust_level="trusted"}}`) starts without the trust question, in a
  plain folder and in a git repository's root. If it does not, try other per-launch forms (the dotted
  `projects."<path>".trust_level` key, keying the git root) and use the one that works, with a unit test pinning the
  argv. If none works, the Codex switch uses the config-writing fallback. The report states the result and the Codex
  version. The `{codex:...}` markers stay available in command launches (SPEC.md, the command-launch paragraph naming
  `{codex:trusted-cwd}`).
- **Three new switches.** Settings → managed checkouts shows a switch for Codex, Muse and Pi each, next to Claude's,
  each off by default and stored helm-wide like `checkout_config.claude_trust`
  (`crates/farhelm-helm/src/checkout_config.rs`). Each one's help briefly says what it does (Farhelm answers the agent's
  folder-trust question for folders it cloned, through a per-launch flag; nothing in the agent's own configuration
  changes) and the trust risk (repository hooks, settings and commands then run as if you had said yes). No Claude-style
  file-editing caveats, unless the Codex fallback is used, in which case the Codex switch carries them.
- **Read when a launch is composed, managed checkouts only.** The switch is read whenever the helm composes a launch: a
  new create, Clone, Restart with, Replace with, an agent-requested create, and `farhelm agent clone`. Plain Restart and
  plain Replace re-run the session's stored launch as SPEC.md already specifies, exactly as Claude's switch affects
  "future checkouts only". When the agent's switch is on and the composed launch's destination is a managed checkout,
  the launch carries that agent's trust mechanism: Codex the trusted marker, filled by the supervisor once the
  checkout's path is resolved; Muse `--trust-workspace`; Pi `--approve`. Otherwise it carries none, whatever the client
  or the cloned source sent: a Clone or agent clone of a trusted managed-checkout session into a folder the user picked
  is not trusted. Without the Codex fallback nothing writes vendor trust state, and the SPEC.md sentence "the separate
  opt-in for Claude managed checkouts below is the one exception" stays true.
- **The per-launch choice is gone from every surface** for Codex, Muse and Pi: the launcher's "workspace trust" segment
  (`crates/farhelm-ui/src/launch_controls.rs`, help text in `crates/farhelm-ui/src/launch_composer.rs`), the
  `trust:true`/`trust:false` search actions and the `trust:` search label, the remembered choice (its UI readers and
  writers), the template editor and Save as template, `farhelm agent create`/`spawn` `--trust`
  (`crates/farhelm/src/main.rs`) and its mention in `crates/farhelm/src/agent_instructions.rs`, and the Restart with /
  Replace with controls (`crates/farhelm-ui/src/restart_with.rs`). Approval and YOLO-confirm cards, which render from
  the selection, either show nothing or say the trust comes from Settings; log which as a DECISION.
- **Old data and old clients keep working.** Stored sessions, templates and preferences from before decode and work.
  `LaunchSelection` and `TemplateFields` are `deny_unknown_fields` (`crates/farhelm-proto/src/launch.rs`,
  `crates/farhelm-proto/src/launcher.rs`), and every supervisor stores the selection, so `workspace_trust` stays on the
  wire and in both structs: a template's value decodes and is ignored when applied and dropped when saved; a client's
  value is overwritten by the helm. `preferences.remembered_workspace_trust` is no longer read or written; the column
  stays. The update path from every stable release since v0.23.0 still works (SPEC.md, "Upgrade compatibility and client
  scale"). No downgrade path is needed (user decision).
- **Hint bands.** While an agent's switch is off and its hint not dismissed, and that agent's trust question is visible
  in the agent terminal of a managed checkout, a band like `ClaudeTrustBand` (`crates/farhelm-ui/src/session_view.rs`,
  `crates/farhelm-ui/src/claude_trust.rs`) appears above the terminal, below its tabs, naming the agent and offering
  "Settings" (opening Settings at that agent's switch), "Don't show again" (a shared per-agent dismissal, reversible
  beside the switch, like `preferences.claude_trust_hint_dismissed`) and ×. Codex's recognition is tested against the
  0.159.0 `waiting-trust` fixture (and a current capture if the screen changed) and must not fire on transcript text
  containing the same words (mirror `claude_trust_prompt_matches_captured_dialogs_only` in
  `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`). Muse and Pi get a band where their trust question can be
  captured and recognized the same way. An older UI talking to a newer supervisor must never show Claude's band on
  another agent's session.
- **Spec and docs.** SPEC.md's launcher paragraph, the Muse paragraph ("Muse agent launches also offer a workspace-trust
  choice"), the template field list, the Restart with paragraph, the `farhelm agent create` flag list, the
  remembered-defaults paragraph ("the remembered permission mode and workspace-trust choice") and the managed checkout
  Settings paragraphs describe the new behavior; SPEC_impl.md where it describes the removed parts. Website pages that
  describe the trust choice are updated (`website/src/content/docs/docs/agents/codex.md`, `muse.md`, `pi.md`,
  `agents/index.mdx`, `using/start-a-session.mdx`, `using/launch-templates.mdx`, `using/stop-restart-resume.mdx`, and
  any other page `grep -i trust` finds).
- The last code PR removes the TODO.md entry above.

## Requirement sources

**The user's decisions (2026-10-17), in their words where they explain why:**

- "I forgot this is how it worked. Let's actually completely remove this from the launcher. Just make it a global
  setting just like it is for Claude so we have perfect consistency. [Do] the same thing as we do with Claude where when
  it's turned off there's some indication that hey if you turn it on we can make this go away. But in the case of Codex
  as far as I understand since we can properly do it with a command line argument we don't have to have all those
  warnings about stuff that we have with Claude besides just briefly emphasizing the trust risk itself."
- "I don't think I actually observed a bug. I think it was mostly just me not remembering that for Codex it was an
  option in the launcher, which is what prompted me to want to do this change to be consistent. ... there's no clear
  evidence of bug, just UX inconsistency."
- The switch covers managed checkouts only, exactly like Claude's; folders the user picks keep the agent's own question.
- Muse and Pi go the same way: their launcher trust choice is removed and they join the setting.
- One switch per agent in the same Settings section (Claude, Codex, Muse, Pi), since they carry different caveats.
- Off by default, like Claude's.
- If no per-launch form stops current Codex's question, the Codex switch writes Codex's saved trust for each managed
  checkout the way Claude's switch edits Claude's file, with Claude-style caveats, and the entry is removed when the
  checkout is archived.
- Muse and Pi get the band where their trust question can be recognized.
- The executor may run the installed `codex`, `muse` and `pi` locally in throwaway folders to see their trust questions,
  sending no prompts.
- No downgrade path is required for this plan's database and protocol changes (the upgrade path stays intact), and the
  trust choices saved in existing templates and the launcher's remembered trust choice are kept but ignored.
- An earlier question about launches with no trust choice was superseded by these decisions.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- SPEC.md: the launcher paragraph on workspace trust; plain Restart and plain Replace re-run the stored launch; Clone
  and `farhelm agent clone` carry the source's stored launch; "Settings also offers an off-by-default, all-hosts choice
  to answer Claude's..." and the two paragraphs after it (the mechanism and caveats, and the hint band); the capture
  paragraph's no-agent-configuration rule ("What capture never does is write to the agent's own configuration...");
  "Upgrade compatibility and client scale"; the remembered-defaults paragraph ("Only what the user explicitly selects in
  the GUI may affect the GUI's future defaults").
- The YOLO host guard (`check_agent` in `crates/farhelm-helm/src/yolo_guard.rs`) vouches for an agent launch only when
  `launches::compile(selection)` reproduces the stored launch exactly, so compile output must stay a function of the
  selection alone.
- Root `AGENTS.md`, "Code rules": per-harness behavior goes in the places the module docs of
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` map; no `kind == X` or `matches!` over named harnesses in shared
  code. `LaunchHarness::offers_workspace_trust` (`crates/farhelm-proto/src/launch.rs`) is the exhaustive per-harness
  fact; the flags live in the helm's compiler (`crates/farhelm-helm/src/launches.rs`, the
  `match (selection.harness, selection.workspace_trust)` block under the comment "Workspace trust is independent of tool
  approval"). Muse and Pi run as `AgentKind::Generic`, so a Muse or Pi trust recognizer follows the launch-harness
  reader pattern in `screen_reader.rs` (`status_line_yolo`), not the per-kind reader.
- `docs/agent-screen-fixtures.md`: committed screens come from real agents, carry no personal data, and hand-made
  screens the capture tool cannot reach are named `*-derived-*`.
- Root `AGENTS.md`, "The live install is off-limits" and "Sharing the machine with other agents".
- Root `AGENTS.md`, "Changelog fragments" and "Instructions by path" (`website/` → read `website/AGENTS.md` before
  editing a page).

**Planner proposals (checked by a fresh-context planning review):**

- Carry trust in the existing `LaunchSelection.workspace_trust` field and set it in the helm: on every launch the helm
  composes, it overwrites the field with `Some(true)` when that agent's switch is on and the destination is a managed
  checkout (a new checkout, where `claude_trust` is resolved into `ResolvedGithubCheckout` in
  `crates/farhelm-helm/src/sessions.rs`; an agent-requested checkout; or Restart with / Replace with / Clone onto a
  session whose working copy is a managed checkout), and `None` otherwise. Compile, `offers_workspace_trust`, the
  placeholder fill and the stored format stay as they are, older supervisors need nothing new, and the YOLO guard keeps
  matching. Keep the `Some(false)` compile arms (Codex untrusted, Pi `--no-approve`) so old stored launches still
  recompose; only the UI paths that produced them go away.
- The selection is part of the create fingerprint: a keyed retry that crosses a switch flip must reuse the first
  attempt's value, the way `claude_trust` is frozen for retries, or the retry is refused as key reuse.
- Store the three switches beside Claude's (`checkout_config` columns) and the per-agent dismissals beside Claude's
  (`preferences`), in one schema bump. Extend `GET/PUT /api/checkout-claude-trust` with optional per-agent fields (or
  one generic route); a PUT carrying only `enabled` keeps meaning Claude, so older clients keep working.
- Bands: implement `trust_prompt` on the Codex screen reader (the trait method already defaults to false and the ticker
  samples it for every reader). Keep the `claude_trust_prompt` wire field Claude-only, since `show_hint` in
  `crates/farhelm-ui/src/claude_trust.rs` does not check the harness and an older UI would show Claude's band; add one
  harness-neutral boolean that defaults to false, and let the UI name the agent from the session's own harness.
- Muse and Pi screens: run the installed `muse` and `pi` in a throwaway folder without the trust flag, sending no
  prompt. If a trust question appears, save it, scrubbed per `docs/agent-screen-fixtures.md`, as a `*-derived-*` fixture
  under `muse/<version>/` and `pi/<version>/`, and recognize it from there. Pi's `--approve` is described in SPEC.md as
  approving project resources; check what Pi actually shows without it before writing a recognizer.
- Never answer a trust question in these experiments: Codex's "esc back" leads to an "Agent Command Center", so kill
  your own process, and nothing is written to `~/.codex/config.toml` or another vendor file. Codex 0.157+ may start a
  machine-wide background server when it runs; it can be shared with the maintainer's own Codex sessions, so never stop
  it.
- If the Codex fallback is needed, model it on Claude's mechanism (`crates/farhelm-supervisor/src/agent_kind/claude/`
  `trust.rs` and its call in `crates/farhelm-supervisor/src/launch.rs`): edit only that checkout's trust entry in
  `~/.codex/config.toml` on the target host, preserving the rest of the file, with an atomic replacement, immediately
  before the agent starts; best-effort removal on archive; a launch never fails because the edit failed. SPEC.md's "one
  exception" sentences and the no-agent-configuration rule then name two exceptions.

## Outline

A linear stack, roughly:

1. `feat:` Settings switches for Codex, Muse and Pi and the helm setting `workspace_trust` on composed launches into
   managed checkouts (including Clone and agent clone normalization), with SPEC.md and docs; fragment `kind: added`. If
   the Codex verification failed, the Codex config-writing fallback lands here or as its own `feat:` PR right after
   (editing this fragment rather than adding another).
2. `feat!:` removal of the per-launch trust choice from the launcher, search actions, remembered choice, template
   editor, CLI `--trust`, Restart with / Replace with and agent instructions, keeping the wire fields readable, with
   SPEC.md and docs; fragment `kind: breaking` (the agent command's `--trust` flag and the launcher choice are gone;
   templates that set trust ignore it).
3. `feat:` hint bands: Codex's recognizer, Muse and Pi where captured, the harness-neutral signal and the dismissals (an
   edit of PR 1's fragment, or its own `kind: added`). Removes the TODO.md entry.

No `fix:` commit: the user saw no bug. Reorder if it shrinks churn; keep the switches landing before the removal so
there is never a state where a managed checkout cannot be trusted at all.

### Validation

Choose per `.agents/validation.md`; at least `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, and through the
recorder the affected Rust tests: the helm's launch compiler, YOLO guard and `checkout_config` tests including a
schema-upgrade test like `claude_trust_schema_upgrade_and_persistence`, decoding of stored selections and templates that
carry a trust value, the keyed-retry fingerprint, the supervisor's `fill_cwd` and screen-reader tests (and the config
edit's tests if the fallback is built), the CLI and agent-instructions tests, and `farhelm-ui` unit tests. Browser specs
on Chromium and WebKit through the recorder, after `dx build`: `claude-trust.spec.ts` plus new coverage for the new
bands, `templates.spec.ts`, `remembered-permissions.spec.ts`, `restart-with.spec.ts`, `replace.spec.ts`,
`clone.spec.ts`, `launcher-enter.spec.ts`, `goose-pi-composer.spec.ts`, `github-checkouts.spec.ts`,
`tooltip-coverage.spec.ts`, and any other spec that fails to compile or references the removed control. The website
build (`cd website && bun install --frozen-lockfile && bun run build`), `python3 releasing/check-changelog.py format`,
and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when browser or Rust tests change. The real
Codex, Muse and Pi checks are manual and recorded in the log; the report names the versions.
`e2e/docs-shots/start-a-session.spec.ts` may capture the removed control; do not re-capture docs screenshots (that is
`.agents/docs-media.md`'s flow), and say in the report if a published screenshot now shows it.

## Fallbacks

- **No per-launch form stops current Codex's trust question** (user decision): the Codex switch writes Codex's saved
  trust for each managed checkout the way Claude's switch edits Claude's file, with Claude-style caveats in its help,
  and removes the entry when the checkout is archived. Proposal above.
- **Muse's or Pi's trust question cannot be captured or recognized reliably, or there is none:** ship that agent's
  switch without a band, log a DECISION, and say so in the report.
- **Pi's `--approve` answers something other than a folder-trust question** (SPEC.md describes it as approving project
  resources): still move it to the setting as decided, word its help for what it does, and report the finding.
- **Older clients or supervisors cannot be kept working** with the fields kept on the wire: that weakens the upgrade
  promise and is a block, not a decision.

Examples of departures that need the scope reassessment: a new compile-time input outside the selection, recompiling
plain Restart or Replace, a general per-agent settings framework beyond the three switches, migrating old trust values
into the switches, writing any vendor configuration file other than Codex's under the agreed fallback, a new
screen-capture tool.

Decisions to log: the Codex verification result and the form used; what approval and YOLO-confirm cards show; the
storage and route shape; how each compose path recognizes a managed-checkout destination; the retry freeze; each agent's
band, or why there is none.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-codex-trust-prompts-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/codex-trust-prompts/<nn>-<short-name>` (for example `plan/codex-trust-prompts/01-trust-settings`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

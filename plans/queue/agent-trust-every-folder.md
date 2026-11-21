# Agent trust in every folder

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers two TODO.md entries (bucket "Near term"), merged into one plan at the maintainer's request. Verbatim:

> **Fix "answer trust dialog" for Claude Code.** fix "answer trust dialog" feature for claude code. confirmed to not
> work in 0.39.0 for a plain folder. might just be that we only enabled the behavior for managed checkouts. either way
> it's infuriatingly annoying. let's also test out codex while we're at it. Plan:
> `plans/queue/agent-trust-every-folder.md`.

> **Consider consistent workspace-trust scope across agents.** Antigravity's workspace-trust switch applies to all
> launches (including plain folders), whereas Claude, Codex, Muse and Pi under Settings → Agent trust currently apply
> only to managed checkouts. Decide whether to make the scope consistent across agents and under what conditions (see
> also "Fix 'answer trust dialog' for Claude Code"). Plan: `plans/queue/agent-trust-every-folder.md`.

The 0.39.0 report is the specified behavior, not a regression: Claude's edit runs only in fresh-checkout preparation and
its hint band is hidden outside managed checkouts, so a plain folder could never be helped. The maintainer decided to
widen every agent's trust switch to every folder, as Antigravity's already is.

This plan runs after `agent-hook-setup.md`, which moves the trust switches, hint switches and Claude's caution box into
each agent's own Settings section and adds a per-launch, per-kind supervisor step with a protocol bump that this plan's
Claude step follows; that plan's work must be on main first. Wherever this file names Settings → Agents → Agent trust,
read "the agent's own Settings section" as agent-hook-setup left it.

### Where it stands (from planning research at main 3617ee450)

| Agent       | Mechanism                                                                                            | Scope today                                       | Persists?               |
| ----------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ----------------------- |
| Claude      | Edits `projects["<path>"].hasTrustDialogAccepted = true` in Claude's own config file, under its lock | fresh managed checkouts only, at clone time       | yes; removed at archive |
| Codex       | per-launch `-c projects={"<cwd>"={trust_level="trusted"}}` from the `{codex:trusted-cwd}` marker     | New/Clone/Restart with/Replace with, managed only | no                      |
| Muse        | `--trust-workspace`                                                                                  | same as Codex                                     | no                      |
| Pi          | `--approve`                                                                                          | same as Codex                                     | no                      |
| Antigravity | `--settings '{"trustedWorkspaces":["<canonical cwd>"]}'`                                             | every structured launch                           | no                      |

- **Claude:** the only call site is checkout preparation (`crates/farhelm-supervisor/src/launch.rs`, about 1967-1991,
  `if preparation.claude_trust`), fed by `ResolvedGithubCheckout.claude_trust` (`crates/farhelm-helm/src/sessions.rs`,
  about 1565), which exists only for fresh GitHub checkouts. The file work is in
  `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs` (resolves the config file as Claude does, keeps a removal
  record, edits atomically, leaves an absent file absent); removal at archive is in `service/teardown.rs`.
  `LaunchHarness::Claude.offers_workspace_trust()` is false (`crates/farhelm-proto/src/launch.rs`, about 222). SPEC.md
  about 1739-1741: "It never applies to a directory the user selects themselves." Even inside managed checkouts, an
  existing checkout cloned while the switch was off never gets trust later.
- **Codex, Muse, Pi, Antigravity:** the helm writes `LaunchSelection.workspace_trust` when composing a launch
  (`crates/farhelm-helm/src/workspace_trust.rs`, about 65-149); scope comes from
  `workspace_trust_requires_managed_checkout` (`crates/farhelm-helm/src/launches.rs`, about 735: true for Codex, Muse
  and Pi), which looks up managed-checkout membership through `client.managed_checkout_for_cwd` (also used by Browse, so
  that read stays). The flags are compiled in `launches.rs` (SPEC_impl.md about 5128-5143). SPEC.md about 1765-1781
  ("Ordinary user-selected directories receive no trust override", Antigravity the exception) and 1119-1124.
- **Hint bands:** `eligible()` in `crates/gui/farhelm-gui/src/session_pane/trust_band.rs` (about 121-122) hides the band
  outside managed checkouts for managed-only agents (`TrustAgent::managed_checkouts_only`,
  `crates/gui/farhelm-gui/src/settings/agent_trust.rs`, about 314). Recognizers: `claude_trust_prompt` and
  `trust_prompt` in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`.
- **Settings wording** lives in exhaustive `TrustAgent` methods (`label`, `mechanism`, `help`, `band_words`, the
  caution) and the search catalog (`settings/search.rs`). SPEC.md about 3930-3932: the Claude trust edit is "a separate,
  narrowly scoped exception to the no-agent-configuration rule".
- **History of the restriction:** Claude's (2026-10-15, `git show 62ed56aa3:plans/queue/claude-trust-prompt.md`) was
  chosen for frequency, with no recorded safety reason; Codex, Muse and Pi (2026-10-17,
  `git show 11b087d6e:plans/queue/codex-trust-prompts.md`) copied it "for perfect consistency". Antigravity (2026-11-20,
  `plans/queue/antigravity-workspace-trust.md`) chose all folders.
- **Unverified:** whether today's Claude still honors `hasTrustDialogAccepted` keyed at the git root (researched on
  2.1.300; newest captured 2.1.319), and whether today's Codex still shows the 0.159.0 trust dialog and honors the
  per-launch override (the 0.166.0 recapture saved no `waiting-trust` screen).

### Acceptance criteria

1. **Live verification first.** With `scripts/agent-tui.py` (`.agents/real-agent-tui.md`) in fresh scratch folders,
   observing without answering and sending no prompt: Claude with and without the key set, in a plain non-git folder, a
   git repository root and a subfolder of a repository (where does Claude look for the key?); Codex with and without the
   filled override, in a plain folder, at a git root and in a subfolder of a repository (Codex resolves project trust
   against the repository root, while the override is keyed at the cwd). Setting the key for Claude means writing
   `hasTrustDialogAccepted` for the scratch paths into the executing host's real Claude config file (an isolated
   `CLAUDE_CONFIG_DIR` would stop at onboarding or sign-in first). That is authorized here, on these conditions: write
   with the same lock discipline `trust.rs` uses (or through `trust::edit` itself), only for the scratch paths you
   created, and remove exactly those keys afterwards, recording both in the working log. Record versions and quote
   screens. If Codex's dialog changed, recapture with `scripts/capture-agent-screens.py --ui-only` per
   `docs/agent-screen-fixtures.md` and update the recognizer. If a mechanism no longer works on the current version,
   fixing it is in scope. This real-agent run is authorized by the TODO entry ("let's also test out codex while we're at
   it"); `--ui-only` spends no model turn.
2. Every trust switch (Claude, Codex, Muse, Pi; Antigravity unchanged) applies to every newly composed launch in any
   folder, managed checkout or not: New, Clone, Restart with, Replace with and agent-requested launches. Plain Restart
   and plain Replace keep the stored launch (SPEC.md about 1772-1776).
3. Switches already on widen with the new scope; no migration turns them off.
4. Claude reuses `LaunchSelection.workspace_trust` (settled in planning review: the approval card's chip, freezing into
   stored launches and fingerprints, plain Restart/Replace replay, and every composition path, Clone, inherited spawn,
   Restart with and agent-requested launches, come free once `offers_workspace_trust()` is true for Claude, and
   `workspace_trust_arguments` returning nothing for Claude works as is). On the supervisor, one exhaustive per-kind
   answer in the `agent_kind` map says "this selection's trust means a pre-exec Claude config edit" and fills a
   `#[serde(default)]` bool on `LaunchSpec`; the shim, which already runs the existing edit (`run_checkout_preparation`
   from `exec_launch_spec_with_seam`, `launch.rs` about 1189 and 1970), applies the edit before exec in any folder
   through a helper lifted from that block. The protocol version is bumped as a semantic bump (no new wire field: an
   older supervisor would accept a Claude selection with trust and silently do nothing). The edit stays best effort and
   never fails a launch; `trust::edit` already skips the write when the key is true, under the lock, so add no pre-check
   outside it. A Claude launch with trust shows the existing "Trusted workspace" chip on its approval card (the chip is
   drawn only there). 4a. Records and removal (planner's choice from planning review, option (a)): the fresh-checkout
   preparation edit stays exactly as it is, with its removal record and removal at archive, including SPEC.md's
   "checkouts initially started with another agent" (do not retire `ResolvedGithubCheckout.claude_trust`). Every other
   Claude launch with trust, whether into a plain folder or an existing managed checkout, gets the shim edit with no
   record. The leak this leaves (a checkout cloned while the switch was off, later opened with Claude, keeps its entry
   after archive) is the same accumulation the maintainer's release note already accepts for plain folders.
5. Codex, Muse and Pi: the scope change in the helm, with everything that becomes dead removed rather than kept "for
   later": the trust-only membership lookups, `workspace_trust_requires_managed_checkout`,
   `normalize_launch_workspace_trust`'s `client`, `fresh_checkout` and `cwd: &mut String` parameters (the canonical-path
   rewrite existed only to pin the path that supplied trust; check nothing else relied on `req.cwd` being canonicalized
   before dispatch), and the `managed` parameter of `configured_launch_workspace_trust` with its callers' computation
   (`agent_requests.rs` about 4476, `sessions.rs` about 3495-3512). Still nothing persists.
6. Hint bands show in any folder while the question is on screen and the switch is off, with the new words.
7. Settings wording follows the approved direction (mockup below); the Claude caution box says that for a folder you
   chose, the entry stays after the session ends, as if you had answered Yes, and that Farhelm removes the entry at
   archive for checkouts it cloned while the switch was on (tightened from the mockup to match 4a). The chip's hover
   text (`TRUSTED_WORKSPACE` in `approvals/model.rs`, "Enabled in Agent trust settings") names wherever the switches
   live after agent-hook-setup.
8. SPEC.md (about 1119-1124, 1739-1790, 3930-3932, and the Settings text), SPEC_impl.md (about 3476-3546, 5128-5143) and
   the docs pages (`using/settings.md`, `agents/claude.md`, `agents/codex.md`, `agents/muse.md`, `agents/pi.md`,
   `agents/index.mdx`, and whatever else `grep -il trust website/src/content/docs` lists) match.
9. Tests: helm composition for plain folders (inverting the inherited-spawn and
   `agent_clone_normalizes_trust_for_the_target_before_dispatch` family for plain folders), shim-level Claude edit tests
   reusing the existing child-process fixture `run_shim_in_child_with` (as
   `claude_trust_launch_is_opt_in_and_best_effort_after_ready` does, so the test process's environment never changes),
   band visibility unit tests, and the changed Settings and band goldens. The Codex override form is already pinned by
   `codex_project_trust_uses_the_final_cwd_as_one_config_argument`; change it only if the live check finds a different
   form. Both TODO entries are removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Scope:** every folder, every agent, like Antigravity (not managed-only for any agent, no per-agent three-way choice,
  no exception for agent-requested launches).
- **Existing switches widen**, and the release note says so.
- **The Settings wording direction** (mockup below) is approved.
- **Release note:** the maintainer's own text, in `## Release note` below.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort, plus the mandatory Opus visual review
  (see "Visual review").
- **No-workhorse mode:** the plans default, stated on the planning page.

Already settled before this plan and unchanged: switches stay off by default, all-hosts, one per agent, each with its
own caveats; Codex, Muse, Pi and Antigravity never write vendor configuration; custom permission flags that already pass
the agent's own trust option win (`launches::workspace_trust_blocked_by_selection`); widening the scope does not change
that interaction.

## Requirement sources

**The maintainer's request and decisions:** the two TODO entries and the decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited; root `AGENTS.md`, "Code rules" (per-harness
behavior goes where `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs say; no `kind == X`, `matches!` over
harnesses or `_` arm in shared code); `.agents/real-agent-tui.md` and `docs/agent-screen-fixtures.md` for the live
check; the helm–supervisor protocol-version rules (a semantic bump for Claude's trust; bump from whatever number main
has when you get there, since other plans bump too, and renumber on rebase); `.agents/test-authoring.md`; the GPUI
goldens check; `website/AGENTS.md` for the docs pages; root `AGENTS.md`, "Conventional Commits", "Changelog fragments",
"Tests and validation", and its "Instructions by path".

**Planner proposals:**

- The Claude mechanism is acceptance 4's: reuse the selection field, one per-kind supervisor answer, a `LaunchSpec`
  bool, and the existing shim block lifted into a helper with an optional record sink. Do **not** copy
  agent-hook-setup's spawn-time opt-in: that plan reads its switch at every start, including plain Restart, while this
  bit is frozen at composition and replayed by plain Restart (acceptance 2).
- The edit stays in the shim, not in `prepare_launch`: `trust::Environment::current()` must see the login-shell
  `CLAUDE_CONFIG_DIR`.
- Reuse `trust.rs`'s `resolve`/`edit` as they are; generalize only where they are called. `trust.rs` keys at
  `fs::canonicalize(cwd)` and never looks for a repository root; if the live check shows Claude keys a repository
  subfolder by its root, root discovery is new logic (run the scope reassessment first).

**Mockup (approved direction; tighten the words):**

```
Claude:  (o) Answer Claude's "trust this folder?"
         Before Claude starts, Farhelm marks the session's folder trusted in Claude's own settings file, in folders
         you choose and in managed checkouts, including agent-requested ones. On a remote host, the edit is made to
         that host's Claude settings.
         [amber caution] … For a folder you chose, the entry stays after the session ends, just as if you had answered
         Yes. For a checkout Farhelm cloned while this was on, Farhelm removes it when the checkout is archived.
Codex:   (o) Answer Codex's folder-trust question
Muse:    (o) Answer Muse's workspace-trust question
         For every launch, in folders you choose and in managed checkouts, Farhelm passes [the agent's trust option]
         for that launch. [Agent]'s saved configuration is unchanged. (Keep the New / Clone / Restart with / Replace
         with sentence and the risk sentence.)
Pi:      (o) Approve Pi project resources   (same pattern)
Antigravity: unchanged.

Band (any folder):
  Claude: "Claude asks this for every new folder. Farhelm can answer it for you, but it's off by default: there's no
           supported way to do it, so Farhelm would have to edit Claude's own settings file, which comes with caveats."
  Codex/Muse: "Codex asks about trust for this folder. Farhelm can answer it through a command-line flag. It's off by
           default because repository hooks, settings and commands are then trusted."
```

## Outline

1. `plan/agent-trust-every-folder/01-verify-current-agents`: the live verification (acceptance 1), any recognizer
   recapture, and any fix to `trust.rs` or the Codex override form the check shows is needed. `test:` or `fix:`
   depending on what it finds (a `fix:` carries a fragment). If the check finds nothing to change, record the evidence
   in the working log and the report and fold this step into PR 2 (log it).
2. `plan/agent-trust-every-folder/02-flag-agents-every-folder`: `feat:` Codex, Muse and Pi in every folder (helm scope,
   dead lookups removed), their Settings words and bands, the helm tests, SPEC/SPEC_impl and docs for those agents.
   Changelog fragment (`kind: changed`).
3. `plan/agent-trust-every-folder/03-claude-every-folder`: `feat:` the Claude trust bit, the shim edit in any folder,
   the chip, the band and caution words, SPEC/SPEC_impl and the Claude docs, the shim, composition and band tests, and
   goldens. Edits PR 2's fragment into one entry. Removes both TODO entries.

## Fallbacks

- If the live check shows Claude keys trust somewhere other than where `trust.rs` writes it (for example a subfolder of
  a repository), fix `trust.rs` in PR 1 to match the observed behavior and record the evidence.
- If Codex no longer honors the per-launch override, find the current supported form; if none exists, block with that as
  the question.
- If `triage-spec-text.md` has landed its sentence on fresh Claude checkouts and the birth-time rule, keep it when
  rewriting that SPEC paragraph.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-trust-every-folder-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-agent-trust-every-folder-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/agent-trust-every-folder/01-verify-current-agents`, `plan/agent-trust-every-folder/02-flag-agents-every-folder`,
  `plan/agent-trust-every-folder/03-claude-every-folder`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entries this plan covers ("Fix "answer trust dialog" for Claude Code" and
  "Consider consistent workspace-trust scope across agents"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
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

### Visual review

The maintainer also demands a visual review for every PR that changes what the desktop app draws: a fresh-context
`claude-opus-5-5` agent at `high` effort, launched natively where your harness can (otherwise through
`$scode-harness-shellout`), separate from the reviewer above. Give it headless renders of every changed scene in both
the light and the dark appearance (the GUI golden scenes, regenerated with
`python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run writes), the design this file
describes (its mockup text and wording are the agreed design), and the PR's diff. It must be satisfied that the rendered
result matches the intent of the agreed design; loop until it reports no mismatch. A mismatch with the agreed design is
binding and is fixed; any other observation it makes is an input, handled like the reviewer's findings above. Do not
write a launch command here or in the log.

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
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entries this plan covers.
Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

The setting to automatically cause agents to "trust" the folders you launch sessions in (to avoid the annoying "trust
this folder?" prompts), are now more widely applied across agent types - and all agent launches, not just those using
managed GitHub checkouts. Note that for some agents, this will cause an accumulation of settings entries as the set of
unique project directories you launch into with farhelm over time increases.

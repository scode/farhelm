# Managed checkouts in a session's own sandbox or container need no checkout folder setup

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term"). Verbatim:

> **Set up the default checkout folder from a new environment launch.** The session launcher offers a managed checkout
> in a new environment even when the all-hosts default checkout folder is unset. The helm refuses it before creating the
> environment and sends the user to Settings. Read the all-hosts setting, show the shared inline folder field when it is
> missing, and wait until the saved setting is confirmed before enabling Launch. A host's own override cannot satisfy
> this requirement. The recipe exception applies to previewing the checkout path, so correct the comment at the
> installed-host preview check. Keep quick launch waiting for the global read. Cover an unset default with a host
> override, a configured default, and a shortcut that reveals the setup field when the default is missing or the read
> error when the setting could not be loaded. Plan: `plans/queue/environment-checkout-without-setup.md`.

**The maintainer replaced the entry's proposed fix while planning.** Their words: "we shouldn't be asking the user to
pick a checkout. The user doesn't care. The whole point is that it's just a session running mysteriously in the cloud
without a bunch of extra hassle." So the fix is the opposite of the entry's: a managed checkout on a session-owned host
(a sandbox session's or container session's own sandbox or container) never asks about, waits for or refuses over a
checkout folder. It uses the all-hosts default checkout folder when one is set, and otherwise that machine's home
folder. The launcher shows nothing new; no setup field, no read, no quick-launch wait.

### Where it stands (from planning research and review at main 3617ee450)

- **The helm refuses a new environment's checkout up front** in `validate_sandbox_launch`
  (`crates/farhelm-helm/src/sessions.rs`, about lines 1356-1401, the check at about 1389-1401): it tests
  `resolve_checkout_config(None).root.is_none()` and answers "set a default checkout folder in Settings → Checkouts
  before launching a repository in a new sandbox". Both providers reach it through `start_session_environment`
  (`crates/farhelm-helm/src/provisioning/tensorlake_add.rs`, about 346); agent-requested environment launches too
  (`agent_requests.rs`, about 4489).
- **After the new machine connects**, `finish_session_launch` (`tensorlake_add.rs`, about 1115) calls
  `create_in_session_sandbox` (`sessions.rs`, about 1867), which previews through `preview_github_checkout` (reading
  `resolve_checkout_config(Some(host))` at about 444-452) and then creates through `create_fresh_session_resolved`,
  which reads it **again** (about 2606-2611) and hands it to `github_checkout_resolution` (about 1511-1535, refusing on
  no root and putting the root into `ResolvedGithubCheckout.root`, which the supervisor checks against the preview's
  root).
- **The repository suggestions route** reads the same per-host config (about 318-321) to set `needs_checkout_root`, and
  refuses its scan with "no checkout folder is configured" (about 347-348). The launcher shows its inline setup field
  from that flag.
- **Replace with / Restart with stay in the existing sandbox** (SPEC.md "Sandbox sessions"), and Replace with can choose
  a fresh managed checkout (`replace_with_fresh_checkout`, `sessions.rs` about 4775-4785, through the same create path;
  the launcher's preview goes through `/api/github-checkout-preview` → `preview_github_checkout`). For those, the
  launcher's `checkout_host` returns the sandbox host, so today an unset default shows the setup field there.
- **Session ownership is one row-level fact:** `HostRow::is_session_owned()` (`crates/farhelm-helm/src/store.rs`, about
  1194; mirrored as `is_session_owned` in the manager status), constrained to environment rows by the schema and shared
  by both providers. Shared code already branches on it (`create_target`, `create_in_session_sandbox`, Replace's
  wording); `create_in_session_sandbox`'s Folder branch already sets `req.cwd = "~"` for any session-owned host (about
  1904-1906), the direct precedent for this plan's fallback. `"~"` is already a valid root (`validate_root` accepts it,
  the target supervisor expands it), and the trash and spare directories inside a root are hidden (`.farhelm-trash`,
  `.farhelm-spares`), so a home-folder root leaves no visible clutter.
- **The launcher** skips the setup check for a new environment (`checkout_host` returns `None` when `draft.target` is
  set, `crates/gui/farhelm-gui/src/launcher/view/checkout.rs`, about 330-339). Its comment cites the recipe exception as
  the reason "the host's checkout folder setup is not its concern"; that exception is about previewing the path. The
  `request.rs` comments (about 22-31, 397-399: "the environment has no installation to verify and no folder to preview
  yet") stay accurate.
- **Specs:** SPEC.md "Managed checkouts" (about 1725-1727: "There is no default root … The default is resolved at
  launch") and "Sandbox sessions" (about 1570-1571). SPEC_impl.md "Checkout configuration and discovery" (about
  4856-4863: "the effective root continues to resolve from the current global default at launch"). Website docs need no
  change (`using/start-a-session.mdx` already scopes the setup step to ordinary hosts).

### Acceptance criteria

1. A managed checkout on a session-owned host is never refused, and never offered the setup field, for a missing
   checkout folder: launches that create a sandbox or container (user launches, quick launches, template launches,
   Clone, agent-requested environment launches) and Replace with / Restart with into a fresh managed checkout inside an
   existing session-owned sandbox or container. The checkout goes under the all-hosts default checkout folder when one
   is set, and otherwise under that machine's home folder (`~`, which the target supervisor expands), named after the
   session as usual.
2. Installed hosts, including persistent Tensorlake hosts added through Add host, are unchanged: a host with no folder
   in effect still gets the launcher's inline setup field and the helm's refusal.
3. The fallback is decided once, in one helper next to the existing checkout config reads in `sessions.rs` (for example
   `checkout_config_for(state, host)`): it calls `resolve_checkout_config(Some(host))` and, when the root is `None` and
   the host is session-owned, substitutes `"~"`. All three per-host reads (suggestions, preview, create) go through it,
   so they agree by construction. It is keyed on the existing session-ownership fact, which is the same for every
   provider; no new host-kind, provider or binding method. It stays out of the store's `resolve_checkout_config` /
   `read_snapshot`, which also serve the host checkout-root route and `farhelm checkout show` ("what is stored, with
   inheritance"). The up-front refusal in `validate_sandbox_launch` is removed.
4. The launcher shows nothing new. The comment at `checkout.rs` (about 330-333) says correctly why a new environment
   needs no folder setup (its checkout falls back to the machine's home folder); `request.rs` may gain a clause but
   needs no correction.
5. SPEC.md "Managed checkouts" (and "Sandbox sessions" / "Container sessions" where they describe the checkout) says
   that a managed checkout in a session's own sandbox or container uses the default checkout folder when set and
   otherwise that machine's home folder, and never asks for one. SPEC_impl.md "Checkout configuration and discovery"
   gets the matching line.
6. Tests: a helm variant of `a_recipe_checkout_binds_its_new_machine_preview_before_one_create` (`sessions_tests.rs`,
   about 379-525) with no default set, asserting the launch is accepted, the preview's root is `~` and the create
   frame's `checkout.root == "~"`; a helm test that Replace with into a fresh managed checkout on a session-owned host
   with no default gets `~`; a launcher view test where the host the user came from answers `needs_checkout_root: true`,
   the user picks New container with Managed checkout, and no `launcher-checkout-setup` field shows. The existing tests
   for "a set default is used" (the recipe-checkout test above) and "an installed host with no folder refuses"
   (`create_with_valid_github_repo_without_a_root_points_to_settings`) stay green. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Never ask for a folder for a session's own sandbox or container.** The entry's inline-field design is dropped.
- **Where the checkout goes:** the default checkout folder if one is set, otherwise the machine's home folder.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort. No visual review: nothing new is drawn.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

Planner's call, shown to the maintainer in the planning PR for approval: the fallback also covers Replace with / Restart
with into a fresh managed checkout inside an **existing** session-owned sandbox or container (the page asked only about
new ones). It follows the maintainer's stated principle, costs one more call-site swap, and avoids the launcher showing
a setup field the helm no longer needs.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the maintainer's replacement of its fix (quoted above), and
the decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md as cited; root `AGENTS.md`, "Code rules" (a decision that
differs by provider goes in an exhaustive method; this one does not differ by provider); `.agents/test-authoring.md`;
root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and its "Instructions by path".

**Planner proposals:** the helper shape in acceptance criterion 3; the post-clone command and the other checkout
settings keep resolving as they do today.

## Outline

1. `plan/environment-checkout-without-setup/01-environment-checkout-home-fallback`: `fix:` the helper and its three call
   sites, the refusal's removal, the `checkout.rs` comment, SPEC.md and SPEC_impl.md, the helm and launcher tests, a
   changelog fragment (`kind: fixed`), and the TODO entry's removal.

## Fallbacks

- If another per-host checkout-root read turns up on a path that can serve a session-owned host (an agent-requested
  checkout, say), route it through the same helper and log it.
- If the launcher turns out to refuse or warn about an unset default anywhere for a session-owned host, remove that too;
  nothing should ask.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-environment-checkout-without-setup-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-environment-checkout-without-setup-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/environment-checkout-without-setup/01-environment-checkout-home-fallback`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Set up the default checkout folder from a new
  environment launch"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

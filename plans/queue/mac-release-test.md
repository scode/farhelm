# Agent-driven Mac release test: first slice

Written against main at 5c09230c on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

Build the first slice of item 1 of `lore/2026-10-05-release-brick-protection-plan.md` (read it first; it is the why
behind everything here): an end-to-end release test that an AI agent runs on the maintainer's Mac against a Farhelm
release candidate, using Tart virtual machines, so that a release that bricks existing installations (the new app will
not start on old state, or its helm cannot update the supervisors an older release installed) is caught before it is
promoted to users.

This plan runs on Linux and cannot run Tart or macOS, so it builds only the parts that can be written and checked here,
and leaves the rest, explicitly, to the agent on the Mac:

1. A test-only override of which version the desktop app's in-app updater treats as the latest release.
2. The recipe: the per-release test procedure, addressed to an agent on the macOS host.
3. The bring-up document: the one-time setup of the test environment, addressed to the same agent, which fills in what
   this plan left open and lands its result as its own PR.

The maintainer will start that agent with one sentence: "Read `releasing/mac-vm-test/BRING-UP.md` and do what it says."
Everything that agent needs must therefore be reachable from that file.

Acceptance criteria:

- A linear stack of two draft PRs exists, shaped as in Outline.
- The updater override works as Decisions D3 to D5 specify, with focused unit tests, and SPEC_impl.md documents it.
- `releasing/mac-vm-test/` holds `RECIPE.md` (with the per-release addendum template and the report format as sections
  of it) and `BRING-UP.md`, written for an agent that has never seen Farhelm's internals, and consistent with each other
  and with the override as built.
- Nothing in those documents pretends to have been run: every detail this plan could not verify is marked as a
  recommendation or an open item for the bring-up agent.
- The last PR removes the TODO.md entry "Agent-driven Mac release test, first slice."
- Every PR that changes code or tests passed the review gate. No PR is marked ready; nothing is merged.

## Requirement sources

**The user's request (2026-10-05)**, in the maintainer's words: "I want to get as close as possible to 0% chance that we
ship a bricked release that won't start, or fails to upgrade supervisors. I've been manually tesitng RC releases." And:
"let's assume I will have a Tart VM with a cleanish macOS image, where i can run an agent to drive additional tests
(including having computer use avaialble to it)", with "an 'end to end testing recipe' targeting an agent running on a
clean sandbox macos host to go through the real flow. And the recipe is tweaked as needed for a given release." The
maintainer ranked this first: "step one is getting the actual real macos smoke test in tart vm going, and much of the
rest is making things fail sooner, earlier", accepting that "for a few releases I'm more likely to run a slow test only
to see it fail and have to fix."

**The user's decisions (2026-10-05):**

- D1. Two documents for one agent. "we need both the intended recipe, AND a comprehensive 'here is what we are trying to
  build and what you need to do' hand-off document" for an agent that "you dont have to resolve every little detail, but
  the overall shape. i can cover with Q&A from the agent. i want precise details like what linux image to use etc to be
  resolved by the agent in the actual environment, but you tell it stuff like we're gonna use a separate tart linux vm,
  here is a recommendation (ubunt whatever version etc). the agents job is to fill in the blanks the overnight execturo
  left. the agent will have access to the farhelm git repo and be able to open a PR on its own." The hand-off is a file
  in the repository, not a separate document: "since the agent has access to the repo we don't need a separate handoff
  necessarily, just a sentence".
- D2. The agent runs on the macOS HOST, not inside a VM: "the target agent is running on the macos _HOST_, and will have
  access to create/edit/delete tart vm and run things in them." It drives the VMs from outside: commands inside them,
  and computer use on the macOS VM's display for the app's GUI.
- D3. The updater override is an environment variable naming a version, provisionally
  `FARHELM_DESKTOP_UPDATE_LATEST=v<version>`; the maintainer agreed to that shape. It names only a version, never a
  different download site: everything is still fetched from the real get.farhelm.io and every existing check still
  applies (signature and trusted comment, installer hash, stable versions only, newer than the running build).
- D4. The remote host is a separate Linux Tart VM ("simpler and more 'for realz'"), not a container inside the macOS VM.
  The test Mac is an M2 Max for now (an M-series Mac mini later).
- D5. Sessions on the remote host can be plain commands: "yeah remote can be plain commands". Real, signed-in Claude and
  Codex sessions run on the macOS VM.
- D6. Environment setup is separate from the test: "we should separte 'set up the test environment' (linux vm and macos
  vm base images), from the actual test. the actual test will re-use base images for the VMs so its fast to create the
  VM. but yes, the fresh install should create a fresh vm (based off of a prepared base image)". Setup is the bring-up
  document's job; every test pass clones fresh VMs from the prepared base images.
- D7. Review gate: "opus 5.5 and asta high, no swarm".
- D8. Upgrade path freeze: SPEC.md "Upgrade compatibility and client scale" now requires every stable release from
  v0.23.0 on to update cleanly. The recipe's upgrade pass is how that is checked on a Mac.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog;
the live install is off-limits; Sharing the machine with other agents; Agent scratch space; public-repository hygiene:
no personal usernames, hostnames or local environment details in anything committed), `releasing/AGENTS.md` (the site
layout, signing, and what an RC and a dev release are today), SPEC_impl.md "The desktop app's updater" and "Verification
chain (D3)", and `plans/AGENTS.md` (Executing).

**Planner proposals** are everything in Outline that the decisions above do not fix: file names, the shape of the two
passes, the report format, the PR types. Challenge them through Scope reassessment rather than treating them as
requirements.

## Background the documents must carry

Facts established while planning (verify them again; line numbers drift, find code by name):

- The in-app updater (`crates/farhelm-ui/src/desktop/updater.rs`) is active only when the app runs from
  `$HOME/Applications/Farhelm.app` with a `Versions` folder and is a release build. It learns the latest version from
  one GET of `https://get.farhelm.io/latest` (`probe_latest_release`, parsed by `latest_from_site`), installs version X
  by verifying `get.farhelm.io/vX/SHA256SUMS` and its minisig against the compiled-in key ring and then running that
  release's `install.sh` with `FARHELM_VERSION` and `FARHELM_INSTALL_SUMS_FILE`. Its TLS uses webpki roots, so a test
  machine cannot redirect it by trusting a local certificate authority. SPEC_impl.md already says "The probe is one
  function, so a later channel setting can replace it."
- The override only helps when the OLD side of the upgrade pass is a release that carries it, and only for a stable
  candidate (it refuses prereleases, as `/latest` does). Until then, and for any prerelease candidate (today's RCs; the
  lore plan replaces RCs with stable candidates later), the recipe updates by running the candidate's `install.sh` by
  hand inside the VM with `FARHELM_VERSION=v<candidate>` and `FARHELM_INSTALL_SUMS_FILE` pointing at the candidate's
  downloaded `SHA256SUMS` (its minisign signature checked if the tool is available in the VM; an open item for the
  bring-up agent). That second variable is the installer's permanent contract with every shipped updater (SPEC_impl.md
  "The desktop app's updater"), so the by-hand run must exercise it. The old app re-reads its `Versions/installed`
  record about once a minute, so after a by-hand install its readout should turn to "Restart to update", which the
  recipe then uses: the relaunch path is tested either way. The only parts of the real path a by-hand run skips are the
  old app's own probe and its signature and trusted-comment checks.
- The installer (`scripts/install.sh`) installs a named version with `FARHELM_VERSION`; `get.farhelm.io/<tag>/` serves
  any published release, and `/latest` names only the latest stable one. The helm downloads remote-host payloads for its
  own version from `get.farhelm.io/v<version>/`, verified against the same key ring, so a candidate that is signed and
  published at its version's path works unmodified before `/latest` names it.
- A candidate is testable once it is signed and published at `get.farhelm.io/v<version>/`. Whether `/latest` already
  names it decides only whether the run is a gate or an after-the-fact check. Changing the release procedure to stage a
  candidate before moving `/latest` is item 5 of the lore plan and out of scope here; the maintainer's signing and
  publishing tooling lives outside the repository.
- Sessions live in a tmux server that outlives the supervisor, so they are expected to survive a supervisor restart and
  an update. The macOS app runs its own local supervisor; when the app quits, that supervisor goes away, and the new app
  starts its own version's supervisor.
- Suspected, unverified: a new desktop app that finds an old local supervisor still answering reuses it, and the local
  host then shows "needs update" with its Update action unavailable. The recipe should make this visible if it happens
  rather than work around it.
- `docs/desktop-web-triage.md` says where the desktop app's log lives on a Mac; `docs/manual-mac-checklist.md` is the
  existing manual Mac checklist (GUI behaviors, not updates), which the recipe may point at but does not replace.

## Outline

### PR 1: the updater's "latest" override

A `chore:` or `test:` PR (no user-visible change; log the type as a DECISION). If the type requires a changelog
fragment, add one with `kind: none` and the reason.

- When `FARHELM_DESKTOP_UPDATE_LATEST` is set in the app's environment, the updater's latest-version probe returns that
  version instead of fetching `/latest`. Everything after the probe is unchanged. The value goes through the existing
  `latest_from_site` parser (a `v` and a stable release version), with an error context naming the variable; a malformed
  value fails every check and never falls back to the site, so a test cannot silently test the wrong release. The
  variable's name and the override being in effect go to the app's log only: the readout the user sees and its wording
  do not change.
- It applies to automatic checks and to checks the user starts. It is read where the updater's dependencies are built
  (`start` in `updater.rs`), read once as an `Option<OsString>` and turned into the probe by a pure helper, keeping the
  `Deps` seam the tests use. The relaunch helper opens the bundle without the variable, so the relaunched app is back on
  `/latest`; say so in the SPEC_impl.md paragraph.
- The installer's environment is untouched: the updater already strips every `FARHELM_*` variable, which keeps this one
  out of `install.sh`.
- SPEC_impl.md, "The desktop app's updater": a short paragraph on the override, its purpose (the release test in
  `releasing/mac-vm-test/`), and why it cannot weaken verification. SPEC.md's sentence that the updater asks
  get.farhelm.io which release is latest stays true for users; add a clause only if leaving it alone would be wrong.
- Focused unit tests for the parsing and the refusal of a malformed or prerelease value, in the style of the existing
  updater tests. No test may set environment variables of the test process (root `AGENTS.md`, and the maintainer's
  standing rule): read the variable once at the edge and pass the value in.

### PR 2: the recipe and the bring-up document

`docs:`. One PR for both documents, since they are written for the same reader, point at each other, and are mostly at
risk of disagreeing with each other. New directory `releasing/mac-vm-test/` (planner proposal; another name under
`releasing/` is fine if logged). `RECIPE.md` is the per-release procedure for the host agent of D2. It assumes the
bring-up document's environment exists (base images, scripts, and the names BRING-UP.md settles) and says so at the top,
with a pointer to BRING-UP.md for anything missing.

#### `RECIPE.md`

- Inputs: the candidate version, the previous stable version (how to find it: `get.farhelm.io/latest` before promotion,
  or the release history), and the per-release addendum.
- Two passes, each on fresh clones of the base images (D6), deleted afterwards:
  1. Fresh install: a new macOS VM and a new Linux VM; install the candidate with the public installer
     (`FARHELM_VERSION`); open the app; add the Linux VM as a host, so the candidate's helm provisions it from scratch;
     start a real Claude session and a real Codex session locally and a plain-command session on the Linux host (D5).
  2. Upgrade: a new macOS VM and a new Linux VM; install the previous stable release; build up real state (the Linux
     host provisioned by the old release, local sessions with real agents and with some conversation in them, a
     long-running plain command on the remote host whose output proves it kept running, a template, a changed setting);
     update to the candidate by the rule below, ending with the app's own "Restart to update"; then check that the app
     starts at the candidate version, the local host is connected (not "needs update"), existing sessions are still
     there and responsive and can be restarted and resumed, the remote host is offered an update, the update succeeds,
     the remote supervisor comes back at the new version, and the remote session kept running; then quit and reopen the
     app and check again.
- How the upgrade pass updates, by one explicit rule: if `/latest` already names the candidate, the plain in-app update
  (an after-the-fact check, not a gate); if the candidate is a stable release that `/latest` does not name yet and the
  previous release carries the override, start the old app with `open --env FARHELM_DESKTOP_UPDATE_LATEST=v<candidate>`
  and use its update; otherwise, including every prerelease candidate, the by-hand path in Background. The recipe says
  which path a run took in its report, and says plainly that the bring-up dry run (a candidate already on `/latest`)
  exercises only the first.
- What to collect as evidence: a screenshot per check, the app's log, `farhelm --version` inside the VM, and whatever
  else a failure needs; where it goes on the host (outside any repository).
- The report format: a verdict (pass, fail, or could not run), the candidate and previous versions, one line per check
  with its result and evidence, and for a failure, what was seen against what was expected. Never a credential, host
  name or user name in a report that might be shared.
- The per-release addendum template, a section of `RECIPE.md`: what changed in this release that deserves hands-on
  checking, drafted from the release's changelog fragments (`releasing/changelog.d/`) and any protocol or schema version
  change, with the extra steps to run. Proposal: addenda are written per release and kept with that run's report on the
  host, not committed.
- Guardrails for the host agent: the VMs are disposable and the only thing it changes; never touch a Farhelm
  installation on the host itself (the host Mac may run the maintainer's own Farhelm; root `AGENTS.md`, "The live
  install is off-limits", applies there too); the base images hold real agent credentials, so they are never pushed to a
  registry, and nothing from them is committed.

#### `BRING-UP.md`

`releasing/mac-vm-test/BRING-UP.md` is addressed to the same host agent, started by the maintainer's one sentence (see
The goal). It explains what is being built and why (point at the lore entry), what this plan built and what it
deliberately left open, then what the agent must do, as recommendations and open items rather than settled details (D1):

- Two base images, both Tart VMs. macOS: a clean recent macOS image (the agent picks one, for example from Cirrus Labs'
  public Tart images), with the Claude and Codex CLIs installed and signed in (the maintainer signs in; the agent asks),
  and whatever the recipe needs inside it, but no Farhelm. Linux: recommend Ubuntu 26.04 LTS, falling back to 24.04 LTS
  if no good Tart image exists, with a user the macOS VM can ssh into by key and a systemd user manager (the remote
  supervisor runs as a systemd user service), and no Farhelm.
- Open items to settle in the real environment: how the two clones reach each other (Tart's shared network and `tart ip`
  are the likely path), ssh keys and host keys between them, how the agent runs commands in the macOS VM (`tart exec`,
  or ssh) and drives its display (the VM's window or VNC) with computer use, how `open --env` passes the override to the
  app, where reports and evidence go, and the macOS limit of two concurrently running macOS VMs.
- Host-side scripts the recipe calls (clone, start, wait for readiness, find addresses, collect evidence, delete), which
  the agent writes and commits under `releasing/mac-vm-test/` as part of its PR, written so that nothing in them names
  this machine, its user, or a credential.
- A first dry run of RECIPE.md with the latest stable release as the "candidate" and the release before it as the
  "previous" one, fixing the recipe where it is wrong.
- Its deliverable: a PR (Conventional Commits, per root `AGENTS.md`) with the scripts, the filled-in BRING-UP.md and
  RECIPE.md, and a short record of the dry run's result; questions go to the maintainer, not into guesses.

PR 2 also removes the TODO.md entry "Agent-driven Mac release test, first slice."

### Validation

Follow root `AGENTS.md` "Finishing work" and pick the smallest checks that cover each PR:

- PR 1: `cargo fmt --all -- --check`; `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`;
  `cargo check -p farhelm-ui --features desktop`; and a focused nextest selection for the updater tests through
  `python3 scripts/record-test-run.py` with `-p farhelm-ui --features desktop` and `--tmux none` (the pinned nextest
  setup is in `docs/test-run-evidence.md`). `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if
  Rust tests changed.
- PR 2: `dprint check` on the changed files. No runtime tests (documentation only).
- Read the two documents once more, together, as the host agent would: does BRING-UP.md alone get it started, does
  RECIPE.md name every environment piece BRING-UP.md is asked to provide, and does every command or variable match PR 1
  as built.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-mac-release-test-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/mac-release-test/<nn>-<short-name>`.
- The stack follows Outline: PR 1, then PR 2. Never add something in one PR that a later PR of this stack deletes.
  Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat:`,
  `fix:`, `perf:`, `style:` or `revert:` PR adds a changelog fragment per root `AGENTS.md` (Releases and the changelog);
  validate any fragment with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Write the documents in the maintainer's voice with the `scode-voice` skill, and code comments and docstrings per the
  maintainer's documentation rules.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate two independent reviews of that PR's changes,
and address what both find before moving on. The user demands exactly these reviewers, and no review swarm (D7):

- a fresh-context agent on Opus 5.5 at high effort;
- a fresh-context agent on gpt-6-astra at high effort, shelled out to the harness that serves that model when the
  executing one cannot reach it natively.

If either reviewer cannot be launched after the retries galaxy-brain's routing allows, that is a block, not a reason to
run with one reviewer.

Both get the same prompt, carrying the full charter, because neither reviewer has anything else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a separate
findings file per reviewer (in the scratch directory), and the acceptance criteria for that PR: its part of Outline, the
decisions that apply to it, and the goal's acceptance criteria. For a PR that changes tests or fixtures, include the
full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. PR 2 is documents, but they are the
whole deliverable for an unattended reader, so it gets the review gate too; for it, "correctness" means: is every
factual claim about Farhelm true of the code, is every step something an agent on a macOS host could actually do, and is
anything the plan could not verify presented as settled. Where you disagree with a finding, decide on the merits and log
the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: an override that names a
download site rather than a version, a new persisted setting, changes to the installer or the release workflow, a test
harness that tries to run any of this on Linux), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions
above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the PR types and any changelog fragment, the override's final name and parsing
rules, the directory and file names, the shape of the two passes and the report, what was left open for the bring-up
agent, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the two draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR passed the review gate. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

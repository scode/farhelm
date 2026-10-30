# Containers: run the agent as the host user's account

Written against main at 462b0268 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

**Depends on `podman-local-image.md`** (its INDEX.md line says `(after podman-local-image.md)`): that plan replaces the
OCI archive with a local Podman image name. This plan inspects the image in a second, short-lived container, which from
an archive would cost a second full import (about 12 s for a 2.4 GiB image) under the creation fence that Stop and
Delete wait at most 30 s on; from a local image it costs about half a second. Both plans also rewrite the same
container-creation code, tests, spec paragraphs, docs page and changelog fragment. Build on main as that plan left it.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Containerized sessions: match the host account inside the container.** Consider running the agent with the same
>   uid, user name and paths as the user's host account, so files, paths and tools look the same inside and out, instead
>   of the image's uid-1000 user.

Today every containerized session runs as uid 1000 (`--userns=keep-id:uid=1000,gid=1000` in `create_command`,
`crates/farhelm-supervisor/src/container.rs`), so the agent runs as whatever account the image has at uid 1000 (`ubuntu`
on Ubuntu 24.04 images, home `/home/ubuntu`), and keep-id maps that uid to the host account so files written in the
mounted checkout belong to the host user. Nothing about 1000 is required; it was the original plan's choice to match the
stock Ubuntu test image (`plans/queue/podman-sandbox.md`, frozen).

### What changes

The container's account is chosen to match the host account by default, adapting to what the image already has. In the
maintainer's words: "The default assumed 'most convenient' behavior is that uid+username matches in the container." With
`<name>`, `<uid>`, `<gid>` and `<home>` the host account's user name, uid, gid and home path, read from the host's user
database (`getpwuid_r` for the supervisor's uid, not `$USER` or `$HOME`), the account is chosen from the image as it is
before anything is mounted, in this order:

1. **Neither `<name>` nor `<uid>` exists in the image's user database:** create the account the usual Ubuntu way, named
   `<name>`, with uid `<uid>`, gid `<gid>` and home `<home>`, seeded from `/etc/skel`. The maintainer: "just create it
   per usual via /etc/skel using normal tools provided by ubuntu (we assume ubuntu for now)."
2. **Else, `<home>` exists as a directory in the image:** run as whichever account owns that directory, with it as the
   home. The maintainer: "if ~/USERNAME already exists, use whatever user that home belongs to. Since we remap uids
   anyway, it'll mostly be fine. Useres are more likely to have hard coded their usernames, than their uids."
3. **Else, `<uid>` exists in the image:** run as that account, with its home directory.

Whichever account is chosen, keep-id maps the host account onto that account's uid and gid, so checkout files keep
belonging to the host user. Images can be prepared for this: an image pre-seeded for a specific user is used as that
user (rule 2), and an image that pre-seeds `/etc/skel` gives any created account (rule 1) a working home. The
maintainer: "This way we adapt. And users can choose to create a pre-seeeded image for a specific user (or even pre-seed
/etc/skel with whatever they want working for any user)." This rule is written into SPEC.md, as the maintainer asked.

What the order means on common setups, which the spec and docs state plainly:

- A host account at uid 1000 with a stock Ubuntu image (`ubuntu` at uid 1000, no `<home>`) hits rule 3: the agent runs
  as `ubuntu` with `/home/ubuntu`, as today.
- A host account at another uid, with an image that has neither `<name>` nor `<uid>`, hits rule 1: a fresh account
  seeded from `/etc/skel`. Images prepared the old way (agents on the uid-1000 user's `PATH`, their configuration in
  that user's home, `sudo` for that user) then give an account without the agent, its configuration or `sudo`. The docs
  and changelog say so, and what to do: install agents system-wide or put their configuration in `/etc/skel`, or
  pre-seed the user's own account.

### What does not change

The checkout bind at its host path, networking, the token, the creation fence, Stop, Delete, and how tabs and the agent
reach the container (they inherit the container's account and `HOME`). Linux only, as today. No new launch setting: this
replaces the uid-1000 behavior outright (maintainer decision), so there is no wire or template change. A container that
already exists keeps the account it was created with until Replace.

### Acceptance criteria

- Container creation picks the account by the three rules above, in that order, from facts read out of the image before
  any mount, so the checkout bind's parent directories (which Podman creates as root, and which usually include
  `<home>`) cannot be mistaken for an existing home.
- Rule 1 creates the account with Ubuntu's own tools inside the container and seeds `<home>` from `/etc/skel` whether or
  not the checkout bind already created `<home>`, owning the home directory and the seeded entries without changing
  anything inside the bind.
- The agent, every terminal tab and every other `podman exec` run as the chosen account with `HOME` set to its home,
  with no per-exec user or `HOME` state (tabs rebuild their exec from the stored launch and know nothing about the
  account); files written in the checkout belong to the host account on the host.
- Choosing or creating the account is part of creation under the fence. If any step after `podman run -d` fails, the
  helper removes the just-created container (confirmed absent, still under its lock) before it fails, so a retry never
  starts a container without its account. An image whose facts cannot be read, a host name or uid Ubuntu's tools refuse,
  or missing account tools are refused with a clear message rather than falling back to uid 1000.
- The pure choice (from the image's user and group databases plus `<home>`'s existence and owner) has unit tests for
  every rule and every boundary listed under Planner proposals.
- The real-Podman end-to-end tests in `crates/farhelm/tests/e2e/containerized_sessions.rs` stop asserting uid 1000 and
  assert rule-independent facts instead: `id -un` resolves, `$HOME` equals the user database's home for `id -u` and is
  writable by that account, and files written in the checkout belong to the host uid. They also cover each rule,
  including a `<home>` that is a parent of the checkout.
- SPEC.md ("Containerized sessions", including its uid-1000 sentences, the ownership paragraph and "The agent runs
  inside the container with the image's environment", which now gains `HOME`) states the rule, its order and the two
  setups above; SPEC_impl.md ("Rootless Podman sessions") records how the account is chosen and created, why the choice
  is made before mounting, and why `HOME` is set at creation; the containerized-sessions docs page
  (`website/src/content/docs/docs/agents/containerized-sessions.md`) replaces "A user with UID and GID 1000" with how an
  image can prepare an account and states the consequence for images prepared the old way.
- Containerized sessions have shipped only in a release candidate, not a stable release, so per root `AGENTS.md`,
  "Changelog fragments", the PR edits the existing unreleased `releasing/changelog.d/containerized-sessions.md` fragment
  (keeping `kind: added`), including the consequence for images prepared the old way, rather than adding a new entry.
  The PR title is `feat:`, without `!`.
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29, on the planning page and in conversation:

- **Match uid and gid, user name and home path**, as the entry says, by default.
- **Replace the uid-1000 behavior**, rather than a per-launch choice; containerized sessions are experimental.
- **The account is chosen by the three rules above**, in the maintainer's words quoted there, and the rule goes into
  SPEC.md. The maintainer rejected Farhelm renaming the image's user or moving its home ("why rename and stuff, why not
  just create the user we are gonna use?").
- **Assume Ubuntu images for now**: rule 1 uses Ubuntu's normal account tools.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md, "Containerized sessions"; SPEC_impl.md, "Rootless Podman sessions" (the
creation fence and ticket, the scrubbed Podman environment, Podman's status not being a lifecycle oracle); root
`AGENTS.md`, "Code rules", "Sharing the machine with other agents", "Tests and validation" (including no tests that
change the test process's environment variables), "Conventional Commits", "Changelog fragments"; `website/AGENTS.md`;
`.agents/test-authoring.md`; `.agents/validation.md` (how the real-Podman e2e tests run).

**Planner interpretation** of rule 2 (the maintainer wrote "if ~/USERNAME already exists, use whatever user that home
belongs to"): `<home>`, the host account's home path (normally `/home/<name>`), exists as a directory in the image, and
its owner is the account to run as. The alternative reading, the home the image's user database gives `<name>`, makes
"whatever user that home belongs to" almost always `<name>` itself and would miss an image that pre-seeded
`/home/<name>` under another account. The maintainer saw this reading when the plan was written; if it proves wrong,
that is a follow-up, not a block.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code:

- **Reading the image.** One short-lived inspection container of the same local image, without keep-id, mounts or
  `--workdir`, with `--pull=never`, `--network none`, `--user 0:0` and a shell entrypoint printing `/etc/passwd`,
  `/etc/group` and the owner and type of `<home>`, run from the creator (`container::prepare`, under the fence) before
  `podman run`. The creator is a synchronous helper: use `control_command`'s scrubbed environment in its existing
  synchronous style, not the async `ContainerRuntime::run` runner. Do not use `podman image mount` (rootless, it needs
  `podman unshare`).
- **Creating the container.** `--userns=keep-id:uid=<chosen uid>,gid=<chosen gid>`, `--passwd=false` so Podman does not
  inject its own passwd entry for the mapped user (which would make rule 1's `useradd` fail on a duplicate; verify the
  flag on Podman 4.9.3 and that it also covers group entries), and `--env HOME=<chosen home>` so the keeper and every
  exec inherit it; verify `podman exec` does not recompute `HOME` over it. The keeper unit test's assertion that
  creation passes no `--env` changes deliberately.
- **Rule 1, one path.** `groupadd` when `<gid>` is free (otherwise use the existing group), `useradd -M -l` (no home
  creation; `-l` avoids the lastlog blow-up with large uids), `mkdir -p <home>`, copy `/etc/skel/.` into it without
  overwriting, and `chown` the home directory and the copied entries only, never recursively through the home. If the
  checkout bind is `<home>` itself or contains it, skip seeding so nothing lands in the user's real checkout.
- **Boundaries to decide and log, with these defaults:** `<home>` owned by uid 0 or by a uid with no account falls
  through to rule 3; an account at `<uid>` (rule 3) whose home is missing gets that home created and seeded the rule-1
  way; a host uid at or above the subordinate-id range that keep-id cannot map is refused clearly. Name these in the
  report's "things you should know", since the maintainer's rules do not cover them.
- **E2E without derived images.** Let the supervisor's test seams supply the host identity (name, uid, gid, home) that
  the creation ticket carries to the helper, instead of the helper reading it itself; that is dependency injection, not
  an environment change. The stock offline test image then covers all three rules, including a `<home>` that is a parent
  of the temporary checkout, with no image builds to name and clean up.
- One PR.

## Outline

1. **Choice.** The pure function from the image's user and group databases plus `<home>`'s existence and owner to the
   account (and whether to create it), with unit tests for every rule and boundary; the inspection that reads those
   facts; the host identity carried in the creation ticket.
2. **Creation.** `create_command` with the chosen uid and gid, `--passwd=false` and `HOME`; rule 1's account creation
   and seeding; removal of the container when a later step fails.
3. **Exec.** Confirm the agent and tabs inherit the account and `HOME` with `exec_command` unchanged.
4. **Tests.** Unit tests for the choice and the command lines; the real-Podman e2e tests as the acceptance criteria
   describe.
5. **Spec and docs.** SPEC.md, SPEC_impl.md and the docs page. This is an incidental docs edit in an unattended run:
   start no preview server and name the changed page in the report.
6. **Bookkeeping.** Edit the `containerized-sessions.md` changelog fragment; remove the TODO.md entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy for the changed crates with
`--all-targets -- -D warnings`, the supervisor crate's tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`, the containerized
sessions e2e tests on real rootless Podman. Run `python -B scripts/check-test-sleeps.py` when Rust tests change.

## Fallbacks

Departures that need the scope reassessment: a per-launch or per-template setting for the account; renaming or moving an
image's existing user; supporting non-Ubuntu account tools; mounting any host directory beyond the checkout (the host
home included); changing the rule order; persisting the chosen account for per-exec use.

Decisions to log: how the image's facts are read, how each boundary above is handled, how Podman's own passwd injection
is handled, how `HOME` reaches execs, how the e2e tests supply the host identity, and every review finding you decided
not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-container-host-account-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/container-host-account/<nn>-<short-name>`
  (the expected single PR: `plan/container-host-account/01-host-account`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes and what does not,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

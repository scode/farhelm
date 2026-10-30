# Containers: launch from a local Podman image instead of an archive file

Written against main at 462b0268 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Speed up Podman session launches.** Starting each new container from an OCI archive makes Podman unpack the image
>   even when its layers are already in local storage. Investigate selecting an existing local image, using an OCI
>   layout directory or reusing an imported image, including how changes to the supplied image, concurrent launches and
>   cleanup work. The [measurements and investigation](https://snippets.scode.org/s/farhelm-podman-startup-performance/)
>   compare archive, layout-directory and local-image starts; measure full Farhelm launches too.

The linked measurements (rootless Podman 4.9.3, a 2.4 GiB archive whose layers were already stored): about 12 s per
container from `oci-archive:`, about 1 s from an OCI layout directory, about 0.5 s from an image already in Podman's
local storage. The maintainer chose the local image.

### What changes

A containerized launch names a **local Podman image** instead of an OCI archive file. The launcher's **Image file**
field becomes **Image**, holding an image name or ID as Podman's local storage knows it (for example
`localhost/agent:latest`). The user loads or builds that image into Podman on the session's host themselves; Farhelm
never imports, pulls, tags or removes images.

- The name is resolved when each container is created, so after the user rebuilds the image under the same name, the
  next new container (New, Replace, Replace with) uses it. An existing container keeps the image it was created from.
- Farhelm never contacts a registry: creation passes Podman's never-pull policy, and a name that is not in the host's
  local storage is refused before launch with a message naming the image and saying it is not in Podman's local storage
  on this host (the supervisor's existing refusals say "on this host"; do not thread a host name into it).
- The archive path is gone, with no compatibility handling beyond this: a stored template, recent setup or session
  launch that holds the old archive-path setting reads as containerized with an empty image name. Such a launch is
  refused for its missing image until the user fills one in, exactly like any other empty required field; a template can
  be edited to add one, and an old session can be replaced through Replace with, which the refusal may mention.

### What does not change

Everything else about containerized sessions: the Containerized switch and its other fields (API proxy port, GitHub
token file), keep-id mapping, the checkout bind, networking, the creation fence, Stop, Delete, tabs, and that Delete
leaves images alone.

### Acceptance criteria

- The launcher's Agent tab shows **Image** (a required local Podman image name, with help text saying it must already be
  in Podman's storage on the session's host) where **Image file** was; templates, Save as template, recent setups, Clone
  and Replace with carry it as they carried the path.
- Container creation runs Podman against that image name with pulling disabled; no `oci-archive:` source remains in the
  code.
- The supervisor's preflight checks the image exists in local storage on the host (replacing the regular-file check),
  and refuses with a message naming the image when it does not; an empty image name is refused there and in the launcher
  form as missing.
- Old stored data (supervisor session rows, the helm's templates and recent setups, and creation tickets) that carries
  the archive-path setting loads without error, as a containerized choice with an empty image name; an old template
  still applies. Nothing else migrates or converts it. Tests cover a stored session launch and an old template.
- The protocol version moves from 66 to 67.
- Unit tests (with the fake Podman) and the real-Podman end-to-end tests in
  `crates/farhelm/tests/e2e/containerized_sessions.rs` use a local image instead of a saved archive; the e2e tests still
  cover isolation, Stop/Delete, tabs and the failed-retry case, plus a missing-image refusal.
- SPEC.md (the launcher's Containerized bullet, "Containerized sessions") and SPEC_impl.md ("Rootless Podman sessions")
  describe the local image; the containerized-sessions docs page
  (`website/src/content/docs/docs/agents/containerized-sessions.md`) tells users to `podman load` or build the image on
  the host and name it, and drops the archive instructions.
- Containerized sessions have shipped only in a release candidate, not a stable release, so per root `AGENTS.md`,
  "Changelog fragments", the PR edits the existing unreleased `releasing/changelog.d/containerized-sessions.md` fragment
  (keeping `kind: added`) to describe a local Podman image instead of an archive, rather than adding a new entry; a body
  paragraph tells the curator that release-candidate archive settings now read as empty. The PR title is `feat:`,
  without `!`.
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29 on the planning page:

- **A local-image source**, chosen over accepting an unpacked OCI layout directory and over importing the archive once
  and reusing it through a Farhelm-managed cache. This needs SPEC_impl.md's "No image manager or registry step is added"
  reworded, not dropped: there is still no image manager; the user's own Podman storage is the source.
- **Replace the archive path entirely**, rather than offering both. In the maintainer's words: "No complexity for
  compatibility with old templates/sessions. Any template/session with a path just gets read as template with an empty
  image name. No other complexity added."
- **No launch-time measuring.** In the maintainer's words: "not worth it, we know what was slow here. dont waste token
  measuring." Do not add a benchmark or spend runs timing launches; the e2e tests' ordinary runs are enough.
- **Defaults stated on the page and not objected to:** the name is resolved at each container creation; Farhelm never
  pulls from a registry and refuses a name not in local storage; Farhelm never removes images.
- **Out of scope:** bootstrapping an image on a remote host automatically. The maintainer added that as a separate Near
  term TODO entry in the planning PR.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's launcher bullet on containerized launches, "Containerized sessions", and
"Upgrade compatibility and client scale" (read it before changing the stored or wire shape: old stored data must still
load, and how protocol versions move is decided there); SPEC_impl.md, "Rootless Podman sessions" (strict wire surfaces,
absent settings omitted so fingerprints do not change, the preflight list, the creation fence); `ContainerConfig` in
`crates/farhelm-proto/src/launch.rs` (`deny_unknown_fields`); root `AGENTS.md`, "Code rules", "Tests and validation",
"Conventional Commits", "Changelog fragments"; `website/AGENTS.md`; `.agents/test-authoring.md`; `.agents/validation.md`
(how the real-Podman e2e tests run and what they need).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code:

- Rename the field rather than reinterpret it. Handle old data once, in `ContainerConfig`'s deserialization (for example
  `#[serde(from = ...)]` over a private strict wire struct holding a defaulted `image` and an ignored `image_file`), so
  every stored surface is covered and the public struct carries no legacy field.
- `validate()` stays a shape check that accepts an empty image. It runs when the supervisor loads stored sessions (one
  bad row fails the whole load), when a template is applied, and when one is saved, so refusing an empty image there
  would stop an upgraded supervisor from loading its sessions and make old templates unappliable. "Required" is checked
  only where a new container is created: the supervisor's preflight and the launcher form's launch gate.
- The shape check refuses whitespace, control characters and a leading `-`; the last closes option injection for both
  Podman commands that take the image as a positional argument.
- The existence check is `podman image exists`, run with the same scrubbed environment as other Podman control commands,
  asynchronously and bounded the way `ContainerRuntime::run` bounds its commands (Podman can block on its storage lock
  while the user is loading a large image). Exit 1 means absent; any other failure also refuses, with Podman's stderr.
  Confirm that transport-prefixed names (`docker://...`, `oci-archive:...`) are refused rather than fetched. The unit
  test passes the Podman path in rather than changing `PATH`, which root `AGENTS.md` forbids in tests.
- `podman run` gets `--pull=never`.
- One PR, or two if the e2e test rework reviews better on its own.

## Outline

1. **Settings shape and protocol.** In `ContainerConfig` (`crates/farhelm-proto/src/launch.rs`), replace the archive
   path with the image name, keeping the struct strict otherwise, with the old-data read and the `validate()` shape
   rules above. Bump `PROTOCOL_VERSION` from 66 to 67 per its doc comment (a launch field added and one removed), rename
   the pinned-version test, and update SPEC_impl.md's "protocol 66" sentence. Container launch fingerprints change
   shape; that only affects a keyed create retried across the upgrade and needs no handling.
2. **UI.** `crates/farhelm-ui/src/container_controls.rs` and the forms that carry the field (`create_form.rs`,
   `templates.rs`, `save_template.rs`, and whatever else the compiler finds): label, help text, the required-image gate.
3. **Supervisor.** `crate::container::preflight` checks the image exists locally instead of checking a file;
   `create_command` in `crates/farhelm-supervisor/src/container.rs` names the image with `--pull=never`. The creation
   fence, ticket and generation stay as they are.
4. **Tests.** Update the fake-Podman unit tests (the keeper test asserts the image argument), add a missing-image
   preflight test, a supervisor store test that a stored launch with the old field loads, and a launcher test that an
   old template applies with an empty image. Switch the real-Podman e2e tests from `podman save` to the locally present
   test image, adding a missing-image case. Update the browser specs that use the "Image file" label or the `image_file`
   key (`e2e/tests/templates.spec.ts`, `e2e/tests/github-checkout-composer.spec.ts`).
5. **Spec and docs.** SPEC.md, SPEC_impl.md and the containerized-sessions docs page, including the sentences about
   image-copy progress, an image import holding the creation fence and delaying cleanup (and the matching "container
   image preparation is still running" message in `ContainerRuntime::acquire`, if nothing slow remains under the fence),
   and imported images left in storage. This is an incidental docs edit in an unattended run: start no preview server
   and name the changed page in the report. Leave `plans/queue/podman-sandbox.md` and `lore/` alone though they mention
   `image_file`; they are frozen.
6. **Bookkeeping.** Edit the `containerized-sessions.md` changelog fragment as above; remove the TODO.md entry in the
   last PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy for the changed crates with
`--all-targets -- -D warnings`, the proto, supervisor and UI crates' tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`, the containerized
sessions e2e tests on real rootless Podman and the browser specs that cover the containerized launcher fields. Run
`python -B scripts/check-test-sleeps.py` when Rust or browser tests change.

## Fallbacks

Departures that need the scope reassessment: keeping the archive path alongside the image; any import, cache, tag or
cleanup of images by Farhelm; pulling from a registry; migrating old archive paths to image names; pinning the resolved
image ID in the stored launch; adding launch-time measurement.

Decisions to log: the stored and wire shape of the new field and how old data is read, where and how the existence check
runs, how transport-prefixed names behave, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-podman-local-image-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/podman-local-image/<nn>-<short-name>`
  (for example `plan/podman-local-image/01-local-image`).
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

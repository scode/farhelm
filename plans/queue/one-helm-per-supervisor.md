# One helm owns each host

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Enforce that one helm owns each supervisor.** SPEC.md (Topology) says the helm that registers a host owns its
> supervisor and every session on it, and that neither moves between helms, but nothing enforces that: a supervisor
> treats every full-authority connection alike, so a second helm can register a host another helm owns and drive its
> sessions. Enforcing it needs each host to store the identity of its owning helm, refuse connections from any other
> helm, and make taking a host over an explicit operation the user asks for. That takeover has to cover what SPEC.md
> already supports implicitly, adding a host again from a brand-new helm whose registry was lost, so that losing the
> helm still never strands a host; and the stored identity has to stay the same across everything that does not change
> the owner (restarts, updates, reconnects, and the desktop app and a standalone helm taking turns on one state
> directory). Asked for on 2026-11-08, when agent commands from sessions that were not on screen turned out to fail
> because the supervisor's routing was built on the assumption that several helms might share one supervisor.

### Acceptance criteria

1. **The helm's owner id.** Each helm identifies itself to supervisors by an owner id derived one-way from the random id
   its database already keeps (`registry_namespace`, which is deliberately never sent to supervisors), with a domain
   separation of its own, the way the 4-character helm tag is derived. It stays the same across helm restarts, updates
   and reconnects, and changes only when the helm's database is lost. Every hello a helm sends with full authority (its
   host connection and its provisioning probe) carries the owner id and the helm tag, in one optional helm field that
   replaces today's `owning_helm` routing flag (carrying the id, the tag, whether it asks to take over, and whether it
   answers agent requests), so there is one helm signal in the hello rather than two.
2. **The supervisor stores its owner.** A supervisor schema step adds the owning helm's id, its tag and the last time
   the owner connected. A supervisor with no stored owner (a fresh install, or any host set up before this change, after
   its update) records the first helm whose host connection connects as its owner, silently. A probe hello is checked
   against the stored owner but never claims the host (planner choice; log it if the code argues otherwise).
3. **Admission.** A hello from the stored owner is admitted. A hello from a different helm is refused at the hello, and
   the refused helm learns the owner's tag and whether the owner is connected now (or when it last connected). Either
   the refusal carries those (a new message shape, since error kinds carry no data today) or the supervisor's hello
   reports its stored owner the way it already reports its host identity and schema version, with the refusal a plain
   new kind; your call, logged. Never reuse `ErrorKind::TakenOver`, which means a terminal attach was displaced. A
   full-authority hello that carries no helm field (the supervisor's own startup check, test clients, other tools) is
   admitted as today but never counts as a helm: it cannot claim the host, and agent requests are never routed to it,
   which holds by construction once the routing flag lives in the helm field.
4. **Take over.** A host-connection hello that asks to take over replaces the stored owner and ends the old owner's
   connections through the supervisor's existing helm links and their shutdown (`HelmLink::retire`), with no new
   per-connection registry. The old owner then sees its own refusal when it reconnects. A probe never asks to take over.
5. **Protocol.** This is a helm-supervisor protocol change with a version bump. Older hosts show version skew and get
   Update as with any bump; the upgrade floor does not change.
6. **The helm's side.** A host whose supervisor refuses the helm because another helm owns it is a frozen Hosts row,
   modelled on the identity-mismatch state and its Adopt, reading close to "Managed by another Farhelm (cd34), connected
   now", with Take over, Remove and Retry. Take over borrows Adopt's route, frozen-row pattern and immediate retry, but
   unlike Adopt it writes nothing durable in the helm: it arms an in-memory one-shot flag that the next host-connection
   hello to reach the supervisor consumes (a dial that fails before reaching it leaves the flag armed). Setup, Update
   and uninstall on such a host are refused until it is taken over, by adding the new state to the per-state refusals
   that already exist (`require_update_trusted`, the uninstall refusal for an unusable host, `attach_refusal`), not by a
   new refusal layer.
7. **Add host.** When the probe finds a supervisor already owned by another helm, Add host asks before registering it,
   close to: "devbox is managed by another Farhelm (helm ab12). That Farhelm connected 3 days ago. Taking over makes
   this Farhelm the host's manager: the other one loses access to the host and its sessions until it takes it back."
   with Cancel and Take over. Take over there registers the host as a discovered host is registered today and arms the
   same one-shot flag as the row's Take over, so there is one takeover path. A supervisor with no owner, or owned by
   this helm, registers as today.
8. **No exemptions.** The desktop app's own local host follows the same rule: a second helm that registers it must take
   it over, and the Mac app's own row then shows the frozen state. After the app's own data is lost, its local host
   shows the same Take over choice as any other host.
9. **Agents never take over.** An agent's request to add a host never takes over one another helm owns: the agent path
   maps the probe's new "owned by another helm" reply to a refusal with the owner's tag.
10. **SPEC.md** "Topology": the NOTE that ownership is not enforced is replaced by the rule; the recovery sentence
    (provisioning doubles as recovery, so losing the helm never strands a host) says that re-adding a host after the
    helm's data was lost now asks to take it over; the sentences that allow another helm to register the Mac app's
    supervisor change to match criterion 8. **SPEC_impl.md**'s hello and ownership passages (around its lines 2844 to
    2861 at planning time) describe the owner id, admission and takeover.
11. **Tests:** the hello wire format; claim on first contact; refusal of a second helm with the owner's tag; takeover
    closes the old owner's connection; the owner id is unchanged across a helm restart over the same database; an
    upgraded host with no owner is claimed; the supervisor schema step; an id-less hello is admitted and never routed
    to; the helm's frozen state, Take over, and the refused Setup, Update and uninstall; the Add host question; GPUI
    screenshot goldens for the frozen row and the Add host question. The real-Mac lost-registry test
    (`provisioning/tests/macos_real_tests.rs`) is updated to confirm the takeover. No new Linux self-SSH test: the
    remote path is a byte proxy, so in-process helm provisioning tests against a real supervisor cover the same
    admission, unless `.agents/validation.md` demands SSH coverage for a protocol bump.
12. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
    entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed mockups of the Add host question and the
frozen Hosts row:

- **A second helm has to take over the Mac app's own host,** and the Mac app's row then shows "managed by another
  Farhelm"; v1 does not support several helms. Rejected: exempting the local host, and leaving that case unenforced.
- **Hosts with no recorded owner after the update are claimed silently by the first helm to connect.** Rejected: asking
  once per host.
- **The local host after lost app data shows the same Take over choice.** Rejected: taking it over automatically.
- **Setup, Update and uninstall are refused on a host another helm owns until it is taken over.** Rejected: allowing
  them.
- **Owner id derived from the helm's existing database id** (planner choice, stated on the page). Rejected: a new id
  table, and sending the raw id.
- **Ownership guards against mistakes, not attackers** (stated on the page): anyone on the same Unix account can reach
  the supervisor anyway (SPEC.md's threat model), so the owner id is not a secret and this is not a security boundary.
- **Release note:** the maintainer's own wording (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the Decisions above.

**Binding repository constraints:** SPEC.md "Topology" (ownership, the recovery rule), its threat model, and its
compatibility rules; SPEC_impl.md's hello, identity-mismatch and Adopt passages; the protocol rules at the top of
`crates/farhelm-proto/src/lib.rs` and `crates/farhelm-proto/src/version.rs`; the supervisor's schema ladder and its
historical-schema fixtures; root `AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and
validation"; `.agents/test-authoring.md`. The real-Mac test needs the Mac VM that `.agents/validation.md` describes.

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- Today the only ownership signal is `owning_helm: bool` in the hello (`crates/farhelm-proto/src/lib.rs`, added in
  protocol 73 by #3261), set by the helm's host connection (`crates/farhelm-helm/src/client.rs`); the supervisor
  registers an agent-request link only for full-authority hellos that declare it and routes to the newest
  (`crates/farhelm-supervisor/src/service/connection.rs`, `agent_relay.rs`). Any hello without `auth` on the supervisor
  socket gets full authority. Full-authority hellos come from the helm's host connection (`start_for_host`), the
  provisioning probe's plain handshake (`provisioning/backend.rs`), the supervisor's own startup check, and test
  clients.
- `registry_namespace` (`crates/farhelm-helm/src/store.rs`) is a singleton of 16 random bytes created with the helm's
  database; the helm tag is derived from it (`tensorlake/config.rs`). Derive the owner id the same way with its own
  domain string.
- The supervisor's host identity (`supervisor_meta.host_identity`) and the helm's identity-mismatch freeze and Adopt
  (`manager.rs`, the `/api/hosts/{id}/adopt` route, `ui_channel.rs`'s host states) are the pattern for the new frozen
  state and the take-over route. Adopt itself is durable (a compare-and-swap in the store, a cache purge, then
  `retry_now`); Take over reuses only the route shape, the frozen row and `retry_now` (criterion 6).
- The supervisor's `helm_links` (`service/core.rs`) and `HelmLink::retire` (`agent_relay.rs`) already end a helm
  connection from outside; record the owner id on each link and retire the mismatched ones on takeover.
- The probe's `ProbeReply::Discovered` registers an existing supervisor at once today
  (`crates/farhelm-proto/src/provisioning.rs`); a new reply variant carries "owned by another helm" so Add host can ask.
- The standalone helm was removed on 2026-11-13, so the TODO's "desktop app and a standalone helm taking turns on one
  state directory" case is moot; one helm per state directory is already enforced by the app lock.
- The pending plan `triage-misc-nov18` edits `connection.rs` near the hello handling; whichever lands second rebases
  across it.

## Outline

1. `plan/one-helm-per-supervisor/01-owner-in-hello`, `feat!:` or `feat:` (a host on an older build refuses to connect
   until updated, as with any protocol bump; pick the type per root `AGENTS.md`, "Conventional Commits", and log it):
   the owner id and tag in the hello, the supervisor schema step, admission, takeover, the protocol bump, SPEC.md and
   SPEC_impl.md, and the proto, supervisor and helm tests.
2. `plan/one-helm-per-supervisor/02-helm-owned-elsewhere`, `feat:`: the helm's frozen state, the take-over route, the
   probe's new reply, the refused Setup, Update and uninstall, agents never taking over, their tests, and the real-Mac
   lost-registry test's update (PR 1 makes a fresh helm's probe refused, so that test's `Discovered` assertion must
   change in the PR where the new reply and Take over exist; if PR 1 alone already breaks it, move the update there).
3. `plan/one-helm-per-supervisor/03-take-over-ui`, `feat:`: the Hosts row, the Add host question, their tests and
   goldens, and the TODO.md entry removal.

Only one fragment describes the feature (in the PR whose type calls for it first); later PRs of the same unreleased
feature edit it rather than adding another (root `AGENTS.md`, "Changelog fragments").

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the touched crates, the proto, supervisor (schema ladder and fixtures included), helm and GUI unit tests, the end-to-end
host-connection and agent-relay tests, the GPUI goldens, the real-Mac provisioning test on the Mac VM, and whatever
`.agents/validation.md` names for a protocol bump, all through `python3 scripts/record-test-run.py`, plus
`python3 releasing/check-changelog.py format`.

## Fallbacks

- If the Mac VM is unavailable for the real-Mac test, finish the rest, keep that PR's test change, and report the test
  as not run, with why.
- If some full-authority client other than those listed must also identify as a helm (or must not), decide by criterion
  3's rule and log it.
- If taking over needs the old owner to be told something beyond the refusal it sees on reconnecting (a live
  notification to a connected owner, say), that is a departure: run the scope reassessment.

Decisions to log: the owner id's derivation, the hello's new fields, the schema step, the refusal's error kind and its
fields, the new protocol version, the frozen state's name and wording, the take-over authorization, every test changed
and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-one-helm-per-supervisor-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-one-helm-per-supervisor-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/one-helm-per-supervisor/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Farhelm now won't automatically "take over" a remote host which you have already added to some other Farhelm
installation. Concretely if you for example use "Add host" and Farhelm discovers a pre-existing Farhelm supervisor
running on that host, owned by another Farhelm, it will prompt you for whether you intend on taking it over. If you do,
the other Farhelm is kicked out - there is only ever one owning Farhelm.

# Hosts: set the alias while adding a host, and refuse adding a host twice

Written against main at 71dd7eba on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

It was planned together with `agent-add-host`, which runs after this plan has landed and reuses the alias handling built
here: an agent's "add host X with alias Y" prefills the add host dialog this plan extends.

## The goal

Hosts already carry an optional alias (SPEC.md, the host passages around "A host can carry an optional alias"), shown in
place of the destination everywhere except the host's details. Today it can only be set after the host exists, from the
host settings dialog (`crates/farhelm-ui/src/hosts/settings_dialog.rs`, `POST /api/hosts/{id}/alias`). The add host
dialog (`AddHostForm`, `crates/farhelm-ui/src/hosts.rs`) asks only for the ssh destination.

Deliver two things:

1. **An alias field in the add host dialog.** The user can type an optional alias next to the destination, and the host
   is added with it. The alias travels with the add through the helm's check-and-add requests (`POST /api/hosts/probe`,
   then the pending setup confirmation and `POST /api/hosts/provision` when Farhelm must be installed;
   `crates/farhelm-helm/src/provisioning/`), and is written in the same store transaction that registers the host, so an
   alias that turns out to be taken never leaves a host added without it. A taken alias is refused before the helm
   contacts the destination over ssh (a cheap read), and again inside the registration write, which is the only
   race-free place.
2. **Adding a host that is already added is an error.** Today the probe path silently reuses an existing host entry
   whose destination equals the typed one (the "converge" branch of `register_probed_ssh_host` in
   `crates/farhelm-helm/src/store.rs`). After this plan, an add from the add host dialog whose destination is already a
   host entry is refused before anything contacts ssh, with a message that names the existing entry by its display name
   ("already added as <name>"). The other callers of the same requests (a host row's "re-run setup" of a failed add,
   which also repairs stale or `~` paths; the second registration inside a confirmed add; update confirmation) keep
   today's behavior unchanged.

### Acceptance criteria

- The add host dialog shows, after the destination: a field labelled "alias (optional)" with help text saying it is
  shown instead of the destination everywhere except the host's details, and that it can be changed later in the host's
  settings. The destination's help text is reworded so it no longer says "alias": it says "user@host, or a Host name
  from your ssh config", keeping the rest of today's sentence.
- The alias follows the existing rules exactly (SPEC_impl.md's alias paragraph): trimmed, empty means none, no control
  characters, at most 64 characters, unique among host display names. The dialog mirrors the shape rules before sending,
  with the settings dialog's wording; the helm owns the collision check.
- An alias that collides is refused before any ssh contact, and the refusal shows in the dialog's error line like other
  add errors. A collision that appears between the probe and the registration is refused by the registration write, and
  nothing is registered.
- A setup confirmation that is pending keeps the alias; confirming registers the host with it.
- An add from the dialog whose exact destination string is already an ssh host entry is refused before any ssh contact,
  naming the entry by its display name (its alias when it has one), and nothing changes. A duplicate that appears
  between the probe and the confirmation is refused at confirmation or in the write. What counts as "already added" is
  the destination exactly as typed, plus the existing same-machine identity checks that already refuse a second entry
  reaching a machine another entry holds. No normalization of spellings.
- The row re-run, a confirmed add's own second registration and update confirmation behave exactly as before, and their
  existing tests (in `crates/farhelm-helm/src/provisioning` and `crates/farhelm-helm/src/store.rs`, and the row re-run
  tests in `e2e/tests/provisioning.spec.ts`) pass unchanged, except tests that pinned the duplicate add from the dialog,
  which change to pin the refusal.
- `--ensure-hosts` and `POST /api/hosts` behave as before (the first is additive on its own path; the second already
  refuses a duplicate destination).
- SPEC.md says, under adding hosts, that adding a destination that is already a host entry is refused, naming that
  entry, and that an existing entry is set up again from its own row; and that the add dialog takes an optional alias.
  SPEC_impl.md describes the new-entry field and where the refusals happen.
- The docs page that explains adding a host mentions the alias field and the refusal, if it describes the dialog.
- A `feat:` changelog fragment for the alias field, with a paragraph for the duplicate refusal (a behavior change a user
  can notice), per root `AGENTS.md`, "Changelog fragments".
- The last code PR removes the TODO.md entry "Allow aliasing hosts in add host flow" (Near term).

## Decisions already made

Settled with the maintainer on 2026-10-22, on the planning page:

- **Separate plan; `agent-add-host` runs after it.** The agent plan depends on this one landing. Do not build any agent
  command here.
- **Field wording.** "alias (optional)", the same word as the settings dialog, with the destination's help reworded so
  the two fields do not both say "alias" (the destination help today says "a Host alias from your ssh config").
- **A duplicate add is an error, whatever alias was typed.** The maintainer's words: "this surfaces ridiculous behavior.
  if someone tries to add a host that is already added they should get an error. this is almost certainly a mistake. fix
  spec + code on this."
- **"Already added" means the exact destination string**, plus today's same-machine identity checks. The known gap stays
  as it is: two spellings of one machine where Farhelm is not installed yet both get entries, and the second shows as a
  duplicate once setup has run (SPEC.md, the passage beginning "An entry that reaches a machine another entry already
  holds").
- **Reviewer:** gpt-6.1-sol at high effort (see Review gate). If it cannot be reached when a review is due (CLI missing,
  quota, outage) and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
  "Executing one plan" (step 10), after pushing what is consistent. No substitute reviewer.
- No-workhorse mode.

## Requirement sources

**The user's request:** the TODO.md entry "Allow aliasing hosts in add host flow", the decisions above, and the
maintainer's wish (stated for `agent-add-host`) to be able to say "$farhelm add host foobar.bla.com with alias baz".

**Binding repository constraints:** SPEC.md's host passages (adding hosts is discovery-first; aliases; one entry per
machine) and "Upgrade compatibility and client scale"; SPEC_impl.md's alias rules and host registration passages; root
`AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md` for tests; `website/AGENTS.md` before editing any docs page.

**Planner proposals** (from a read of main at 71dd7eba, revised after a planning review; change them if the code says
otherwise, logged as a DECISION):

- The add dialog, and only the add dialog, sends one optional field on `POST /api/hosts/probe` that marks the request as
  a new entry and carries the alias (for example `new_entry: { alias }`; `ProbeRequest` already takes optional
  `#[serde(default)]` fields). Without the field, the probe behaves exactly as today, so the row re-run, path repair,
  update confirmation, their tests, and a browser tab left open across a helm update need no change. An old tab's add
  dialog keeps today's converge until it reloads; that is acceptable.
- The marker rides `ProbeRegistration::Ssh` into the pending confirmation (`PendingConfirmation::Add` already holds a
  `ProbeRegistration`, so the alias is kept for free) and into `register_probed_ssh_host` as one optional argument. With
  the marker set, the converge branch returns a duplicate refusal instead of converging, and the insert branch writes
  the alias and checks it against display names in the same transaction. Do not split the store write or add a
  converge-by-id write.
- The refusals before ssh need no new lookup: `probe_owned` already computes the stored row for the destination before
  probing, and `claim_register_and_start` already looks up the existing row under the `add_confirmations` mutex. Refuse
  there when the marker is set. A confirmed add's second registration already uses a row-derived registration
  (`registration_for_row`), which carries no marker, so it converges onto the row it just inserted as today.
- `HostStoreError::DuplicateDestination` carries the destination, not the display name; carry the existing entry's
  display name for the message (looked up in the same read or transaction).

## Outline

A suggested stack; reshape it if the code says otherwise:

1. `feat:` the helm side: the new-entry field with its alias, the duplicate refusal before ssh, at confirmation and in
   the write, the alias collision checks, SPEC.md and SPEC_impl.md, helm tests for each refusal and for the alias being
   kept through a setup confirmation.
2. `feat:` the add host dialog's alias field and reworded help, sending the new-entry field, with client-side shape
   checks; browser tests for adding with an alias, a taken alias, an invalid alias, and adding a duplicate destination;
   docs page; changelog fragment; TODO.md entry removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the helm and UI unit tests through
`python3 scripts/record-test-run.py`, and narrowly selected browser specs: adding hosts, provisioning (row re-runs and
setup confirmation), and the host settings alias editor.

## Fallbacks

Examples of departures that need the scope reassessment: normalizing destination spellings, changing `--ensure-hosts`,
adding an alias to `POST /api/hosts`, a new store table, changing how re-entries find their row, or any change to how a
host's identity is recorded.

Decisions to log: the new-entry field's shape, every existing test whose expectation changed and why, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-alias-in-add-host-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/alias-in-add-host/<nn>-<short-name>` (for
  example `plan/alias-in-add-host/01-refuse-duplicate-add`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit for `feat` and `fix`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch mechanics). If
gpt-6.1-sol cannot be reached, block as Decisions already made says; never substitute another model.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

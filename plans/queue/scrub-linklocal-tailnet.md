# Scrub link-local and tailnet IPv4 addresses in agent screen-capture privacy guards

Written against main at 10973c17e on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Scrub link-local and tailnet addresses in agent screen-capture privacy guards.** Neither the screen-capture script
> (`scripts/capture-agent-screens.py`) nor the Rust screen-fixture privacy check treats link-local addresses
> (`169.254.0.0/16`) or tailnet/CGNAT addresses (`100.64.0.0/10`) as private, so a captured screen showing one could
> reach a committed fixture. Teach both guards to scrub or refuse both ranges and add tests.

In product terms: Farhelm keeps captured terminal screens from real agents in this public repository to test status
detection. Both the Python capture scrubber and the Rust committed-fixture privacy test already catch `localhost`,
loopback, RFC 1918 IPv4, IPv6 link-local (`fe80::/10`), and IPv6 unique-local (`fc00::/7`, covering Tailscale IPv6), but
both miss IPv4 link-local (`169.254.0.0/16`) and IPv4 tailnet/CGNAT (`100.64.0.0/10`) URLs, including IPv4-mapped IPv6
forms. Teach both guards to scrub/refuse both ranges and add positive and boundary tests.

### Where it stands (from planning research and planning review at main 10973c17e; line numbers approximate)

- **Python capture scrubber (`scripts/capture-agent-screens.py`, about 94-220; tests in
  `scripts/test-capture-agent-screens.py`, about 24-89).** Replaces URL authorities matching `localhost`, `host`,
  `LOCAL_IPV4` (`0.0.0.0`, `127/8`, `10/8`, `192.168/16`, `172.16/12`), and IPv6 loopback/unspecified/link-local/ULA
  (and IPv4-mapped IPv6 via `LOCAL_IPV4`) with `http://proxy.invalid`, and refuses any surviving match in `check()`.
- **Rust screen-fixture privacy check (`crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs`, about 194-295,
  358-451).** `local_authority` checks `localhost`, `host`, IPv6 loopback/unspecified/link-local/ULA, IPv4-mapped IPv6
  (about 260-262 — note that `Ipv4Addr::is_private()` checks RFC 1918 only and returns `false` for both `169.254.0.0/16`
  and `100.64.0.0/10`), and plain IPv4 (about 276-282). Keep this test-only privacy guard separate from
  `farhelm-helm/src/browser.rs::local_ipv4` (different crate and different contract: SSRF/approval policy vs.
  public-repo fixture privacy).
- **Committed fixtures.** Checked all files under `crates/farhelm-supervisor/tests/fixtures/screens/`; zero currently
  contain link-local or tailnet addresses.

### Acceptance criteria

1. **URL-only scope (matching both guards today).** Do not scrub or refuse bare IP addresses outside `scheme://...`
   URLs.
2. **Python scrubber (`scripts/capture-agent-screens.py`):**
   - Classify `169.254.0.0/16` and `100.64.0.0/10` (`100.64.0.0`–`100.127.255.255`) as local/private for both plain IPv4
     URLs and IPv4-mapped IPv6 URLs (`[::ffff:169.254.x.x]`, `[::ffff:100.64.x.x]`), sharing one clean IPv4 helper using
     `ipaddress.IPv4Address` and **explicit `IPv4Network` checks** (`0.0.0.0/32`, `127.0.0.0/8`, `10.0.0.0/8`,
     `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`, `100.64.0.0/10`). Do **NOT** use Python's
     `IPv4Address.is_private`: in Python 3.11+ it excludes `100.64.0.0/10` and includes documentation ranges like
     `192.0.2.1` (TEST-NET-1), which would over-scrub public example URLs.
   - Update the comment above `LOCAL_IPV4` (about 95-102) so it matches the new ranges.
   - Both scrubbing (to `http://proxy.invalid`) and `check()` refusal cover both ranges.
3. **Rust fixture privacy check (`crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs`):**
   - Share one `is_local_ipv4(Ipv4Addr) -> bool` helper between plain IPv4 authorities (`host.parse::<Ipv4Addr>()`) and
     `address.to_ipv4_mapped()` covering `is_loopback()`, `is_unspecified()`, `is_private()`, `is_link_local()`
     (`169.254.0.0/16`), and `100.64.0.0/10` (`octets[0] == 100 && (64..=127).contains(&octets[1])`). (Confirmed no
     existing test uses 2- or 3-octet shorthand IPv4, so strict 4-octet `Ipv4Addr` aligns with Python.)
   - Update the docstring around lines 211-213 to list IPv4 link-local (`169.254.0.0/16`) and CGNAT/tailnet
     (`100.64.0.0/10`).
4. **Tests, without sleeps (`.agents/test-authoring.md`):**
   - In both `scripts/test-capture-agent-screens.py` and `crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs`:
     - Positive cases: `169.254.169.254`, `100.64.0.1`, `100.127.255.254`, `[::ffff:169.254.1.1]`,
       `[::ffff:100.100.100.100]` (and an explicit Tailscale IPv6 `fd7a:115c:a1e0::1` case if helpful).
     - Boundary negative cases (must NOT be scrubbed/refused as private): `100.63.255.255`, `100.128.0.0`,
       `169.253.0.1`, `169.255.0.1`.
5. **Bookkeeping:**
   - Remove the `Scrub link-local and tailnet addresses in agent screen-capture privacy guards` entry from `TODO.md`.
   - Dev/test hygiene only (`test:`), so no changelog fragment is needed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Scope:** URLs only (matching both existing guards), covering `169.254.0.0/16` and `100.64.0.0/10` in plain IPv4 and
  IPv4-mapped IPv6, with positive and boundary tests.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- `docs/agent-screen-fixtures.md` (do not edit committed screen fixtures under
  `crates/farhelm-supervisor/tests/fixtures/screens/`).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Tests and validation", "The live install is off-limits", and its
  "Instructions by path" for every path you touch.

**Planner proposals** (from research and the independent planning review):

- Keep the entire change in one small PR (`test: scrub link-local and tailnet IPs in screen-capture privacy guards`).

## Outline

1. `plan/scrub-linklocal-tailnet/01-scrub-linklocal-tailnet`: update `scripts/capture-agent-screens.py`,
   `scripts/test-capture-agent-screens.py`, and `crates/farhelm-supervisor/src/agent_kind/screen_fixtures.rs`, and
   remove the TODO.md entry. `test: scrub link-local and tailnet IPs in screen-capture privacy guards`.

## Fallbacks

- A reviewer finding asking to scrub bare IPv4 addresses outside URLs or to share a helper with
  `farhelm-helm/src/browser.rs` is declined by decision; log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-scrub-linklocal-tailnet-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-scrub-linklocal-tailnet-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run `python3 -B scripts/test-capture-agent-screens.py`, the `agent screen fixtures` nextest selection from
`.agents/validation.md`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Scrub link-local and tailnet addresses in agent
  screen-capture privacy guards"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you; never edit that section.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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

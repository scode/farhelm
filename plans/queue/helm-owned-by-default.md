# Helm: state-changing requests are helm-owned by default

Written against main at 18e51c3e on 2026-10-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `host-entry-session-ids.md`: the queue holds it back until that plan has landed on main. That plan
rewrites session identity across the helm's session routes and handlers (its PRs touch about 80 files), which are the
handlers this plan rewraps, so building on it avoids a large conflict. It also runs after
`transfer-leftover-cleanup.md`, which changes what a cancelled upload's and download's guards clean up when their
handler is dropped: that drop-time cleanup is exactly the cancellation behavior this plan's exemptions must preserve, so
the inventory has to read it after that change, not before. It is one of five structural refactor plans written together
(`supervisor-request-modes`, `helm-owned-by-default`, `helm-ingress-types`, `ui-peer-text-type`, `ui-session-actions`);
`helm-ingress-types` runs after this one.

## The goal

This is a structural refactor: it changes no behavior, except for the clear fixes described below, each in its own PR.
Its purpose is to make one recurring class of helm bug hard to reintroduce as agents keep adding routes.

SPEC_impl.md, "Who owns an accepted action", requires that once the helm accepts a state-changing request, the work runs
on a task the helm owns, not on the task serving the client's connection, because axum drops a handler's future when the
client disconnects, reloads or is cancelled, and a handler written as a sequence of steps is then left half done. Today
that is opt-in per handler: some handlers wrap their work in `run_owned` (`crates/farhelm-helm/src/lib.rs`, around
2273), some shared helpers spawn on their own, and some handlers are not owned at all. Seven fixes were a handler that
forgot (#1196, #1197, #1319, #1404, #1409, #1460, and #1419 on the agent relay); #1319's message says a rename "was
missed when those were moved". The next agent to add a step to an unowned handler, or to add a route, can reintroduce
it.

Make ownership the default at the router:

1. **A layer that owns every state-changing request.** On the authenticated router, every non-GET request's handler
   future runs on a helm-spawned task that the request future only awaits, so a client that goes away loses the reply,
   never part of the work. Panics keep the response they produce today (`run_owned` resumes a handler's panic on the
   awaiting task; keep whatever status and body the client sees now).
2. **An explicit exemption sub-router.** Routes that must stay cancellable live in a separately merged sub-router that
   does not get the layer, each with a comment stating why. A new route is then owned unless someone deliberately puts
   it in the exempt router.
3. **Remove what the layer makes redundant.** Per-handler `run_owned` wrapping and ad-hoc spawn-and-await helpers that
   the layer now covers are removed where removing them changes nothing; keep any whose scope differs from "the whole
   handler" (for example, a handler that deliberately owns only part of its work) and say why in a comment.

### Design principles

- Every route keeps its current cancellation behavior. A route that is cancellable today either goes in the exempt
  sub-router with its reason, or it is an accidental omission of exactly the class this plan targets, fixed first in its
  own `fix:` PR (see Decisions already made) so the layer PR itself changes nothing.
- Routes that are cancellable on purpose stay exempt. Known examples on main at 18e51c3e (verify them, and find the
  rest): attachment upload, whose upstream abort fires from the handler's destructor when the client resets the
  connection (the test `upload_attachment_cancelled_handler_still_aborts_upstream` in
  `crates/farhelm-helm/src/uploads.rs` proves it); downloads; read-only POSTs such as browsing, repository listings,
  checkout previews, stat and trash listings; and Stop and tab open/close, which #1404 deliberately left un-owned.
- The layer must compose with the existing auth middleware, body-size limits (for example the feedback route's own
  `.layer(..)`), tracing spans and error mapping without changing which status a rejected request gets or which span a
  log line lands in, as far as tests or operators can observe.
- WebSocket upgrades are GET and stay outside the layer. The helm's answers to agent requests relayed by a supervisor
  (`client.rs`, the agent relay) are out of scope: a router layer cannot reach them.

### You find the instances

The file and line references in this plan are illustrations from main at 18e51c3e, and other work, including
`host-entry-session-ids.md`, keeps landing before this plan runs. Do not treat them as a worklist. Your first step is an
inventory, on the main you start from: every route registered on the helm's routers with its method, and for every
non-GET route whether it is owned today (whole handler, part of it, or not at all), cancellable on purpose (with the
evidence: a test, a SPEC passage, a commit message such as #1404's), or unowned by accident. Accidental means a handler
that performs more than one state-changing step, or a state change followed by a notification or event bump, with no
recorded reason to be cancellable; the agent-trust and Claude-trust settings routes (`checkout_config.rs`, around 78 and
165: a store write followed by an inline event bump) look like examples. Record the inventory in the working log,
re-check it after every rebase, and summarize it in the report.

### Acceptance criteria

- Every non-GET route on the authenticated router is owned by the layer or listed in the exempt sub-router with a stated
  reason; no handler relies on its own `run_owned` call for whole-handler ownership anymore.
- A test proves the layer's guarantee: a non-GET handler whose client disconnects mid-request still completes its work.
  A test, existing or new, proves an exempt upload is still aborted upstream when its handler is dropped.
- Every accidental omission found by the inventory is fixed in its own `fix:` PR, landed in the stack before the layer,
  with a changelog fragment and a regression test where one is practical.
- Apart from those fixes: no behavior changes, confirmed by the no-functional-change reviewer for every PR.
- The last code PR removes this plan's TODO.md entry (under "Code cleanup").
- Docstrings explain the layer's guarantee, the exemption rule and the reason on each exemption, per the repository's
  documentation norms; SPEC_impl.md's "Who owns an accepted action" is updated to say where the rule is enforced.

## Decisions already made

The maintainer's words when asking for this plan (2026-10-19): "Make sure that all plans explicitly call for a special
reviewer beyond the usual that ONLY focused on making sure there is no functional difference (the only exception is
CLEAR bugfixes that fix broken behavior that precisely fixes the kind of bug the refactor was intended to prevent in the
first place). Please make sure the plan contains the key design principles, but that the executors still must find all
the exact specific _instances_ where to apply them, since other work is landing while this is being planned and queued
etc. For this work, I want an astra high adversarial reviewer (both for the general adversarial review as well as the
extra no-functional-change reviewer)."

- **No behavior change, with one exception.** A behavior change is allowed only when it is a clear fix of broken
  behavior of exactly the class this refactor targets: an accepted state-changing request that a client disconnect can
  leave half done. Such a fix goes in its own `fix:` PR, with a changelog fragment per root `AGENTS.md`, "Changelog
  fragments", and the no-functional-change reviewer must confirm it qualifies. Anything else you notice goes in the
  report's follow-ups.
- **Exemptions preserve today's behavior.** Uploads, downloads, read-only POSTs, and Stop and tab open/close (left
  un-owned on purpose by #1404) stay cancellable. Converting any of them to owned is not part of this plan.
- **The agent relay is out of scope.**
- **No route registry.** A test that walks every registered route was considered and dropped: axum cannot list its
  routes, so it would need a parallel registry. Default-on ownership with a separate exempt sub-router meets the goal.
- **Reviewers:** both per-PR reviews run on gpt-6-astra at high effort (see Review gate).
- **If gpt-6-astra is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, which `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above. The plan's origin is a 2026-10-19 whole-codebase structural assessment
whose adversarial review ranked this second of five: defaulting to the safe direction beats today's opt-in, provided the
exemptions are visible.

**Binding repository constraints:** SPEC_impl.md, "Who owns an accepted action" and "Errors crossing levels of
abstraction"; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `.agents/test-authoring.md` for tests; `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`.

**Planner proposals:** the layer-plus-exempt-sub-router shape above. A smaller design that meets the same acceptance
criteria is fine, logged as a DECISION.

## Outline

A suggested stack; reshape it if the inventory says otherwise:

1. One `fix:` PR per accidental omission (or one per closely related group of handlers), each wrapping the handler's
   work in the existing `run_owned`, with a fragment.
2. `refactor:` the ownership layer, the exempt sub-router with every exemption and its reason, and the test of the
   layer's guarantee.
3. `refactor:` removal of per-handler `run_owned` calls and ad-hoc spawns the layer makes redundant (may fold into 2 if
   small).
4. The SPEC_impl.md update and the TODO.md entry removal ride on the last code PR.

Facts from the planning session's reading of main (verify them): `run_owned` has about 32 call sites, mostly in
`sessions.rs` and `hosts.rs`; shared helpers spawn on their own at `templates.rs` around 178 and `preferences.rs` around
227; Stop (`sessions.rs` around 2548), tab open and close (around 3919 and 3940) and a tunnels route (`tunnels.rs`
around 477) are unowned; the authenticated router is built in `api_router` in `lib.rs`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the helm's HTTP contract tests, the upload cancellation tests and the tests
of every handler the inventory touches, through `python3 scripts/record-test-run.py` with a narrow selection, plus the
test-sleep check.

## Fallbacks

Examples of departures that need the scope reassessment: a route registry or macro that wraps route registration, owning
the agent relay, changing any exempt route to owned, or a per-handler opt-in attribute in place of the sub-router.

Decisions to log: the inventory's classification of each non-GET route and its evidence, every exemption and its reason,
every accidental omission fixed, how panics and middleware ordering were preserved, and every review finding you decided
not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-helm-owned-by-default-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/helm-owned-by-default/<nn>-<short-name>`
  (for example `plan/helm-owned-by-default/01-ownership-layer`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `refactor` for behavior-preserving restructuring, `test` for a change only to tests or their fixtures,
  `fix` (with a changelog fragment in the same commit) only for the clear-bugfix exception, `docs` for documentation
  only. A `fix` never shares a PR with refactoring.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch two reviews of its changes yourself and address what each finds before moving on. The
user demands exactly these reviewers, and no review swarm. Each is a single fresh-context agent on gpt-6-astra at high
effort, launched through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns
the launch mechanics). Run them independently; neither sees the other's findings. If gpt-6-astra cannot be reached,
block as Decisions already made says; never substitute another model.

**1. General adversarial review.** The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

**2. No-functional-change review.** The prompt carries this full charter:

> This PR is part of a structural refactor whose contract is that it changes no behavior. Your only question is whether
> this PR changes observable behavior compared with its parent commit. Ignore style, naming, design quality and test
> quality, except where a test change hides or accommodates a behavior change. Observable behavior includes: wire
> messages and their order; replies, error kinds, error messages and their precedence; HTTP statuses, headers and
> bodies; what is rendered, escaped, copied to the clipboard or put in URLs; which await, lock acquisition or check
> happens first; when a permit, guard or lock is released; whether work is cancelled when a client disconnects or a
> connection closes; database writes and their transaction boundaries; panics versus errors; logs that tests or
> operators rely on. For every difference, report the file, the quoted code before and after, a concrete scenario that
> behaves differently, and whether it is an unintended change to revert or a candidate for the exception. The exception
> applies only when all of these hold: the old behavior is clearly broken rather than a choice that SPEC.md,
> SPEC_impl.md or a recorded decision endorses; the new behavior fixes exactly the bug class this refactor exists to
> prevent, quoted below; and the PR is a `fix:` PR that contains only that fix and a changelog fragment. A behavior
> change inside a `refactor:` or `test:` PR is always a finding, even when it is a good fix. If you find no difference,
> say so and list what you checked. Report to the named file; edit nothing; touch no VCS state.

Each prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range of this one PR;
- a findings file in the scratch directory, one per reviewer;
- the acceptance criteria: this file's goal, design principles, Decisions already made and the outline; for the
  no-functional-change reviewer, quote the targeted bug class from Decisions already made.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the general
review's prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION; a no-functional-change finding you reject
needs the concrete reason the scenario cannot occur. Do not write a launch command here or in the log; the
harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular those named under Fallbacks above, every use of the clear-bugfix exception, and
every review finding you decided not to follow. The user will ask for these later.

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
passed both reviews of its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-10-20: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-20 (claim e37b0a).
>
> ### Decision needed: when a disconnected request becomes an action
>
> May the helm start carrying out a valid state-changing request after its caller disconnects, if the request had
> reached authenticated routing but had not finished parsing?
>
> This plan makes the helm finish accepted actions even when their caller goes away. Today each handler must arrange
> that itself; forgetting can save a setting without notifying other clients, or remove a tunnel without announcing its
> removal. The agreed design makes ownership automatic for ordinary authenticated requests, with explicit cancellation
> exceptions. The maintainer also required preserving behavior except for clear fixes where an accepted action could be
> left half done.
>
> ### What is already built
>
> Two separate, fully gated draft fixes are preserved:
> [trust-setting and hint notifications](https://github.com/scode/farhelm/pull/2443/changes), and
> [tunnel-close completion and notification](https://github.com/scode/farhelm/pull/2445/changes). Each passed its
> independent general and no-functional-change reviews on the required model, and focused validation. The automatic
> routing change and removal of handler wrappers are unimplemented. The plan's TODO entry remains because that
> structural work is unfinished. Neither draft has been marked ready or merged.
>
> ### What execution found
>
> The current handlers take ownership after parsing their request. The proposed routing layer takes ownership before
> parsing. In a minimal real HTTP/1 comparison using the repository's framework versions, a client sent a complete valid
> JSON body and closed its connection while parsing was pending. Across three sequential comparisons, the current
> placement started no action; the proposed placement finished parsing and performed the action after the connection
> ended.
>
> This used a stand-in mutation, not a Farhelm settings write. It establishes a possible difference at this boundary,
> not its frequency in browsers or behavior over HTTP/2. Recorded probe
> `cargo run --offline --manifest-path <probe>/Cargo.toml`, run `6689cb93-6c59-48e1-8997-f10b2500ca14`, completed with
> complete retained output; these are observations, not production test counts. The probe source and results are
> retained privately and described in the plan's working log.
>
> The old action never started, so this is outside the approved exception for an accepted action left half done.
> Ordinary framework middleware cannot take ownership after typed parsing. Keeping both outer and inner ownership does
> not preserve the earlier cancellation boundary.
>
> ### Options and recommendation
>
> - **Allow the earlier boundary for ordinary owned routes.** After authentication and entry into the ownership layer,
>   the helm may finish parsing and carry out a valid request even if the caller disconnects. This permits the small
>   default routing layer and explicit exceptions originally proposed. Existing authentication, rejection behavior,
>   transfer cancellation and narrower helper contracts still have to be preserved. I recommend this option because it
>   achieves the safe default with the smallest implementation, provided this behavior change is explicitly accepted and
>   documented.
> - **Preserve the current boundary after parsing.** Keep the two consistent fix drafts and revise the structural design
>   before continuing. Retaining per-handler ownership preserves that boundary but leaves the recurring omission risk; a
>   generic adapter after parsing would be a separate mechanism to assess, not an ordinary routing-layer change.
>
> This block asks only for that acceptance-boundary decision. It does not request changing uploads, downloads, read-only
> requests, Stop, tab operations, or the other intentionally cancellable routes.

The maintainer's answer:

"yes accept", meaning the recommended option, in this agreed restatement:

Accepted: for routes the new default owns, the helm may carry out a valid, authenticated request whose caller
disconnected before parsing finished. Document this boundary in SPEC_impl.md, "Who owns an accepted action". This
changes nothing else: exempt routes stay cancellable, and rejection behavior is unchanged.

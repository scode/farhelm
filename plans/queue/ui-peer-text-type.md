# UI: text from other machines is its own type

Written against main at 18e51c3e on 2026-10-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `host-entry-session-ids.md`: the queue holds it back until that plan has landed on main. That plan
changes how session ids are represented and rendered across the UI, including the approval-card rendering of full ids in
the same peer-text code this plan touches. It also runs after `terminal-link-and-copy-fixes.md` and
`template-editor-and-listing.md`, which fix the two review-queue items of this plan's bug class (unescaped terminal
notices, the template name editor) that the maintainer first asked this plan to fix; they were scheduled separately
afterwards, so this plan builds on their fixes instead of repeating them. It is one of five structural refactor plans
written together (`supervisor-request-modes`, `helm-owned-by-default`, `helm-ingress-types`, `ui-peer-text-type`,
`ui-session-actions`); `ui-session-actions` runs after this one.

## The goal

This is a structural refactor: it changes no behavior, except for the clear fixes described below, each in its own PR.
Its purpose is to make "text from another machine is escaped before a person sees it" something the compiler enforces
instead of something every call site must remember.

Text that a host or an agent controls (session titles, host names, folders, template names, error details) can carry
invisible or direction-changing characters that disguise what a user is approving or reading. The UI's rule, stated in
`crates/farhelm-ui/src/peer.rs`'s module docs, is that "everything a user sees has been through `display_peer`", which
makes such characters visible, with `PeerLine` and the detail-part types isolating each relayed run's direction. Nothing
enforces the first half: the values are plain `String`s, there are about 236 references to the helpers across 30 files
on main at 18e51c3e, and fixes keep arriving one site at a time (#1797, #2204, #2359 among others; `7705b7f6`,
`8aa9448f`, `60759c56`, `4037437f`, `4dc1a25c`, `5b88a9b0`, `5361d207`, `d824dafa`, and `467e71d9`, where shortening a
label cut an escape marker in half). Agents can set session titles and template names, so this is a trust boundary
agents write across.

Deliver a newtype for text that came from another machine, carried by the UI's own decoded copies of the helm's replies:

- **No `Display` and no implicit conversion to `String`.** It offers only explicit accessors: the escaped form for
  showing (what `display_peer` produces today), the other existing presentation forms (`editable_peer`,
  `display_identity`) where those apply, and an explicit raw accessor for values that must round-trip unchanged (URLs
  and route paths, request bodies, clipboard text, filter and search matching, test selectors, comparison keys such as
  the adopt request's reported identity). Every use site then fails to compile until someone chooses, which is the
  point.
- Truncating or shortening helpers take the newtype and never cut inside an escape marker.
- The direction-isolation layer (`PeerLine`, the detail-part types, `PeerBlock`) stays; it accepts the newtype where it
  takes relayed text today.

### Design principles

- Choose the accessor that reproduces each site's current behavior. A site that shows a value escaped today uses the
  escaped accessor; a site that uses the raw value today (whether correctly, for a URL, or wrongly, for display) uses
  the raw accessor. Do not fix display sites inside the refactor; that is what the separate fix PRs are for.
- Every display site that uses the raw accessor gets a short comment saying why raw is correct there, or, if it is
  wrong, is listed for the fix PRs described under Decisions already made.
- Scope is the Rust UI crate. Values the UI decodes directly into `farhelm-proto` types (approvals, session launches,
  launch templates, checkout trash listings, and any others you find) keep using the escaping helper at the access site:
  changing `farhelm-proto` would reach the helm, CLI and supervisor and collide with other plans. The newtype does not
  extend to the terminal's JavaScript or the CLI's printed output; the one JavaScript fix below is a `fix:` PR, not part
  of the type's scope.
- One definition of the unsafe characters. `farhelm-proto`'s `text::is_presentation_unsafe` is the set the UI, the CLI
  and the helm's logs share. A fix in the terminal's JavaScript must not hand-copy that set without a test that pins the
  copy to the Rust set; prefer having Rust code (the UI or the helm, whichever already handles that text) produce the
  escaped form, and log the choice as a DECISION.

### You find the instances

The file and line references in this plan are illustrations from main at 18e51c3e, and other work, including
`host-entry-session-ids.md`, keeps landing before this plan runs. Do not treat them as a worklist. Your first step is an
inventory, on the main you start from: every field of the UI's decoded helm-reply types that carries host- or
agent-controlled text, and every site that reads one, with what it does with the value today (shows it escaped, shows it
raw, sends or compares it raw). Record the inventory in the working log, re-check it after every rebase, and summarize
it in the report.

### Acceptance criteria

- Every host- or agent-controlled text field in the UI's own decoded reply types is the newtype; the newtype has no
  `Display` and no implicit conversion to a plain string.
- Every display of such a value goes through an explicit accessor; every raw display site carries a reason or is in the
  fix list.
- Apart from the clear fixes in their own PRs: no rendered text, URL, clipboard content, request body, filter result or
  test selector changes. The no-functional-change reviewer confirms this for every PR.
- No review-queue item of this class is left open: every one still open when you start is fixed and removed, each in its
  own `fix:` PR.
- The last code PR removes this plan's TODO.md entry (under "Code cleanup").
- `peer.rs`'s module docs describe the type as the enforcement of the rule, per the repository's documentation norms.

## Decisions already made

The maintainer's words when asking for this plan (2026-10-19): "Make sure that all plans explicitly call for a special
reviewer beyond the usual that ONLY focused on making sure there is no functional difference (the only exception is
CLEAR bugfixes that fix broken behavior that precisely fixes the kind of bug the refactor was intended to prevent in the
first place). Please make sure the plan contains the key design principles, but that the executors still must find all
the exact specific _instances_ where to apply them, since other work is landing while this is being planned and queued
etc. For this work, I want an astra high adversarial reviewer (both for the general adversarial review as well as the
extra no-functional-change reviewer)."

- **No behavior change, with one exception.** A behavior change is allowed only when it is a clear fix of broken
  behavior of exactly the class this refactor targets: host- or agent-controlled text shown to a person without the
  escaping SPEC requires. Such a fix goes in its own `fix:` PR after the refactor that exposed it, with a changelog
  fragment per root `AGENTS.md`, "Changelog fragments", and the no-functional-change reviewer must confirm it qualifies.
  Anything else you notice goes in the report's follow-ups.
- **Fix everything of this class the work exposes, including open review-queue items.** The maintainer chose this over
  leaving queue items for triage (2026-10-19): "But please tell me why you recommend leaving them. What am I missing
  here? Why wouldn't we just fix them?" The two queue items of this class open when this plan was written (the template
  name editor, `template-name-editor-conceals-formatting.md`, and the terminal notices,
  `terminal-peer-failures-hide-format-controls.md`) were then scheduled in `template-editor-and-listing.md` and
  `terminal-link-and-copy-fixes.md`, which this plan runs after; their fixes are on main when you start. Convert those
  sites to the new type preserving the fixed behavior. If either outcome was discarded rather than fixed (both have size
  gates) and its queue item is still open, fix it here: the template name editor follows SPEC.md's rule for such fields
  (search SPEC.md for "shown escaped and read-only"), and the terminal notices follow the one-definition principle
  above.

  Each fix is its own `fix:` PR with a changelog fragment and a test, removes its feedback file and `INDEX.md` line if
  it has one, and comes after the refactor PR that touches the same code. Any other clearly unescaped display of host-
  or agent-controlled text that you find, with or without a queue item, is fixed the same way.
- **Scope is the Rust UI crate** (see Design principles).
- **Reviewers:** both per-PR reviews run on gpt-6-astra at high effort (see Review gate).
- **If gpt-6-astra is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, which `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above. The plan's origin is a 2026-10-19 whole-codebase structural assessment
whose adversarial review placed this fourth of five: mechanical, on a boundary agents write across, with fixes still
arriving.

**Binding repository constraints:** SPEC.md's requirement that host-originated GUI text show invisible and
direction-changing characters visibly (search SPEC.md for "invisible formatting"); SPEC_impl.md's GUI passages on peer
text; root `AGENTS.md`, "Talking to the user", "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md` for tests.

**Planner proposals:** the newtype and accessor shape above. A smaller design that meets the same acceptance criteria is
fine, logged as a DECISION.

## Outline

A suggested stack; reshape it if the inventory says otherwise. The conversion is large and mechanical, so split it by
area so each PR stays reviewable:

1. `refactor:` the newtype with its accessors, docs and unit tests, and the truncation helper taking it.
2. `refactor:` one PR per area converting the decoded types' fields and their use sites (for example: sessions and the
   session list; hosts and provisioning; templates and the launcher; approvals and notices), each preserving current
   behavior.
3. `fix:` one PR per clear unescaped display site found (or one per closely related group), each with a fragment and a
   test, each removing its review-queue item if it has one.
4. The TODO.md entry removal rides on the last code PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the `farhelm-ui` unit tests through
`python3 scripts/record-test-run.py`, and the browser specs that cover peer-text escaping and the areas each PR
converts, narrowly selected.

## Fallbacks

Examples of departures that need the scope reassessment: an escaping `Display` or `Deref<Target = str>`, changing
`farhelm-proto` types, a type or wrapper layer in the terminal JavaScript beyond the one notice fix, a second
hand-maintained copy of the unsafe-character set, or a general sanitization framework.

Decisions to log: the accessor set, every raw display site and its reason, every clear fix taken, where the terminal
notices get their escaped text, how proto-typed values are handled, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-ui-peer-text-type-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/ui-peer-text-type/<nn>-<short-name>` (for
  example `plan/ui-peer-text-type/01-peer-text`).
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

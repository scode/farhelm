# Launcher: one Launch press for a fresh checkout on a suspended host

Written against main at 47b0d59a on 2026-11-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term", subsection "Tensorlake"), verbatim:

> - **One click for a fresh checkout on a suspended host.** Launching a fresh GitHub checkout on a suspended Tensorlake
>   host resumes the host, then stops and asks the user to review the refreshed checkout preview and press Launch again.
>   The preview made over the connection from before the suspension must not authorize work on the new connection, which
>   is why the second press exists; existing-folder and template launches resume and launch in one press. Find a way to
>   make the fresh-checkout case one press without letting a stale preview authorize the create (for example, launch
>   automatically when the refreshed preview matches what the user reviewed, and stop only when it differs).

The maintainer chose a simpler rule than the entry's example: "just say it will be auto-allocated. no control/review. we
just promise it's some folder." Only for fresh GitHub checkouts on a suspended host.

### What changes

- **Before Launch**, when the selected host is suspended and the destination is a fresh GitHub checkout, the composer
  does not show a folder path. A line says the folder is picked automatically when the host is ready. Launch is already
  enabled in this case (`fresh_destination_ready` in `crates/farhelm-ui/src/list/create_form.rs`), so admission does not
  change. This line belongs where the preview-state lines and the folder summary/placeholder are rendered.
- **One press:** Launch resumes the host and waits for the fresh connection. Once the form's preview on the new
  connection is Ready, the session is created from that preview in the same press. Today the suspended + Github branch
  of `submit_composer` sets "The host resumed. Review the refreshed checkout preview, then launch again" and returns. On
  a successful resume, arm the existing one-shot pending submit (`guarded_submit_pending` / `GuardedSubmitStamp`, "one
  accepted submission waiting only for its refreshed checkout preview") instead. That mechanism waits while the preview
  is Pending, calls `submit_composer` exactly once on Ready, reports the message on Failed, and is cancelled by any
  draft or opening change. Stamp it with the press's opening, replace guard, prompt and conversation, as the existing
  uses do.
- **One adjustment that mechanism needs:** its stamp compares the whole preview authority, so it would cancel itself
  during the window where the form's host reader still reports the pre-resume connection (the window
  `ResumeDisplayHandoff` exists for). Either stamp the authority without its connection field and treat "live connection
  is still the pre-resume one" as waiting, or stamp the fresh connection from the host list the resume returns and treat
  the old one as waiting.
- **No stale authorization:** the create is admitted only with a Ready preview whose incarnation matches the live
  connection claim, as today. No preview obtained outside the form's own authority signals; no separate preview fetch
  inside the resume task.
- **Resuming text:** `HostResumePreparation.review_checkout` and its "you'll review the checkout and launch again"
  phrasing go away; both paths say the session starts when the host is ready. Update the struct's docstring ("fresh
  checkouts still need a reviewed preview and a second press").
- **Templates too:** ⚡ and number-key templates with a fresh GitHub destination reach the same branch today and also
  need two presses on a suspended host; they become one press as a consequence. Quick launch keeps its existing route;
  do not add a new one.

### What does not change

- **Folder naming:** unnamed checkouts get the lowest free `repo-N`, and template base names and untouched Clone
  defaults advance through the existing `-2`..`-50` suffixing (`CheckoutNameSequence`) on the fresh connection, with no
  new code. **A typed or fixed session name whose folder is taken stays refused** as on any host (maintainer decision);
  it is not renumbered. For a typed name, the composer line says the folder is named after the session rather than
  "picked automatically".
- **A lost race** (another create takes the previewed folder between preview and create) keeps today's connected-host
  behavior: the session is not launched, a fresh preview appears, and the user presses Launch again. SPEC.md forbids an
  automatic second create ("it never submits automatically"), and the one-shot submit is spent before it is sent. Do not
  add a retry loop.
- Existing-folder launches, launches on hosts that are not suspended, and failure wording ("the session was not
  launched", without the uncertain-outcome promise).

### Acceptance criteria

- A fresh GitHub checkout on a suspended host, from the New session form and from a ⚡ or number-key template, takes one
  press: the host resumes, connects, and the session starts in a free folder.
- The composer shows no path for that case before Launch, and says the folder is picked automatically (or named after
  the typed session name).
- A preview from before the suspension never authorizes the create; a lost race and a taken typed name behave as on any
  connected host.
- SPEC.md: the managed-checkout rule "Before Launch becomes available, the composer shows the exact host and path" gains
  this second exception beside the ⚡/number-key one; the Tensorlake suspend/resume text ("For a fresh GitHub checkout,
  Resume refreshes the checkout preview; the user reviews it and launches again...") and the launcher's Resuming
  preparation text ("on the fresh-checkout path it says to review the checkout and launch again") describe the one-press
  behavior and keep the lost-race and typed-name rules. Check SPEC_impl.md for text describing the second press.
- Docs: check `website/src/content/docs/docs/using/tensorlake-sandboxes.md` and `start-a-session.mdx` for the two-press
  description (an incidental docs edit in an unattended run: start no preview server and name the changed page in the
  report).
- Tests: browser tests that assert the second press on a suspended host change to assert one press; add a template case
  and a typed-name-taken case. Unit tests for the stamp adjustment if it carries logic.
- A changelog fragment (`kind: changed`) and removal of this TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-05 on a planning page:

- **Auto-allocated folder, no review,** only for fresh GitHub checkouts on a suspended host.
- **A taken typed name stays refused** after the host wakes, as on any host (chosen over numbering both the folder and
  the session name).
- **No automatic retry** on a lost race (a planning-review finding the maintainer saw stated: SPEC.md forbids it).
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's managed-checkout section (exact path before Launch and its template
exception; naming rules; "If another create wins the displayed path, Launch reports the conflict and obtains a new
preview; it never submits automatically or silently chooses another directory"; "A typed name or a template's fixed name
remains a conflict rather than gaining a suffix") and its Tensorlake suspend/resume section; root `AGENTS.md`, "Code
rules", "Tests and validation", "Changelog fragments", "TODO.md and FLAKES.md"; `website/AGENTS.md`;
`.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: reuse the guarded-answer one-shot submit rather than quick launch's `requestSubmit`
route; which of the two stamp adjustments to use; the composer wording.

## Outline

One PR, since the behavior, its spec text and the tests that assert the press count belong together:

1. `feat:` one-press fresh checkout on a suspended host: the composer line, arming the one-shot submit after a
   successful resume with the stamp adjustment, removing `review_checkout`, SPEC.md (and SPEC_impl.md if needed), the
   docs check, test updates, the changelog fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the UI crate, the UI
crate's tests, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and, through
`python3 scripts/record-test-run.py`, the browser specs covering Tensorlake suspend/resume launches on Chromium and
WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests change. No real Tensorlake sandbox is needed.

## Fallbacks

Departures that need the scope reassessment: a retry loop; any change to naming rules; a helm-side create option; any
change to the create protocol.

Decisions to log: the stamp adjustment chosen, the composer wording, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-one-press-suspended-checkout-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/one-press-suspended-checkout/<nn>-<short-name>` (for example
  `plan/one-press-suspended-checkout/01-one-press-resume`).
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
Wording in this file that turns out inaccurate (a line number, a file name, a function name) is corrected to the truth.

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

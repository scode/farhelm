# UI: one readable link style everywhere

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Settle on a standard link style.** The link color in settings is unreadable against its background (deep blue). Fix
> it, audit anything else affected, and settle on one standard visual for all links.

The cause, from a read of main at aa245430 (line numbers approximate): the settings link "How prepared checkouts differ
from a fresh clone" (`crates/farhelm-ui/src/checkout_spare.rs` around 73-81) has no author style rule at all.
`crates/farhelm-ui/assets/app.css` has no general rule for `a`, `:link` or `:visited`, and never declares a
`color-scheme`, so the browser falls back to its light-scheme link colors on the dark settings body (`--bg-2`,
`#1b222c`): `#0000EE` (1.70:1) unvisited, `#551A8B` (1.45:1) visited in Chromium. AA needs 4.5:1.

The audit, all in the app's UI, which the web UI and the desktop app share:

|   | Where                                                                                                           | Element                                     | Today                                                                                         |
| - | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------- |
| A | Settings, checkouts help: "How prepared checkouts differ from a fresh clone" (`checkout_spare.rs` around 73-81) | `a`                                         | no rule: browser default                                                                      |
| B | Welcome card: "Read Get started ↗" (`crates/farhelm-ui/src/welcome.rs` around 61-66)                            | `a`                                         | `.welcome-item a { color: var(--accent) }` (`app.css` around 2940), underlined by the browser |
| C | Welcome card: "Send feedback now" (`welcome.rs` around 74-78)                                                   | `button.welcome-feedback`                   | accent, underline on hover only (`app.css` around 2971-2973)                                  |
| D | Add host dialog: "Start its connections yourself first." (`crates/farhelm-ui/src/hosts.rs` around 5076-5081)    | `button.add-host-ssh-help.welcome-feedback` | same as C                                                                                     |

Out of scope: terminal links (xterm.js decorations with their own security-driven styling, `assets/terminal-links.js`);
the help menu's rows and the launcher's "?" permission-help glyph button, which are not inline text links; the docs
website, which has its own stylesheet; the invisible download anchor in `assets/terminal-files.js`.

Constraints in the stylesheet: the `:root` token layer forbids literal colors at use sites
(`crates/farhelm-ui/js-tests/app-css-tokens.test.js` enforces it). SPEC_impl.md's GUI styling paragraphs (around
410-475) give the one accent a closed list of uses that does not include links, and the `:root` comment (around
`app.css` 215-243) splits `--info` ("a light tone that carries a WORD") from the accent ("never a run of prose"). Only a
dark palette exists.

### Acceptance criteria

- A new role token `--link` in `:root`, valued `#78b7ff` (the docs website's dark-theme link color), with a comment
  saying what it is for and why it is neither `--accent` nor `--info`. The accent's closed list is unchanged.
- One global rule styles every link, so a future anchor gets the look without anyone remembering it (the same reasoning
  as the sheet's global `:focus-visible`): `--link` color, underlined at rest, the visited state identical to the
  unvisited one, and no separate hover change (the link is already underlined at rest). A shared class gives buttons
  that act as inline links (C and D) exactly the same look, with the button reset (inline, no border, padding or
  background, inherited font, pointer cursor) moved into it from `.welcome-feedback`.
- The ad-hoc `.welcome-item a` and `.welcome-feedback` color and hover rules are removed; C and D use the shared class
  (D stops borrowing the welcome card's class).
- Links that open a page outside the app carry a trailing `↗`, as B does: A gains one. C and D act inside the app and do
  not.
- Focus stays on the sheet-wide `:focus-visible` accent ring, which already applies.
- SPEC_impl.md's GUI styling paragraph gains the link role: one standard link look, its token, underline at rest because
  the color alone does not set a link apart from the surrounding grey prose (1.05-1.65:1 against `--fg-1`/`--fg-dim`,
  where WCAG wants 3:1 before color alone may mark a link), no visited color, and the `↗` rule for links that leave the
  app.
- Tests:
  - `crates/farhelm-ui/js-tests/app-css-contrast.test.js` gains a `[--link, --bg-2, 4.5]` binding (both live links sit
    on the dialog body), and bindings for other surfaces a link can sit on if any applies.
  - The existing browser tests that find link A (`e2e/tests/settings.spec.ts` around 83) and link B
    (`e2e/tests/welcome.spec.ts` around 167) assert the computed color and the underline (`toHaveCSS`), and so does a
    test reaching D (the add-host dialog). The bug was a link that no rule reached, which a token-level test cannot see.
    Run them on both browser engines.
- Frozen material stays frozen: the release shot in `e2e/docs-shots/release-notes.spec.ts` that photographs link A is
  not refreshed (root `AGENTS.md`, "Frozen material").
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page, which showed each option rendered on the real settings
background:

- **Color: a new link blue, `#78b7ff`**, as its own token. Chosen over the existing accent `#5896e8` (which would have
  added links to the accent's closed list) and over neutral underlined text.
- **Underline: always**, over underline on hover only.
- **Visited: the same as unvisited**, over a distinct visited color.
- **Buttons that look like links: identical look**, over a distinct look for in-app actions.
- **`↗` on links that leave the app**, as part of the standard: the settings link gains one. Chosen over dropping it
  from the welcome card or leaving each as it is.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC_impl.md's GUI styling paragraphs (token layer, accent list); SPEC.md's Welcome
card section (around 2900-2912), which names the "Read Get started" link; the `app.css` token tests; root `AGENTS.md`,
"Frozen material", "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Selector `a:any-link` for the global rule (the hidden download anchor has no visible rendering, so it is unaffected).
- A plain `text-decoration: underline` in the link color, optionally with a `text-underline-offset`, which needs no
  token. No second underline token or partial-alpha underline: the maintainer chose one link color, underlined always.
- Not chosen: declaring `color-scheme: dark` on `:root` to let the browser pick its dark link color. The color would
  vary by engine, and it would change every native control's rendering app-wide. A scoped rule for the settings link
  alone is not a standard and would leave the next link unstyled.

## Outline

One PR, `plan/standard-link-style/01-link-style`: the token, the global rule and shared class, the class swaps in
`welcome.rs` and `hosts.rs`, the `↗` on link A, the SPEC_impl.md paragraph, the tests, the changelog fragment and the
TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, the
`crates/farhelm-ui/js-tests` suite (token and contrast tests), clippy on `farhelm-ui` for the class swaps, and through
`python3 scripts/record-test-run.py` the touched browser specs (`settings.spec.ts`, `welcome.spec.ts`, the add-host
spec) on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: a light-theme variant, `color-scheme` changes, restyling
terminal links, menu rows or glyph buttons, or touching the docs website.

Decisions to log: the final underline treatment, the selector, which tests gained assertions, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-standard-link-style-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/standard-link-style/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
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
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

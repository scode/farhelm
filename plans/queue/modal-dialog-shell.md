# UI: one modal dialog shell that owns focus

Written against main at 47ccf5fb on 2026-10-21. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

It is one of five structural refactor plans written together (`launcher-draft-model`, `modal-dialog-shell`,
`launch-attempt-owner`, `helm-launch-admission`, `popover-anchor-close`). None depends on another; they may run at the
same time on other checkouts. Two overlaps:

- `launcher-draft-model` restructures the session launcher's state in `crates/farhelm-ui/src/list/create_form.rs`. This
  plan touches only that dialog's root, its Tab trap and its page isolation.
- `popover-anchor-close` owns anchored popovers and menus, including the notification list (`list/bell.rs`), which
  carries `role="dialog"` but is an anchored popover. Leave it to that plan.

## The goal

This is a structural refactor: it changes no behavior, except for the aligned fixes described below, each in a `fix:`
PR. Its purpose is to stop every dialog from assembling its own focus handling.

On main at 47ccf5fb there are about 21 `role="dialog"` sites in 15 files under `crates/farhelm-ui/src/`. Each modal
dialog assembles its own pieces:

- **Tab traps:** three hand-written JS ones with different sets of focusable elements, in `rename.rs` (around line 54),
  `list/restart_with.rs` (around 176) and `list/create_form.rs` (around 1873).
- **Page isolation and focus helpers:** five dialogs call `modal_isolation::install_js`, and five call
  `dialog_focus::install_js`, a 54-line opt-in helper. Host dialogs and welcome use a partial helper in
  `hosts/settings_dialog.rs`.
- **Return of focus on close:** at least seven separate functions (in `feedback.rs`, `settings.rs`, `list/trash.rs`,
  `ssh_help.rs`, `list/templates.rs`, `list/quick_switcher.rs` and `hosts/settings_dialog.rs`). Each has to remember to
  release isolation before focusing the opener, and to fall back when the opener is gone.
- **Keyboard-reachable scroll regions:** the rule "a scroll region with no focusable content needs a tab stop" is
  applied per dialog, with each site computing its own condition and setting `tabindex`, `role` and label.

This was the fastest-growing fix cluster in mid-October. Examples of the class:

- focus lost when the control that held it is removed: 077b639d, 813d0ee7, fa5f7c43, 8a9b9b3d, all 2026-10-19 or
  2026-10-20;
- 5de8953b, c59fe55c, e8ad4fa0, 48a9b695;
- the scroll-region rule spreading one dialog at a time: 098f47f9, then 1ccd6fe1, f1606ac1, 79a11381, 00be4ea9,
  9309f15e.

Deliver:

1. **A `ModalDialog` shell component.** It renders the dialog element itself, with the same element, classes, labels and
   attributes the dialog renders today, so CSS and test selectors are unchanged. It installs page isolation and the Tab
   trap, sets the initial focus, and handles busy-aware Escape. The dialog's focusable set, initial-focus target and
   cancel action are explicit props.
2. **A close contract that knows why the dialog closed.** Where focus returns depends on how the dialog closed:
   - the quick switcher does not restore focus when a different session was picked, because that session's terminal
     takes the keyboard;
   - SSH help chooses between two targets;
   - host settings returns to the host row or to the Add button.

   So the shell's close takes the reason, or a caller-supplied return-focus function. Isolation is released
   synchronously at close, before focus moves; unmount only releases whatever is left, as a fallback.
3. **A `ScrollRegion` component.** It adds a tab stop and label only when its content has no focusable descendant.
4. **Every modal dialog converted**, with deliberate per-dialog differences kept as props.
5. **Aligned fixes in `fix:` PRs.** These include the maintainer's decision below: every modal dialog gets page
   isolation and a Tab trap.

### Design principles

- One copy of each focus rule. After this plan, a fix to trapping, isolation, initial focus, return of focus or scroll
  reachability lands in the shell.
- **Order is part of behavior.** Today each dialog installs isolation and sets initial focus from its own mount hook.
  Moving that into a parent component can change its order relative to child mounts. Host settings documents that its
  focus must come first. Preserve the existing order explicitly, and test it where a dialog depends on it.
- **No modal stack or queue.** The `app-dialog-focus` plan chose against one (see its report under `plans/reports/`).
  Keep the existing rule that delayed host questions wait for the current dialog to close (a0013a0c); it is not a queue,
  and dropping it changes behavior.
- **The shell is the seam for native `<dialog>`, without converting to it.** It is where the TODO.md entry "Native
  `<dialog>` for the app's modal dialogs" would later switch to `showModal()`, dialog by dialog, so build it so that
  switch replaces the shell's internals and not every dialog. Do not convert in this plan.
- **Non-modal users of the helpers stay as they are.** The gap-click/Enter helper stays an opt-in prop. Its non-modal
  users (the session header's restart, replace and delete confirmations; the row menu prompts) keep using it as today.

### You find the instances

The file and line references here are illustrations from main at 47ccf5fb, and other work keeps changing this code. Do
not treat them as a worklist. Your first step is an inventory, on the main you start from: every `role="dialog"` and
every modal surface, classified as modal dialog, non-modal panel or anchored popover. For each modal dialog, record:

- whether it isolates the page and traps Tab, and with what focusable set;
- its initial focus and its Escape and busy handling;
- every way it closes, and where focus goes for each;
- its scroll regions;
- the order of its mount hooks.

Record the inventory in the working log, re-check it after every rebase, and summarize it in the report.

### Acceptance criteria

- Every modal dialog renders through the shell. No dialog keeps a hand-written Tab trap, its own isolation install, or
  its own return-of-focus function.
- Every modal dialog isolates the page and traps Tab (the maintainer's decision; delivered in `fix:` PRs).
- Every scroll region whose content can have no focusable descendant uses `ScrollRegion`.
- Rendered markup is unchanged, apart from tab stops and attributes an aligned fix adds.
- Unit or browser tests cover the shell's trap, isolation release order, return of focus per close reason, and
  `ScrollRegion`'s tab-stop rule.
- Apart from aligned fixes in their own `fix:` PRs: no behavior change, confirmed by the no-functional-change reviewer
  for every PR.
- The last code PR removes this plan's TODO.md entry (under "Code cleanup", "Turning rules into structure").
- Docstrings explain the shell's contract, the close-reason model and the native `<dialog>` seam, per the repository's
  documentation norms.

## Decisions already made

The maintainer's words when asking for these plans (2026-10-21): "lets use the planning system to address the 5 issues.
no-workhorse mode. gpt astra high reviewer. In addition to the regular reviewer, we need a dedicated reviewer whose only
job is to verify that there are no functional changes. There is an exemption to the no-functional-changes rule, both in
planning and for the reviewer. If there's a functional change that is aligned with the problem that the refactor is
trying to fix, that's okay. For example, if the whole point is that behavior might drift, and we're fixing that by
refactoring, and that causes some changes to be consistent where before they were drifted, that's okay. But we don't
want any unrelated, unintentional changes that have nothing to do with the problem that's being solved."

- **The aligned-change exemption.** The problem this plan targets: each dialog assembles its own focus handling (trap,
  isolation, initial focus, return of focus, scroll reachability), and those copies drift. A behavior change is allowed
  only when it is aligned with that problem: it makes drifted dialogs follow the one rule, or fixes a focus or
  reachability bug of that kind. Such a change goes in a `fix:` PR, with a changelog fragment per root `AGENTS.md`,
  "Changelog fragments", and the no-functional-change reviewer must confirm it qualifies. Group fix PRs by kind, not one
  per dialog. A fix never shares a PR with refactoring. Any other change you notice goes in the report's follow-ups.
- **Every modal dialog isolates the page and traps Tab.** About ten modal dialogs on main at 47ccf5fb do neither:
  rename, feedback, two setup dialogs, templates, welcome, trash, the quick switcher, and two in the hosts panel. Asked
  whether the shell should give every modal dialog both, the maintainer chose "all modal dialogs get isolation and a Tab
  trap". Deliver it in a few grouped `fix:` PRs, after the behavior-preserving conversion. Users will notice: Tab no
  longer leaves those dialogs, and the page behind them cannot be clicked. The changelog fragment says so.
- **Open review-queue items.** Fix an item when the new structure covers it, in its own `fix:` PR that also removes the
  item's file and `INDEX.md` line (`review_feedback_queue/AGENTS.md`, "Lifecycle"). Leave the rest, and list them in the
  report. Candidates on main at 47ccf5fb: `ssh-help-response-steals-approval-focus.md`,
  `short-window-outage-blocks-pointer-recovery.md`, `pinned-host-dialog-chrome-can-hide-content.md`. Check each against
  the code; do not assume. Notification-list items belong to `popover-anchor-close`.
- **Reviewers:** both per-PR reviews run on gpt-6-astra at high effort (see Review gate).
- **If gpt-6-astra is unreachable** when a review is due (CLI missing, quota, outage): block the plan per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. No substitute
  reviewer.
- No-workhorse mode, as the maintainer asked and `plans/.agents/planning.md` requires.

## Requirement sources

**The user's request and decisions:** above. The plan comes from a 2026-10-21 whole-codebase assessment of recurring fix
classes and an adversarial review of it. Dialog focus was the fastest-growing cluster, with three open review-queue
items when the plan was written.

**Binding repository constraints:**

- SPEC.md's passages on dialogs, keyboard use and focus;
- SPEC_impl.md's GUI passages on modal isolation, focus return and delayed host questions;
- TODO.md, "Native `<dialog>` for the app's modal dialogs" (not to be done here);
- root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
- `.agents/test-authoring.md` for tests;
- `docs/desktop-web-triage.md` for WebKit focus behavior.

**Planner proposals:** the shell, close-reason and `ScrollRegion` shapes above. A smaller design that meets the same
acceptance criteria is fine, logged as a DECISION. A planning review contributed several requirements: the close
contract instead of return-on-unmount, the unchanged markup, the mount-order rule, and the exclusions of non-modal
helper users and the notification list.

## Outline

A suggested stack; reshape it if the inventory says otherwise:

1. `refactor:` the shell and `ScrollRegion`, with tests, and the first dialog or two converted.
2. `refactor:` the remaining dialogs converted, a few per PR, each keeping its current isolation and trap behavior as
   explicit props.
3. `fix:` every modal dialog isolates the page and traps Tab (grouped, with a fragment), and the props that preserved
   the old gaps are removed.
4. `fix:` other aligned drift found by the inventory, grouped by kind, and any queue item the structure covers.
5. The TODO.md entry removal rides on the last code PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect:

- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets -- -D warnings`, including the UI's web target;
- the `farhelm-ui` unit tests and any JS tests for touched assets through `python3 scripts/record-test-run.py`;
- narrowly selected browser specs for each converted dialog's focus, Escape and close behavior, on the engines
  `.agents/validation.md` names. WebKit focus timing is the usual risk.

## Fallbacks

Examples of departures that need the scope reassessment: a modal stack or dialog queue, converting to native `<dialog>`,
folding menus or popovers into the shell, or a generic focus-management framework beyond dialogs.

Decisions to log: the inventory's classification of each surface, each dialog's preserved differences and the evidence
they are deliberate, every aligned fix and the drift it removes, every queue item fixed or left, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-modal-dialog-shell-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/modal-dialog-shell/<nn>-<short-name>`
  (for example `plan/modal-dialog-shell/01-dialog-shell`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `refactor` for behavior-preserving restructuring, `test` for a change only to tests or their fixtures,
  `fix` (with a changelog fragment in the same commit) only for the aligned-change exemption, `docs` for documentation
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

> This PR is part of a structural refactor whose contract is that it changes no behavior, apart from changes aligned
> with the problem the refactor exists to fix. Your only question is whether this PR changes observable behavior
> compared with its parent commit, and whether each change qualifies for that exception. Ignore style, naming, design
> quality and test quality, except where a test change hides or accommodates a behavior change. Observable behavior
> includes: what is rendered, enabled, focused, escaped, copied to the clipboard or put in URLs; requests sent, their
> contents and order; replies, error kinds, error messages and their precedence; which await, lock acquisition or check
> happens first; when a permit, guard or lock is released; which effects rerun on which changes; what is cancelled when
> a component unmounts or a client disconnects; database writes and their transaction boundaries; panics versus errors;
> logs that tests or operators rely on. For every difference, report the file, the quoted code before and after, a
> concrete scenario that behaves differently, and whether it is an unintended change to revert or a candidate for the
> exception. The exception applies only when all of these hold: the change is aligned with the problem quoted below (it
> makes drifted copies of a rule consistent, or fixes a bug of exactly that kind); the old behavior is not a choice that
> SPEC.md, SPEC_impl.md or a recorded decision endorses; and the PR is a `fix:` PR containing only such changes and a
> changelog fragment. A behavior change inside a `refactor:` or `test:` PR is always a finding, even when it is a good
> fix, and so is any change unrelated to the quoted problem. If you find no difference, say so and list what you
> checked. Report to the named file; edit nothing; touch no VCS state.

Each prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range of this one PR;
- a findings file in the scratch directory, one per reviewer;
- the acceptance criteria: this file's goal, design principles, Decisions already made and the outline; for the
  no-functional-change reviewer, quote the aligned-change exemption from Decisions already made.

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
alternatives considered: in particular those named under Fallbacks above, every use of the aligned-change exemption, and
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

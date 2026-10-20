# Host settings and SSH help keep keyboard focus inside the dialog when a save or load ends

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

While a host setting is saving, the host settings dialog's scrollable body becomes a keyboard tab stop (so it stays
scrollable while every control is disabled). If a keyboard user Tabs into that body during the save, then when the save
ends the body stops being focusable, the browser drops focus to the page, and the dialog's refocus step still picks the
body as "the last usable element". Focus is left outside the dialog, and the dialog's focus isolation swallows the next
keystroke to bring it back. Fix it so that when a save ends, focus is always on a control inside the dialog that can
take keyboard focus.

TODO.md entry this plan covers (Near term): "**Keep focus inside host settings after a refused save.** A keyboard user
can Tab into the host settings dialog's scrollable body while a save is running. If the save is then refused, the body
stops being focusable, but the dialog's refocus step still picks it as the last usable element, so focus can land
outside the dialog instead of on the destination field, and the dialog's focus isolation swallows one keystroke to
recover. A successful save is unaffected. SSH help has the same shape if its commands arrive while its body has focus.
Requiring the refocus target to be keyboard-focusable would close it; add a test that checks focus after a refused save.
Found while landing the keyboard scroll audit (#2222, #2226)."

The planning session's reading of the code found the problem is wider than the entry says: a successful checkbox,
appearance or page-wide write has the same problem when the body held focus, because only a successful destination or
alias save closes an editor and refocuses its edit button first. SSH help has no refocus step at all, so the predicate
fix alone does not cover it.

### Acceptance criteria

- **Host settings:** after any save ends (refused or successful, field, checkbox, appearance or page-wide write), if
  focus is not already on a keyboard-focusable control inside the dialog, it goes to the remembered last-focused control
  if that one can take keyboard focus, and otherwise to the existing fallback chain (the destination field when its
  editor is open, then the first checkbox, then the dialog). "Can take keyboard focus" means at least: an `HTMLElement`,
  connected, inside the dialog, not disabled, and `tabIndex >= 0`. The check applies to `document.activeElement` as well
  as to the remembered element, because in some engines the blur from the body losing its tab stop may not have happened
  yet when the refocus step runs.
- On a refused destination save with the body focused, focus lands on the destination field, and the next typed
  character goes into the field (proving no keystroke was swallowed).
- **SSH help:** when its commands arrive while its body (focusable until then) held focus, focus moves to SSH help's
  close button, where it starts when the dialog opens. If focus was on something still focusable inside the dialog, it
  stays there.
- Browser tests cover: a refused destination save with the body focused; a successful checkbox write with the body
  focused; SSH help's commands arriving with its body focused. Each asserts where focus is and that the next key is not
  swallowed.
- Successful field saves keep today's behavior (focus on the edit button). Nothing else about the dialogs' focus
  behavior changes.
- A changelog fragment (`kind: fixed`): the keyboard scroll change that introduced the focusable body shipped in
  v0.30.0.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Scope: "Every end of a save, refused or successful (the same check covers both)", chosen over refused saves only.
- SSH help: "Fix in this plan; focus goes to its close button, where it starts on open."
- Review gate and mode are in the run instructions below.

**Binding repository constraints:** SPEC.md's keyboard-scroll rule (every scrollable region is reachable and scrollable
by keyboard in every engine; a region with nothing focusable gets its own named tab stop) and the host settings dialog
and SSH help sections; SPEC_impl.md's modal isolation contract (the paragraph about a disabled control dropping focus to
the page body). Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md` for test
changes.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- The dialog is `crates/farhelm-ui/src/hosts/settings_dialog.rs`. The body gets `tabindex="0"`, `role="region"` and an
  `aria-label` only while busy (around lines 636-641). Focus memory is installed by `install_dialog_with_focus` (around
  271-309): a `focusin` listener records every focused element except the container in `__farhelmLastFocus`. The refocus
  step is `refocus_after_write` (around 328-352), run by an effect when busy goes from true to false; its "usable" check
  is only `Element`, connected, inside the dialog and `!disabled`, which a plain div passes. `focus_edit_button` (around
  361-370) runs one frame earlier when an editor closes.
- Modal isolation is `crates/farhelm-ui/src/modal_isolation.rs`; its capture-phase keydown net (around 143-156) swallows
  a key that arrives with focus outside the dialog and focuses the container.
- SSH help is `crates/farhelm-ui/src/ssh_help.rs`: the body has `tabindex="0"` until commands arrive (around 76-79), it
  installs through `install_dialog_with_selector`, and it has no refocus step.
- The Templates dialog has a stricter copy of this logic, `restore_template_focus` in
  `crates/farhelm-ui/src/list/templates.rs` (around 272-286), which checks `HTMLElement`, `!disabled` and client rects
  and also checks `document.activeElement`. There is no shared "is keyboard-focusable" helper in product code; the
  test-side `tabbable` in `e2e/tests/modal-layout.spec.ts` is the closest definition.
- Tests: `e2e/tests/keyboard-scroll.spec.ts` already holds a destination save with a 409 and Tabs into the body (around
  189-224) but never checks focus afterwards; SSH help's held "loading" read is around 86-125. Refusals are produced by
  `page.route` interception with `fulfillAsHelm` (`e2e/tests/helpers/terminal-suite.ts`), so no multi-host or ssh setup
  is needed. `e2e/tests/terminal-multihost.spec.ts` "host-settings-refused-save" covers the plain refused save but needs
  ssh to localhost; leave it as it is.

Proposed shape (planner proposal), one `fix` PR unless it reads better as two:

- A stricter predicate in `refocus_after_write`, applied to both `document.activeElement` and the remembered element.
  Checking where focus currently is, not only the remembered element, is required: the existing early return keeps focus
  on anything inside the dialog, so an engine that leaves focus on the body after it stops being a tab stop would
  otherwise defeat the fix. The container exclusion and the client-rects check in the acceptance criteria are cheap belt
  and braces (both dialog containers have `tabindex="-1"`, which `tabIndex >= 0` already excludes); keep or drop them,
  and log it.
- Do not share the predicate with the Templates dialog: Templates' check deliberately accepts the dialog container, and
  its rows use a roving tab stop, so a remembered row can have `tabindex="-1"` and would be rejected by `tabIndex >= 0`,
  changing its behavior. Sharing a small JS fragment between host settings and SSH help only (SSH help already imports
  its dialog setup from `settings_dialog.rs`) or inlining it twice are both fine; log which.
- A new effect in SSH help: SSH help fetches its commands once per opening, so "has commands" only goes from false to
  true, once; an effect that runs when it is true is enough, with no tracking of the previous value. It refocuses the
  close button (`.ssh-help-close`) when focus is not on something keyboard-focusable inside the dialog.
- The browser tests in `e2e/tests/keyboard-scroll.spec.ts`: the refused-save test is a new sibling of the existing
  held-save test (whose docstring says it does not wait for the save), not an edit to it. Where focus lands on a
  checkbox or button, prove that no key was swallowed with Tab (Space or Enter would start a write or close the dialog).
- A pointer click on the body's text during a save also gives it focus, so mouse users can hit the bug too; the
  changelog wording should not say keyboard only.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the build steps the browser suite needs, and the touched browser specs
(`keyboard-scroll.spec.ts`, plus `modal-layout.spec.ts` and `ssh-help.spec.ts`) through the recorder on both engines;
the test-sleep checker for the new tests; `python3 releasing/check-changelog.py format`. The Templates specs are not in
scope, since Templates is not touched.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: rewriting the dialogs' focus memory, a
general focus manager shared by every dialog, or changing modal isolation's keydown net.

Decisions to log: inline versus shared (host settings and SSH help only) predicate, and the exact predicate.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-settings-focus-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/host-settings-focus/<nn>-<short-name>` (for example `plan/host-settings-focus/01-refocus-check`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

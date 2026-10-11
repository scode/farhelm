# Trash and host settings dialogs: margins, pinned headers, delete-all placement, delete progress

Written against main at 95bb60a7 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Four TODO.md entries (Near term) about two dialogs, planned together because they touch the same component and the same
dialog CSS, so separate plans running at once would conflict:

1. **"Give the trash and host settings dialogs some vertical margin."** Both dialogs take up almost the entire height of
   the window, which makes them feel jammed in. Leave visible space above and below them.
2. **"Keep the host settings dialog's header and footer in view."** The footer's close button and its "changes save as
   you make them" note, and the header, stay where they are and always visible. Only the settings between them scroll.
3. **"Put the trash's 'delete all' at the top right."** The global "delete all" sits in the trash dialog's footer, below
   every host's list, so with more than a few archived checkouts you have to scroll to find it.
4. **"Show that deleting from the trash is still working."** While a confirmed delete runs, the trash dialog only
   disables its buttons (close included) until every host has answered, so a delete that takes a while feels broken.

Acceptance criteria:

- A linear stack of draft PRs, in the order under Outline, each passing its review gate.
- Every modal dialog in the app (the twelve listed under PR 1) leaves the same visible margin above and below itself
  when its content is taller than the window, and none of them touches the window's top or bottom edge.
- In host settings, the header (with its close control) and the footer (the "changes save as you make them" note and the
  close button) are visible at every scroll position; only the settings between them scroll.
- The trash dialog's header holds the title, the overall total, "delete all", and close, and stays visible while the
  host list scrolls. The confirmation for "delete all" appears just below the header, and its cancel and confirm buttons
  are reachable at any window size. There is no footer.
- While a confirmed trash delete runs, the dialog says on how many of the hosts involved it is still running ("deleting
  on 2 of 3 hosts…", the user's own wording), and each host still deleting says so in its own section until its answer
  arrives, at which point the existing result display takes over.
- Each `feat`, `fix`, `perf`, `style` or `revert` PR has its changelog fragment. The last code PR removes the four
  TODO.md entries above.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10):** plan these four TODO entries (with two others about the trash and host settings
dialogs' neighbours planned separately).

**The user's decisions (2026-10-10):**

- One plan for the four entries, built as one stack.
- Delete progress is per host: "deleting on 2 of 3 hosts…" overall, and each host's section says it is still deleting
  until that host answers, then shows its result. No helm or supervisor change. (Per-checkout progress, which would need
  the supervisor to stream updates, was offered and not chosen.)
- The trash header layout, as the user approved it in a sketch:

  ```
  ┌ trash   3 checkouts · 1.2 GB   [delete all] [close] ┐
  │ (confirm band appears here when asked)               │
  ├──────────────────────────────────────────────────────┤
  │ host-a   2 · 800 MB        [delete all on host-a]   │
  │   checkout …                                         │
  │ host-b   1 · 400 MB        [delete all on host-b]   │  ← scrolls
  └──────────────────────────────────────────────────────┘
  ```

  The header is pinned like host settings' header, so both dialogs behave alike.
- The vertical margin applies to all twelve modal dialogs, not only the two the TODO entry names, so that none of them
  fills the window and they share one rule.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; the rule against tests that modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.
- SPEC.md describes the trash dialog and host settings. Read their sections before changing either, and keep SPEC.md
  accurate in the PR that changes what it says. It does not say where "delete all" sits; PR 4's progress display is the
  change most likely to deserve a sentence there.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 95bb60a7 and
corrected by a fresh-context planning review (which found no unnecessary product machinery, and fixed the confirmation's
reachability, the progress counter's state, a CSS ordering trap in PR 2, and the test fixtures).

## Outline

Line numbers drift; find the code by name.

### PR 1: one vertical margin for every modal dialog

Today, in `crates/farhelm-ui/assets/app.css`:

- Nine dialogs share `.host-settings-backdrop` (`display: grid; place-items: center; padding: 20px; overflow: auto`):
  app settings and feedback (`.app-settings-dialog`), add host, host settings, host removal and the two provisioning
  dialogs (`.host-settings-dialog` variants), templates (`.templates-dialog`) and trash (`.checkout-trash-dialog`).
- Rename (`.rename-dialog-backdrop` / `.rename-dialog`), restart-with (`.restart-with-backdrop` /
  `.restart-with-dialog`) and the launcher (`.launch-composer-backdrop` / `.create-session-form`) copy the same backdrop
  geometry under their own selectors.
- The cards cap their height three different ways: `calc(100vh - 40px)`, `calc(100dvh - 40px)`, and the trash dialog's
  `calc(100dvh - 32px)`, with narrow-width overrides (templates at 620px, the launcher at its phone breakpoint).

The quick switcher is top-aligned with its own geometry and is not one of the twelve; leave it alone.

Give all twelve one shared vertical margin, defined once (a custom property, say) and used by the backdrops' top and
bottom padding and every card's `max-height`, using `dvh` consistently. Change only the vertical padding: the launcher's
width depends on today's 20px side padding. Pick a margin that reads as visible space on a laptop-height window
(somewhere around 5vh with a sensible minimum; judge it from screenshots) and a smaller one at phone widths, where space
is scarce, at one breakpoint (templates and the launcher use two different ones today; pick one and log it). Log the
values as a DECISION.

Tests: one Playwright layout spec that, at a short viewport where the content overflows, opens one dialog per distinct
style rule (the shared backdrop's host-settings family, templates, trash, rename, restart-with, and the launcher; app
settings shares host settings' rule and adds nothing), choosing content that actually overflows at that size, and
asserts each card's top and bottom sit at least the margin away from the viewport edges. The larger margin shrinks every
dialog's maximum height at the default test window size, so also run the existing layout-sensitive specs for these
dialogs (at least the launcher's refusal tests and `e2e/tests/templates.spec.ts`). Look at screenshots of trash and host
settings before and after.

### PR 2: host settings keeps its header and footer in view

`crates/farhelm-ui/src/hosts/settings_dialog.rs` renders `div.host-settings-header`, `div.host-settings-body`, and
`div.host-settings-actions.host-settings-footer` inside `.host-settings-dialog.host-settings-refreshed`. Today the whole
card scrolls: the comment above the `.host-settings-refreshed` rules in `app.css` says "The dialog owns vertical
scrolling", and `.host-settings-refreshed > * { flex-shrink: 0 }` exists so a short window scrolls down to the footer
rather than overlapping it.

Make the card a non-scrolling flex column (`overflow: hidden` on the card, header and footer `flex-shrink: 0`) and the
body the only scroll region (`overflow: auto; min-height: 0`, and allowed to shrink). Adding that to the body's rule is
not enough: `.host-settings-refreshed > * { flex-shrink: 0 }` comes later in the stylesheet at the same specificity and
would win, leaving the settings clipped rather than scrolled. Replace that rule rather than fighting it. Update that
comment, and scope the change to `.host-settings-refreshed`: the base `.host-settings-dialog` classes are shared with
other dialogs (the comment above `.host-settings-refreshed` warns about this). Keyboard focus moving to a field below
the fold must still scroll it into view, and the outcome lines under fields must stay readable.

Tests: extend the host settings coverage in `e2e/tests/terminal-multihost.spec.ts` (the dialog lifecycle tests) with one
case at a short viewport: the body scrolls, and at the top and bottom scroll positions the header's dismiss control and
the footer's close button and note are inside the viewport and not covered. Existing tests that select
`.host-settings-close`, `.host-settings-dismiss` or the footer text must still pass, as must
`e2e/tests/tooltip-coverage.spec.ts`.

### PR 3: trash header holds the total, "delete all" and close, and stays in view

`crates/farhelm-ui/src/list/trash.rs` renders `div.checkout-trash-header` (title and close), a description paragraph,
the per-host `section.checkout-trash-host` list, the global `ConfirmBand` (rendered when `confirmation.host` is `None`)
and `div.checkout-trash-footer` (the total and `button.trash-delete-all`).

Move the total and "delete all" into the header, before close, and remove the footer. Lay the dialog out like PR 2: the
header pinned, the description and host list scrolling. The global confirmation band appears directly below the header,
but it names every host involved, so on a short or narrow window a pinned band could push its own cancel and "delete
permanently" buttons out of reach in a card that clips overflow. Either make the band the first thing in the scrolling
region and scroll it into view when it opens, or keep it pinned with its own bounded scrolling; log which, and check it
with a screenshot at a short, narrow window. `.checkout-trash-dialog` is a plain block today, so it becomes a flex
column. Keep every enablement rule "delete all" has today (`busy`, `all_ready`, empty targets) and the per-host bands
where they are. At narrow widths the header may wrap; it must not clip the buttons. When the global confirmation is
cancelled, focus returns to "delete all"; after a submit, it goes where it goes today when the delete finishes (the
close button).

Tests: no existing test references the footer, so the trash tests in `e2e/tests/github-checkouts.spec.ts` should pass
unchanged; run them. Add a layout test: with enough archived checkouts to overflow at a short viewport, "delete all" and
close are visible without scrolling and stay visible after scrolling the list, and the open confirmation's buttons are
reachable at a short, narrow viewport. Real archived checkouts each cost a session launch and a delete, so stub the
trash listing (and, for PR 4, the delete reply and a second host) with `page.route`, as an existing host settings test
does for a fake host.

### PR 4: trash delete shows per-host progress

Today `submit` in `trash.rs` sets `remaining` to the number of host targets, clears the confirmation, and spawns one
task per host calling `empty_checkout_trash`. Each reply updates that host's listing and `notices`, then decrements
`remaining`. Nothing is shown while waiting, and the total is not kept once `confirm.targets` is consumed.

Replace the bare `remaining` counter with the set of hosts still deleting (and the total) for the run, so there is one
piece of state to update per reply rather than two that must agree, and show:

- an overall status near the top (the header area from PR 3), live-announced (`role="status"`), along the lines of
  "deleting on 2 of 3 hosts…" ("deleting…" is enough when one host is involved);
- in each host section still deleting, a "deleting…" indicator that disappears when that host's answer arrives and its
  existing result display (`notices` / `Issue`) takes over.

No helm, protocol, or supervisor change: the helm route `POST /api/checkout-trash/delete` and the supervisor reply only
once per host, which is all this needs. The existing guards stay (close and Escape disabled while busy, the listing
effect not re-listing mid-delete), and they already prevent closing the dialog or starting a second delete mid-run. A
host whose request fails must leave the set just as one that succeeds does.

Tests: a Playwright test with a stubbed two-host trash listing that holds each host's `/api/checkout-trash/delete`
response with `page.route`. While both are held, the overall status says 2 of 2 and both hosts show "deleting…"; after
releasing one (with one success and, in a second case, one failure), it says 1 of 2 and only the other host shows
"deleting…"; after releasing the last, the status and indicators are gone and each host shows its result.

The same PR (or a final one, if that reads better) removes the four TODO.md entries this plan covers.

### Validation

Follow root `AGENTS.md` "Finishing work" per PR, choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop` if a change touches desktop-only code (none is expected);
- the named Playwright specs on Chromium and WebKit through the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown;
- `python3 releasing/check-changelog.py format`.

These are UI changes found by looking, so take a Playwright screenshot of each changed dialog state and look at it
before and after. Do not run the full battery without a specific risk that calls for it. `e2e/docs-shots/` specs that
capture these dialogs (for example `manage-hosts.spec.ts`) are not gates; if one breaks because of a selector this plan
changed, update the selector, but do not regenerate or publish images.

Conventional Commit types and changelog kinds are yours to choose per root `AGENTS.md` (the type reflects the
user-visible effect); log each as a DECISION. A single fragment edited by later PRs is fine for closely related entries.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-trash-and-host-settings-dialogs-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/trash-and-host-settings-dialogs/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/trash-and-host-settings-dialogs/01-dialog-margin`).
- One PR per item under Outline, in that order, as one linear stack. Keep them bite-sized and avoid churn: code added in
  one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR that
  needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entries this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the PR's part of the outline.

For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. For PR 4, ask the reviewer to check that the per-host state cannot get stuck showing "deleting…"
when a host's request fails, the dialog closes, or a second delete starts.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a helm, protocol or supervisor change,
  per-checkout progress, a shared dialog component replacing the existing ones, or edits well outside the files the
  outline names beyond tests and bookkeeping (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- the margin values and how they are shared;
- how the scroll region is structured in each dialog, and how focus behaves after the trash confirmation moves;
- the progress wording and where it sits;
- where the global confirmation band sits and how it stays reachable;
- each Conventional Commit type and changelog kind;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a protocol change for per-checkout progress,
leaving some of the twelve dialogs without the margin, or keeping a trash footer.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

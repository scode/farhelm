# Release notes in the `?` menu

Written against main at 4c9e9612 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The `?` menu in the sidebar's bar gains a **release notes** item that opens Farhelm's release notes on the docs website
in one click. Today the only link to them is What's new, in the update menu behind the red version readout, which
appears only in the desktop app while an installed update waits for a restart. So someone on the web UI, or on a desktop
app with no pending update, has no way to reach the release notes from inside Farhelm.

### Acceptance criteria

- The `?` menu's items, in order: send feedback, documentation, **release notes**, then check for updates where it is
  offered today (a desktop app whose updater runs). The new item is offered in the web UI and in the desktop app alike.
- The item's label is `release notes` (maintainer decision; the update menu's `what's new` keeps its own label and stays
  where it is). Description line: `opens the release notes in your browser`. Hover text, in the style of the other bar
  menu items:
  `release notes: open Farhelm's release notes at farhelm.io/docs in your browser, each release with what changed`.
  Adjust the wording only to correct something untrue, and log it.
- It opens the same page What's new opens (`RELEASE_NOTES_URL`, the general release notes page, not an anchor for the
  running version), through the same `open_external_page` path the documentation item uses: the system browser from the
  desktop app, a new tab from the web UI.
- The bar menus' keyboard behavior (arrow keys, Home/End, Escape, focus return) covers the new item like the others.
- The `?` button's own hover text lists the menu's items (one version with check for updates, one without); both name
  release notes too.
- Comments that speak of the release notes URL as What's new's alone (the docstring on `RELEASE_NOTES_URL`, the "never
  in this menu's list" match arms) are updated to match.
- SPEC.md "Feedback" (which defines the `?` button and its items, counting them as "two items" plus "a third item")
  lists the new item, with the counts adjusted. SPEC_impl.md does not name the `?` menu's items and needs no change.
- Docs website: the last paragraph of `using/send-feedback.md`, the only place that lists the `?` menu's items, mentions
  the new item, following `website/AGENTS.md` and `website/EDITORIAL_RULES.md`. Never edit `release-notes.mdx`: it is
  generated published release history.
- Tests: extend `e2e/tests/feedback.spec.ts`, which covers the `?` menu (it currently expects exactly two items, and its
  docstring says so): the item is present in the stated order and opens the release notes URL.
  `e2e/tests/tooltip-coverage.spec.ts` already sweeps every bar menu item's hover text, so it covers the new item as is
  (raising its minimum item count is optional). No Rust unit test of `help_menu_items` exists, and none is needed, since
  the displayed order comes from `BAR_MENU_ITEMS` (see the outline).
- A changelog fragment (`kind: added`), as root `AGENTS.md` requires for a `feat` PR.
- The TODO.md entry ("Release notes in the `?` menu.") is removed in the same PR.
- One draft PR (or a linear stack, if review shapes it that way), passing its review gate. No PR is marked ready, and
  nothing is merged.

Out of scope: linking to the running version's section of the release notes, changing the update menu, adding a release
notes page inside the app, and any other `?` menu change beyond the item and the text that lists the items.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Release notes in the `?` menu.** Add an item to the `?` menu that opens the release notes on the docs website in one
> click. Today the only link to them is What's new, in the menu behind the version readout, which appears only while an
> update is waiting to be applied.

**The user's decisions (2026-10-14):**

- The label is "release notes", chosen over reusing the update menu's "what's new" item as is, from a preview that put
  it after documentation with the description line "opens the release notes in your browser".
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Feedback" (the `?` menu), SPEC.md "Installation and updates" (the update
menu and What's new); SPEC_impl.md's bar menu description; root `AGENTS.md` Docs website, Conventional Commits,
Finishing work, Releases and the changelog, Talking to the user; `.agents/test-authoring.md` for every test change;
`plans/AGENTS.md`, Executing one plan.

**Planner proposals:** the general release notes URL rather than a per-version anchor (the page already lists every
release, newest first, and a development build has no anchor), the hover text, and the test placement.

## Outline

Line numbers drift; find the code by name. Everything lives in `crates/farhelm-ui/src/app_bar.rs`:

- `BarMenuItem` lists every bar menu item, with `label`, `tooltip` and `description` per item. `BAR_MENU_ITEMS` sizes
  the shared `MenuOrder` and is also the order both menus display their items in: the menus ignore the order of the list
  `help_menu_items` returns.
- `help_menu_items(has_updater)` decides which items the `?` menu offers; `UPDATE_MENU_ITEMS` is the update menu's.
- `RELEASE_NOTES_URL` and `DOCS_URL` are the two constants; the click handlers call `open_external_page`.

The work: add a `ReleaseNotes` item (a separate variant from `WhatsNew`, since its label, description and hover text
differ), insert it into `BAR_MENU_ITEMS` between documentation and check for updates (the array grows from five to six),
offer it from `help_menu_items`, wire its click to `open_external_page(RELEASE_NOTES_URL)`, update the `?` button's
hover text and the comments, then tests, SPEC.md, docs, fragment and TODO removal. One PR is the expected shape.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for `feedback.spec.ts` and `tooltip-coverage.spec.ts`;
- the website build if docs change; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-release-notes-menu-item-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
  `plan/release-notes-menu-item/<nn>-<short-name>` (for example `plan/release-notes-menu-item/01-release-notes-item`).
- Expected shape: one PR. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a new feature, so `feat`. The PR adds its changelog
  fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog).
  Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a per-version link, a new menu
  component, or a change to the update menu (examples, not a blacklist);
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
alternatives considered. In particular: any wording change, where the docs mention the item, and every review finding
you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallback: wording above that turns out inaccurate is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a different label, dropping the item from the
web UI, changing the update menu.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

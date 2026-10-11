# Launcher folder shortcuts become a dropdown on the folder field

Written against main at 95bb60a7 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Redesign the launcher's folder shortcuts": under the folder field, the launcher shows a
grid of dotted-underline links: recently used folders, cut off so that several look identical, then "home" and "local
home" on a row of their own. Nothing labels them, so the grid reads like a rendering bug until you stare at it long
enough to guess that it might be history. Redesign it so it is obvious at a glance what the links are and that clicking
one fills the folder.

The user chose to replace the links with a dropdown on the folder field, like the repository picker.

Acceptance criteria:

- The grid of links under the launcher's folder field is gone. Focusing the folder field (or pressing the down arrow in
  it) opens a dropdown with a "recent" group and a "reset" group, as in the sketch under Decisions.
- The "recent" group lists the host's remembered folders, newest first, at most 8. Once the user has typed in the field,
  it lists only those that match what is typed (case-insensitive substring). Until then (the field opens prefilled with
  `~` or the cloned session's folder) it shows the 8 newest.
- Long paths are cut off at the start, so the end that tells sibling folders apart stays visible, and the full path is
  available on hover.
- "home" and "local home" sit in the "reset" group with their meaning spelled out ("~ on this host", "~ on this
  machine"), and are always offered.
- Choosing an entry, by mouse or keyboard, does exactly what clicking the corresponding link does today, with the same
  guards, and fills the field; it never launches. Arrow keys move through the entries and Enter chooses the highlighted
  one. Nothing is highlighted until an arrow key is pressed, so Enter on a typed path still does what it does today
  (launch). Escape closes the dropdown without closing the launcher. The dropdown is a proper combobox and listbox for
  assistive technology, as the repository picker is.
- In managed-checkout mode there is no folder dropdown, as there are no links today.
- The dropdown is usable at the launcher's narrow (phone) width and is not clipped by the launcher's scrolling.
- One draft PR, passing its review gate, with its changelog fragment, and removing the TODO.md entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10):** plan this TODO entry.

**The user's decisions (2026-10-10):**

- Direction: a dropdown on the field, as approved in this sketch (the labeled list and the chip designs were offered and
  not chosen):

  ```
  folder  [ ~/git/farh│                    ] [browse]
          ┌ recent ───────────────────────────┐
          │ …/git/farhelm-fh-plan             │
          │ …/git/farhelm-fh-review           │
          ├ reset ────────────────────────────┤
          │ home (~ on this host)             │
          │ local home (~ on this machine)    │
          └───────────────────────────────────┘
  ```

- Contents: typing filters the remembered folders by substring, up to 8 matches show, newest first; with the field
  unchanged, the 8 newest show. Arrow keys, Enter and Escape work like the repository picker. (Newest 5 or newest 3
  without filtering were offered and not chosen.)
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Recent setups": the helm remembers bounded successful launches and used folders per target install, and
  folder search looks through that history without walking a filesystem. SPEC.md's managed-checkout section: leaving
  checkout mode restores the prior editable path, Browse and recent folders. Keep both true.
- SPEC.md lists each launcher control's Enter behavior. Add the folder field's (Enter chooses a highlighted entry,
  otherwise launches as before) in the same PR.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; the rule against tests that modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 95bb60a7 and
corrected by a fresh-context planning review. It found no unnecessary machinery, and caught that copying the repository
picker's always-highlighted first entry would make Enter replace a typed path instead of launching, that "untouched"
needed defining, and that hiding "local home" for this machine (a planner idea, not the user's) is not equivalent to
"home", since "local home" also pins the host. Hiding was dropped.

## Outline

Line numbers drift; find the code by name.

Here's what exists today, in `crates/farhelm-ui/src/list/create_form.rs` and `crates/farhelm-ui/assets/app.css`:

- The links are `div.launch-composer-folder-links` (aria-label "recent folders"), rendered only outside checkout mode:
  `recent_history.folders.iter().take(3)`, one button each, labelled with the full escaped path (`display_peer`), cut
  off at the end by CSS (`text-overflow: ellipsis`), with the full path in `data-tooltip`. Clicking one runs
  `draft_transition_allowed`, `admit_history_destination`, `promote_fetched_history_snapshot`, the browse invalidation,
  `select_existing_directory`, and clears `intent_key`.
- The resets are built once as `destination_resets` (two `button.launch-composer-folder-reset`s, "home" and "local
  home", with their aria-labels "reset folder to home" and "reset destination to local home"); the comment above it
  mentions two launcher surfaces, which is out of date: there is one folder field, in the destination area both launch
  tabs share, so there is one place to change. "local home" also moves the host to this machine and takes the cloned
  session's host off automatic handling (`CloneHostState::UserTookOver`).
- `recent_history` is the launch history (`GET /api/launch-history`, which already returns up to 100 remembered folders
  per host install, newest first) gated on matching the current target. The composer search already matches folders by
  lowercase substring (`launch_composer.rs`), which is the matching rule to reuse.
- The repository picker (`role="combobox"`, `aria-autocomplete="list"`, a `role="listbox"` with options that use
  `aria-selected`, `tabindex=-1` and `onmousedown` with `prevent_default`) is the pattern to follow for semantics, but
  not for highlighting: it always highlights its first entry, so Enter picks it. The model field opens on focus and
  highlights nothing until an arrow key, which is the behavior this field needs. The launcher search's grouped results
  (with group headings) are the pattern for the "recent" and "reset" groups.
- The form already tracks whether the user has typed in the folder field (`cwd_edited`), and choosing any folder resets
  it; that is the "typed" condition for filtering.
- Clipping is already solved twice in the same dialog: the repository picker's list sits in normal layout, and the model
  and search lists float over the content. Use one of those; do not build a new positioning mechanism.

The work:

- Make the folder field a combobox with a listbox below it, grouped into "recent" and "reset". Open on focus and on
  ArrowDown, and reopen on typing after Escape; close on Escape, on Tab, on blur, and after a choice. Consume Escape
  only while the list is open, so a second Escape still closes the launcher. Highlight nothing until an arrow key; Enter
  with nothing highlighted keeps today's behavior. Filter the recent folders once the user has typed, without changing
  what typing in the field already does.
- Choosing a recent folder runs exactly today's click handler; choosing a reset runs exactly today's reset handler. Keep
  one source for each handler (move them into functions or closures shared by mouse and keyboard), so the guards cannot
  drift.
- Show each recent folder cut off at the start. CSS `direction: rtl` tricks mangle paths with leading punctuation, so
  prefer a real start-ellipsis (a computed label, or a layout that keeps the tail visible) and log how. Keep `dir="ltr"`
  and `display_peer` for peer-supplied text. Each entry's accessible name stays the full path, so screen readers and
  tests can tell siblings apart.
- Remove the old grid's markup and CSS, including its two-column workaround and the narrow-width override, and the
  dotted-underline styles that only it used.
- If `website/` pages describe the links under the folder field, update the text. `e2e/docs-shots/` and
  `e2e/readme-video/` specs are not gates; if one selects the old links, update the selector so it still runs, but do
  not regenerate or publish images.

Tests: this is the bulk of the work. Existing browser tests select the old links or the reset buttons and need rewriting
to the dropdown, keeping what each one proves:

- `e2e/tests/sidebar.spec.ts`: "composer holds offered history steady…", "composer path actions keep typing inert…",
  "composer rejects held and queued stale browse activation…", the two local-home reset tests, "composer destination
  block keeps host, browse, folder, and recent links together", "composer folder history tracks edited destinations at
  narrow width", and "composer menu-closed Tab order…" (the resets are no longer Tab stops);
- `e2e/tests/destination-authority.spec.ts` (the two tests that pick `/tmp` and `/var/tmp` from the links);
- `e2e/tests/f20_visual_capture.spec.ts`, which never touches the links but whose captures would now show the dropdown
  open wherever the folder field has focus.

Three of these cannot keep their old proof as-is; decide each and log it:

- the narrow-width history test proves that the link for the current folder shows as pressed; the dropdown has no such
  marker, so that assertion goes, or the current folder's entry gets one;
- the destination-block test clicks "local home" while this machine is already the selected host; keep the click and
  assert what it does there (it pins the host);
- the destination-authority race test needs the host list and the folder entries on screen at once, and the dropdown
  closes on blur, so it needs another way to set up the race.

Beyond those, about 60 places across some 20 spec files type into the folder field. With open-on-focus the dropdown is
then open for the next step, which can block clicks, especially at phone width. Run the launcher-heavy specs and fix
what breaks, by closing the list in the helper that fills the field if that is cleanest, rather than by changing the
product behavior.

Add coverage for what is new: browser tests for keyboard choice (arrows, Enter on a highlighted entry, Enter with
nothing highlighted still launching, Escape closing only the list), the full path on hover, and no dropdown in checkout
mode; Rust unit tests for the filter (untouched versus typed), the 8-entry cap, and the start-cut label.

The PR removes the TODO.md entry this plan covers. Changelog: `kind: changed`.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop`;
- the changed and new Playwright specs on Chromium and WebKit through the recorder, after the builds the browser suite
  needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, and the website build if a `website/` page changed;
- `python3 releasing/check-changelog.py format`.

Take Playwright screenshots of the dropdown (untouched field, filtered, narrow width, long shared-prefix paths) and look
at them. Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-folder-shortcuts-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/launcher-folder-shortcuts/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/launcher-folder-shortcuts/01-folder-dropdown`).
- One PR: a stage with both the old links and the dropdown would have duplicate controls. Keep it focused and avoid
  churn: code added in one PR and deleted in a later one means the stack should have been shaped differently. Within
  this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier
  run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
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
`AGENTS.md` requires. Ask the reviewer to check that the dropdown's choices run the same guards as the old links and
resets, that Escape closes only the dropdown, that the combobox and listbox semantics match the repository picker's, and
that every rewritten test still proves what the old one did.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a helm or API change, a general
  combobox component replacing the repository picker too, or edits well outside the files the outline names beyond tests
  and bookkeeping (examples, not a blacklist);
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

- how the start-cut label is produced;
- when the dropdown opens and closes, and how it avoids clipping;
- how the click and keyboard paths share their handlers;
- what each rewritten test now asserts, where it differs from before;
- the Conventional Commit type and changelog kind;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: keeping the old links next to the dropdown,
showing fewer or more than 8 recent folders, dropping keyboard support, or changing what Enter does on a typed path.

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

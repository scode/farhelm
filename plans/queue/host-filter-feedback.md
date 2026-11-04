# UI: show which host the session list is filtered to

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> - **Host click in the navigation rail gives no feedback.** Clicking a host in the navigation rail shows no sign of
>   what it did, especially with an empty session list. Make "No sessions match this filter" name the active filter —
>   showing the host with icon and name when a host is filtered on — and show the host in the filter bar. Generally
>   rethink the filter experience.

The maintainer's words on the planning page about the last sentence, verbatim: "let's keep a left over near term
"generally rethink session filtering experience" in the todo list".

### What changes

Choosing a host in the rail sets the page-local host filter and sends a server-side query (`choose_host` in
`crates/farhelm-ui/src/hosts.rs`; `filter` signal and `request_host_filter` in `crates/farhelm-ui/src/list/view.rs`).
Today the only signs are the selected avatar and the "N of M" count beside the list title, and an empty result says "No
sessions match this filter" (`rows::no_match_line` in `crates/farhelm-ui/src/rows.rs`, rendered as
`div.status.filter-empty`). There is no filter bar as such; the All / Running / Waiting / Idle segmented row
(`.session-status-filter`) is the nearest thing.

- **Host chip:** while a host is chosen, the status row shows a chip with that host's mark and short name (the same
  identity the rail uses: `icons::HostMark` and the rail's identity helper `rail_identity` in `hosts.rs`, which is
  private today and needs `pub(crate)`; the local machine's short name is "Local") and a ✕ that returns the filter to
  All. Extract the rail's All/host choice handler (an inline closure today) and call it from the ✕, so the ✕ behaves
  exactly as clicking All in the rail by construction. The chip's tooltip is the host's name plus its destination only
  (not the rail tooltip's phase, update and attention lines), and its accessible name is like "Filtered to host devbox;
  remove filter". For a removed host the chip mirrors the rail's placeholder ("?" mark, "no longer registered" wording),
  and its ✕ still works even though the rail's placeholder button is disabled. Use `HostMark`, not `SessionHostIcon`:
  the latter appends a hidden "Local"/"Remote" word that would garble the chip and the message.
- **Empty message:** when the filtered list is empty, the message names the active filters in a sentence, with the host
  shown by mark and name: "No sessions on [mark] devbox."; with a status as well, "No waiting sessions on [mark]
  devbox."; with only a status, "No waiting sessions." Below it, buttons styled as links clear each active filter ("Show
  all hosts", "Show all statuses"); they change page state and navigate nowhere, so they are buttons. The message is
  built from the query the listing on screen answered (`listing_query` in `view.rs`, set in `commit_listing` on
  success), not from the `filter` signal: while a new filter's read is in flight the old listing stays on screen, and
  reading `filter` would name the new host over the old host's empty result. The chip is a control and follows `filter`,
  like the rail and the status segments. Keep the function in `rows.rs` pure: it takes the listing and the query that
  listing answered, and resolves no host itself. Keep the `div.status.filter-empty` element and class (four test sites
  select on it), with the clear buttons outside the text node the tests read, or adjust those tests. After a clear
  button or the chip's ✕ removes the focused element, focus moves to the rail's All button. The truncated-listing
  variant of `no_match_line` stays as it is (only reached past the helm's listing cap). The unfiltered empty fleet still
  says "No sessions".
- **TODO:** the PR removes this entry and adds a new entry directly under the Near term bucket (not under "Trivial")
  reading, in the maintainer's words, "Generally rethink the session filtering experience", as a bold-handle entry with
  one sentence of context pointing at what this plan did, because the maintainer asked to keep that part as a leftover.
  This is the maintainer's explicit request, so it is not an agent-initiated TODO entry.

### What does not change

How filters work: host and status remain page-local, unpersisted, and combine as today; the rail stays the host picker;
the quick switcher's reset of both to All when it opens a hidden session; the count beside the title.

### Acceptance criteria

- With a host chosen, the chip shows that host's mark and short name; its ✕ returns to All (rail selection and query
  both), and keyboard users can reach and activate it.
- The empty message names the host (mark and name) and status as described, with working clear links; it reads correctly
  for a host with an alias, a removed host (the existing "no longer registered" tombstone wording), and the local
  machine.
- Narrow sidebars: the chip truncates the name with a tooltip rather than overflowing. At the minimum sidebar width the
  chip does not fit beside the four status segments and takes its own line; that is acceptable, but it must not overlap
  or clip them.
- While a different host's read is in flight, the message keeps describing the listing on screen.
- SPEC.md's filter text says the list shows the active host as a removable chip and that an empty filtered list names
  its filters. SPEC_impl.md does not describe this wording and needs no change. Docs:
  `website/src/content/docs/docs/using/session-list.mdx`, "Find a session".
- Tests: only the `rows.rs` unit test for `no_match_line` and `e2e/docs-shots/visual-review.spec.ts` (around the "No
  sessions match this filter" assertion) check the exact old text; update both. `filters.spec.ts` and
  `status-filter.spec.ts` check only that `.filter-empty` is visible and the count reads `0 of N`; extend them with
  cases for the chip and its ✕ (including keyboard), each message variant, the clear buttons, a removed host, and the
  in-flight case. Before editing anything under `e2e/docs-shots/`, read `docs/docs-shots/SPEC.md`; the visual-review
  `*-status-empty` captures become stale (name them in the report; do not refresh them).
- A changelog fragment (`kind: changed`), removal of this TODO.md entry, and the new leftover entry described above.

## Decisions already made

Settled with the maintainer on 2026-11-04 on a planning page with mockups (a dark mockup of the rail with devbox
selected, the list title "Recent activity 0 of 12", the status row with a chip "[DB] devbox ✕" after Idle, and the
centered message "No sessions on [DB] devbox." with a "Show all hosts" link):

- **The host shows as a removable chip in the status row,** not in the list title.
- **Scope:** the message and the chip only; "generally rethink the filter experience" stays in TODO.md as a new Near
  term leftover, in the maintainer's words above.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's rail and filter text (host choice is a server-side query and not
persisted; a filter with no matches says so in the list); root `AGENTS.md`, "Code rules", "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `website/AGENTS.md`;
`.agents/test-authoring.md`; `.agents/validation.md`. Docs screenshots under `e2e/docs-shots/` are fenced media
(`.agents/docs-media.md`, `docs/docs-shots/SPEC.md`): update the visual-review assertion's text, but do not refresh
captures; name in the report the shots this change makes stale.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code: the chip as a small component next to the status segmented control in `list/view.rs`,
using `HostMark`; the message built from `listing_query` by a pure function in `rows.rs` returning structured parts (so
the host mark renders as markup rather than text). `app.css` still has dead `.filter-host` rules from an old host
dropdown; they are not a filter bar to reuse (removing them is fine).

## Outline

One PR, since the change is small and splitting it would leave the exact-text tests broken between PRs and separate the
changelog fragment from its change:

1. The chip with ✕ and the shared choice handler; the structured empty message with clear buttons; their styling in
   `app.css`.
2. SPEC.md, the docs page (an incidental docs edit in an unattended run: start no preview server and name the changed
   page in the report), test updates, the changelog fragment, the TODO.md removal and the leftover entry, all in the
   same commit.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the UI crate, the UI
crate's tests (including `rows.rs`), `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`,
and, through `python3 scripts/record-test-run.py`, `filters.spec.ts` and `status-filter.spec.ts` on Chromium and WebKit,
plus the visual-review case whose assertion changed if it can run without refreshing captures. Run
`python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: a general filter bar or new filter kinds; persisting filters; moving the
host picker out of the rail.

Decisions to log: the chip's placement within the status row, the message wording for each variant, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-filter-feedback-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/host-filter-feedback/<nn>-<short-name>`
  (for example `plan/host-filter-feedback/01-host-chip-and-message`).
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
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

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
passed its code review gate, and the last PR removes the TODO.md entry and adds the leftover entry the goal describes.
Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

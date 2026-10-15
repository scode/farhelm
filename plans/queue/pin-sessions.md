# Pin sessions to the top of the sidebar

Written against main at 5de6290c on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Let the user pin sessions so they stay at the top of the sidebar. The TODO.md entry (Near term, "Pin sessions to the top
of the sidebar") said only "Details TBD"; the maintainer settled the details below from a mockup.

### Acceptance criteria

- **Pin and unpin from the row menu.** One menu item whose label follows the current state, "Pin to top" or "Unpin", in
  the menu's first group directly after Mark read/unread (a metadata edit, like Rename and Mark read, not a lifecycle
  action). It works when the session's host is unreachable, because the pin is helm data.
- **Pinned rows come first, in the list's chosen order.** Pinned rows are shown ahead of all other rows; among
  themselves, and among the rest, rows keep the order the list is set to (most recent activity, created, title). No
  manual or pin-time order (the maintainer's choice). Under most recent activity the pinned rows sit ahead of the
  connected running/waiting group, so a stale pinned row on an unreachable host sorts above live running work; SPEC says
  so explicitly. Pinned rows still obey the host selector and every other filter.
- **The look is the mockup's variant B** (the maintainer's choice; the mockup itself lives outside the repository, so
  this is the whole of the design): each pinned row carries a pin icon immediately before its title, in the title's
  muted foreground tone, from the vendored Nerd Font like the UI's other glyphs; after the last pinned row a separator
  that is clearly heavier than the ordinary hairline between rows, with a small gap below it. There is no "Pinned"
  heading. The separator is drawn only when an unpinned row follows the pinned ones. A pinned row's accessible text says
  it is pinned, and the icon has the same tooltip/accessible name.
- **The separator follows the displayed order.** While the reorder hold keeps rows still (pointer over the list, or a
  row menu open), the displayed order can disagree with pinned-first: a row pinned during a hold stays where it was, a
  row unpinned during a hold stays inside the pinned block, new sessions are appended at the bottom. The separator goes
  after the leading contiguous run of pinned rows as displayed, and only when an unpinned row follows. When the hold
  releases, rows take their pinned-first places.
- **Shared and live.** The helm stores pins per session; every browser and the desktop app see a change straight away
  (the change bumps the event revision, as read/unread does). A pin is removed when its session is deleted.
- **Replace carries the pin over** (the maintainer's choice): Replace and "Replace with..." create a new session in the
  old one's place and delete the old one; if the old one was pinned, the new one is pinned. Restart keeps the session
  id, so it needs nothing.
- **The quick switcher (Cmd+K) is unaffected** (the maintainer's choice). It reuses the sidebar's list request in
  activity order, so the helm's order must not change; see the outline.
- **Upgrade note, accepted by the maintainer:** the helm's database gains a schema step, so after upgrading, an older
  release's helm refuses that database (no data loss, no downgrade). Keep SPEC.md "Upgrade compatibility and client
  scale" otherwise satisfied.
- **Tests:** helm store tests (pin, unpin, changed-flag, cleared by delete, carried by replace), route tests (including
  the event bump only on a real change and the 422 on a body without `pinned`), the HTTP contract fixture on both sides,
  `rows.rs` unit tests for the pinned-first partition and the separator under the hold, a UI unit test for the menu
  label, and a browser spec on Chromium and WebKit (pin from the menu shows the icon; after the pointer leaves the list
  the row moves to the top with the separator; unpin returns it). The browser spec must move the pointer off the list
  (or wait out the hold's stillness) before asserting positions.
- **Docs:** SPEC.md "Session list" (which today says "One flat list" and defines each order exactly) and the row menu
  description; SPEC_impl.md for the table and route; the docs page
  `website/src/content/docs/docs/using/session-list.mdx`, following `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md`.
- **Changelog fragment**, `kind: added`.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-15):** variant B from a mockup of three variants (heading + separator, icon + separator,
icon only); pinned rows in the list's chosen order; quick switcher unaffected; helm per-session storage with the schema
step accepted; Replace carries the pin over; review gate: one fresh-context gpt-6.1-sol reviewer at high effort, no
swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Session list" (flat list, defined orders, the reorder hold, the list served
whole with a per-host cap, "grouping and the key are authoritative helm and supervisor data, so every client agrees
without keeping a private rank"); SPEC.md's rule against per-client persistence of shared choices; SPEC.md "Upgrade
compatibility and client scale"; the changelog and docs-site rules in root `AGENTS.md`.

**Planner proposals** (reviewed by a fresh-context planning review): grouping in the sidebar rather than in the helm's
sort; a presence-only table; one toggle menu action; the route shape.

## What planning found

- No pin concept exists anywhere yet.
- The closest precedent is the per-session read/unread stamp: table `session_seen` and route
  `PUT /api/sessions/{id}/seen` (`crates/farhelm-helm/src/store.rs`, `sessions.rs` `mark_seen`, joined into list rows in
  `aggregate.rs`), shipped in #366 and #368; and the notification marks (#1621 helm side, #1622 UI side, which also
  touched the HTTP contract fixture `crates/farhelm-helm/http-contract/session-list.json`).
- Ordering is done on the helm (`aggregate::sort_rows`, three orders). The quick switcher
  (`crates/farhelm-ui/src/list/quick_switcher.rs`) fetches the same list in activity order and keeps the helm's order.
- The reorder hold is `rows::held_display_order` / `hold_should_release` in `crates/farhelm-ui/src/rows.rs`.
- Row menu: `MenuAction` and `MENU_ACTIONS` in `crates/farhelm-ui/src/list/row.rs`; `MarkSeen` is already one variant
  whose label follows the state.
- `sessions.rs::finish_replacement` deletes the replaced session and clears its seen state and notification marks
  itself, without going through `delete_session`.
- Every supervisor cuts its own list at the row cap before the helm sees it, so no ordering can save a pinned row a
  host's cap cut. SPEC already treats a fleet past the cap as out of scope; do not promise otherwise and add no
  mechanism for it.

## Outline

1. **Helm storage and route.** A presence-only table, `session_pins (session_id TEXT NOT NULL PRIMARY KEY) STRICT`,
   keyed by session id alone with no foreign key, for `session_seen`'s documented reasons (retarget and adoption keep
   the session); the next schema step, with the fresh-create DDL and the migration sharing one constant as the existing
   migration test requires. Accept the same bounded stray rows `session_seen` accepts (host removal, deletion through
   another helm); no reaper. Route `PUT /api/sessions/{id}/pin` with a required `{pinned: bool}`, mirroring `PUT /seen`:
   the same helm-local existence check as `mark_seen` (not routed to the supervisor), the event revision bumped only
   when the store reports a change. `delete_session` and `finish_replacement` clear the old id; `finish_replacement`
   pins the new id when the old one was pinned. The list rows gain `pinned: bool` from one batched read beside the seen
   and notification-mark reads, covering both row-building loops (cached hosts and hosts without an identity).
   `aggregate::sort_rows` is not changed. HTTP contract fixture updated.
2. **UI.** `Session.pinned` (serde default false). A pure function in `rows.rs` applies the stable pinned-first
   partition to the served order before the reorder hold, and a pure helper places the separator on the displayed order;
   both unit-tested. The pin is computed from helm data, so every client agrees, which is what SPEC's rule against a
   private rank protects; say so in SPEC. The quick switcher does not call the partition. Menu: one `TogglePin` (or
   similar) after `MarkSeen`. Icon, separator and accessible text as above, CSS with the existing tokens in
   `crates/farhelm-ui/assets/app.css`.
3. **Docs and spec**, fragment, TODO removal.
4. **PRs:** (1) helm storage, route, replace/delete handling, contract; (2) UI, SPEC, SPEC_impl, docs page, fragment,
   TODO removal. One PR is fine if it stays reviewable.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the affected helm and UI tests through the recorder (including the HTTP
contract tests on both sides and the migration tests), the JS unit harness if asset JS changes, the selected browser
specs on Chromium and WebKit through the recorder (with the builds they need), the test-sleep checker,
`python3 releasing/check-changelog.py format`, `dprint check`, and the docs website build.

## Fallbacks

Examples of departures that need the scope reassessment: sorting pins inside the helm (which would change the quick
switcher), a manual pin order, pin-related keyboard shortcuts, a reaper for stray pin rows, anything that makes pinned
rows survive a host's cap, or per-device pins.

Decisions to log: the icon glyph, the separator's exact styling, the route and table names, the PR split.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-pin-sessions-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/pin-sessions/<nn>-<short-name>` (for example `plan/pin-sessions/01-helm-pins`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat`. Add a changelog fragment (`kind: added`) per root
  `AGENTS.md`.
- The last PR removes the TODO.md entry this plan covers.
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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

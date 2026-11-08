# A notification bell in the rail, starting with trash that cannot be freed

Written against main at 5331b984 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's decisions change them, as listed below.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Show the user when the trash cannot finish freeing space.** With `instant-trash-delete`, a deleted checkout leaves
> the trash at once and the host removes its files in the background, retrying after restarts when it cannot (files
> owned by a container's root user, say). Decided on 2026-11-07 while planning it: for now such a folder only gets a
> logged warning, and nothing may get stuck or break because of it. Consider improving how visible this is to the user.

The maintainer's instruction on the planning picker, verbatim: "this is a good opportunity to add a non-session specific
notification system. let's add a generalized notification system as well. i suggest a notification bell in the
navigatino rail. same color as current items there by default, and then it turns very obviously read/alerting when there
is something to say. this would be the first thing to show up there." ("read" there means red.)

So this plan builds two things: a general place for notices that are not about one session (a bell in the navigation
rail), and its first use, telling the user when a trashed checkout's files could not be removed.

How it works today, from a read of main at 5331b984 (line numbers approximate; correct them to the truth as you go):

- **Background trash removal.** At supervisor start, `checkout_trash::resume_retirements`
  (`crates/farhelm-supervisor/src/checkout_trash.rs` around 800-848) queues every retired (`.rm:`) record, then one
  background remover works through them (`crates/farhelm-supervisor/src/service/trash_remover.rs` around 148-195,
  started from `service/core.rs` around 8619-8623). Each folder gets exactly one try per supervisor run
  (`remove_retired`, `checkout_trash.rs` around 851-893); a failed folder is never put back in the in-memory list
  (`trash_remover.rs` around 3-5). The only retry is the next supervisor start, which retries every retired record.
- **A failure** is only a log warning ("background trash removal failed; record retained until next startup",
  `trash_remover.rs` around 182 and 185; `checkout_trash.rs` around 842 and 887-892). Nothing keeps the error: the
  checkout's record stays marked trashed with its folder renamed to a `.rm:` name (`checkout_trash.rs` around 45-54).
  When removal succeeds, the record is dropped (`forget_retired`, around 880).
- **The trash listing** skips retired records (`checkout_trash.rs` around 387-389), so a folder that failed to free
  vanishes from the Trash dialog. The listing's `removing` flag (`crates/farhelm-proto/src/lib.rs` around 2756-2763, set
  from `core.rs` around 5990) reports only an active removal (`Remover::removing()`, `trash_remover.rs` around 72-74).
  The listing also carries `issues` (diagnostic notes that make totals "partial"), which reach the dialog
  (`list/trash.rs` around 511 and 1162).
- **Delete of a trashed checkout** already means "accepted for background removal" (`CheckoutTrashDeleted`,
  `farhelm-proto/src/lib.rs` around 2765-2772; the supervisor side around `core.rs` 5999-6032). Delete of a `.rm:` id
  today answers "no recorded archive with this id; nothing was touched" (`checkout_trash.rs` around 596-608).
- **The helm** keeps no trash state; it passes each host's listing through (`crates/farhelm-helm/src/sessions.rs` around
  525-542). Its 3-second session poll and the supervisor's content-free "sessions changed" hint
  (`farhelm-proto/src/lib.rs` around 312-328 and 3879-3899) carry no trash data; the helm's event feed carries only a
  revision number (`farhelm-helm/src/events.rs` around 1-40).
- **The Trash dialog** (`crates/farhelm-ui/src/list/trash.rs`) reads every connected host's listing whenever a host's
  (id, incarnation, connected) signature changes (around 607-672; the incarnation changes on every reconnect, so every
  supervisor restart, `farhelm-ui/src/lib.rs` around 793-813), polls every 3 seconds while a host reports `removing`
  (around 42-44, 673-729), and reads again on its own revision edges (`list/view.rs` around 972-974, 1870-1872). It
  shows a header with count and size, one section per host, per-entry buttons (`entry_target`, around 395), and an
  animated trash button while removal runs (around 985-1006).
- **The navigation rail**: host list at the top (`hosts.rs` around 2840-2900: "All", one avatar per host, "Add host"),
  footer at the bottom (`app_bar.rs` around 976-1022: Hosts toggle, Settings gear, Help menu, Collapse). Rail buttons
  are 40px in `var(--fg-1)`, `var(--bg-2)` on hover (`assets/app.css` around 722-748). The footer's height is a fixed
  168px for exactly four buttons (around 780), and the rail above it is offset by the same constant (around 656,
  `bottom: 168px`); both change together. Steel is the default palette; red tokens are `--danger`, `--danger-strong`,
  `--danger-bg` (around 280-288), and no literal colors are used at use sites (around 43-46).
- **Icons**: `crates/farhelm-ui/src/icons.rs` already has `BellIcon { loud }` for session rows (around 829-857), filled
  when loud and painted `--danger-strong` by CSS (`app.css` around 4829-4834), drawn in a 12-unit box; rail icons are
  16px.
- **Session bells**, the model the maintainer chose: SPEC.md "Status" (around 2450-2512). Entries are marked read when
  the list closes (around 2503-2504); resolved entries count as read and never make the bell loud (around 2502-2503);
  repeated checks while a problem stays unresolved do not repeat it (around 2477-2479). Their marks are watermarks over
  a history the supervisor keeps (`service/notifications.rs` around 1-15; helm `store.rs` around 504 and 542-551).
- **Other notification-like features**, none of which is this: release announcements (helm `announcements.rs`), the host
  attention list and the rail's amber host mark (`hosts/attention.rs`), the app update readout, ntfy waiting alerts.
  There are no toasts.
- **Specs:** SPEC.md "Managed checkouts" (around 1537-1560) says a failed removal logs a warning and is retried at the
  next startup. SPEC.md "Status" (around 2507-2510) and "Non-goals for v1" (around 4027) both say notifications about a
  host or the whole app are not part of v1. SPEC_impl.md "Archived-checkout trash" (around 4175, especially 4232-4245)
  matches the trash behavior; its session-notification storage is around 5857.

### Acceptance criteria

The bell:

- **Placement:** a bell button at the top of the rail's footer group, above the Hosts toggle, the same size as the other
  footer buttons, drawn with a 16px bell icon (a rail-sized variant of the existing bell, or a new icon drawn the same
  way). The footer's height and the rail's matching offset grow by one button.
- **Quiet look:** with nothing unread, the bell looks exactly like the other rail buttons (same color, same hover).
- **Alerting look:** with something unread, the bell is filled and uses the same `--danger-strong` treatment as the
  session bells, with a count of unread items on it. A browser test checks the computed fill in both the Steel and Steel
  Light palettes, and the report includes a screenshot of each state.
- **The panel:** clicking the bell opens a panel beside it listing the current items, newest first, each with a title
  naming its host, a one- or two-sentence explanation, and its actions. As with the session bells, items are marked read
  when the panel closes, which returns the bell to its quiet look. An item stays listed until its condition is fixed;
  then it shows as resolved (greyed) until the user clears it, and the panel has a way to clear resolved items. A
  resolved item counts as read and never makes the bell loud, so a folder that fails and is freed before anyone looks
  leaves the bell quiet. The panel is keyboard reachable and closes on Escape and outside click, like the app's other
  menus.
- **No re-alerting:** a notice that stays unresolved does not become unread again, whether the folder fails again at a
  supervisor restart or after a Retry; its reason and time update in place. This matches the session bells.
- **Shared state:** notices and their read and cleared state live in the helm, so the desktop app and a browser tab
  agree.
- **Built to grow:** a notice has a kind with its own title, text and actions; adding another kind later must not
  require reworking the bell. Only the stuck-trash kind ships in this plan. A kind enum with one variant, rendered by a
  match, is enough; do not build a registry.

The stuck-trash notice:

- **When:** after the first failed removal of a trashed checkout's files. Each folder gets one try per supervisor start,
  so one failure means it is stuck until a retry.
- **What the host reports:** the supervisor remembers each failed removal's short reason and time in memory, and its
  trash listing reports retired folders instead of hiding them, each marked as stuck (with reason and time) or as being
  removed now. Every supervisor start retries every retired folder, so a failure always recurs in the run that reports
  it; nothing about failures needs to be stored on disk.
- **What it says:** which host, which checkout, how much space it holds if known, and the reason in plain terms (for
  example "permission denied on files owned by another user, often a container's root user"), plus that Farhelm tries
  again when the host's supervisor restarts.
- **Actions:** "Retry now", which asks the host to try removing that folder again at once, and "Open Trash", which opens
  the Trash dialog.
- **The Trash dialog** keeps a stuck folder listed in its host's section as an ordinary trash entry, marked as could not
  be freed with the reason, and its per-entry button offers Retry. Its size counts toward the totals like any other
  entry; it does not make the totals partial.
- **Identity and resolution:** a notice is identified by (host, checkout), never by failure time. It resolves only when
  a successful listing from a supervisor that reports retired folders no longer contains that checkout. A folder being
  removed right now does not resolve it; a failed read, an unreachable host or an older host never resolves anything.
  While a host is unreachable its notices keep their last known state; when a host entry is removed, its notices go with
  it.
- **Older hosts** never list retired folders, so they never raise the notice and are never offered Retry. A decode test
  of a listing without the new fields, plus `serde(default)` on them, is the evidence for SPEC.md's "Upgrade
  compatibility and client scale"; no harness runs an older supervisor binary.

Everything else:

- **Specs:** SPEC.md "Status" and "Non-goals for v1" stop saying host- and app-level notifications are out of v1 and
  describe the bell; "Managed checkouts" describes the stuck listing, the notice and Retry; SPEC_impl.md covers the
  in-memory failure record, how the helm records notices from the listings it forwards, and their storage.
- **Docs:** the user docs pages that describe the trash and the rail (find them under `website/src/content/docs/`)
  describe the bell and the stuck-trash notice; follow `website/AGENTS.md` (an unattended run starts no preview server
  and names the changed pages).
- **Tests:** supervisor tests that a failed removal is listed as stuck with its reason, that Retry re-attempts it, and
  that success drops it (a folder made unremovable with a read-only directory, which the removal never changes; note in
  the report that this does not work when tests run as root); helm tests for recording, updating without re-alerting,
  resolving, read and clear; UI unit tests for the bell's quiet, alerting and resolved states; browser tests on Chromium
  and WebKit for the bell, the panel, Retry, Open Trash and the Trash row.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: added`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08 on the planning page, which showed a mockup of the rail (quiet bell, alerting
bell with a count, the open panel with one stuck-trash item offering "Retry now" and "Open Trash", and a Trash dialog
section with the stuck folder listed and marked in red):

- **Scope:** only the stuck-trash notice goes in the bell in this plan, built so other host- and app-level notices can
  be added later. Host setup and update problems stay with the amber attention mark, and announcements stay as they are.
- **Placement:** top of the rail's footer group, above Hosts.
- **Alerting look:** a filled red bell with a count. Rejected: no count; a gentle pulse.
- **Read model:** the bell goes quiet once the user has opened it and seen the item; the item stays listed until the
  problem is fixed, then shows as resolved until cleared, like the session bells. Rejected: staying red until the
  problem is fixed. Following "like the session bells", items are marked read when the panel closes, resolved items
  never make the bell loud, and an unresolved notice does not re-alert.
- **Actions:** "Retry now" and "Open Trash" on the notice, and the Trash dialog lists the stuck folder, marked, with
  Retry. Rejected: no Retry; leaving the Trash dialog as it is.
- **Timing:** the notice appears after the first failed try. Rejected: after a second failure.
- **Stated on the page and not objected to** (planner proposals the maintainer saw): read and cleared state lives in the
  helm; an older host never raises the notice; the spec's "no host- or app-level notifications in v1" lines change to
  describe the bell.
- **Changed after the page, by the planner (the maintainer reviews it in the planning PR):** the page said the host
  saves the failure's reason and time with the trashed checkout. The planning review showed that is unnecessary, because
  every supervisor start retries every stuck folder, so a failure is always re-observed in the run that reports it, and
  only the helm can remember a resolved notice anyway. The supervisor keeps failures in memory instead, which avoids a
  database migration. Nothing the user sees changes.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's picker instruction quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md "Managed checkouts", "Status", "Non-goals for v1" (changed by this plan as
listed), "Upgrade compatibility and client scale" for every protocol addition; SPEC_impl.md "Archived-checkout trash";
the instant-trash-delete rule that nothing may get stuck or break because of a folder that cannot be freed; root
`AGENTS.md`, "Code rules", "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `docs/desktop-web-triage.md`; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and
`scripts/check-test-sleeps.py`; `website/AGENTS.md` for docs edits.

**Planner proposals, from the planning review** (change them if the code says otherwise, logged as a DECISION):

- **Supervisor:** an in-memory map from checkout id to (reason, time) on the remover, filled from its failure arm and
  from `resume_retirements`' failure arm. The trash listing reports retired records as ordinary `ArchivedCheckout`
  entries with an optional stuck field (reason and time; `serde(default)`, skipped when empty), not as listing issues,
  which would make totals partial. Make `Remover::removing()` true while its queue is non-empty, not only while a
  removal is active: today a listing served between the startup enqueue and the worker's first pick says "not removing",
  so the UI never starts its poll and misses that run's failures.
- **Retry reuses the existing trash Delete request:** a Delete of a stuck retired id re-queues it and answers as an
  accepted background removal, instead of "no recorded archive". No new request type, client method, helm route or UI
  API function; the UI's existing poll while removing delivers the outcome.
- **Helm:** the helm records notices from the trash listings it already forwards (`sessions::list_checkout_trash` and
  the Delete reply path), in one helm.db table whose rows cascade when the host entry is deleted, the way session tables
  do. Each row holds the host id, kind, key (the checkout id), a content snapshot (checkout name, repository, size if
  known, reason, failure time), raised, resolved, read and cleared state. Add one list endpoint and one endpoint for
  marking read and clearing resolved. Bump the fleet event revision only when the stored notices actually change, so the
  other clients' bells refresh without a feedback loop. No new channel, no new polling, no change to the sessions hint:
  the Trash dialog's existing reads (on every host reconnect, which covers each supervisor restart, and every 3 seconds
  while a host is removing, which covers Delete and Retry) reach every moment a failure can happen. A failure then
  reaches the bell within about 3 seconds while a client is open, otherwise at the next app load. Check that the Trash
  dialog's reading component is mounted in every layout the app can be in, and log what you found.

## Outline

A stack of about four PRs; reshape it if the code argues for another cut, keeping each PR working on its own:

1. `plan/rail-notification-bell/01-stuck-trash-listing`: the supervisor's in-memory failure record, retired folders in
   the listing with their stuck field, Retry through Delete, the `removing()` fix; protocol fields added compatibly;
   Rust tests.
2. `plan/rail-notification-bell/02-trash-stuck-row`: the Trash dialog's stuck row with Retry; UI and browser tests.
3. `plan/rail-notification-bell/03-helm-notices`: the helm's notice table, recording from forwarded listings, the
   endpoints and the revision bump; helm tests.
4. `plan/rail-notification-bell/04-rail-bell`: the bell in the rail footer, its looks and the panel with Retry now and
   Open Trash; UI unit tests and browser tests on both engines.

Then SPEC.md, SPEC_impl.md, the docs pages and the changelog fragment, either folded into the PRs whose behavior they
describe or as a final `05-bell-docs` PR, with the TODO.md removal in the last code PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
supervisor, helm and UI unit tests touched, the e2e selections for the checkout trash through
`python3 scripts/record-test-run.py` with the pinned nextest and tmux setup, `python -B scripts/check-test-sleeps.py`
with the interpreter `docs/test-sleep-check.md` prepares, the browser specs for the trash dialog, the rail and the new
bell on both engines, and the website build (`cd website && bun install --frozen-lockfile && bun run build`) for the
docs changes.

## Fallbacks

Examples of departures that need the scope reassessment: a new push channel from supervisor to helm, a new polling loop,
storing failures in the supervisor's database, toasts or desktop notifications, moving any existing notice (host
attention, announcements, session bells) into the bell, retrying failed removals automatically on a timer, deleting
files as another user or with elevated rights, or a settings page for notifications.

Decisions to log: the shape of the stuck field, how Retry rides on Delete, the notice table and its endpoints, how and
when the helm records and resolves notices, whether the Trash dialog's reader is always mounted, every test changed and
why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-rail-notification-bell-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/rail-notification-bell/<nn>-<short-name>`, as the outline names them.
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

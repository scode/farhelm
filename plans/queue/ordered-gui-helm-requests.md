# Order each GUI's requests after its own saves

Written against main at 5de6290c on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Every save, read and action the GUI (browser or desktop app) sends to the helm is an independent HTTP request, and the
helm serves them concurrently, so a request the user started after a save can be answered as if the save had not
happened. Two review findings showed this and were discarded in triage on 2026-10-14 as too rare to fix one by one; read
both entries in `TRIAGE_OUTCOMES.md` (`quick-launch-acknowledgment-revives-previous-mark.md` and
`pending-command-save-overwrites-newer-save.md`). The TODO.md entry (Near term, "Consider an ordered transport between
the GUI and the helm") asked for an ordering mechanism instead. Build it inside the GUI.

### Acceptance criteria

- **One rule, in one place:** a dependent request from a GUI waits until every tracked save this GUI started before it
  has been answered (success, failure, timeout or cancellation). Implemented in the GUI's request funnel
  (`send`/`send_read`/`send_within` → `send_inner` in `crates/farhelm-ui/src/api.rs`), not per call site.
- **Not one request at a time.** In the maintainer's words, not "a synchronous one request at a time, with them having
  to wait for round trips". Saves to different settings go out in parallel; saves to the same setting keep their
  existing latest-wins behavior. A GUI with no tracked save pending never waits. Reads and actions run concurrently with
  each other.
- **Tracked saves:** configuration saves that a later request reads or that the helm applies at launch: the quick-launch
  mark and the number-key template shortcuts (preference fields), template save and delete, checkout root, post-clone
  command, and ntfy settings. Not tracked: read/unread and notification marks and the list-view preferences (order,
  selected session, compact rows), because no read depends on them in a way ordering fixes, and tracking them would put
  a round trip in front of every session switch; not host settings (alias, ssh destination), which wait on host
  operations in the helm and would hold everything up. The maintainer chose this scope: "this is precisely the concern
  that I had about not adding a round trip for everything. Yes, configuration saves only."
- **Dependent requests:** every other request through the funnel: reads of helm state, reads that ask a host, and
  actions, explicitly including session create (the helm fixes the checkout root and post-clone command when it accepts
  a create). Not affected at all: terminals and the event feed (their own WebSockets), and the page JavaScript's own
  HTTP calls (attachments, file stat and download, client log, clipboard).
- **Registration is synchronous.** A tracked save registers at the moment the user's action starts it, before any await,
  so a request started afterwards always sees it. Preference saves already record synchronously
  (`PreferenceWrites::record`); the async save functions (`save_checkout_post_clone`, `save_checkout_root`,
  `put_template`, template delete, `save_ntfy_settings`) do not, because their bodies first run when their task is
  polled. The declaring function (for example a `send_save` beside `send` and `send_read`) is a plain function that
  registers and then returns a future (the `preference_write_completion` `impl Future + use<>` pattern).
- **No deadlocks, no starvation:**
  - "Pending" means a request in flight or queued behind one, never "not yet acknowledged": a failed preference write
    stays dirty and is not retried, and the reseed after re-authentication would otherwise wait for a save that only
    restarts after that read returns. Release waiters on `writer_lost` and on a failed finish.
  - Release on cancellation as well as completion: template saves run in tasks the Templates dialog owns and closing it
    drops them mid-request. Use a guard released on drop, as `WriterGuard`/`SeenWriterGuard` do.
  - A waiter captures the sequence numbers of the saves registered before it and waits only for those, so a stream of
    newer values to the same setting cannot starve it.
  - The wait counts against each request's own deadline, measured from entry into the funnel (10 s reads, 5 s for the
    preference seed, 60 s for remote work); a request whose deadline passes while waiting fails as a timeout. Otherwise
    one hung save would hold every read for over a minute, undoing the read timeout split recorded in
    `lore/2026-09-16-rotation-recovery-unanswered-reads.md` and the seed timeout that keeps the page from blanking.
- **Both findings are fixed, with tests:** quick launch (mark B while the save is held, press the shortcut, B launches)
  and the post-clone command (save A while held, close and reopen Settings, save B; B wins and the reopened field shows
  A until B is saved). The post-clone case is closed by the rule alone: the reopened field's read waits for A, and the
  field stays locked until that read lands. Add no second writer for it.
- **A lost reply** (client timeout) releases waiters; later requests may then run while the helm still applies the save.
  That is the same unknown outcome as today; document it, add no machinery for it.
- **No helm change and no protocol change.** It works against any helm version.
- **Docs:** SPEC_impl.md states the ordering rule, what is tracked and dependent and why, the deadlock and deadline
  rules, and the lost-reply limit; check SPEC.md for contrary text (its preference and quick-launch text, SPEC_impl.md's
  "A pending or failed mark write cannot silently launch the previous mark"). No docs-site change unless a page
  describes this.
- **Changelog fragment**, `kind: fixed`.
- The last PR removes the TODO.md entry. (The planning PR already added the Maybe later entry about reconsidering an
  ordered WebSocket channel; do not add or edit it.)

## Requirement sources

**The user's request and decisions (2026-10-15):** the TODO entry; rejection of synchronous one-at-a-time sending; after
an architecture overview and a deep dive comparing three options (this one at roughly 150-300 lines plus queue work;
helm-enforced sequence numbers at roughly 450-600; an ordered WebSocket channel at roughly 5,000-8,000 lines with a
lasting second transport and 461 browser-test interceptions to migrate), the maintainer chose this one and asked for the
WebSocket idea to be kept as a Maybe later TODO entry; large transfers are never ordered. Review gate: one fresh-context
gpt-6.1-sol reviewer at high effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md and SPEC_impl.md on preferences (sparse per-field patches, same-field writes
serialized latest-wins, replay after re-authentication), quick launch, templates ("no optimistic-concurrency check on
template writes is wanted"), and cross-client last-writer-wins; the timeouts in `api.rs`; root `AGENTS.md` test rules
and `.agents/test-authoring.md`.

**Planner proposals** (reviewed by a fresh-context planning review): the tracked/dependent split, the sequence-number
waiter, the guard-based release, and the test shapes.

## What planning found

- The funnel and its timeouts: `api.rs` (`REQUEST_TIMEOUT` 60 s, `READ_TIMEOUT` 10 s, `PREFERENCE_SEED_TIMEOUT` 5 s).
- `PreferenceWrites` (`api.rs`): per-field slots, at most one PUT in flight per field, latest wins, replay of
  unconfirmed values after re-authentication; `preference_write_completion`; `template_bindings_after_read`;
  `prepare_template_delete` waits for binding writes. `SeenWrites` is the same pattern per session.
- Quick launch: the `quick_launch` callback in `crates/farhelm-ui/src/list/view.rs` reads preferences, then templates,
  then applies `template_bindings_after_read`, which overlays only still-dirty bindings; the mark is saved from
  `list/templates.rs`.
- Post-clone command: `CheckoutPostCloneField` in `crates/farhelm-ui/src/checkout_post_clone.rs` keeps its `saving` lock
  in component state; `checkout_root.rs` has the same shape.
- Helm save handlers for preferences, templates, checkout config and ntfy commit, then bump the event feed, then reply;
  none waits on a GUI read, so there is no helm-side deadlock.

## Outline

1. **Tracker and funnel.** A process-wide save tracker in `api.rs` (sequence numbers, in-flight set, drop guards), a
   declaring function for tracked saves, and the wait in the funnel for every other request, inside its deadline. Wire
   `PreferenceWrites`'s binding fields to it using the queue's own in-flight state (no removal of the same-field
   hold-back). Unit tests: no wait when idle; a read waits for an earlier save and not a later one; failure, timeout and
   drop release waiters; a stream of saves cannot starve a waiter; the deadline includes the wait.
2. **Call sites and findings.** Declare the tracked saves; the two finding tests (unit where possible; a browser test
   holding the save with a route interception where the UI wiring is the point, on Chromium and WebKit through the
   recorder); SPEC_impl.md; fragment; TODO removal.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop` (the funnel compiles for
desktop too), the UI's unit tests and desktop tests through the recorder, the browser specs touching quick launch,
templates, Settings and preferences on Chromium and WebKit through the recorder (with the builds they need), the
test-sleep checker, `python3 releasing/check-changelog.py format` and `dprint check`. This touches every GUI request, so
also run the broader browser specs that exercise reconnect and re-authentication.

## Fallbacks

- If a tracked save's helm handler turns out to reply before its write commits, ordering cannot help it: make the
  handler reply after the commit only if that is a small, local change, otherwise leave that save untracked and say so
  in the report.
- Examples of departures that need the scope reassessment: any helm or protocol change, sequence numbers on the wire, a
  WebSocket transport, tracking seen marks or list-view preferences (unless Decisions says so), a second writer for the
  post-clone command, retries or replay of lost saves.

Decisions to log: the final tracked list, the tracker's API, how the binding fields hook in, and the test shapes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-ordered-gui-helm-requests-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/ordered-gui-helm-requests/<nn>-<short-name>` (for example `plan/ordered-gui-helm-requests/01-save-tracker`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `fix`. Add a changelog fragment (`kind: fixed`) per root
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

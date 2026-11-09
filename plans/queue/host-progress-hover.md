# One hover card for a running host's progress

Written against main at 8e876c14 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit", where it landed by accident: the maintainer added it to Near
term, and a heading rename minutes later put it first in the new bucket), verbatim:

> **Show the host dialog's detail on hover over the sidebar's progress for a running host.** The line shows the step
> count and a truncated step name, and the host dialog shows the numbered steps and the run's own message (the failed
> connection attempt's error, say). Hovering the line lists the steps but leaves out the run message, and the bar the
> sidebar shows at the top while the hosts panel is closed has no hover detail at all. Both should give the same
> information as the dialog: every step with its status, the step in progress, and the run's message.

Three things in the entry do not match the code, and the maintainer settled them while planning (see Decisions already
made):

- There is no host dialog with steps. The numbered steps and the run message are drawn by `ProvisioningPanel` in the
  host's expanded row in the Hosts panel (`crates/farhelm-ui/src/provisioning.rs` around 4160-4235: the
  `provisioning-steps` list and `provisioning-run-message`).
- A running run has no run message. The helm sets `ProvisioningView.message` only when a run ends (failure, use-as-is
  completion or recovery, degraded completion: `crates/farhelm-helm/src/provisioning/service.rs` around 2462-2668,
  `tensorlake_add.rs` around 525); a new run starts with `message: None` (`http.rs` around 205).
- "The failed connection attempt's error" is the host's connection detail, not a run message: `state_detail`
  (`crates/farhelm-ui/src/hosts.rs` around 973-1010) renders `HostPhase::Connecting` as "Attempt N: waiting for the next
  connection attempt…; the last one failed: <error>".

What is true today, from a read of main at 8e876c14 (line numbers approximate):

- Hosts panel open: `UpdateProgressLabel` (`hosts.rs` around 3984-4062) shows a dot, the verb (not for Update),
  `done/total`, the truncated current step name and the elapsed time. Hovering opens `UpdateProgressPopup` (around
  4179-4278): a one-off, pointer-only, `aria-hidden` popup that measures its own placement and lists the count, the
  elapsed time and every step with its status, keeping the current step scrolled into view. It gets its data from
  `UpdateProgressSummary` (`provisioning.rs` around 601-618, built by `summarize_running_view` around 859-906), which
  never copies the run message.
- Hosts panel closed: the attention list (`crates/farhelm-ui/src/hosts/attention_list.rs`, `AttentionHostRow` around
  249-324) shows a working row as the host, the verb ("Updating", "Setting up", "Uninstalling") and a `<progress>` bar
  whose plain tooltip reads "{done} of {total} steps finished"; the identity button's tooltip reads "{identity}:
  {words}; open Hosts for details". The row keeps only `progress: Option<(usize, usize)>`.
- The shared tooltip (`crates/farhelm-ui/assets/tooltip.js`) supports cards through `data-tooltip-card`, naming a
  hidden, Rust-rendered source element that it clones (never parses), with the 300 ms delay, keyboard-focus display, and
  no display on touch. Its only card user today is the session row (`crates/farhelm-ui/src/list/row.rs` around 1780;
  styles `.farhelm-tooltip[data-kind="card"]` and `.session-summary*` in `app.css` around 1915-1990).
- SPEC.md around 133-145 specifies the inline line and its hover (count, elapsed, every step with its status, current
  step kept in view; no message); around 1962-1964 and 2377-2382 the attention list's working rows (verb and progress
  bar, no hover content specified); around 2333 the general hover rule.
- Tests: `e2e/tests/provisioning.spec.ts` (the popup following the current step in a short window around 1069, using
  `configureBackend({hold_actions: true})` and a routed progress reply with 16 steps; current step and fonts around
  2762), `e2e/tests/tensorlake-hosts.spec.ts` around 860 (a step explanation in the popup), and
  `e2e/tests/host-updates.spec.ts` around 88-175 and 426-429 (a faked 3-step run and the working row). A faked reply can
  carry a run `message`. No test covers hovering the attention row.

### Acceptance criteria

- One hover card, built as a shared Rust component and shown through the shared tooltip's card mechanism, replaces
  `UpdateProgressPopup` on the Hosts panel's progress line and is added to the attention list's working rows. Both show
  the same content for the same host.
- The card holds, in this order:
  - a header with the host's mark and name, and "<Verb>: N of M steps done" with the elapsed time;
  - every step with its status, in the `provisioning-steps` style, with the current step marked, each step's explanation
    (as today's popup shows), and a degraded or failed step's message;
  - a **Connection** section with the `state_detail` text whenever the host's phase is one `state_detail` describes
    other than connected (in practice `HostPhase::Connecting` with its attempt and last error), read from the host at
    render time rather than copied onto the progress summary;
  - the run's own message whenever the run has one (carried onto the summary, which today drops it). Since the helm sets
    it only when a run ends, this is nearly always empty while the card is shown; carry it as decided and test it in
    Rust, not with a browser test.
- Keeping the current step in view: the shared tooltip clones the card's source on every change (the elapsed clock
  changes it every second), and every clone starts scrolled to the top, so Rust-side scrolling like today's popup's
  cannot work. Add a small opt-in to `crates/farhelm-ui/assets/tooltip.js`: after cloning a card, scroll any list in it
  marked `data-tooltip-follow` so its `[data-current]` item is centered. Session cards carry no marker and do not
  change. The card's layout lets only the step list shrink when the window is short, as today's popup does.
- Placement: the card uses the tooltip's existing beside-the-sidebar placement for a row target, as the mockup showed;
  SPEC.md's general hover rule (around 2333) is reworded if it says hovers sit above their control.
- Card sources render beside the rows (as the session card's source does), not inside them, with distinct ids per
  surface, since the Hosts panel stays mounted while closed and both sources exist at once; row-scoped selectors in
  existing tests (`host-updates.spec.ts` around 517, `provisioning.spec.ts` around 1382, `tensorlake-hosts.spec.ts`
  around 855) must not start matching the hidden copies.
- Opening: on hover after the shared delay, and not on touch. On the attention list, the working row's identity button
  carries the card (so keyboard focus opens it there too, and the tooltip's hover-coverage rule holds); the host mark's
  and the identity button's own plain tooltips go on working rows, as does the bar's "N of M steps finished". On the
  Hosts panel's progress line, hover only: do not add a new tab stop to a status label.
- An attention working row names the current step after the verb ("Updating · Restart the supervisor"), ellipsized like
  the Hosts line. The step name stays out of the attention list's layout key, so a step change does not count as a
  layout change that closes session menus.
- SPEC.md's passages around 133-145 and 2377-2382 say what the card shows in both places; SPEC_impl.md around 1328,
  which describes the old popup, is updated.
- Browser tests on the engines `.agents/validation.md` names, extending `provisioning.spec.ts`, `host-updates.spec.ts`
  and `tooltip.spec.ts`: the card appears on hover in both places with the same steps and explanations; the Connection
  section appears for a connecting host and not for a connected one; keyboard focus on the attention row opens it; the
  current step stays in view in a short window and follows a step change while open (the existing test, moved to the
  card); session cards are unaffected by the follow opt-in. The tooltip script's own unit tests cover the opt-in.
- A `feat` PR with a changelog fragment. The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page:

- **Which message:** the host's connection detail while it is not connected, plus the run's own message when there is
  one. The alternative, the run's message alone, was not taken because it rarely exists during a run.
- **One shared hover card in both places**, replacing today's one-off popup, rather than extending the popup and copying
  it to the attention list.
- **The current step's name in the attention row**, as the mockup showed it.
- **The mockup the maintainer approved** showed the card beside the attention row, the row ringed as the tooltip target:
  the host mark and bold name, then "Updating: 4 of 6 steps done" with "0:42 elapsed" right-aligned on the same line, a
  divider, the numbered steps with done checkmarks and statuses right-aligned, the running step highlighted, and a
  divider before a "Connection" heading over the detail text in the warning color, with the SSH error in the mono font.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold** at the maintainer's request; that changes nothing about how it runs.

## Requirement sources

**The user's request:** the TODO.md entry, read with the corrections above, and the decisions above.

**Binding repository constraints:** SPEC.md's passages named above; the tooltip contract in `tooltip.js`'s header (peer
text through `display_peer`, card sources cloned as DOM nodes); root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`.

**Planner proposals:** carrying the run message on the progress summary and reading the connection detail from the host
at render time, rather than a new request; building the card from one component that both the Hosts line and the
attention row render as a hidden source beside their rows; the `data-tooltip-follow` opt-in in `tooltip.js`; dropping
the bar's and the identity button's plain tooltips on working rows; no new tab stop on the Hosts line.

## Outline

1. `plan/host-progress-hover/01-progress-card`, typed `feat`: the shared card component and its data (the run message
   reaching the summary, the connection detail), the `tooltip.js` follow opt-in, the Hosts line switched to it, the old
   popup and its placement code removed, SPEC.md, the existing popup tests moved to the card, and a changelog fragment.
2. `plan/host-progress-hover/02-attention-card`, typed `feat`: the attention row's hover card, the current step in the
   row's text, the bar tooltip removed, SPEC.md, the attention-row tests, the first PR's fragment edited to cover both
   (root `AGENTS.md`, "Changelog fragments": an unreleased follow-up edits that fragment), and the TODO.md removal.

One PR is also fine if the second is small; do not split further.

### Validation

`e2e/tests/provisioning.spec.ts`, `e2e/tests/host-updates.spec.ts`, `e2e/tests/tensorlake-hosts.spec.ts` (the popup
cases) and `e2e/tests/tooltip.spec.ts` on the engines `.agents/validation.md` names, through
`scripts/record-test-run.py`; the tooltip script's unit tests (find them through `.agents/validation.md`'s inventory);
the UI crate's Rust tests for what changed; `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

## Fallbacks

Examples of departures that need the scope reassessment: having the helm set a run message while a run is in progress,
adding new host or run data to the protocol, changing the shared tooltip's behavior for session cards beyond the opt-in
above, making the card interactive (scrollable by the user), or redesigning the attention list.

Decisions to log: where the card's data comes from, how the current step is kept in view inside the tooltip, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-progress-hover-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/host-progress-hover/<nn>-<short-name>`,
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-11-09: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-09 (claim 7be251).
>
> ### Long host diagnostics cannot fit beside a readable current step
>
> This plan gives the Hosts progress line and the sidebar's working host row the same hover card: every step and its
> status, the current step, and connection detail including the last SSH failure. The recorded decision includes the
> run's own message when present. The card stays non-interactive, and the current step must remain visible in a short
> window.
>
> The shared card is implemented, but review found an overflow conflict. A normal composed SSH error can be longer than
> a 300-pixel-high window. Keeping that diagnosis fixed while shrinking only the steps can hide the current step and
> clip the error's tail. Browser checks reproduced current-step clipping on Chromium and WebKit while an update was
> running and its host was reconnecting. Both the required code reviewer and an independent scope review found that the
> existing plain-tooltip line limit does not authorize truncating this card.
>
> How should the card handle connection detail or a run message that cannot fit alongside a readable current step?
>
> - Keep the card non-interactive and show a bounded diagnostic preview, visibly saying that full details are in Hosts.
>   Preserve space for the current step. Clicking the existing host identity still opens Hosts, where the full diagnosis
>   is available. This is the smallest change, but a user may have to open Hosts to read the important tail of a long
>   error; it is an explicit exception to the full-detail-on-hover requirement.
> - Allow a scrollable diagnostic region inside the card so the whole message can be read there while the current step
>   stays visible. This preserves access to the text on hover but changes the agreed non-interactive behavior, including
>   pointer ownership, dismissal and keyboard access, and needs more implementation and testing.
>
> Recommendation: the bounded preview with a visible full-details indication. No preview, extra interaction or new
> overflow mechanism has been implemented without this decision. The draft work remains incomplete and must not land
> until this choice is recorded and its behavior is implemented and reviewed.

The maintainer's answer:

The landing monitor answered this on the maintainer's behalf on 2026-11-09, under `plans/.agents/landing.md`, "Answering
blocks"; a later change of mind is a follow-up, not a reason to stop the plan.

Take the first option. The card stays non-interactive. When the connection detail or the run's message cannot fit beside
a readable current step, the card shows a bounded preview of that text and says visibly that the full details are in
Hosts; the current step always keeps its space. Clicking the host identity still opens Hosts, where the whole diagnosis
is readable. Record this in the specification as a narrow exception to the full-detail-on-hover requirement: only text
that does not fit is cut, and a message that fits is shown whole as before. A scrollable region inside the card was not
taken, because it would make the card interactive (pointer ownership, dismissal, keyboard access) against the decision
that it stays non-interactive, and would need more implementation and testing for a rare long error.

### 2026-11-09: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-09 (claim 9f0cb9).
>
> ### A new progress-card test fails on the cloud machines used for landing checks
>
> The landing of this plan stopped before merging anything. Main holds none of #3272.
>
> The plan gives the Hosts progress line and the sidebar's working host row one shared hover card. Its new browser test
> `host progress previews only diagnostics that cannot fit beside its current step` (in
> `e2e/tests/host-updates.spec.ts`) gives a working host a long connection error and shrinks the window to 300 pixels
> high. It then hovers the host's identity button in the sidebar and expects the card, with its "Full details in Hosts"
> preview. The landing ran the browser tests on Tensorlake sandboxes, cloud Linux machines the maintainer asked the
> landing monitor to use on 2026-11-09 (not Tensorlake hosts in the product). There the test fails on the plan as
> rebased onto current main:
>
> - In the landing's run of the host update, tooltip, provisioning, Tensorlake-hosts, rail-warning and request-card
>   specs, it was the only failure: Chromium, the card never appeared after the hover (run `197c18fb`). Everything else
>   passed, WebKit's copy of this test included.
> - Run alone ten times per engine on sandboxes, it failed 10 of 10 on Chromium (run `9640bd72`) and 2 of 10 on WebKit
>   (run `589e5217`). On Chromium the card was not found after the first hover at 300 pixels (line 608) in 9 runs; in
>   the other failures the card was found but then hidden or replaced (line 627 read the card's pointer style as empty,
>   and line 639 found it hidden after the window was made short again).
> - On the landing machine the same build passed it 6 of 6 on both engines (run `96062ef9`).
>
> So the result depends on the environment. The sandboxes run the same Playwright browser builds from the same packed
> checkout, but on a different machine shape (likely fewer CPUs, possibly other installed fonts) with a slightly older
> Node; the card is also rebuilt every second by its elapsed-time clock, which the executor's earlier report already
> found could replace a card mid-check. Either the test relies on timing or geometry that only holds on some machines,
> or the card really fails to show (or is replaced while hovered) in a window this short under some font or layout
> conditions. A user with a short window could then see no card at all.
>
> How should this go forward? The options are to send the plan back to find which it is and fix it (the test, the card,
> or both), keeping the requirement that a short window still shows the card with a readable current step; or to land it
> as it is and record the failure as a known problem. Failing 10 of 10 on Chromium is not a flake: every later sandbox
> browser run would fail on it unless it were skipped there. Recommendation: send it back, since the test is the plan's
> own evidence for the overflow decision and it currently fails on a common machine. The executor has no sandbox access
> of its own; it can look for the cause locally (CPU throttling, a font change, holding the clock), and the landing
> monitor reruns the test on sandboxes when it lands the plan again.

The maintainer's answer:

The landing monitor answered this on the maintainer's behalf on 2026-11-09, under `plans/.agents/landing.md`, "Answering
blocks"; a later change of mind is a follow-up, not a reason to stop the plan.

Send it back to be fixed, as recommended. Find out whether the long-diagnosis test fails because of the test (timing or
geometry it assumes, such as the card being rebuilt by its elapsed-time clock while the test reads it) or because the
card really does not show, or is replaced while hovered, in a 300-pixel window under some conditions. Fix the card if it
is the card, the test if it is the test, and keep the requirement: a short window still shows the card with a readable
current step and the "Full details in Hosts" preview. Look for the cause on this machine (for example under CPU
throttling, with another font, or with the clock held). A test made robust must still observe the visible card and the
readable current step, not merely stop checking them. The landing monitor will run the test again on Tensorlake
sandboxes, on both engines, when it lands the plan.

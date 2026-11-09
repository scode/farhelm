# The Settings dialog keeps one size across sections

Written against main at e7b47815 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Stop the Settings dialog resizing when the section changes.** The dialog takes the height of whichever section is
> showing, so moving between Sessions, Appearance, Credentials and the rest makes the whole modal jump and the footer's
> Close button move with it. Fix is TBD; keep the dialog one size across sections.

What is true today, from a read of main at e7b47815 (line numbers approximate; correct them to the truth as you go):

- **The component:** `crates/farhelm-ui/src/settings.rs`. Sections in `SETTINGS_SECTIONS` around 62-73 (Sessions,
  Appearance, Hosts, Tensorlake, Credentials, Terminal, Checkouts, Agent trust, Notifications, App; App only with the
  desktop updater). The `ModalDialog` (classes `app-settings-dialog host-dialog-framed`) holds a header, a
  `div.dialog-body.host-settings-body` with the section navigation and `div.app-settings-content`, and a `DialogFooter`
  with the Close button. Switching sections only toggles `hidden` on panels that all stay mounted.
- **The CSS** (`crates/farhelm-ui/assets/app.css`): the shared dialog card sets `width: min(480px, 100%)`,
  `max-height: calc(100dvh - 2 * var(--dialog-vertical-margin))` and no height, so the card is as tall as its content;
  the backdrop centers it vertically, so a height change moves both header and footer. `.host-dialog-framed` makes the
  header and footer fixed and the body the only scroller (`flex-shrink: 1; min-height: 0; overflow: auto`). The Settings
  rules (around 3953-3996) fix the width at `min(760px, 100%)` and lay the body out as a two-column grid with a sticky
  navigation. At 620px wide or less (around 4032-4038) the body is one column showing either the section list or one
  section. The Tensorlake and Credentials editors have sticky bottom action bars that need the body to stay the scroll
  container.
- **Section heights:** Sessions, Appearance, Hosts and App are short; Terminal, Checkouts and Notifications medium;
  Agent trust long; Tensorlake and Credentials grow with the user's recipes and credentials.
- **Tests:** `e2e/tests/settings.spec.ts` around 869 ("pinned around its grouped scrolling body") compares header and
  footer boxes across body scrolling at 1280 and 390 wide, 560 tall; around 925-932 it opens Agent trust in a 1280x5000
  window and asserts the body does not scroll; around 945 a test steps through every section and the narrow-screen back
  path. Helpers in `e2e/tests/helpers/settings.ts`. (`e2e/tests/modal-layout.spec.ts` covers the per-host settings
  dialog, not this one.)
- **Specs:** SPEC.md around 1984-1993 describes the sections and the pinned header and footer, but not the dialog's
  size; SPEC_impl.md around 912-925 the mounted hidden panels.

### Acceptance criteria

- The Settings dialog has one fixed height whatever section shows: a fixed value of about 640px, capped by the window
  (`min(<height>, 100dvh - 2 * var(--dialog-vertical-margin))` or equivalent). Fine-tune the value against the real
  sections so that every section except Agent trust and long Tensorlake or Credentials lists fits without scrolling at a
  normal desktop size, and log the chosen number as a DECISION. Short sections leave blank space below their content;
  long ones scroll inside the body as today. Header, footer and Close button do not move when the section changes.
- The same rule applies on narrow windows (620px wide or less), where the dialog shows the section list or one section.
- Width is unchanged. The sticky navigation and the editors' sticky action bars keep working.
- The existing step-through test (`settings.spec.ts` around 945) gains the check that the dialog's box and the Close
  button's box stay unchanged across every section. Run it at a window tall enough that the fixed height, not the window
  cap, sets the dialog's height (for example 1280x900), and at a narrow size; its current 1280x560 tests only the cap.
- The 5000px-tall Agent trust check (around 925-931, "a tall window must fit the settings without scrolling") protects
  what this change deliberately gives up. Replace it: in a very tall window the dialog stays at the fixed height, not
  taller, and Agent trust scrolls inside the body with header and footer pinned.
- SPEC.md's Settings description says the dialog keeps one size across sections, scrolling its body when a section is
  taller; SPEC_impl.md gets the CSS mechanism if it describes the layout at that level.
- A `fix` PR with a changelog fragment.
- The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08, on the planning page, which showed a mockup of a fixed-height dialog with a
short section (blank space below) and a long one (scrolling inside):

- **Fixed height, capped by the window.** The alternative was always filling the window's height, rejected because a
  tall window would make a very tall dialog of mostly blank space.
- **About 640px, the executor fine-tunes.** The alternative was a height that fits Agent trust too.
- **The same rule on narrow windows.** The alternative was keeping the dialog content-sized there.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold**, at the maintainer's request, so it runs only once they unhold it.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's Settings section; root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for the browser test;
`.agents/validation.md` for which browser engines to run.

**Planner proposals:** CSS only, no Rust measurement of section heights; the specific test shape above.

## Outline

One PR, `plan/settings-dialog-fixed-size/01-fixed-height`, typed `fix`: the CSS height rule, the extended and replaced
browser checks, the SPEC.md (and SPEC_impl.md if needed) wording, the changelog fragment, and the TODO.md removal.

### Validation

Run the Settings browser spec on the engines `.agents/validation.md` names for UI layout changes (WebKit included: the
Mac desktop app is WebKit), through `scripts/record-test-run.py`. No Rust tests are needed unless the change touches
Rust; run `cargo fmt --check` and the UI crate's checks if it does. `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` since browser tests change.

## Fallbacks

Examples of departures that need the scope reassessment: measuring section heights in Rust or JavaScript, changing the
dialog's width, restructuring the Settings component, or changing other dialogs' sizing.

Decisions to log: the final height and how it was chosen, how the narrow layout's height is expressed, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-settings-dialog-fixed-size-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/settings-dialog-fixed-size/<nn>-<short-name>`, as the outline names them.
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

> Blocked on 2026-11-09 (claim db591c).
>
> ### May Notifications scroll inside the fixed Settings dialog?
>
> This plan fixes Settings jumping and moving its Close button when you switch sections. It calls for a fixed height of
> about 640px, capped by the window, and says every ordinary section except Agent trust and long Tensorlake or
> Credentials lists should fit without scrolling.
>
> The partial draft uses 780px. Measurements showed that the desktop Terminal controls need about 727px including the
> header and footer, and Checkouts needs about 774px. The draft keeps the frame and Close button fixed while longer
> content scrolls inside the body.
>
> The remaining fit requirement does not match Notifications' existing form. A browser sizing check using the current
> stylesheet, fonts and default form measured a required frame of about 972px. The real application also shows that form
> scrolling on Chromium and WebKit. A 900px-tall window permits only 810px after the existing margins, so increasing the
> fixed height alone cannot make it fit there. The sizing fixture does not run the native desktop app.
>
> May Notifications scroll inside the fixed 780px frame, or should this plan expand to rearrange that form so it fits?
>
> Recommendation: keep 780px and allow Notifications to scroll, along with the other long sections. This preserves the
> requested stable dialog and Close button without changing the form. Rearranging or condensing Notifications would be
> additional UI work and would change more than the dialog's size. Merely raising the fixed height to about 972px would
> still leave it scrolling in shorter windows and would make short sections mostly empty in tall ones.
>
> The partial draft, [#3231](https://github.com/scode/farhelm/pull/3231/changes), must remain unlanded until this
> decision is recorded. Application measurements are retained in browser run `2c8a6413-b945-4750-b15f-de4825f0c03d`,
> with native-Terminal sizing in `7c44be84-05e6-42bf-9dd1-b060430b01eb` and default Notifications sizing in
> `4135b862-a8bb-42e9-8f2d-e133e9d36cb0`.
>
> The final 780px application check, run `b3ead4c5-ecac-4012-9f61-b93492fcf776`, passed all four selected layout cases
> across both engines, including Checkouts fitting and stable frame and Close positions across every section on desktop
> and phone. Notifications still measured 823px of content in a 631px body. This pass validates the partial fixed-frame
> change; it does not resolve the plan's Notifications fit requirement.

The maintainer's answer:

The maintainer decided on 2026-11-08, in conversation with the landing monitor ("yes scrolling notifications is fine").

Keep the fixed 780px dialog, capped by the window, and let Notifications scroll inside its body, like the other long
sections. Do not rearrange or condense the Notifications form. The plan's requirement that every ordinary section fits
without scrolling no longer applies to Notifications.

# Quick-launch sequences: accurate hints and a pill you cannot miss

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan. It covers two TODO.md entries, planned together at the maintainer's choice
because both edit the same SPEC.md bullet, docs section and browser tests.

## The goal

Quick-launch sequences (the sequence prefix, Cmd+X in the Mac app and Ctrl+Shift+X elsewhere, then number keys or ⚡,
then Enter) shipped in v0.30.0. Two things are left: several hints are wrong or misleading, and the status pill that
shows the queued templates is easy to miss.

TODO.md entries this plan covers (Near term):

1. "**Tidy the launch-sequence hints.** The "cannot launch on its own" banner also appears for ⚡ alone with an
   incomplete template, and for a one-template sequence, where it tells the user to use the prefix they just used;
   SPEC.md describes it only for a single number key. If a sequence that includes ⚡ fails because a number is unbound,
   Templates also shows the "Mark a template with ⚡" hint even when one is marked. Templates can call a template
   launchable when the launch check does not, for example a template set to run a command that names an agent but leaves
   the command empty; its number key then shows the "cannot launch on its own" banner. The Templates help paragraph does
   not yet mention partial-template keys or sequences. Found while landing the quick-launch sequences (#2220, #2225,
   #2228)."
2. "**Make the launch-sequence pill obvious.** After Cmd+X (Ctrl+Shift+X off the Mac) starts a quick-launch sequence,
   the status pill that floats over the bottom of the terminal area is easy to miss. Make it more prominent, probably by
   placing it in the middle of the terminal area instead of at the bottom and giving it a more noticeable color. Details
   TBD."

Since the first entry was written, #2355 changed sequences to capture each key's template name at Enter. That fixed the
main case of the "Mark a template with ⚡" bug (a bound ⚡ is no longer passed as a bare ⚡ reference). One variant of
it remains (a ⚡ mark set in another window), plus a related gap (a sequence naming a deleted template); both are in
scope (below).

### Acceptance criteria

**Hints (first PR):**

- **The "cannot launch on its own" banner depends on where the launch came from.** Pass the origin of a quick launch
  explicitly to the code that opens New: an origin enum (number key, ⚡ chord, template list, quick switcher, sequence)
  in place of today's keyboard flag, and a banner enum in place of the banner's template-name field. (The keyboard flag
  can already tell a sequence from a Cmd+J pick; an explicit origin is still the clearer design.)
  - A single number key whose template cannot launch on its own: today's banner, unchanged ("{name} cannot launch on its
    own. Use {prefix} to combine template keys, then Enter to launch, or complete this setup below.").
  - The ⚡ chord pressed on its own (Cmd+P, or Ctrl+Shift+P), a Cmd+J (template list) pick, and a quick switcher "Quick
    launch" row whose template cannot launch on its own: "{name} needs more choices before it can launch; complete it
    below." It does not mention the sequence prefix. (A sequence holding only ⚡ is a sequence, below.)
  - Any sequence (one template or several) whose combined setup cannot launch: "This sequence doesn't make a complete
    setup; complete it below." This now also covers multi-template incomplete sequences, which show nothing today.
  - All three keep the banner's existing prominence, six-second expiry and dismissal on edit or reseed.
- **One launchability rule.** Templates' notion of whether a template can launch on its own becomes the launch check's
  rule (judge the template's active launch kind: an agent launch needs an agent, a command launch needs a nonblank
  command; a template that sets no kind is judged as an agent launch, which is where New starts). It applies wherever
  Templates or the helm uses the current rule: whether ⚡ can be set, the ⚡ tooltip, the number-key tooltip, the
  editor's quick-launch state line, and the helm's refusal of an agent's template write that marks ⚡ (with the agent
  instructions text that states the rule). Move the existing combined-templates check into `farhelm-proto` and judge a
  single template as a one-item list, so there is one function, not two. A template already marked ⚡ that fails the new
  rule can still be unmarked (both the UI and the helm already allow this; keep it).
- **The explanation texts change with the rule.** Every sentence that states the old rule becomes wrong and is rewritten
  to the new one: Templates' "Can't be the quick launch: it chooses neither an agent nor a command" and the ⚡ tooltips,
  the helm's refusal "⚡ needs a template that chooses an agent or a nonblank command" (and its tests, which match that
  text), the agent instructions in `crates/farhelm/src/agent_instructions.rs`, SPEC.md's "A template that chooses
  neither an agent nor a nonblank command cannot be marked in the interface", and the docs' "must choose an agent or a
  command". Suggested wording: an agent launch needs an agent, a command launch needs a nonblank command (adapt per
  surface; log the final wording).
- **Leftovers:** a sequence that names a template that has since been deleted or renamed opens New with the existing
  "template X is no longer available" error, as a one-template sequence already does today (SPEC.md: a refusal names its
  template and reveals New), instead of opening Templates with no explanation. A ⚡ mark or number binding set in
  another window is honored when a sequence runs: refresh the preferences once when the sequence starts (written the
  same way the dispatch worker already writes them), so the pill and the Enter-time capture see it. Do not refresh at
  Enter: SPEC_impl.md's rule that an unbound reference stays unbound would break.
- **Templates help paragraph** gains the sentence "A number key can hold a partial template; press {prefix} and then
  several keys to combine them, Enter to launch." with `{prefix}` rendered as the platform's prefix chord, as the
  paragraph already renders the ⚡ and number chords.
- Unit tests for the origin-dependent banner choice and the shared launchability rule; browser tests for the three
  banner texts, a sequence naming a deleted template, a ⚡ mark set after this window loaded (for example through the
  API) being honored by a sequence, and ⚡ plus an unbound number key (no "Mark a template with ⚡" hint when ⚡ is
  marked; #2355 already fixed this, so the test guards that fix).

**Pill (second PR):**

- **Position:** centered, horizontally and vertically, over the terminal area of the open session; with no session open,
  centered over the main area. Plain CSS centering in the main area is close to exact in normal use (on the web the top
  row and the details bar are both 30px); where they differ (the Mac app's 48px top row), correct it with a fixed-offset
  rule like the existing one that lifts the pill above the details bar. Conditional bands may shift the true terminal
  center slightly; that is acceptable. While the session launcher is open (the pill says "apply"), it stays where it is
  today, at the bottom of the main area, so it does not cover the launcher.
- **Look:** the amber attention family (`--warn` text or border on `--warn-bg`, through tokens, never literals), larger
  than today (the mockup used about 12.5px text and 8px by 16px padding, against 12px and 9px by 14px today; pick sizes
  that read clearly and log them), with a soft amber halo around it. Keep it non-modal and `pointer-events: none`, and
  keep its text and `role="status"` as today.
- **Motion:** a short entrance when the sequence starts (fade and slight scale in, a few hundred milliseconds), no
  looping animation, and no motion at all under `prefers-reduced-motion`. Respect the repository's CSS animation rules
  (`crates/farhelm-ui/js-tests/app-css-animations.test.js`).
- The pill draws above other overlays in the terminal area, as today (it is transient and the sequence ends on any click
  or other key).
- Contrast for the new color pair is pinned in `crates/farhelm-ui/js-tests/app-css-contrast.test.js` like the existing
  pairs, and the token test still passes.
- The browser test that asserts the pill's geometry (`e2e/tests/quick-launch.spec.ts`, inside the terminal panes and
  above the details bar) asserts the new placement, including the launcher case.

**Both:**

- SPEC.md ("Sessions" → "Creation": the quick-launch, template number shortcut and sequence bullets) and SPEC_impl.md
  ("Launch templates") describe the banner rules, the launchability rule, the pill's placement and look. The docs page
  `website/src/content/docs/docs/using/launch-templates.mdx` (the ⚡ eligibility sentences, "Pressed alone, a partial
  template…", and "A pill at the bottom of the main area shows what you have queued") matches.
- Each PR carries a changelog fragment (`kind: fixed` for the hints, `kind: changed` for the pill).
- The last code PR removes both TODO.md entries.
- Out of scope: Enter with nothing queued still cancels the sequence silently, as SPEC.md specifies ("Enter with nothing
  queued cancels without opening anything"); the separate plan for the session launcher's silent Enter also leaves it
  alone.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Plan both entries together: "One plan, two PRs: hints first, then the pill."
- Banner: "Number key: keep today's banner. ⚡, Cmd+J, quick switcher: "X needs more choices before it can launch;
  complete it below." Any sequence: "This sequence doesn't make a complete setup; complete it below.""
- Eligibility: "Make Templates (⚡ marking, key tooltip, editor text, the agent API) use the launch check's rule", with
  the note that a template already marked ⚡ that fails can still be unmarked.
- Leftovers: "Fix both (explain the missing template; refresh the mark before deciding)". Reusing the existing "template
  X is no longer available" error in New, and refreshing when the sequence starts, are the planning review's smallest
  forms of that choice.
- Help paragraph: add the sentence above.
- Pill: "A. Centered over the terminal, amber, larger" (chosen over centered accent blue and bottom amber). Amber was
  recommended because it already means "needs your attention, nothing is broken", while the accent is a closed list kept
  for selection and focus.
- While the launcher is open: "Keep it at the bottom of the main area while the launcher is open."
- Motion: "A short entrance (fade/scale in), no looping; none with reduced motion."
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md "Sessions" → "Creation": the "Quick launch:" bullet (including "A template that chooses neither an agent nor a
  nonblank command cannot be marked in the interface" and "An edited marked template that no longer supplies an agent or
  command opens New with it applied"), the "Template number shortcuts:" bullet (the banner sentence and "only the
  combined setup must be launchable"), and the sequence bullet (the pill sentence "A non-modal status pill floats over
  the bottom of the terminal area…", "the pill says "apply" instead of "launch"", and the incomplete-result and
  unbound-key fallbacks). "Desktop window chrome" for the chords and Cmd+J.
- SPEC_impl.md "Launch templates": the capture-at-Enter paragraph ("A coarse fold over launch kind, agent and command
  decides whether to attempt direct launch"), the listener and pill paragraph, and "Number bindings accept partial
  templates; the bolt keeps its runnable-template affordance…".
- `crates/farhelm-ui/assets/app.css`: the accent is a closed list (do not use it for the pill); use sites use tokens;
  the CSS token, contrast and animation tests in `crates/farhelm-ui/js-tests/`.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md`
  before editing the docs page; `.agents/docs-media.md` before touching anything under `e2e/docs-shots/`. The v0.30.0
  release shot of a template sequence is frozen (root `AGENTS.md`, "Frozen material"): leave it and its test alone.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- Chords: `TEMPLATE_SHORTCUTS` in `crates/farhelm-ui/src/launch_sequence.rs`; the capture listener is
  `crates/farhelm-ui/assets/template-shortcuts.js`. The queue is the `LaunchSequence` signal; references are
  `TemplateReference` (`Bolt`, `Number`, `Name`, `Unbound`). `capture_sequence_bindings` turns bound references into
  `Name` at Enter; `resolve_template_references` is all-or-nothing.
- Dispatch: the `quick_launch` worker in `crates/farhelm-ui/src/list/view.rs` (around lines 3455-3641). On the page it
  resolves, opens New, runs the launch check, and sets the banner name when `!launchable && templates.len() == 1`
  (around 3579); a single missing `Name` opens New with "template X is no longer available"; otherwise Templates opens
  with hint flags (around 3597-3612). Inside the launcher it builds a refusal text instead.
- The launch check is `stacked_quick_launch_eligible` in `crates/farhelm-ui/src/list/templates.rs` (around 32-56);
  Templates' notion is `quick_launch_eligible` in `crates/farhelm-proto/src/launcher.rs` (around 301-311), used for the
  bolt's disabled state and tooltip, the number-key tooltip, the editor's state line, and the helm's agent template
  writes (`crates/farhelm-helm/src/agent_requests.rs`, "⚡ needs a template that chooses an agent or a nonblank
  command"), whose rule `crates/farhelm/src/agent_instructions.rs` also states.
- The banner is rendered in `crates/farhelm-ui/src/list/create_form.rs` (around 5745-5761). The "Mark a template with
  ⚡" and number hints and the help paragraph are in `templates.rs` (around 1680-1688).
- The pill is `LaunchSequenceIndicator` in `launch_sequence.rs` (around 347-394), mounted once as the last child of
  `div.app-main` in `crates/farhelm-ui/src/lib.rs`. CSS is `.launch-sequence-indicator` in `app.css` (around 665-695:
  absolute, z-index 50, bottom 14px, `--bg-2`, lifted above the details bar). The pill lives above the session view so
  it survives navigation and an empty fleet; keep that property.
- Tests: `e2e/tests/quick-launch.spec.ts` (sequences, the pill geometry test, unbound keys, incomplete ⚡),
  `e2e/tests/templates.spec.ts` (banner text and the key tooltip regex),
  `e2e/tests/launcher-template-shortcuts.spec.ts`, `e2e/tests/template-list-shortcut.spec.ts`; unit tests in
  `launch_sequence.rs` and `templates.rs`; helm tests for the refusal text in `agent_requests.rs`; the docs shot
  `launch-templates/panel` (`e2e/docs-shots/launch-templates.spec.ts`) shows the help paragraph, so it becomes stale
  until a docs refresh, which is acceptable.

Proposed shape (planner proposal):

1. `fix` PR: the origin passed into quick launch and the three banner texts; the shared launchability rule and its use
   sites (UI, helm, agent instructions); the two leftovers; the help sentence; tests; SPEC, SPEC_impl and docs sentences
   about hints and eligibility; fragment.
2. `style` PR (this repository uses `style:` for visual changes, for example #1305, #1860 and #959; `style` takes a
   changelog fragment): the pill's placement, look and entrance; the contrast pin; the geometry test; SPEC, SPEC_impl
   and docs sentences about the pill; fragment; remove both TODO.md entries.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the UI crate's and proto's unit tests, the helm's agent-request tests for
the changed refusal, the JS unit tests (`crates/farhelm-ui/js-tests/`, including the CSS token, contrast and animation
tests), and the browser specs named above through the recorder on both engines; the test-sleep checker for changed
browser tests; the website checks `website/AGENTS.md` names for the docs edit;
`python3 releasing/check-changelog.py format`.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: a live measurement of the terminal area to
place the pill, a new preferences subscription or push channel to keep the ⚡ mark fresh, mounting the pill inside every
session view with a second copy for the empty state, a new color token family, a third hint line in Templates for
missing templates, or changing how sequences are captured.

Decisions to log: the origin and banner enums, the final wording of the rule on each surface, the pill's sizes and its
centering offset, and the entrance's duration and easing.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launch-sequences-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
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
  `plan/launch-sequences/<nn>-<short-name>` (for example `plan/launch-sequences/01-hints`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes both TODO.md entries this plan covers (each ends with this plan's `Plan:` reference). TODO.md
  is a shared bookkeeping file: on a rebase conflict there, keep both sides.
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
each PR passed its review gate, and the last code PR removes both TODO.md entries. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

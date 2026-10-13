# Polish the send feedback dialog

Written against main at 5cd50442 on 2026-10-12. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Spruce up the feedback window": the "send feedback" dialog works but looks unfinished.
The message and contact fields are bright white boxes on the dark dialog, unlike the dark fields everywhere else; the
field labels ("message", "how to reach you (optional)") are in larger type than the dialog's own heading; "send" and
"cancel" sit flush against each other with no gap, and a disabled "send" barely reads as a button; and there is no ×
close at the top.

The target design is the maintainer-approved mockup at https://snippets.scode.org/s/farhelm-feedback-dialog-polish/.
Read it first; it draws the dialog in each state (empty, filled, sending, failed, sent) at the app's real sizes and
colors, next to today's dialog. Exact spacing is yours to settle; the listed changes are requirements.

Acceptance criteria:

- **Header**, in the pattern the host dialogs use (`HostDialogHeader` in
  `crates/farhelm-ui/src/hosts/settings_dialog.rs` and the `.host-settings-header` styles): a tile with a feedback
  glyph, the title "send feedback", one summary line saying where the feedback goes ("goes privately to Farhelm's
  maintainer, not a public tracker", or close to it), and a × that does exactly what cancel does (its handler reads
  `may_close` as cancel does, so a × queued behind a send waits), with a tooltip and an accessible name, disabled while
  sending as cancel is. The app has no feedback icon yet, so draw one, without an SVG `<title>` (the tooltip coverage
  test fails native titles).
- **Dark fields**: the message textarea and the contact input use the app's standard recessed field style (the
  `var(--well)` background, `var(--border)` outline, `var(--fg-0)` text, 13px, the accent outline on focus), in both
  Chromium and WebKit (the desktop app's engine family).
- **Quiet labels**: the field labels use the 12px muted label style of the other dialogs, smaller than the 14px heading.
- **What is sent** (version, app, operating system) sits in a small recessed box, as drawn.
- **Pinned footer**, laid out as drawn: the hint on the left and the buttons on the right of one row, like the host
  settings footer (`div.host-settings-actions.host-settings-footer` with a `span` hint, a `space-between` row), not the
  host confirmation dialogs' footer, which stacks its hint above the buttons. The buttons are cancel then send, one
  size, with the standard gap (`.host-dialog-answers` inside the row gives both; check its narrow-window stacking next
  to a wrapping hint). The hint reads "nothing is sent until you press send" while editing, "nothing was sent" after a
  failed send, and is absent while sending and once sent (when only "close" remains). The body scrolls between the
  pinned header and footer when the window is short.
- **Text changes the mockup draws**: the checkbox label becomes "re-use for future feedback" and the attached heading
  "sent with it, and nothing else" (no colon), matching the app's lowercase labels. SPEC.md names the label in its usual
  title case and needs no change.
- **A disabled send still reads as a button**: its outline and label dim, rather than the whole button fading to half
  opacity.
- **Behavior unchanged**: what is sent, the send and failure flow, the reuse-contact checkbox and when it shows, the
  thanks text and the automatic close after it, Escape and focus handling (initial focus in the message field, focus
  back to the help toggle after close), and SPEC.md's Feedback section all stay as they are.
- **Nothing else restyles**: the app settings dialog shares classes with this one (`app-settings-dialog`,
  `host-settings-title`, `host-settings-actions`), and `.host-settings-actions` also serves host settings. New styles
  are scoped to the feedback dialog, or the feedback dialog opts in to the host dialogs' framed classes; no shared rule
  changes its meaning for other dialogs. A separate pending TODO entry ("Pin the app settings dialog's header and
  footer, and add an × close") covers app settings; do not do it here.
- Tests as listed under Outline, on Chromium and WebKit.
- One draft PR, passing its review gate, with its changelog fragment, removing the TODO.md entry. No PR is marked ready,
  and nothing is merged.

## Requirement sources

**The user's request (2026-10-12):** plan this TODO entry. The maintainer asked for an SVG mockup first, reviewed it,
and chose "build as drawn".

**The user's decisions (2026-10-12):**

- The mockup above is the target design.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Feedback" (what the dialog contains and how sending behaves; it says nothing about layout).
- The host dialogs redesign landed on main as the shared header/footer pattern; build on it rather than inventing a
  second one.
- Root `AGENTS.md`: Conventional Commits; Finishing work (browser tests through the recorder on Chromium and WebKit,
  `scripts/check-test-sleeps.py` for test delays); Releases and the changelog; Agent scratch space; Sharing the machine
  with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main and corrected by a
fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/src/feedback.rs`, `FeedbackDialog`: `div.host-settings-backdrop` >
  `div.app-settings-dialog.feedback-dialog` (`role=dialog`, `aria-label="send feedback"`), with
  `h2.host-settings-title`, a `p.host-settings-help` privacy note, `label.feedback-field` wrapping
  `textarea.feedback-message` and `input.feedback-contact`, `label.app-settings-choice` with `input.feedback-reuse`,
  `div.feedback-attached` (a help line and a `dl`), `p.feedback-error[role=alert]`, `p.feedback-thanks` once sent, and
  `div.host-settings-actions` with `button.feedback-send` and `button.feedback-cancel`. States:
  `SendState::{Editing, Sending, Sent, Failed(String)}`. `install_dialog_with_selector` sets up focus; Escape is gated
  by `may_close`.
- `crates/farhelm-ui/assets/app.css`: `.feedback-field` (flex column) and its fields get only width, box-sizing and
  `font: inherit`, so the browser's white control shows and labels inherit 16px. The standard dark field is spelled out
  in several dialogs (for example `.templates-field input`, `.rename-input`, `.add-host-form input`).
  `.host-settings-actions` has no gap. `.btn:disabled` is `opacity: 0.5`. The framed dialog pattern is
  `.host-dialog-framed` with `.host-settings-header`, a scrolling body and `.host-settings-footer.host-dialog-footer` >
  `.host-dialog-answers` (gap 8px, stacked under 560px).
- `HostDialogHeader` is host-specific: its tile always draws a host mark (the default host glyph when given no host),
  and it has no way to disable the ×. Reusing it needs a caller-supplied tile and a dismiss-disabled flag, without
  changing what host dialogs render; otherwise render the same structure in `feedback.rs`. Log the DECISION.
- A help line moved into a new body wrapper loses `.app-settings-dialog > .host-settings-help`'s `margin: 0` and picks
  up the base rule's 24px indent; check every moved line.

Here's the work:

1. Restructure `FeedbackDialog` into header, scrolling body and pinned footer as drawn, reusing the framed-dialog
   classes and adding feedback-scoped rules for the fields, labels, the attached box, the disabled send and the footer
   hint.
2. Keep every selector the tests and the frozen release-notes docs shot rely on (`.feedback-dialog`,
   `.feedback-message`, `.feedback-contact`, `.feedback-reuse`, `.feedback-send`, `.feedback-cancel`,
   `.host-settings-title`), or update the tests that use them. The published docs shot "v0.25.0 feedback contact" is
   frozen and is not recaptured.
3. Tests, in `e2e/tests/feedback.spec.ts`:
   - the × closes like cancel, is disabled while a send is in flight, and a close queued behind a send still waits for
     it (the existing Cancel/Escape test, extended to the ×);
   - the fields render with the dark field background, not white (computed style against the token's value);
   - the field labels are smaller than the heading, and send and cancel do not touch (a measured gap);
   - a disabled send keeps full opacity;
   - the footer hint in each state (editing, failed, sending, sent), on the same row as the buttons;
   - in a short viewport, the header and footer stay fully visible, the body scrolls, and send and the × stay reachable;
   - existing tests keep passing, including the native leading checkbox check; the one exact-match lookup of the
     checkbox by its old capitalized name is updated to the new label. Keep `tooltip-coverage.spec.ts` passing (the ×
     needs a tooltip and no native `title`).
4. Bookkeeping: changelog fragment, `kind: fixed`, saying the feedback window now matches the rest of the app. Remove
   the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop`;
- the changed Playwright specs (`feedback`, `tooltip-coverage`) on Chromium and WebKit through the recorder, after the
  builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if any test delay is added;
- `dprint check` on changed Markdown, and `python3 releasing/check-changelog.py format`.

Look at the dialog yourself in a browser at a normal and a short window height, in each state, and compare it with the
mockup before the review gate. Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-feedback-dialog-polish-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/feedback-dialog-polish/<nn>-<short-name>` (for example `plan/feedback-dialog-polish/01-framed-dialog`).
- One PR. Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped
  differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix`, since the
  dialog's unfinished look is the defect). Such a PR adds a changelog fragment under `releasing/changelog.d/` in the
  same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that the result matches the mockup's listed changes;
- that no shared class changed meaning for app settings, host settings or any other dialog;
- that send, failure, Escape, focus and the automatic close behave exactly as before;
- that the × is disabled whenever cancel is, and has a tooltip and an accessible name;
- that the new browser assertions test what they claim on both engines.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a shared dialog component for every
  dialog, restyling app settings, or changing what feedback sends (examples, not a blacklist);
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

- whether `HostDialogHeader` was generalized or its structure repeated;
- how the field style is shared or scoped;
- the summary line's exact wording and the tile's glyph;
- how the disabled send is styled;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: restyling the app settings dialog, changing
any other dialog's look, changing the feedback payload or flow, or departing from the mockup's listed changes.

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

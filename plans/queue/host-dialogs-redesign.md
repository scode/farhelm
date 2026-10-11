# Host dialogs redesign: add host, setup confirmation, remove host, uninstall

Written against main at 7a1d2a8a on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `trash-and-host-settings-dialogs.md`: that plan gives every modal dialog one vertical margin and
gives host settings a pinned header and footer with only the body scrolling. Build on what it landed (read its PRs on
main before starting) rather than creating a second header/footer pattern.

## The goal

The TODO.md entry (Near term), "Fix the layout of the host setup confirmation": when adding a host that Farhelm would
set up, the dialog that shows the setup plan and asks "yes" / "yes, and don't ask in the future" / "cancel" looks
broken. It has no heading, so it opens straight into the plan with nothing saying what is being asked. Plan lines that
wrap continue at the left edge with no hanging indent, so a wrapped step reads like a separate step. The three answers
come out at three different sizes on no common baseline. The "You can turn this back on with the gear…" note sits under
the middle button only, wraps, and pushes cancel down off the row. Cancel's tooltip can cover part of the middle button;
cancel is focused when the dialog opens.

The user asked to fix it in general: "apply current good practices we have in other dialogs and make it look decent and
not janky", across the add-host dialog (both its steps), the remove-host dialog and the uninstall dialog.

**The target design is the approved mockup at `https://snippets.scode.org/s/farhelm-host-dialogs-redesign/`.** Open it
before starting and keep it open; it shows each dialog at the app's real sizes and colors. Exact spacing is yours to
settle, but the structure, labels, order and tiers in the mockup are decided.

Acceptance criteria:

- The add-host dialog (both the ssh-destination step and the setup confirmation), the same setup confirmation reopened
  from a host's row, the remove-host dialog and the uninstall dialog all have:
  - a header like host settings': a mark tile, a title saying what is asked ("add host", "set up host · <destination>",
    "remove host · <name>", "uninstall Farhelm · <name>"), one summary line, and a × dismiss;
  - a pinned footer with the answers right-aligned in one row, the safe answer first and the action last, every button
    the same size on one baseline;
  - for the two dialogs with a permanent answer, the "don't ask again" hint as one line of its own above the buttons.
- Labels name the action: "set up" / "set up, don't ask again" / "cancel"; "remove" / "remove, don't ask again" /
  "cancel"; "uninstall" / "cancel". Tiers stay as SPEC_impl.md describes: the one-off action filled, the permanent
  answer outlined in the same color (primary for setup, danger for removal), cancel neutral.
- Each step of a plan wraps under its own text with a hanging indent; lines that are not steps (the plan's heading line,
  the uninstall plan's "Kept: …") wrap flush. The text shown and copied is still exactly the helm's.
- Cancel still takes focus when a confirmation opens (so a stray Enter changes nothing), and keeps its visible focus
  ring, but no tooltip appears merely because the dialog opened and focused it.
- The add-host dialog keeps the one width it has across both steps (560px), and uninstall becomes that wide too; remove
  host stays at 480px.
- At phone width the answers stack full width with the primary on top, and the hint moves under them. Titles wrap there,
  as host settings' does; the mockup's ellipsis is only the drawing.
- The new × in each header has a tooltip and does exactly what that dialog's cancel does.
- Behavior does not change: what each answer does, the permanent preferences, when cancel and Escape are live (today the
  setup and uninstall dialogs keep both live while another operation runs, and remove host disables Escape then; keep
  each as it is), focus returning to the host row, and each dialog's accessible name (tests find the dialog reopened
  from a host's row by its name, "host setup").
- SPEC_impl.md's button-tier paragraph and any SPEC.md text that quotes the old labels say what now holds.
- Each `feat`, `fix`, `perf`, `style` or `revert` PR has its changelog fragment. The last code PR removes the TODO.md
  entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10):** "lets plan that. please make a scode-snippet with an svg mockup so we are clear on
what it's gonna look like. fix it in general, apply current good practices we have in other dialogs and make it look
decent and not janky", and then one snippet covering all the dialogs.

**The user's decisions (2026-10-10):**

- Scope: the add-host dialog (both steps, and the setup confirmation reopened from a host's row), plus the remove-host
  and uninstall dialogs, so all host dialogs share one header, footer and button row. (Only the setup confirmation, or
  only the add-host dialog, were offered and not chosen.)
- Answers: a footer bar like host settings', buttons in one row on the right, safe answer leftmost and primary
  rightmost, the hint as its own line. (Primary-first order, and stacked full-width choices, were offered and not
  chosen.)
- Labels name the action, as listed under acceptance criteria. (Keeping "yes", or "always set up", were offered and not
  chosen.)
- This plan waits for `trash-and-host-settings-dialogs.md` to land and reuses its header/footer layout.
- The mockup was approved as drawn, including the planner's details in it: the header subtitles, "checking…" on the add
  button while the helm checks a destination, the local host named as the rest of the UI names it, the stacked answers
  at phone width, the hint reworded to "undo "don't ask again" with the gear at the top of the sidebar", and cancel
  keeping focus without its tooltip popping up. After approval, a planning review found two lines of mockup text that
  would have been false, and the planner corrected the mockup (the URL shows the corrected version): the destination
  step's help line no longer promises the plan is always shown first (it is not when "set up new hosts without asking"
  is on), and an intro line above the setup plan was dropped because it was wrong for the dialog reopened from a host's
  row and for this machine. For this machine, the summary line says "here", not "there". The setup title shows the
  destination as typed; do not parse it. The "B" tile in the mockup stands in for the host's own icon, and the server
  glyph for whatever icon the app has for a host not yet added.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- The plan text is the helm's, rendered from the same value execution consumes, and the UI deliberately does not
  re-render `ProvisioningPlan.actions` (see the docs on `PlanConfirmation` in `crates/farhelm-ui/src/provisioning.rs`
  and on `.provisioning-plan` in `app.css`). The hanging indent must come from presenting the helm's lines, not from
  walking the plan's actions.
- SPEC_impl.md's paragraph on button tiers (it quotes "yes, and don't ask in the future" and "remove, and don't ask
  again" and says the permanent answer is outlined while the one-off is filled). Keep the tiers; update the quoted
  labels.
- SPEC.md's Settings dialog text on `set up new hosts without asking` / `remove hosts without asking` and the
  preferences that the permanent answers set. Unchanged in behavior.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; the rule against tests that modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 7a1d2a8a and
corrected by a fresh-context planning review, which found no unnecessary subsystem but replaced a new line-splitting
function with the per-line rendering `PeerBlock` already does, narrowed the shared "frame" to a host-header component
plus reuse, pointed the tooltip fix at an existing keep-down state, and corrected the mockup text noted above.

## Outline

Line numbers drift; find the code by name.

Here's where things are today:

- `crates/farhelm-ui/src/hosts.rs`, the add-host component: a `.host-settings-dialog.host-add-dialog` with
  `aria-label="add host"` and no visible heading. Its `form.add-host-form` holds either the "ssh destination" field and
  an "add" submit (reading "probing…" while the helm probes), or, once the helm offers a plan, `SetupPlanConfirmation`
  in its place; cancel sits outside the form on its own row.
- `crates/farhelm-ui/src/provisioning.rs`: `SetupPlanConfirmation` (the three setup answers, shared by the add dialog
  and the host row's rerun dialog, `aria-label="host setup"`), `UninstallConfirmation`, and `PlanConfirmation` (the
  inline update confirmation in the host row, which is not a modal and is out of scope).
- `crates/farhelm-ui/src/hosts/settings_dialog.rs`: `HostRemoveDialog` (it has an `h2` title but no header bar), the
  focus helpers `install_add_dialog`, `focus_add_cancel`, `install_remove_dialog`, `install_uninstall_dialog`, and the
  refreshed host settings dialog, whose `.host-settings-header` / `-body` / `-footer` structure is the pattern to reuse.
- `crates/farhelm-ui/assets/app.css`: the three answers get three sizes because "yes" carries `.provisioning-confirm`
  (12px, small padding), the permanent answer plain `.btn`, and cancel `.add-host-cancel`; `.host-permanent-answer`
  wraps the permanent button and its hint in a 180–260px column, which is what wraps the label and pushes cancel off the
  row. The host row's inline update confirmation also uses `.provisioning-confirm`, so do not restyle that class
  globally.
- `crates/farhelm-ui/assets/tooltip.js` shows a tooltip on `:focus-visible`, and already has a state that keeps a
  tooltip down after a press or Escape. Focus moved by script right after a keyboard submit counts as focus-visible. The
  TODO entry says it is unclear whether the screenshot's tooltip came from that focus or from a hover; the fix below
  covers the focus case, and a pointer resting where "add" was will still hover "set up" when the plan arrives (mention
  that in the report, do not build anything for it).
- The plan text is already rendered by `PeerBlock`, which emits one element per line of the helm's text.

The work, as a short stack:

1. **Host dialog header (refactor).** Host settings' header, body and footer classes are mostly unscoped already, so
   most of the frame is reuse. The part worth a component is the host header (mark tile, title, status and version),
   which host settings, remove host and uninstall all need: extract it from host settings, with host settings as its
   first caller and no visible change. Build on the pinned header/footer that `trash-and-host-settings-dialogs.md`
   landed, and reuse its phone breakpoint. Keep everything scoped to the host dialogs; app settings, feedback, templates
   and the trash dialog must not change.
2. **Setup confirmation and add host.** Move both add-host steps and the setup confirmation (in both places it opens)
   onto that layout, matching the mockup: titles and subtitles, the destination step's help and error lines, "checking…"
   while probing, the new labels, the reworded hint on its own line, and the phone-width stacking. Add the footer's
   button-row rules here, where they are first used: one size and baseline for every tier, without restyling
   `.provisioning-confirm` for the host row's inline update confirmation. For the hanging indent, mark the lines
   `PeerBlock` already renders that start with "- " and indent them with CSS limited to the plan block; the host row's
   inline update confirmation may pick up the indent too. Making lines block-level can change what a copy yields, so
   check that copying the whole plan block still gives exactly the helm's text. For the local host, use the UI's
   existing name for this machine (`gui_host_name`).
3. **Remove host and uninstall.** Move both onto the same layout, with the host's own mark and status in the header,
   uninstall at 560px, and the uninstall plan indented like the setup plan.
4. **Focus without a stray tooltip.** Keep cancel as the initially focused control in all three confirmations, with its
   focus ring, but keep its tooltip down after that programmatic focus. Reuse `tooltip.js`'s existing keep-down state
   through one small hook the dialogs' focus helpers call; do not hide focus-visible styling to get there, and do not
   disable tooltips on these buttons. A later keyboard move or hover still shows tooltips. Update SPEC_impl.md's tooltip
   paragraph to match.

SPEC and docs: update SPEC_impl.md's tier paragraph to the new labels, and any SPEC.md or `website/` text that names the
old answers. `website/src/content/docs/docs/get-started/add-a-remote-host.mdx` and
`website/src/content/docs/docs/using/manage-hosts.mdx` describe these dialogs in screenshot alt text, and the first also
tells the reader to choose **yes** in its body text; update the words (after reading `website/AGENTS.md` and
`website/EDITORIAL_RULES.md`), but do not regenerate or publish screenshots. `e2e/docs-shots/` specs
(`add-a-remote-host.spec.ts`, `manage-hosts.spec.ts`) are not gates; if one selects a changed control, update the
selector so it still runs.

Tests:

- Existing browser tests select these controls by class and label and need updating, keeping what each proves:
  `e2e/tests/provisioning.spec.ts` (the add-dialog and setup-answer tests), `e2e/tests/terminal-multihost.spec.ts` (the
  uninstall and removal tests), `e2e/tests/settings.spec.ts` (permanent answers reversed through settings),
  `e2e/tests/buttons.spec.ts`, `e2e/tests/tooltip-coverage.spec.ts`, and `e2e/tests/tooltip.spec.ts`. Two tests check
  the old hint wording.
- Add a layout test for the setup confirmation at a typical width and at phone width: the title is visible; the three
  buttons share one height and one baseline (or stack full width at phone width); the hint does not sit between buttons;
  a wrapped plan step's continuation starts to the right of its "- "; copying the plan block yields exactly the helm's
  text; and no tooltip is visible after the dialog opens from a keyboard submit. That last assertion has to wait past
  the tooltip's show delay, so first prove that a tooltip does show in the same setup when it should (a keyboard move to
  another button), or give the wait a `sleep-ok` reason per `docs/test-sleep-check.md`. Cover remove host and uninstall
  with the same height-and-baseline and no-tooltip-on-open checks, and check that each new × does what cancel does,
  above all in the dialog reopened from a host's row, whose handlers are written separately from the add dialog's.

Changelog: a fragment carries one kind, so use one `kind: fixed` fragment for the setup confirmation and add-host
layout, and one `kind: changed` fragment for the relabelled answers and the redesigned remove and uninstall dialogs;
later PRs may edit the fragment their change belongs to.

### Validation

Follow root `AGENTS.md` "Finishing work" per PR, choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop`;
- the changed and new Playwright specs on Chromium and WebKit through the recorder, after the builds the browser suite
  needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, and the website build if a `website/` page changed;
- `python3 releasing/check-changelog.py format`.

Take Playwright screenshots of every redesigned dialog (both add-host steps, the setup confirmation at typical and phone
width, remove host, uninstall) and compare each with the mockup before finishing its PR. Do not run the full battery
without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-dialogs-redesign-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/host-dialogs-redesign/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/host-dialogs-redesign/01-dialog-frame`).
- One PR per item under Outline, in that order, as one linear stack. Keep them bite-sized and avoid churn: code added in
  one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR that
  needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
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
`AGENTS.md` requires. Attach the mockup URL (`https://snippets.scode.org/s/farhelm-host-dialogs-redesign/`) and the
screenshots you took, and ask the reviewer to check that each dialog matches the mockup's structure, labels, order and
tiers; that the plan text shown and copied is still exactly the helm's; that no answer's behavior, permanent preference,
or focus return changed; and that no other dialog's styling changed.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a change to what the helm sends,
  walking the plan's actions in the UI, restyling dialogs outside the four named here, a general dialog framework, or a
  change to how tooltips work for the whole app beyond the focus-on-open case (examples, not a blacklist);
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

- the shape of the host header component, and how it builds on what the other plan landed;
- how the plan lines are marked and indented, and the copy check;
- how the tooltip-on-open is prevented;
- any place the implementation departs from the mockup, and why;
- each Conventional Commit type and changelog kind;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: keeping "yes" labels, a different button order
or tier, leaving remove host or uninstall unchanged, or changing what an answer does.

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

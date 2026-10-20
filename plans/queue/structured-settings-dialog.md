# Settings dialog: a sidebar of sections instead of one long page

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan.

## The goal

The Settings dialog (opened from the gear in the sidebar's app bar) has grown into one 480px-wide scrolling column of
about 25 controls in six or seven sections. Restructure it so related settings are grouped and any one of them is easy
to find: a left sidebar listing the sections, and the selected section's settings beside it, the way the Templates
dialog pairs a list with an editor.

TODO.md entry this plan covers (Near term): "**Give the Settings dialog more structure.** Settings has grown into one
long page that takes endless scrolling to get through. Overhaul it with more structure, such as tabs, so related
settings are grouped and any one of them is easy to find. The grouping and layout are TBD."

The agreed design, in words (the maintainer chose it from a mockup you cannot see):

- **Layout:** a left sidebar of section names and a pane showing the selected section, inside the existing dialog frame
  (the framed header with the gear tile, title, summary and ×, and the footer with its note and close button, both
  pinned; only the content between them scrolls). The dialog widens to fit the sidebar (the mockup used about 760px;
  pick a width that fits the widest section without crowding, and log it). The selected section is marked the way
  Templates marks its selected row (accent inset and accent fill).
- **Sections, in this order, with these contents:**
  1. **sessions**: skip session confirmations; name sessions on quick launch; color terminal frames by host.
  2. **hosts**: set up new hosts without asking; remove hosts without asking (with the section's intro help).
  3. **terminal**: the terminal link click radio group; in the desktop app also allow tunnels from remote sessions, and
     file types to open with Cmd-click (Ctrl-click off macOS).
  4. **checkouts**: checkout folder; post-clone command.
  5. **agent trust**: the Claude, Codex, Muse and Pi trust blocks, each with its own hint switch where it has one, its
     caveat box and its error line, as today.
  6. **notifications**: the sounds switches (with their "this device only" intro) and the ntfy alerts block.
  7. **app**: install updates automatically. Shown only where the updates section is shown today (the desktop app with
     an active updater); when it is absent, the sidebar has no "app" entry.
- **Drafts survive switching:** an unsaved field (checkout folder, post-clone command, ntfy fields, file types) keeps
  its draft, outcome line and any pending save when the user switches sections and comes back; only closing the dialog
  discards drafts, as today.
- **Which section opens:** the first section (sessions) every time the gear opens it. The two trust hint bands
  ("Settings and caveats" for Claude, "Settings" for Codex and Muse) open it at the agent trust section, at their
  switch, as they open it at the switch today.
- **No search box.**
- **Phone width:** at the dialog's existing narrow breakpoint (620px), the dialog follows the Templates dialog's narrow
  pattern (one pane at a time: the section list, or a section with a "‹ settings" back control to the list; use the
  closest equivalent of Templates' "‹ templates" and log the wording). The maintainer chose the sidebar option with this
  narrow behavior stated in its description. At narrow width the dialog still opens on the sessions section (with the
  back control visible), not on the list, and a trust hint band opens it on the agent trust section, so the rules below
  hold at every width.

### Acceptance criteria

- The dialog shows the sidebar and one section at a time, with the sections and contents above, in that order. Every
  control that exists today exists afterwards, with its label, help text, save behavior and scope wording unchanged
  except where a sentence named a section location that moved (for example help or footer text that says "under
  sessions"); fix those to the new names.
- Switching sections keeps every draft and in-flight save. Panels stay mounted (hidden when not selected) rather than
  unmounted, unless you find a cleaner way that provably keeps drafts, pending saves and the sounds switches working;
  `crates/farhelm-ui/assets/sounds.js` finds its inputs with a document-wide query, so check it still binds when its
  section is hidden.
- Keyboard: the section entries are keyboard-reachable under the platform's normal Tab rules (on macOS WebKit Tab skips
  buttons by default, per SPEC.md's keyboard rule; do not add extra tab stops to work around that). A plain list of
  buttons, as the Templates dialog uses, is acceptable and needs no key handling; if you choose the vertical `tablist`
  pattern instead, it must have its arrow keys (Up/Down, Home/End) as that pattern requires. Log which. Opening the
  dialog from the gear keeps today's opening focus (the first switch of the sessions section) unless the chosen pattern
  needs otherwise; Escape and either close control still return focus to the gear. Container Enter stays inert, as
  SPEC.md says for Settings. A scrollable pane with nothing focusable still gets its own named tab stop (SPEC.md's
  keyboard-scroll rule), and every new control has a tooltip (`data-tooltip`, no native `title`), as
  `e2e/tests/tooltip-coverage.spec.ts` requires.
- The trust hint bands select the agent trust section, then focus and scroll to the agent's switch block, as today
  (`focus_claude_trust` and `focus_workspace_trust` in `crates/farhelm-ui/src/settings.rs`); no caller needs any other
  section, so generalizing those props is optional.
- Each pane has a heading naming its section; the notifications pane keeps "sounds" and "ntfy alerts" as its two
  sub-headings. Plain-text pointers elsewhere that say where a setting lives (for example the host dialogs' "undo "don't
  ask again" with the gear at the top of the sidebar", the tunnel warning's "You can turn tunnels off in Settings", the
  launcher's "You can change it later in Settings") stay correct; turning them into deep links is out of scope.
- Web and desktop share the one component, as today; the desktop-only controls stay gated as today.
- The narrow-width behavior above works at 390px wide, as the current tests exercise.
- SPEC.md ("Session list", the Settings paragraph, and the "Managed checkouts" sentences that say Settings opens "at the
  switch" or name "the same Settings section") and SPEC_impl.md ("GUI: Dioxus", the Settings paragraph, which is already
  stale: it omits the sessions and ntfy sections) describe the new structure. The docs pages that name a Settings
  section or tour the dialog describe the new sections (see Requirement sources).
- Browser tests that assert the old single-page structure assert the new one; tests that open the gear and use a control
  outside the sessions section select its section first, ideally through one shared helper.
- The last code PR removes the TODO.md entry. A changelog fragment (`kind: changed`) describes the new layout for users.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Layout: "Left sidebar with sections, like the Templates dialog" (chosen over tabs across the top and collapsible
  sections).
- Grouping: as proposed (the seven sections above; the trust switches get their own section rather than staying under
  checkouts, and sounds and ntfy share "notifications").
- Switching sections with an unsaved field keeps the draft; only closing discards it, as today.
- The dialog always opens on the first section, except when opened from a trust hint.
- No search box.
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md: "Session list" (the Settings paragraph, roughly lines 1310-1345 at the time of writing), "Managed checkouts"
  (Settings saves each field explicitly and never rewrites a value on open; the trust switches; "Settings and caveats
  opens Settings at the switch"; the hint "offers Settings at its switch"), the keyboard-scroll rule ("every scrollable
  region is keyboard-reachable in every engine…"), the container-Enter rule for dialogs without one main action.
- SPEC_impl.md: "GUI: Dioxus" (the Settings paragraph and the save registry), the naming and frame switch sentences that
  say "beside session confirmations" and "under Sessions", and the sentences that call the trust switches
  "managed-checkout Settings" (around lines 3108 and 4920 at the time of writing).
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md` for test changes;
  `website/AGENTS.md` before editing any page under `website/`; `.agents/docs-media.md` before touching anything under
  `e2e/docs-shots/`.
- Published release shots and their tests (`e2e/docs-shots/release-notes.spec.ts`) are frozen: leave them alone even if
  they no longer match the UI (root `AGENTS.md`, "Frozen material").

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- `SettingsDialog` is in `crates/farhelm-ui/src/settings.rs`. It reuses the host dialogs' frame (`HostDialogHeader` and
  `install_dialog_with_selector` in `crates/farhelm-ui/src/hosts/settings_dialog.rs`), and its body is one
  `div.host-settings-body` holding a flat run of `section.host-settings-section` blocks. It embeds `checkout_root.rs`,
  `checkout_post_clone.rs`, `claude_trust.rs`, `workspace_trust.rs`, `sounds.rs` and `ntfy_settings.rs` (both of which
  render their own `<section>`), `tunnels.rs` (`TunnelSettings`, desktop only) and `OpenExtensionsSettings` (desktop
  only, in `settings.rs`).
- Settings-only CSS in `crates/farhelm-ui/assets/app.css` is scoped by `[aria-label="settings"]`, because the class
  `.app-settings-dialog` is shared with the welcome card, SSH help and feedback. Keep new rules scoped the same way.
- The closest precedents: the Templates dialog's sidebar and editor (`crates/farhelm-ui/src/list/templates.rs`, CSS
  around `.templates-sidebar`, with its narrow-width collapse), and the launcher's agent/command `tablist`
  (`crates/farhelm-ui/src/list/create_form.rs`, `.launch-kind-tabs`), which has roving `tabindex` and arrow keys but is
  hard-wired to two tabs.
- Browser tests that depend on today's structure: `e2e/tests/settings.spec.ts` (checkbox count, the exact section
  heading order, overflow at 560px tall, pinned header and footer, the switch-row layout, first focus),
  `claude-trust.spec.ts` and `workspace-trust.spec.ts` (opening at the switch), and every spec that opens the gear and
  uses a control by name (sounds, ntfy-settings, session-confirmations, terminal, terminal-link-clicks, quick-launch,
  session-action-shortcuts, provisioning), and `e2e/tests/tooltip-coverage.spec.ts`, whose Settings step requires a
  visible text field and sweeps only visible controls, so with hidden panels it must visit every section to keep its
  coverage. There is no shared "open Settings" helper. The docs shot `manage-hosts/app-settings`
  (`e2e/docs-shots/manage-hosts.spec.ts`) addresses regions by the old section names.
- Docs pages that tour the dialog or name a section: `website/src/content/docs/docs/using/manage-hosts.mdx` (the tour
  and its screenshot), `session-list.mdx`, `update-and-uninstall.md`, `stop-restart-resume.mdx`, `start-a-session.mdx`,
  `launch-templates.mdx`, `work-in-a-session.mdx`, `add-a-remote-host.mdx`, and the agent pages under
  `website/src/content/docs/docs/agents/`.

Proposed shape (planner proposal):

1. **The structure PR (`feat`):** the sidebar and section panes in `settings.rs`, the CSS, the narrow-width pattern, the
   keyboard behavior, the trust-band entry points, the browser tests that the restructure breaks (with a shared helper
   for opening Settings at a section), SPEC.md and SPEC_impl.md, and the changelog fragment. If this PR turns out too
   big to review well, split the test and helper migration into a PR before it that introduces the helper (passing
   against today's single page) and moves the specs onto it; log the split.
2. **The docs PR (`docs`):** the docs pages above, and updating the `manage-hosts/app-settings` shot's capture spec per
   `.agents/docs-media.md` (for example one shot of the dialog on a representative section; the old full-page shot no
   longer fits). Check it locally with `--only manage-hosts`, but do not publish it: a publishable capture must run on
   the latest main (`docs/docs-shots/SPEC.md`), so the published image catches up at the next screenshot refresh after
   landing; say so in the report. The README and release shots are not touched. Remove the TODO.md entry in this last
   PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression in what each PR changes, and report checks run, reused and skipped. For the structure PR:
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the UI crate's unit tests, the JS unit tests
if any JS changes, and the browser specs listed above through the recorder, on both engines the Playwright config runs
(Chromium and WebKit); the test-sleep checker when browser tests change. For the docs PR: the website checks
`website/AGENTS.md` names, and the docs-shot capture per `.agents/docs-media.md`.
`python3 releasing/check-changelog.py format` for the fragment.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: unmounting inactive sections with a new
draft store to keep their drafts, a new generic tab or navigation component shared with other dialogs, changing how any
setting saves, or adding a remembered-section preference.

Decisions to log: the dialog width, the ARIA pattern and where initial focus goes, how hidden panels are kept, the
narrow-width back control's wording, and how the trust-band entry points select their section.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-structured-settings-dialog-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/structured-settings-dialog/<nn>-<short-name>` (for example `plan/structured-settings-dialog/01-sidebar`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
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
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.

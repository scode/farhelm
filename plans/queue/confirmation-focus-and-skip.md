# Enter goes ahead on confirmations, and a setting skips the everyday ones

Written against main at e6c80c6c on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Two TODO entries, planned together because they touch the same confirmation prompts:

1. **Focus.** After clicking Delete, Replace, or another button that asks for confirmation, Enter should go ahead. So
   each confirmation opens with keyboard focus on the button the maintainer picked below, which is the confirming button
   everywhere except the YOLO confirmation. Today every one of them deliberately starts on Cancel, as SPEC_impl.md
   specifies.
2. **Skip setting.** A switch in the app's Settings dialog (the gear) that skips the confirmation step for the everyday
   session prompts listed below, so those actions happen on the first click.

### The maintainer's picks (2026-10-13)

The maintainer chose these from a mockup of every prompt with its real wording
(https://snippets.scode.org/s/farhelm-confirmation-focus/). "Today" is where focus goes now.

| Prompt (where it appears)                                               | Focus on open (Enter does)      | Today   | Skipped by the setting                                            |
| ----------------------------------------------------------------------- | ------------------------------- | ------- | ----------------------------------------------------------------- |
| `row-delete`: session row ⋯ menu, delete (confirm in place)             | `confirm delete`                | cancel  | yes                                                               |
| `row-replace`: session row ⋯ menu, replace                              | `confirm replace`               | cancel  | yes                                                               |
| `header-restart`: session header restart while the agent works          | `confirm restart`               | cancel  | yes                                                               |
| `header-replace`: session header replace popover                        | `replace`                       | cancel  | yes                                                               |
| `header-delete`: session header delete popover                          | `delete`                        | cancel  | yes                                                               |
| `card-replace`: interrupted-session card, replace                       | `confirm replace`               | cancel  | yes                                                               |
| `tab-close`: close a terminal tab                                       | `confirm close`                 | cancel  | yes                                                               |
| `yolo`: YOLO confirmation (launcher, Restart with, row/header replace)  | `cancel` (unchanged)            | cancel  | no: the helm enforces it, and the host's own setting turns it off |
| `restart-with`: Restart with dialog                                     | `cancel` (unchanged; see below) | cancel  | n/a: the dialog is the action                                     |
| `host-remove`: host menu, remove (modal)                                | `remove`                        | cancel  | no: it keeps its own "don't ask again"                            |
| `host-setup`: add host / automatic setup plan (modal)                   | `set up`                        | cancel  | no: it keeps its own "don't ask again"                            |
| `host-uninstall`: host menu, uninstall Farhelm (modal)                  | `uninstall`                     | cancel  | no: irreversible                                                  |
| `host-update`: host update plan fallback (inline, normally unreachable) | `confirm update`                | nothing | n/a: Update already applies without asking                        |
| `trash-delete`: trash dialog, delete all / delete all on a host         | `delete permanently`            | cancel  | no: permanent loss of uncommitted work                            |

The pasted code, verbatim:

```
confirm-focus: row-delete=confirm-delete, row-replace=confirm-replace, header-restart=confirm-restart, header-replace=replace, header-delete=delete, card-replace=confirm-replace, tab-close=confirm-close, yolo=cancel, restart-with=stop-and-restart, host-remove=remove, host-setup=set-up, host-uninstall=uninstall, host-update=confirm-update, trash-delete=delete-permanently
skip-setting covers: row-delete, row-replace, header-restart, header-replace, header-delete, card-replace, tab-close
```

After the planning review, the maintainer revised one pick: Restart with keeps Cancel focused at open ("Keep Cancel").
Its primary button is disabled until a setting changes, so it cannot take focus on open, and Enter on an edited control
already restarts.

### Acceptance criteria

- Each prompt above opens with focus on the picked button, by whatever mechanism the prompt uses today (`autofocus`, the
  YOLO confirmation's scripted `focus_cancel`, the host dialogs' quiet focus that shows the ring without the tooltip,
  the trash band's `onmounted` focus). Enter then activates that button. A prompt that re-mounts (the row menu's prompt
  survives the panel closing and re-autofocuses) lands on the same button again. Escape and pointer-outside behavior
  stay exactly as today.
- Most picked buttons can be disabled when their prompt opens (a page-wide busy token, a restart or replace in flight, a
  host dialog's pending request), while Cancel never is. Rule: focus the picked button if it is enabled when the prompt
  opens, otherwise Cancel, as the trash band's cancel already falls back from a disabled control.
- Held Enter: an auto-repeated Enter from the key that opened the prompt (activating the row menu's delete item, say)
  must not answer it. The codebase already ignores auto-repeat for Enter actions (`is_auto_repeating()` in
  `restart_with.rs`, `launch_controls.rs`, `create_form.rs`); apply the same to these prompts' confirming buttons.
- Restart with is unchanged: focus stays on Cancel at open.
- For the host dialogs, the new focus target keeps the quiet initial focus (ring, no hover help) that Cancel has today.
  These dialogs set focus by script as well as by `autofocus`, and the same selector argument also feeds Escape
  (`install_remove_dialog` passes `.host-settings-close`, `install_uninstall_dialog` passes `.uninstall-cancel`,
  `focus_add_cancel` focuses `.add-host-cancel`, all in `hosts/settings_dialog.rs`): change only the initial-focus
  argument and leave Escape's alone. The remove dialog's `remove` button has no class to select it by; add one.
- Comments that justify cancel focus as a safety default are updated with the code: `menu_panel.rs` (`PanelPlacement`,
  the "SAFETY default" comment), `list/row.rs`'s component docs, `session_view.rs` (restart popover, tab close), and
  `settings_dialog.rs` (`install_remove_dialog`, `install_uninstall_dialog`).
- The skip setting is one app-wide boolean, shared like the existing host-confirmation switches: a helm-side preference
  (`Preferences`/`PreferencePatch` in `crates/farhelm-helm/src/store.rs`, the `/api/preferences` handler, and the UI's
  `PreferenceField`/`SharedPreferences`), shown in the Settings dialog (`crates/farhelm-ui/src/settings.rs`), off by
  default. Its label and help text say which actions it skips and that YOLO, host and trash confirmations still ask.
- With the setting on, the seven skipped actions run on the first click with no prompt: the click calls the same confirm
  path the prompt's button calls today, with the guard or consent taken from the state the screen shows (Delete's
  `DeleteGuard` from `status::delete_guard`, Restart's stop consent). A refusal goes through the existing error path
  unchanged, as `e2e/tests/delete-precondition.spec.ts` pins today for the one delete that never asks: the row shows the
  error, the list is re-read, and the next click decides from the current state. No confirmation opens on its own. (The
  only refusal skipping can newly reach is a Delete or Replace of a session shown with its agent ended and tabs open,
  when the agent came back before the click.)
- The header delete button's tooltip ("... after you confirm") and the comment that it "always confirms" become
  conditional on the setting.
- SPEC.md: Lifecycle operations and Session view say which confirmations the setting skips, and the rule that "a
  destructive confirmation authorizes only what the prompt the user answered said would happen" gains its counterpart
  for a skipped prompt (the click authorizes what the screen showed, enforced by the same guards). Session list's
  description of shared preferences, and Errors and diagnostics' list of best-effort helm-side preference fields,
  include the new switch. SPEC_impl.md: every "focus on cancel" statement (the restart popover, the row menu's Confirm
  in place rule, `HostRemoveDialog`, the host confirmations' quiet focus, and any other the audit finds) states the new
  target and why (the maintainer wants Enter to go ahead after an explicit click).
- **Downgrade (maintainer accepted, 2026-10-13).** The new preference column bumps the helm's schema version, after
  which an older helm refuses to open its database, as with earlier preference columns. The maintainer accepted the
  missing downgrade path; say so in the fragment.
- Tests: the e2e specs that pin Cancel focus today are updated to the new targets (notably `terminal.spec.ts`'s "the
  confirm prompt focuses cancel on open; Enter closes it without deleting", which must flip), and at least one test per
  kind of prompt (menu, popover, inline, band, modal) shows Enter performing the picked action. The skip setting's seven
  actions go through four UI paths (row menu, header, interrupted card, tab strip): cover them with unit tests of the
  pure decisions plus a spec that walks the actions with the preference on, rather than seven separate specs. A skipped
  Delete refused by its guard is tested in `delete-precondition.spec.ts` with its existing refusal injection, and the
  prompts the setting must not skip are tested too. The normally unreachable host update fallback needs no dedicated
  test.
- Docs website: wherever a page describes these confirmations or the Settings dialog, it is updated
  (`website/AGENTS.md`, `website/EDITORIAL_RULES.md`).
- A changelog fragment (`kind: changed` for the focus, `added` for the setting, one or two fragments as the stack's
  shape suggests).
- Both TODO.md entries ("Keyboard focus on confirmations" and "Setting to turn off confirmations") are removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: the native `<dialog>` entry under Maybe later; agent approval cards (trust gates, not confirmations); the
CLI's `farhelm uninstall` prompt; any change to which actions confirm when the setting is off; folding the host
setup/remove switches into the new setting.

## Requirement sources

**The user's request (2026-10-13):** plan these Near term TODO entries. Their text:

> **Keyboard focus on confirmations.** After clicking Delete, Replace, or any other button that asks for confirmation,
> the user should be able to press Enter to go ahead, so focus should land on the confirmation's confirm button. Today
> the row menu's confirm-in-place prompts, the restart confirmation, and the remove-host dialog deliberately start focus
> on Cancel, as SPEC_impl.md specifies (the row menu's code calls it a safety default). Audit every button with a
> confirmation in the entire app: list each one, where focus goes today, and where it should go. Changing SPEC_impl.md's
> cancel default is part of this entry; the per-button decisions get settled when the entry is planned. Related: the
> native `<dialog>` entry under Maybe later, which would change how these dialogs manage focus.

> **Setting to turn off confirmations.** A settings toggle that skips the confirmation step on Delete and similar
> actions, so they happen on the first click. Which confirmations it covers, and the rest of the details, are to be
> decided when the entry is planned. Related: the keyboard focus entry above, whose audit of every confirmation in the
> app is the list this setting would apply to.

**The user's decisions (2026-10-13):**

- Plan the two entries as one plan.
- The audit was done in planning and shown as the mockup above; the maintainer picked the focus target and the skip
  coverage for every prompt by pasting the code above ("agree with your default/recommended options").
- After the planning review: Restart with keeps Cancel ("Keep Cancel"), and the missing downgrade path for the new
  preference column is accepted ("Accept all three", covering this and two other plans).
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md Lifecycle operations (which actions confirm, "authorizes only what the
prompt ... said"), Session view, Session list (shared preferences), Managed checkouts; SPEC_impl.md "GUI: Dioxus" (focus
rules quoted above); root `AGENTS.md` Docs website, Conventional Commits, Finishing work, Releases and the changelog,
Talking to the user; global instructions (tests never modify the test process's own environment variables);
`.agents/test-authoring.md` for every test change; `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below, the disabled-button and held-Enter rules, and the setting's wording.

## Outline

Line numbers drift; find the code by name. Here's what exists today (from the planning audit; recheck it, and add any
prompt the audit missed to the work with the nearest analogous pick, logged as a DECISION):

- Row menu prompts: `SessionRow` in `crates/farhelm-ui/src/list/row.rs` (`.confirm-delete`/`.confirm-cancel`,
  `.confirm-replace`/`.replace-cancel`), decided by `ListView::on_delete`/`on_replace` in `list/view.rs` and
  `DeleteTarget` in `list/shared.rs`.
- Session view prompts in `crates/farhelm-ui/src/session_view.rs`: restart popover (`confirming`), header replace and
  delete (`confirming_header_replace`, `confirming_header_delete`, `ConfirmSlot`), interrupted-card replace
  (`confirming_replace`), tab close (`confirming_close`).
- `YoloConfirmation` in `yolo_confirm.rs` (unchanged focus); `RestartWithDialog` in `restart_with.rs`.
- Host prompts: `HostRemoveDialog` in `hosts/settings_dialog.rs`; `SetupPlanConfirmation`, `UninstallConfirmation`,
  `PlanConfirmation` in `provisioning.rs`; `AddHostForm` in `hosts.rs`.
- Trash: `ConfirmBand` in `list/trash.rs`.
- The existing skip switches `skip_host_setup_confirmation` / `skip_host_remove_confirmation` show the preference
  pattern end to end.

Here's the work, as a proposal:

1. **Focus**: move each prompt's initial focus to its picked button; update SPEC_impl.md and the focus tests.
2. **Skip setting**: the preference (helm store migration, API, UI preference plumbing), the Settings switch, the
   skipped paths in the row menu, session header, card and tab strip, SPEC.md, tests.
3. Docs website, changelog, TODO removal.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-ui and farhelm-helm tests touched (preferences, store migration, delete/replace
  decisions), by filter while iterating and the affected packages once;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the specs that pin confirmation focus and confirmation behavior (the planning
  audit found them in `terminal.spec.ts`, `sidebar.spec.ts`, `header.spec.ts`, `replace.spec.ts`,
  `terminal-tabs.spec.ts`, `terminal-restart.spec.ts`, `delete-precondition.spec.ts`, `trash-dialog.spec.ts`,
  `github-checkouts.spec.ts`, `provisioning.spec.ts`, `terminal-multihost.spec.ts`, `yolo-guard.spec.ts`,
  `launcher-enter.spec.ts`, `settings.spec.ts`, `tooltip.spec.ts`, and the helper
  `e2e/tests/helpers/host-dialog-tooltip.ts`); WebKit matters because focus behavior differs there;
- the website build if docs change; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-confirmation-focus-and-skip-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/confirmation-focus-and-skip/<nn>-<short-name>` (for example
  `plan/confirmation-focus-and-skip/01-confirm-focus`).
- Expected shape: the focus change and the skip setting as separate PRs, each leaving main correct on its own. Avoid
  churn: code added in one PR and deleted in a later one means the stack should have been shaped differently. Within
  this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier
  run built too.
- Commit messages and PR titles use Conventional Commits; this is user-visible behavior, so `feat` or `fix` as fits each
  PR. Each such PR adds its changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md`
  (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes both TODO.md entries this plan covers.
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
- the acceptance criteria: this file's goal, the maintainer's picks, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that no prompt outside the skip list can be skipped; that a skipped action still
carries its supervisor guard; and that Enter on a freshly opened prompt does exactly the picked action and nothing else.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a new confirmation framework,
  per-prompt settings, or changing which actions confirm when the setting is off (examples, not a blacklist);
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
alternatives considered. In particular: any prompt the audit missed and the pick you gave it; any prompt where the
picked button is normally disabled at open; the held-Enter handling; the setting's name, label and help text; the
stack's shape; every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- A prompt the planning audit missed gets the pick of its nearest analogue in the table (a session prompt: confirming
  button and skipped; anything irreversible or helm-enforced: not skipped), recorded in the report.
- If a picked focus target cannot be made reliable on WebKit by the prompt's existing focus mechanism, use the mechanism
  the YOLO confirmation uses (scripted focus after mount) and log it.
- If a picked button is unavailable when its prompt opens, focus stays on Cancel (the rule above); log any prompt where
  that is the normal case rather than a busy moment.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: skipping a prompt outside the list, dropping a
supervisor guard, or changing the YOLO confirmation.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes both TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

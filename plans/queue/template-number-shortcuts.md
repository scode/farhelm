# Template number shortcuts: ⌘0–⌘9 each launch a template directly

Written against main at 4c9e9612 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan. It builds on quick launch (⚡ mark plus ⌘P), which is already on main
(#1994).

## The goal

Any launch template can be given one of ten number keys, 1 to 9 and 0. Pressing the chord for that key starts a session
from the template straight away, with no launcher in between, exactly the way ⌘P does for the ⚡ quick launch template.
The chord is **Cmd+digit in the Mac desktop app** and **Ctrl+Shift+digit everywhere else** (the web UI on any platform,
the web UI on a Mac included, and the Linux desktop app).

The maintainer approved this mockup (revision 2): https://snippets.scode.org/s/farhelm-template-number-shortcuts/ (the
key button beside each row's ⚡ in the templates list, its menu, the chord text per platform, and what the chord does).
Treat its wording as the starting point. The editor pane on the right of the templates dialog does not change.

### Acceptance criteria

- **Binding, in the templates list.** Each template row in the templates dialog
  (`crates/farhelm-ui/src/list/templates.rs`) gets a key button beside its ⚡ toggle. With a key it shows the chord
  (`⌘1` in the Mac app, `Ctrl+Shift+1` elsewhere); without one it shows a faint `⌘ –` (or `Ctrl+Shift+ –`). Clicking it
  opens a short menu: "no shortcut", then the ten keys in the order 1 to 9, then 0. A key another template holds is
  labelled with that template's name, and picking it moves the key to this template. One template per key; a template
  has at most one key. A key and the ⚡ mark are independent: a template can have both. Build the menu with the app's
  existing popup menu machinery (`menu_panel`) if it works inside the templates dialog; it has not been used inside a
  dialog before. If that proves impractical, a native `<select>` per row that shows the same choices is the agreed
  fallback, logged as a DECISION, even though it looks a little different from the mockup.
- **Eligibility** is the ⚡ rule (`quick_launch_eligible`: the template chooses an agent or a nonblank command). An
  ineligible template's key button is disabled with hover text saying why. As with the lit bolt, a template that already
  has a key and has since become ineligible keeps a clickable button so the key can be removed; its menu then offers
  only "no shortcut". The rule is enforced in the interface only, as ⚡'s is.
- **Hover help (maintainer's request).** Both the new key button and the existing ⚡ bolt get hover text that explains
  the feature, not just the action: what the chord does (starts a session from this template right away, without opening
  New), that only one template can hold the ⚡ mark or a given key, and the current state (which chord, or how to set
  one). Today's bolt hover ("Make this the quick launch (⌘P)") does not explain what quick launch is; rewrite it. Use
  the app's `data-tooltip` mechanism, and keep `e2e/tests/tooltip-coverage.spec.ts` passing.
- **Chord text per client.** Today `QUICK_LAUNCH_CHORD` is the static string `⌘P / Ctrl+Shift+P`. The number key's chord
  text names the one chord the current client uses (the mockup's platform switch shows both). Do not change how the
  quick launch chord text reads beyond the hover rewrite above.
- **Storage.** The bindings are one new field on the helm's shared preferences row (`Preferences`/`PreferencePatch` in
  `crates/farhelm-helm/src/store.rs`, `/api/preferences`, the UI's `PreferenceField`/`SharedPreferences`), mapping each
  bound key to a template name, beside `quick_launch_template`, not inside the templates. A write replaces the whole
  map; two windows editing bindings at the same moment can lose one edit, which is accepted (no per-key merging on the
  helm). Renaming a template keeps its key: the templates dialog re-points the binding between the save and the delete
  it already does for ⚡, by generalizing the existing ⚡ helpers rather than writing a bindings-only copy. Deleting a
  template frees its key in the helm, in both of the store's template delete functions, covering the dialog, the API and
  `farhelm agent template delete` alike, as for ⚡. Undo of a delete does not restore the key. Validate the field on
  write (keys only `0`–`9`, each name syntactically a template name, at most one key per template) with the same size
  discipline the preferences PUT already applies; like ⚡, do not require the named template to exist. The store's
  schema-rewind test fixtures that rebuild older schemas need the new column, as they did for `quick_launch_template`.
- **Downgrade (maintainer accepted, 2026-10-14).** The new preference column bumps the helm's schema version, after
  which an older helm refuses to open its database, as with the quick launch column. Say so in the changelog fragment.
- **The chord.** Captured before xterm in the capture phase, exactly the way the quick launch chord is
  (`install_quick_launch_shortcut` in `crates/farhelm-ui/src/lib.rs`): an open modal (`role="dialog"` with
  `aria-modal="true"`) keeps the keyboard, auto-repeat does not launch twice, and the chord never reaches the terminal
  program. Match the physical digit keys (`event.code` `Digit0` to `Digit9`, as the text-size shortcut in
  `assets/terminal.js` matches physical keys), so Ctrl+Shift+3 works on layouts where Shift+3 is not `#`; the numeric
  keypad is not part of it. Which chord applies is decided by the build, not by the browser's platform string the quick
  launch listener reads: Cmd+digit (Meta only, no other modifier) where the build is the Mac desktop app, the same
  condition as `HAS_NEW_SESSION_SHORTCUT`; Ctrl+Shift+digit (no Meta, no Alt) everywhere else, a browser on a Mac
  included. Hand the pressed key to Rust the way quick launch does, with its own hidden triggers (one per key, under a
  new class name: an existing test expects exactly one `.quick-launch-trigger`). Keep the key-matching logic a function
  a test can call with either build's setting, because Playwright only drives the web build and can never press
  Cmd+digit.
- **What it does**, per the mockup's table, exactly as ⌘P does for its template:
  - a bound key: launch exactly what New would launch from that template, and select the new session;
  - an unbound key (maintainer decision): open the templates dialog with a line saying how to give a template a number
    key, as ⌘P does with no ⚡ template;
  - a YOLO confirmation, a failed launch, or a bound template that no longer launches anything on its own: show New with
    the template applied and the confirmation or error showing, through the same fallbacks quick launch has.
- **Reuse.** Generalize `quick_launch` in `list/view.rs` to launch a named template, and route both ⌘P and the number
  keys through it. The hidden auto-launching New form in `list/create_form.rs` already takes a template name, so it
  should need no change. Read the bindings and templates fresh from the helm on each press, as quick launch does, so a
  change made in another window takes effect on the next press. Do not write a second launch path.
- **Managed checkouts.** SPEC.md says marking a template quick launch counts as consent for it to create a managed
  checkout and launch without showing the path first. Giving a template a number key counts the same way, since the
  chord does exactly what ⌘P does; extend that sentence.
- **Discoverability.** The templates dialog's intro line mentions the number keys (as in the mockup), and the dialog's
  footer may list the bindings as the mockup does.
- **Browser conflicts.** Test the chords on Chromium and WebKit, the engines the repository's Playwright setup runs. A
  Playwright test cannot prove whether a real browser reserves a chord; if you learn that one does, document that in the
  docs rather than picking another chord.
- SPEC.md: Creation (beside the "Quick launch" bullet) describes number keys, their binding, the chords and fallbacks;
  wherever shared preferences are listed includes the bindings; the keyboard sections that list chords (Desktop window
  chrome's Cmd+N paragraph, Terminal experience) mention them where that helps a reader find them. SPEC_impl.md: where
  the bindings are stored, how the chord is matched, and that ⌘P and the number keys share one launch path.
- Docs website: the templates page (`using/launch-templates.mdx`) and wherever quick launch and keyboard shortcuts are
  documented (`using/session-list.mdx`, `using/start-a-session.mdx`) explain number keys (`website/AGENTS.md`,
  `website/EDITORIAL_RULES.md`). Do not refresh the docs screenshots; if the templates dialog's shots are now stale, say
  so in the report.
- Tests: binding, moving and clearing keys; rename keeping and delete freeing a key (helm store and UI); the eligibility
  rule and the clickable-when-bound exception; both chords launching (Cmd+digit is Mac-desktop-only, so test the logic
  that picks the chord where a browser test cannot press it); the unbound, YOLO and failure fallbacks; the modal guard;
  that the chord does not reach the terminal; and the hover texts. Extend `e2e/tests/quick-launch.spec.ts` and
  `e2e/tests/templates.spec.ts` and the helm store's tests rather than adding new harnesses.
- Changelog fragments (`kind: added`), one per `feat` PR as root `AGENTS.md` requires.
- The TODO.md entry ("Number shortcuts for launching templates.") is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: quick switcher rows for number keys, configurable modifiers or more than ten keys, the numeric keypad, a
per-device binding, the CLI, and changing what ⌘P does.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Number shortcuts for launching templates.** Let the user bind each of Cmd+0 through Cmd+9 to a template, so that
> pressing it launches a session from that template right away, the same way the Cmd+P template quick launch (being
> planned, not yet on main) does once a template is picked. Build on that work once it lands. Browsers keep Cmd/Ctrl
> with a digit for switching tabs and Cmd/Ctrl+0 for resetting zoom, the same reason the web UI has no Cmd+N, so decide
> what the web UI and the chord off macOS get.

(Quick launch has since landed, #1994.)

**The user's decisions (2026-10-14):**

- Chords: Cmd+digit in the Mac desktop app, Ctrl+Shift+digit everywhere else, chosen from a list that stated Chrome's
  and Safari's tab-switching and zoom chords, Ctrl+Shift+0's Windows keyboard-layout chord, and the physical-key
  matching.
- The binding UI: a first mockup put a row of number buttons in the editor pane; the maintainer found it confusing ("so
  why is there a list of shortcuts, some of them highlighted, some of them gray, when a specific template is selected?
  What am I supposed to interpret from that?", and "The left-hand sidebar makes sense. I'm not sure what's going on on
  the right side"). Revision 2 put a key button with a menu in each list row and left the editor unchanged; the
  maintainer approved it: "looks good, lets make sure there is on-hover help on both what we're adding now and the
  existing quick launch bolt".
- An unbound key opens the templates dialog with a hint, rather than doing nothing.
- Storage as a shared preference column, accepting the schema bump with no downgrade path.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md Creation (the launch composer, templates, the "Quick launch" bullet, the
YOLO confirmation that "no client can skip"), Session list (shared preferences), Desktop window chrome (the Cmd+N
paragraph, which explains why the web UI does not take browser chords), Terminal experience; SPEC_impl.md's launch
template, launcher and quick launch sections; root `AGENTS.md` Docs website, Conventional Commits, Finishing work,
Releases and the changelog, Talking to the user; global instructions (tests never modify the test process's own
environment variables); `.agents/test-authoring.md` for every test change; `plans/AGENTS.md`, Executing one plan.

**Planner proposals:** the storage shape and validation, the shared launch path, the key order 1–9 then 0, the
`event.code` matching and keypad exclusion, deciding the chord by build (Mac desktop app versus everything else) rather
than by the browser's platform string, last-write-wins for the whole binding map, the menu built on `menu_panel`, and
the test placement. Several of these came from the planning review.

## Outline

Line numbers drift; find the code by name. Here's what exists today:

- Templates: `farhelm_proto::launcher::{TemplateFields, LaunchTemplate}`; stored in the helm's `launch_templates` table,
  served by `/api/templates`; edited in `list/templates.rs` (the ⚡ toggle is the `templates-quick-mark` button, the
  `mark` handler, rename re-pointing and `prepare_template_delete`).
- Quick launch: the `quick_launch_template` preference column (store, migration, `/api/preferences` size check in
  `crates/farhelm-helm/src/preferences.rs`, `write_quick_launch_template` and `PreferenceValue::QuickLaunchTemplate` in
  `crates/farhelm-ui/src/api.rs`); `install_quick_launch_shortcut` in `lib.rs`, which clicks a hidden
  `.quick-launch-trigger`; `quick_launch` in `list/view.rs`; the hidden auto-launch New form in `list/create_form.rs`;
  the `quick_launch_hint` line in the templates dialog.
- Shortcuts: every global chord is a capture-phase `keydown` listener installed from `lib.rs`;
  `HAS_NEW_SESSION_SHORTCUT` is the Mac-desktop-only build condition.

Here's the work, as a proposal:

1. **Helm storage**: the preference field, its migration, validation, delete handling in both delete functions, tests.
2. **The feature**: the key button and menu in the list, rename re-pointing, the hover texts (both buttons), the intro
   line, the generalized launch path, the chord listener and its triggers, the unbound-key hint, tests, SPEC.md,
   SPEC_impl.md, docs website, changelog, TODO removal.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-helm (store, preferences, templates) and farhelm-ui tests touched;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the templates, quick launch and tooltip coverage specs;
- `cargo check -p farhelm-ui --features desktop`, since the chord condition differs in the desktop build;
- the website build if docs change; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-template-number-shortcuts-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/template-number-shortcuts/<nn>-<short-name>` (for example `plan/template-number-shortcuts/01-key-storage`).
- Expected shape: two PRs, the helm storage (a preference nothing uses yet, which its PR says), then the user-facing
  feature whole, so main never shows a half feature (bindings that cannot be pressed, or chords that cannot be bound).
  This is how quick launch landed. Avoid churn: code added in one PR and deleted in a later one means the stack should
  have been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on
  top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; this is a new feature, so `feat`. Each such PR adds its
  changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the mockup's link, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically: that a number key launches exactly what ⌘P would for the same template; that the
YOLO confirmation can never be bypassed; that "one template per key, one key per template" holds across rename, delete
and concurrent clients; and that the chord cannot reach the terminal or fire under a modal.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a second launch path, a binding stored
  inside templates, a configurable chord, or quick switcher rows (examples, not a blacklist);
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
alternatives considered. In particular: the storage shape, how the quick launch path was generalized, the hover texts,
the browser chord results, wording changes from the mockup, the stack's shape, and every review finding you decided not
to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- A browser that will not let the page claim Ctrl+Shift+digit keeps its own behavior there; document it. Do not pick a
  different chord.
- Wording in the mockup that turns out inaccurate is corrected to the truth.
- If `menu_panel` cannot host the key menu inside the templates dialog, use a native `<select>` per row with the same
  choices.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a different chord, skipping the YOLO
confirmation, moving the binding control into the editor pane, letting a template hold several keys.

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

# Quick launch: ⌘P starts a session from a marked template, with no launcher

Written against main at e6c80c6c on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

For when you just want an agent now: one launch template is marked as the **quick launch** template, and a single key
chord, **Cmd+P** on the Mac and **Ctrl+Shift+P** elsewhere, starts a session from it immediately. The launcher never
opens in between. That is the difference from the quick switcher's `tl:name`, which opens New with the template applied.

The maintainer approved this mockup as is: https://snippets.scode.org/s/farhelm-quick-launch/ (the ⚡ marker in the
templates list, where the chord is advertised, and what the chord does in each case). Treat its wording as the starting
point.

### Acceptance criteria

- **Marking.** Each template row in the templates dialog (`crates/farhelm-ui/src/list/templates.rs`) has a bolt (⚡)
  toggle button. At most one template is the quick launch: marking another moves the mark, and pressing the lit bolt
  clears it. The marked row shows the chord. A template that chooses neither an agent nor a command (it leaves the
  launcher's kind unset or sets only fields like effort) cannot be marked: its bolt is disabled with hover text saying
  why. The editor pane says whether the selected template is the quick launch.
- **Storage.** The mark is one nullable field on the helm's shared preferences row naming the template
  (`Preferences`/`PreferencePatch` in `crates/farhelm-helm/src/store.rs`, `/api/preferences`, the UI's
  `PreferenceField`/`SharedPreferences`), not a field inside the template, where another client's stale editor save
  could silently move it. Templates are keyed by name (`launch_templates`). There is no rename API: the templates dialog
  saves the new name and then deletes the old one, so the dialog re-points the mark between those two calls. Deleting
  the marked template clears the mark in the helm's shared delete path, which covers the dialog, the API and
  `farhelm agent template delete` alike. Undo of a delete re-creates the template but does not restore the mark.
- **Downgrade (maintainer accepted, 2026-10-13).** The new preference column bumps the helm's schema version, after
  which an older helm refuses to open its database, as with earlier preference columns. The maintainer accepted the
  missing downgrade path; say so in the fragment.
- **Freshness.** Clients read shared preferences once at sign-in, so the chord reads the mark (and the templates) fresh
  from the helm when pressed, and a mark moved in another window takes effect on the next press.
- **The chord.** Cmd+P (Mac) / Ctrl+Shift+P (elsewhere) is captured before xterm, exactly the way the quick switcher's
  Cmd+K / Ctrl+Shift+K is (`install_quick_switcher_shortcut` in `crates/farhelm-ui/src/lib.rs`): an open modal
  (`role="dialog"` with `aria-modal="true"`) keeps the keyboard, auto-repeat does not launch twice, and the chord never
  reaches the terminal program. It works in both the desktop app and the browser.
- **What it does**, per the mockup's table:
  - with a quick launch template: launch exactly what New would launch from that template applied to New's defaults
    (host and folder from the open session, else this machine's home folder, as New does), and select the new session,
    as a launch from New does;
  - with no quick launch template: open the templates dialog with a line saying to mark one with ⚡;
  - when the helm asks for a YOLO confirmation, or the launch fails (host offline, folder missing, refused, a managed
    checkout's path taken by another create): show New with the template applied and the confirmation or the error
    showing, so nothing is lost and the user finishes there. Nothing starts until the YOLO question is answered;
  - when the marked template no longer launches anything on its own (edited since it was marked): show New with it
    applied.
- **How it launches (planner proposal from the planning review).** There is no headless composition to reuse:
  `template_edits` replays a template onto the New form's live state, and the create request, remembered permissions,
  the double-submit key, the checkout preview and the YOLO retry all live in `CreateSessionForm`'s submit path. So reuse
  the path the quick switcher's `tl:` already takes into New with a template applied (`open_new` in `list/view.rs`),
  plus an auto-launch flag: the form mounts hidden, applies the template, launches once through its ordinary Launch
  path, and closes and selects the session on success; on a YOLO question or an error it is simply revealed with that
  showing. This gives "exactly what New would launch" and the unskippable YOLO confirmation by construction. The real
  cost is keeping the form hidden (focus, the modal check, re-entry while busy). Do not write a second copy of the
  composition rules. If hiding the form proves impractical, a visible launcher conflicts with the maintainer's "no
  launcher in between": that is a block, not a logged decision.
- **Managed checkouts (maintainer decision).** A quick launch template may create a fresh managed checkout and still
  launch directly: marking a template quick launch counts as the user's consent, so the launch previews the checkout and
  launches without showing the path first. A lost race for the previewed path still never resubmits: it stops and shows
  New with the conflict and the new preview, as the failure fallback above. SPEC.md's rule that "Before Launch becomes
  available, the composer shows the exact host and path" gains this exception, stated with its reason.
- **Advertising.** The Templates button's hover names the quick launch template and the chord (or says to mark one), and
  the quick switcher (`list/quick_switcher.rs`) lists a "Quick launch: <template>" row that does the same thing as the
  chord.
- **Browser conflicts.** Test the chord on Chromium and WebKit, the engines the repository's Playwright setup runs
  (Firefox is deliberately not configured, and a Playwright test cannot reliably show whether a browser reserves a
  shortcut anyway). Cmd+P is the browsers' Print, which pages can override. Ctrl+Shift+P is Firefox's private window,
  which Firefox may not let a page override: document that in the docs (the quick switcher row still works there) rather
  than picking another chord.
- The bolt's eligibility rule (a template must choose an agent or a command) is enforced in the interface only.
- SPEC.md: Creation describes quick launch, the mark, the chord and its fallbacks, and amends the checkout-preview rule
  as above; the quick switcher's description currently says a pick never launches a session, which the new row
  contradicts, so amend that sentence; wherever shared preferences are listed includes the mark. SPEC_impl.md: where the
  mark is stored, why the chord is captured where it is, and how the launch reuses New's form.
- Docs website: the templates and launching pages, and the quick switcher's description in `using/session-list.mdx`,
  explain quick launch (`website/AGENTS.md`, `website/EDITORIAL_RULES.md`).
- Tests: the mark (one at a time, rename and delete, disabled for templates that launch nothing), the chord's launch,
  the no-template, YOLO and failure fallbacks, the modal guard, and that the chord does not reach the terminal.
- Changelog fragments (`kind: added`), one per `feat` PR as root `AGENTS.md` requires.
- The TODO.md entry ("A quick agent mode") is removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: more than one quick launch template, a per-host or per-device quick launch, a configurable chord, the CLI,
and changing how `tl:name` behaves. The Near term entry "Number shortcuts for launching templates" (Cmd+0 to Cmd+9, each
bound to a template) is a later plan that builds on this one: do not build it, but do not shape the mark or the chord
handling so it cannot be added beside them.

## Requirement sources

**The user's request (2026-10-13):** plan this Near term TODO entry. Its text:

> **A quick agent mode.** For when you just want an agent now, with as little fuss and as few choices as possible: a way
> to start an agent session without working through the launcher's fields. What it skips, what it picks instead, and
> where it lives in the UI are to be worked out.

**The user's decisions (2026-10-13):**

- In the maintainer's words: "basically i am tinking a single key chord (e.g. cmd+p - ideally right hand for the
  character on qwerty) and it instantly launches a fixed template. i am thinking we have a special "quick launch"
  template (perhaps it's a setting for the template, like a "star" or similar, that you click int he template list to
  make it the quick launch - not a start but we need some good indicator). so you get to pick what it is with the sample
  flexibility as templates." And: "we already hav cmd-k + tl:name ... NOT same behavior / this one directly launches,
  doesn't take you to the launcher in between".
- The chord: Cmd+P on the Mac, Ctrl+Shift+P elsewhere, following the quick switcher's pattern (chosen from a short list
  with the known browser conflicts stated).
- The mockup above, approved as is.
- After the planning review: a quick launch template that creates a managed checkout launches directly, amending
  SPEC.md's checkout-preview rule ("Launch directly, amend SPEC"); the missing downgrade path for the new preference
  column is accepted ("Accept all three", covering this and two other plans).
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md Creation (the launch composer, templates, the YOLO confirmation that "no
client can skip"), the quick switcher's description, Session list (shared preferences); SPEC_impl.md's launch template
and launcher sections; root `AGENTS.md` Docs website, Conventional Commits, Finishing work, Releases and the changelog,
Talking to the user; global instructions (tests never modify the test process's own environment variables);
`.agents/test-authoring.md` for every test change; `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below, the storage shape, launching through New's hidden form, reading the mark
fresh on each press, the "chooses an agent or a command" eligibility rule, and the fallbacks in the mockup's table
(which the maintainer approved with the mockup).

## Outline

Line numbers drift; find the code by name. Here's what exists today:

- Templates: `farhelm_proto::launcher::{TemplateFields, LaunchTemplate, apply_template}`; stored in the helm's
  `launch_templates` table, served by `GET/PUT/DELETE /api/templates`; edited in `list/templates.rs`; applied in the
  launcher as search edits (`template_edits`).
- Launching: `CreateSessionForm` in `list/create_form.rs` builds the create body and calls `api::create_session`
  (`POST /api/sessions`); the helm's YOLO guard answers with a header that the UI turns into `YoloConfirmation`. New's
  defaults come from `default_create_host` / `effective_create_host` (`list/shared.rs`) and `open_new` in
  `list/view.rs`.
- Shortcuts: `install_quick_switcher_shortcut` and `install_new_session_shortcut` in `crates/farhelm-ui/src/lib.rs`.
- Preferences: `Preferences` / `PreferencePatch` in the helm store and `/api/preferences`; `PreferenceField` and
  `SharedPreferences` in the UI.

Here's the work, as a proposal:

1. **The mark**: storage, the bolt in the templates dialog, rename and delete handling, tests.
2. **The chord and the launch**: the shortcut, the auto-launching hidden New form, the fallbacks that reveal it, the
   quick switcher row and the Templates hover, tests.
3. SPEC.md, SPEC_impl.md, docs website, changelog, TODO removal.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- through the recorder, the farhelm-proto, farhelm-helm (store, preferences, templates) and farhelm-ui tests touched;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, since tests change;
- Playwright on Chromium and WebKit for the templates dialog, the quick switcher and the new chord specs;
- `cargo check -p farhelm-ui --features desktop` if desktop-only code changes;
- the website build if docs change; `dprint check` on changed Markdown; `python3 releasing/check-changelog.py format`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-quick-launch-template-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/quick-launch-template/<nn>-<short-name>` (for example `plan/quick-launch-template/01-quick-launch-mark`).
- Expected shape: the mark, then the chord and launch, each leaving main correct on its own (a mark nothing uses yet is
  fine if its PR says so, or land them together). Avoid churn: code added in one PR and deleted in a later one means the
  stack should have been shaped differently. Within this run, a PR that needs correcting is restructured rather than
  corrected on top, and that applies to PRs an earlier run built too.
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
Ask the reviewer to check specifically: that the quick launch composes exactly what New would launch from the same
template and defaults; that the YOLO confirmation can never be bypassed; that "at most one" holds across rename, delete
and concurrent clients; and that the chord cannot reach the terminal or fire under a modal.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a second composition path that
  duplicates New's form, a new template field schema beyond the mark, or a configurable chord (examples, not a
  blacklist);
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
alternatives considered. In particular: how the hidden form handles focus, the modal check and re-entry; the managed
checkout path; the browser chord results; wording changes from the mockup; the stack's shape; every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks:

- A browser that will not let the page claim the chord keeps its own behavior there; document it, and the quick switcher
  row remains the way to quick launch in that browser. Do not pick a different chord.
- Wording in the mockup that turns out inaccurate is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a different chord, skipping the YOLO
confirmation, more than one quick launch template.

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

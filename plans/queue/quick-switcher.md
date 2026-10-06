# Quick switcher: a keyboard shortcut to jump to a session, start one by name, or launch from a template

Written against main at 07aa85e5 on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

NOTE: This plan runs after `plans/queue/templates-dialog-overhaul.md` has landed (its `INDEX.md` line says so). That
plan reshapes the Templates panel and how a template carries its launch kind; it leaves `apply_template` and the
launcher's template-application path in place. Read what it landed before you start, and reuse its template summary
formatter for the switcher's template rows.

## The goal

A Slack-style quick switcher: one shortcut opens a small dialog, the user types a few letters, and either jumps to an
existing session, starts a new session with the typed name, or (with `tl:`) launches from a template. The agreed visual
design, with four states drawn, is at https://snippets.scode.org/s/farhelm-quick-switcher/ ; the decisions below are
authoritative where the drawing is only illustrative (in particular, do not encode the drawn row order in tests).

The TODO.md entry this plans, verbatim:

> **Keyboard quick switcher.** A keyboard shortcut that opens a quick switcher, like Slack's: type to jump to an
> existing session, or to start a new one. Details TBD.

Acceptance criteria:

- **Opening.** Cmd+K on macOS, Ctrl+Shift+K elsewhere, in the web UI and the desktop app. It works while a terminal has
  focus, and the key never reaches the terminal's program. It does nothing while another modal dialog is open, and it
  does not take the key when there is nothing to open (the sign-in prompt, preferences still loading). Plain Ctrl+K off
  macOS is left alone: in a terminal it is the shell's "delete to end of line". Firefox: Decision 2.
- **Layout.** A modal dialog near the top of the window: a search field, then three stacked areas. The sessions list
  scrolls through every match, with no switcher-side cap. Below it, pinned and visible while text is typed, a row "new
  session named "<text>"". At the very bottom, pinned, a "templates" section that outside `tl:` shows only a hint that
  `tl:` searches templates and that picking one opens New with it applied. With a leading `tl:` the sessions area and
  the new-session row are gone and the list scrolls through matching templates; with nothing after `tl:` it lists every
  template. Plain typing never searches templates.
- **Matching.** Sessions from every host, whatever the sidebar's host selector shows. Case-insensitive fuzzy matching on
  the session title; a match only in the host name or folder ranks below every title match; ties go to the most recent
  activity. With nothing typed, the most recently active sessions are listed first. Each row shows the session's status,
  agent type, host name and folder, with the matched characters marked. `tl:` uses the same label, parsing and template
  matching as the launcher's own search.
- **Keys.** ↑/↓ move through the sessions and then the new-session row (or through the templates in `tl:` mode); Enter
  picks; Escape or a click outside closes. The first row is selected when the dialog opens, so the shortcut followed by
  Enter returns to the most recently active session; when no session matches, the new-session row is selected.
- **Picking a session** opens it as clicking its row would, including the same refusals while something is busy. If the
  host selector hides it, the selector goes back to ALL so its row is visible. Typing right after the jump reaches the
  newly opened session's terminal.
- **Picking "new session named X"** opens New with the name filled in and everything else exactly as New would have it
  (host and folder defaults, remembered permissions). It never launches by itself.
- **Picking a template** opens New with that template applied, exactly as accepting `tl:<name>` in the launcher's own
  search would. The typed text is a search, not a name, and is not used as the session name. Nothing launches by itself.
- **Escape** returns focus to where it was before the switcher opened (often a terminal).
- **Loading and errors.** Until the session list has arrived, the dialog says it is loading: it does not say "no
  sessions match", does not preselect the new-session row, and Enter does not pick anything in the sessions area. If the
  list cannot be fetched, the sessions area says so, and the new-session row and `tl:` still work.
- **The listing cap.** When the helm's listing was cut at its cap (500 sessions), the switcher says it searched only the
  most recently active ones, as SPEC.md requires of the sidebar list.
- SPEC.md describes the switcher in a paragraph beside the Cmd+N paragraph (~1261), plus one line in the terminal list
  (~1100–1114) that the switcher's key never reaches the terminal's program; SPEC_impl.md records the listener mechanics
  (near the font-size chord's, ~720) and the single unfiltered fetch with client-side matching. A docs page section
  describes it (the session-list page or wherever `website/AGENTS.md`'s outline puts keyboard use), following
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md`. `docs/manual-mac-checklist.md` gains a desktop entry for Cmd+K.
- A changelog fragment under `releasing/changelog.d/` (kind `added`) in the same commit as the user-visible change.
- The last PR removes the TODO.md entry "Keyboard quick switcher". Leave "Mark a session suspended", which only mentions
  the switcher.

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Keys:** Cmd+K on macOS, Ctrl+Shift+K elsewhere, in both the web UI and the desktop app, working from a terminal.
2. **Firefox:** Firefox binds Ctrl+Shift+K (and Cmd+K on macOS) to its own tools, and nobody verified whether a page can
   take them back. Keep the chords above with no per-browser keys. Try it in a real Firefox; if Firefox swallows the key
   or also opens its console, record that as a known Firefox limitation in SPEC.md and the docs page. If that test is
   blocked or turns out to be difficult (the repository's Playwright install has no Firefox, and synthetic input may not
   exercise Firefox's own shortcut handling), skip it, say so in the report, and proceed. Do not switch chords.
3. **Matching and ordering** as in the acceptance criteria: fuzzy on title, host and folder weaker, ties by recent
   activity, every host regardless of the selector, the selector reset to ALL on a jump it hides.
4. **Templates layout** as in the acceptance criteria: pinned hint outside `tl:`, templates only with `tl:`; plain
   typing does not search templates. This replaces an earlier idea of listing matching templates below the sessions.
5. **New-session row** pinned below the sessions, reachable with ↓, selected when nothing matches; it opens New and
   never launches.
6. **Out of scope:** suspended sessions (a separate TODO entry), actions other than open and new (rename, stop, ...),
   and opening a session in another window.
7. **Runs after `templates-dialog-overhaul.md`.**
8. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
9. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision, grounded in main at 07aa85e5 and checked by a fresh-context planning
review. Line numbers are approximate and will have moved once the templates overhaul lands.

**Where it lives.** Mount the switcher inside `ListView` (`crates/farhelm-ui/src/list/view.rs`), the way
`TemplatesDialog` is mounted (`if templates_open() { super::templates::TemplatesDialog { .. } }`), not in `AppBody`.
Every action it triggers needs state only `ListView` holds: `guarded_open` (busy refusals and `remember_selection`), the
host selector's `filter` signal, New's host and folder defaults (computed in the New button's `onclick` from
`open_host`, `chosen_host`, `effective_create_host`, `open_destination`, `ordinary_new_cwd`), `host_options` (the host
names; a `Session` carries only a host id), and `ops.busy_now()`. Do not add variants to `HeaderPrefillRequest`: its
consumer unpacks a session, which "new" and "template" picks do not have. Put the dialog in its own module
(`crates/farhelm-ui/src/list/quick_switcher.rs`, say).

**The shortcut.** Follow Cmd+N's precedent (`install_new_session_shortcut` in `crates/farhelm-ui/src/lib.rs`): an
inline, idempotent `document::eval` guarded by a window global, here installed for web and desktop alike. Its
capture-phase `window` keydown listener matches Cmd+K on macOS (meta, no ctrl, shift or alt) or Ctrl+Shift+K elsewhere
(no meta or alt) by `event.code === "KeyK"`, ignores `repeat` and composition, skips when
`[role="dialog"][aria-modal="true"]` is open, and otherwise calls `preventDefault` and `stopPropagation` (so xterm never
sees it) and clicks a stable, visually hidden, `tabindex=-1`, `aria-hidden` element that `ListView` renders. If that
element does not exist, it does nothing and does not prevent the default. Decide macOS the same way terminal.js's
`IS_MAC` does so the two agree. No new JS asset and no node test: neither precedent has one, a new asset must also join
the `asset!()` inventory and the desktop asset parity check (`scripts/check-desktop-assets.sh`), and the Playwright
cases below cover both chords on both engines. No long-lived JS-to-Rust channel.

**The dialog.** `role="dialog"`, `aria-modal="true"`, with modal isolation through
`crates/farhelm-ui/src/modal_isolation.rs` (see `install_dialog_with_selector` in
`crates/farhelm-ui/src/hosts/settings_dialog.rs` and the feedback and templates dialogs). Returning focus to where it
was is new: every existing helper returns focus to a fixed opener button. Record `document.activeElement` (often xterm's
hidden textarea) when the shortcut fires, and on Escape run `modal_isolation::release_js` before refocusing it, because
it is inert until released.

**Focus after a jump.** terminal.js refuses to let a terminal take focus while any modal dialog is mounted, and does not
retry when the dialog closes (its `sync()` focus comment). So the switcher must be unmounted and its isolation released
before the newly opened session's terminal decides whether to take focus; otherwise focus is left on the page body and
the next keystroke goes nowhere. Picking the session that is already open remounts nothing, so treat it like Escape for
focus.

**Data.** On open, one `fetch_sessions(&SessionFilter::default(), ListSort::Activity)` (all hosts; the helm caps at
`LIST_SESSIONS_CAP`, 500, and reports `truncated`) and one templates fetch (on open, or the first time `tl:` is typed).
The helm's activity order is the tiebreak; do not sort on the client (`fetch_sessions`' doc: "No client-side sort").
Reusing the sidebar's listing is not correct in general (it is filtered by the host selector), and helm-side title
filtering cannot match host or folder. No live updates while the dialog is open.

**Matching.** A small pure Rust function, unit-tested: case-insensitive subsequence match on the title is tier 1 (a
contiguous substring may rank above a scattered subsequence within it); a match only on host name or folder is tier 2;
the order is stable within a tier, so the helm's activity order breaks ties. No fzf-style scorer. For `tl:`, call the
launcher's existing `launch_composer::scoped_query` and `template_search_results`; do not write a second template
matcher. Reuse the existing row helpers for the agent label and status mark (`launch_composer::harness_label`, the row's
status drawing) and the template summary the overhaul ends with.

**Opening New with a name or a template.** Give `CreateSessionForm` (`crates/farhelm-ui/src/list/create_form.rs`) one
prop, such as `initial_action: Option<ComposerSearchResult>` carrying `Name(text)` or `Template(name)`, applied once
through the same `template_edits` → `apply_composer_search_result` sequence the launcher's search runs when the user
accepts a result. For `Name`, that is exactly what the `name:` label does (`title.set`, `title_edited.set(true)`). The
accept body is duplicated today at two call sites (the search box's Enter and click handlers, ~4500 and ~4630); factor
it into one closure and call it from both and from the initial application, rather than adding a third copy. Do not
route this through `CreatePrefill`: the form seeds remembered permissions only when there is no prefill
(`if prefill.is_none() { initial_structured_permissions(..) }`), so any prefill would break "everything else as New
would have it". `ListView` opens New for these picks by running the New button's open branch, factored into a closure
shared with the button (busy check, host and folder defaults, `show_create.set(true)`), with `clone_prefill` left
`None`.

The initial action must wait for what the launcher loads asynchronously. Applied before the templates resource resolves,
`template_edits` reports "no template is named …"; before the model catalog settles, it validates against an empty
catalog; template hosts resolve by identity through the host list. Apply it once, after those have loaded and after the
form's own mount-time seeding (so the seeding does not overwrite it), then clear it. New's cancel returns focus to the
New button, as it does today; that is acceptable for these picks.

**Picking a session.** If the host selector hides the session, set `filter.host = None` and request an explicit listing
(a selected session already survives a filter that hides it; the reset is for visibility), then call `guarded_open`.
`guarded_open` refuses silently while something is busy; a refused pick closes the switcher with nothing opened, as a
row click would.

**Tests (proposal).** Rust unit tests for the matcher and ranking tiers, and for the initial-action application next to
the existing search-accept tests. A new Playwright spec, run on Chromium and WebKit through the recorder per root
`AGENTS.md`: the shortcut opens the switcher with macOS faked (`navigator.platform`, as in the font-size tests in
`e2e/tests/terminal-tabs.spec.ts`) and not faked; with a terminal focused the key does not reach the program; jumping to
a session on another host while the selector shows one host resets it to ALL; after a jump, typed text reaches the new
session's terminal; Escape returns focus to the terminal it came from; "new session named X" opens New with the name and
the remembered defaults; a `tl:` pick opens New with the template's fields applied (assert the fields, not just that New
opened); Enter during loading picks nothing. Cmd+K in the desktop app goes on the manual Mac checklist, as Cmd+N did.

**Size and stack.** Moderate. Suggested: PR 1 the shortcut, the dialog, matching and jumping to a session (with SPEC,
docs, the fragment and the checklist entry for what it ships); PR 2 the new-session row and `tl:` templates, including
the launcher's initial-action refactor, the remaining docs, and the TODO removal. Both are `feat`; put the `added`
fragment in PR 1 and extend it in PR 2, or carry one fragment in each, whichever `releasing/AGENTS.md` prefers.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-quick-switcher-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/quick-switcher/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The user-visible changes are `feat` and carry a changelog fragment under
  `releasing/changelog.d/` (kind `added`) in the same commit, per root `AGENTS.md` (Releases and the changelog).
  Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust or browser tests change.
- The last code PR removes the TODO.md entry "Keyboard quick switcher".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, a nextest selection
of the `farhelm-ui` crate's `list` and `launch_composer` tests, `cargo check -p farhelm-ui --features desktop` and
`cargo check -p farhelm-desktop` (the shortcut and dialog ship in the desktop app), the new and changed Playwright specs
on Chromium and WebKit through the recorder (plus `modal-focus.spec.ts` and `tooltip-coverage.spec.ts` if the dialog
touches what they check), the website build for the docs page, and `dprint check` on changed files. Say in the report
which checks ran and why, and what came of the Firefox check (Decision 2).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands: a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at high
effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm. Both
get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (an AppBody-level request bridge, a new JS asset, a
JS-to-Rust event channel, a fuzzy scorer library, a new helm search endpoint, a `CreatePrefill` variant; these are
examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the request, the decisions above, this outline,
the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the trigger element, the ranking tiers, how the initial action waits for the
launcher's loads, how focus is recorded and returned, the Firefox outcome, the PR split, and every reviewer finding you
declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: different chords or per-browser chords; a switcher that searches only the sidebar's filtered sessions; a cap
on the switcher's list; templates in plain-typing results; any pick that launches a session without the user pressing
Launch; a "new session" path that loses New's defaults; and anything from the out-of-scope list.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

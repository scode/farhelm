# The version readout checks for updates, and failed automatic updates show

Written against main at 66af18cd on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan. It covers two TODO.md entries (Near term), merged into one plan with the
maintainer's agreement because both change the same inline update status and the same SPEC paragraph.

## The goal

Two changes to the installed desktop app's version readout, in the sidebar's top bar, built in this order:

1. **Clicking the version readout checks for updates.** Today the readout is plain text unless an update waits. Make it
   a control that starts the same on-demand check as `?` → **Check for updates**, with the same inline progress and
   outcome beside it, and give it a visible hint that it is clickable: a dim ↻ after the version (the maintainer's pick
   from mockups, below).
2. **A failed automatic update becomes visible when it matters.** Today an automatic check or background install that no
   user joined fails silently (only the app's log says so), so a user can sit on an old version without knowing updates
   have stopped. Show such a failure, with its reason, as the inline **check failed** beside the readout, on the rules
   the maintainer chose below.

Acceptance criteria:

- With an updater present and no update waiting, the readout is a keyboard-reachable button. Clicking it calls the same
  check the `?` menu's Check for updates calls (a click during a running check joins it, as that action does). Its
  accessible name and hover say that selecting it checks for updates.
- The readout carries a trailing ↻ glyph in a dim tone (`--fg-2`). On hover and keyboard focus the version and glyph
  brighten (`--fg-0`) and the glyph turns a quarter. While a visible check runs (one a user started or joined; checking
  or installing), the glyph spins in place of the separate spinner beside the readout, and the inline **checking…** /
  **installing…** text stays; the spin respects reduced motion the way the existing spinner does (no animation then,
  keep the text). Whenever the ↻ is not rendered (an update waits and a check starts from the `?` menu or the host row),
  the separate spinner stays as today. Automatic runs still change nothing on screen while they work.
- With an update waiting, the readout is unchanged: red, up-arrow, opens the Restart to update / What's new menu, no ↻.
  With the "must be reinstalled" warning showing, the readout keeps its warning mark and clicking it starts a check.
- Where there is no updater (the web UI, a build from main), the readout stays plain, non-interactive text with no ↻.
- An automatic background **install** that fails shows **check failed** beside the readout at once, with the reason in
  the hover. Anything that fails after a successful probe of `/latest` (including a network failure while downloading
  the release) counts as an install failure.
- An automatic **check** that fails at the probe (the site could not be reached or did not answer usably) shows **check
  failed** only once automatic checks have been failing for 3 days: the updater remembers when the current streak of
  failures began (the first failure after a success sets it), shows the notice once a failing automatic check finds that
  streak at least 3 days old, and any successful check, automatic or on demand, ends the streak. Gaps in which no check
  ran (the Mac asleep, the app quit, an update waiting for a restart, automatic updates off) start no clock on their
  own. The streak start survives restarts. A missing value means no streak; a future-dated value or a clock that went
  backwards resets it to now; turning automatic updates back on clears it.
- An automatically shown failure behaves like a user's failed check: amber, dismissed by clicking it, reason in the
  hover. Clicking the version retries. Unlike today's "cleared when the next run starts", it stays until a check
  succeeds or the user dismisses it, so a later failed run within the 3 days does not silently remove it. After a
  dismissal it comes back only when a later automatic run fails the same rule again.
- Verification failures keep today's behavior ("must be reinstalled" warning); this plan does not change it.
- SPEC.md "Installation and updates" and SPEC_impl.md "The desktop app's updater" (and its app-bar paragraph) describe
  the new behavior; the sentences "Without an update waiting, the readout behaves as it always has…", "Automatic checks
  that no user joins stay invisible.", "A failed automatic check or install is logged … and is not shown, with one
  exception…", "Hover wording is unchanged.", "**check failed** … stays until the next check starts or the user clicks
  it" and the list of on-demand triggers are rewritten, not left contradicting; so are SPEC_impl.md's "an automatic run
  leaves the published state idle from start to finish", "A failed automatic check or install is logged, and the next
  check tries again", the sentence that automatic work creates no user-check identity, and "The last check time is kept
  in memory only" (distinguish it from the new persisted streak start). Hover wording that tells the user how to use the
  readout ("select it then to restart", "Select this to open the menu") is updated to match.
- Test coverage is held to a very high bar (maintainer's decision below): every new state, transition and failure path
  has a test, including restart persistence and its failure modes.
- Changelog fragments (one per user-facing PR, `kind: added` or `changed`), and both TODO.md entries removed.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: changing what counts as a verification failure, notifications outside the app, the web UI, remote hosts'
update actions, changing the automatic schedule or retry interval, and surfacing raw error chains beyond what the reason
text needs (improving a generic reason is allowed if small; log the DECISION).

## Requirement sources

**The user's request (2026-10-13):** plan these two Near term TODO entries. Their text:

> **Check for updates by clicking the version readout.** In the desktop app, the version readout in the sidebar's top
> bar is plain text unless an update is waiting, in which case clicking it opens the Restart to update / What's new
> menu. Make clicking it while no update waits start a check, exactly as "check for updates" in the `?` menu does, with
> the same inline progress and outcome beside the readout. Clicking it while an update waits keeps opening its menu.
> Where there is no updater (the web UI, a build from main), the readout stays plain text.

> **Make failed automatic updates visible.** The desktop app's automatic update checks and background installs (at
> startup and daily) stay invisible unless the user started or joined the check, so when one fails nothing says so, and
> the user can sit on an old version without knowing updates have stopped working. Show a failed automatic check or
> install somewhere the user will notice, with the reason. SPEC.md currently says automatic checks no user joins "stay
> invisible", so this needs a SPEC change alongside it.

**The user's decisions (2026-10-13):**

- Merge the two entries into one plan, click-to-check first, the failure notice on top.
- The clickable hint: "some slick visual indicator that the version might be button-like". From six mockups
  (https://snippets.scode.org/s/farhelm-version-readout-click/), the maintainer picked option 2, the trailing refresh
  glyph: a dim ↻ after the version that brightens with it on hover and turns a quarter, and spins while checking. Treat
  the mockup's CSS as illustrative, not a spec; follow the app's tokens and existing app-bar rules.
- When a failure becomes visible: persisting failures only. A failed install shows at once; a check that fails because
  the site can't be reached shows only after it keeps failing, remembered across restarts. The maintainer first picked
  "no successful check for about 3 days", then, after the planning review showed that rule fires on the first offline
  wake after any long gap, chose "failing for 3 days": measured from the start of the current failure streak, reset by
  any success (the planner fixed "about 3 days" at 3 days).
- Where: the inline **check failed** a manual check shows, reason in the hover; clicking it dismisses it, clicking the
  version retries, the next successful check clears it.
- "since the upgrade failure stuff is rare in real life make sure the test coverage is great."
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Installation and updates"; SPEC_impl.md "The desktop app's updater" and the app bar's description; SPEC.md
  "Upgrade compatibility and client scale" (the new persisted field must not break an upgrade or a downgrade to an older
  release reading `desktop-client.json`).
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; The live
  install is off-limits (never run or touch the maintainer's installed app to test this); Talking to the user.
- Global instructions: tests never modify the test process's own environment variables; inject time, filesystem paths
  and fake dependencies instead.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the outline below and the 3-day figure.

## Outline

Line numbers drift; find the code by name. Almost everything lives in three files under `crates/farhelm-ui/src`:
`app_bar.rs`, `app_updater.rs`, and `desktop/updater.rs`.

Here's what exists today:

- `AppBar` (`app_bar.rs`) draws the readout as a `BarMenuToggle` (`.app-version-toggle`, mounted whenever an updater
  exists and hidden unless `update_ready`) and, otherwise, a plain `span.app-version`. `help_menu_items(has_updater)`
  adds `BarMenuItem::CheckForUpdates`, whose handler calls `check_now()`. `UpdateCheckStatus` beside the readout renders
  `inline_status()` / `InlineFeedback` (keyed by activity and `user_check_id`; failures are a dismiss button,
  `.app-update-failure`). CSS is in `crates/farhelm-ui/assets/app.css` ("Sidebar chrome").
- `desktop/updater.rs`: `Shared` (status, condvar, watch sender; `publish` is the only writer), `Engine::run_forever`
  (60 s tick), `automatic_due()` (24 h `CHECK_INTERVAL`, no waiting update, automatic enabled), `run_check()`, which
  stores a user-joined outcome in `shown_outcome` and otherwise only logs an automatic failure (`tracing::warn!`), and
  `check_and_install()` with its failure kinds (probe failure, `UnverifiableRelease` → `needs_reinstall`, install
  failure). `use_app_updater()` returns `None` without an updater (web, development build), which is how "no updater" is
  decided.
- `desktop/state.rs` persists `desktop-client.json` (`install_updates_automatically`), the natural home for the last
  successful check time.
- Tests: `make_rig()` with fake `Deps` and a controllable clock in `desktop/updater.rs`
  (`automatic_failures_and_up_to_date_checks_stay_invisible` asserts the behavior this plan changes and must be
  rewritten, not deleted); the readout and inline-feedback tests in `app_updater.rs`; `desktop/state.rs` round-trip
  tests; Playwright specs that check the web readout (`e2e/tests/sidebar.spec.ts`, `window-chrome.spec.ts`,
  `tooltip-coverage.spec.ts`) guard the web side staying plain text.

Here's the work, as a proposal:

1. **PR 1, click to check.** Turn the no-update span into a button when an updater exists (sharing the toggle's
   no-button-look styling), wired to `check_now()`; add the ↻ glyph, hover/focus brightening and quarter turn, and the
   glyph spin replacing the separate spinner while a check runs, with reduced motion honoured. Keep the web and
   development-build readout a plain span. Update hover wording, SPEC.md and SPEC_impl.md, and add a changelog fragment.
   Tests: which element renders in each updater state (no updater, idle, checking, installing, update waiting, reinstall
   warning); that clicking calls the same check as the `?` menu; the hover text; CSS contrast and animation tests in
   `crates/farhelm-ui/js-tests` if the new rules fall under them. If the component layer has no render-test seam,
   extract the element choice into a pure function and test that.
2. **PR 2, visible automatic failures.** Reuse the existing machinery rather than adding a parallel state: when an
   automatic failure qualifies, set `shown_outcome = Some(Failed(reason))` and bump the existing `user_check_id` (rename
   it if the name now misleads), so `InlineFeedback`'s dismissal, re-showing after a later failure with the same reason,
   user-check precedence and join-once keep working unchanged. Add the small in-memory rule that keeps such a failure
   until a success or a dismissal instead of clearing it when the next run starts. Persist the failure-streak start in
   `desktop-client.json` as an optional field: read once when the updater starts and kept in memory as the source of
   truth, written through best-effort (a failed write is logged and changes nothing else), using the file's existing
   atomic write. Do not add per-field tolerance for a malformed value; the file is decoded as one struct, as the
   existing automatic-updates field already is. Update SPEC.md, SPEC_impl.md and the fragment, and remove both TODO.md
   entries in the last PR.

### Test coverage (maintainer's bar)

The maintainer asked for great coverage because these failures are rare in practice and a bug here would go unseen.
Cover at least, with the fake-`Deps` rig and an injected clock:

- automatic install failure (including a download failure after a successful probe) → shown at once; automatic probe
  failure with a streak younger than 3 days → not shown; at and after 3 days → shown; the boundary itself;
- a long gap with no checks followed by one failed check → not shown (the false positive the maintainer ruled out);
- success (automatic and on demand) clears the shown failure and ends the streak;
- an automatically shown failure survives a later failed run within the 3 days; dismissal hides it; the next failing
  automatic run re-shows it; a user's own check failing and succeeding interacts correctly with an automatically shown
  failure (no double display, the user's outcome takes precedence the way joined runs do today);
- a request joining an automatic run that fails: shown as the user's, exactly once;
- automatic updates turned off and back on: the streak is cleared, no notice from the off period;
- verification failure still produces only the reinstall warning;
- persistence: the streak start survives a restart; a missing file, a missing field, an unreadable file at startup, a
  future-dated value, and a write failure each leave the updater working (no panic, no stuck state) and behave as
  specified; an older release reading the file with the new field still loads it (the existing round-trip/compatibility
  tests show how);
- clock going backwards;
- the separate spinner still shows for a check started while an update waits (no ↻ then);
- the readout element choice and the ↻ behavior across every updater state, and that the web build stays plain text.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop` and, through the recorder, the desktop Rust targets
  (`cargo nextest run -p farhelm-ui --features desktop`, filtered to the updater, readout and state tests while
  iterating, then the whole package once);
- the web-side farhelm-ui tests that cover `app_updater.rs` and `app_bar.rs`;
- `cd crates/farhelm-ui/js-tests && node --test` if CSS or asset JS changes;
- the Playwright readout specs on Chromium and WebKit if the web readout's markup changes at all (it should not), per
  root `AGENTS.md`;
- `dprint check` on changed Markdown and `python3 releasing/check-changelog.py format`.

No installer, provisioning or full-battery runs. The report asks the maintainer to look at the readout in the installed
app once a release carries it, since nothing here runs the real Mac app.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-update-check-readout-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/update-check-readout/<nn>-<short-name>` (for example `plan/update-check-readout/01-readout-checks`).
- Expected shape: two PRs as in the outline. Split further only when it makes review easier, each PR leaving main
  correct on its own. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits; both PRs are `feat`. Each adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
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
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the test coverage list.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check specifically that every item in the test coverage list is covered by a test that would fail if
the behavior broke, and that no failure path can panic or leave the updater stuck.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a new notification surface, a change
  to the automatic schedule, or a new persisted file (examples, not a blacklist);
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

- how the readout button is structured and styled (and how the glyph spin replaces the spinner);
- the persisted field's name and format, and how absence and bad values are treated;
- how the automatic failure's identity interacts with `InlineFeedback`;
- any reason-text improvements;
- the stack's shape;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a different glyph or affordance than the
maintainer's pick, showing every automatic check failure immediately, a toast or other surface, or changing the check
schedule. If the ↻ glyph cannot be rendered legibly in the bundled font at the readout's size, draw it as an inline SVG
in the icons' style; that is within scope.

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

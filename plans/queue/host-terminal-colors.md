# Host colors: local host appearance, red for every host, an optional colored terminal frame

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions explicitly ask for them to change.

This plan has no dependency on another plan. The independent plan `session-details-bottom` also changes the session view
(it moves the session's identity, host mark included, into a bar under the terminal); it may run before, after or
alongside this one. Keep this plan's session-view change to the terminal frame so the two rebase across each other
cleanly. Both plans need the session view to know its host's appearance, and both add it in the one shape named under
"The shared hosts snapshot" below, so whichever lands second finds it already there.

## The goal

TODO.md entry this plan covers (Near term): "**Per-host terminal background color.** Allow setting a terminal background
color for each host."

The maintainer reshaped the entry during planning. The terminal background does NOT change: a different background
breaks the contrast of the colors agents paint in the terminal. Instead:

1. The local host gets the same appearance choices as remote hosts: an icon and a color. Its defaults stay what users
   see today, the laptop in red.
2. Red becomes an eighth identity color that every host can pick.
3. A new setting, off by default and shared by every device, colors the terminal's frame (the thin dark margin around
   the terminal, described below) in the session's host color. With it on, every host's frame is colored, a
   default-color host's included.

### Acceptance criteria

- The host settings dialog shows the Appearance section for the local host too, with the same icon and color choices as
  remote hosts. Choosing saves through the existing `POST /api/hosts/{id}/appearance` route, which stops refusing the
  local row. A new local host, and every existing one after the upgrade, shows exactly what it shows today: the laptop
  in today's red.
- The color vocabulary (`HostColor` in `crates/farhelm-proto/src/host_appearance.rs`) gains `red`, offered to every host
  in the color picker after the existing seven. Its color is today's local-laptop red: the value of `--danger` in
  `crates/farhelm-ui/assets/app.css`, through its own identity token (see Planner proposals).
- The icon vocabulary can express the laptop, so the local host can pick another icon and go back to the laptop (see
  Planner proposals for how).
- Every place that draws the local host's mark today draws its chosen icon and color instead: the session row
  (`crates/farhelm-ui/src/list/row.rs`), the quick switcher (`list/quick_switcher.rs`), the host picker
  (`list/host_picker.rs`), the host settings dialog header (`hosts/settings_dialog.rs`), and anything else that renders
  `LocalHostIcon`. The local mark keeps its "local: on this machine, where the helm runs" hover text; a remote mark
  keeps its own.
- A new Settings switch, off by default, stored by the helm and shared by every device like the other preferences (the
  `preferences` row behind `GET`/`PUT /api/preferences`, the pattern `name_session_on_quick_launch` followed in schema
  56), turns the terminal frame coloring on. With it on, the frame around a session's terminal takes that session's host
  color at full strength: the `--host-icon-*` token for a chosen color, the ordinary foreground (`--fg-0`, what the
  default-color icon uses) for a default-color host, red for a red host. With it off, the terminal looks exactly as it
  does today.
- Picking a host's color recolors open session views' frames without a reload, through the hosts feed refresh that
  already redraws session rows. Flipping the switch applies at once on the client that flips it; other open clients pick
  it up when they reload, as SPEC.md already says for preferences (the terminal links paragraph around line 1242: "other
  open clients pick it up when they reload"). No preferences-change event is added.
- The frame is the dark margin the maintainer sees. During planning the maintainer sent a screenshot of the session view
  and said "the margin is probably what i'm calling a frame": a near-black border a few pixels wide between the tab
  strip and pane edges and the terminal's own gray (`#282c34`) box. The stylesheet means to paint everything there in
  `--terminal-bg` (`.terminal-panes`, `.terminal-pane` and `.terminal` with its 4px padding, plus the
  `.terminal .xterm-viewport` override, around app.css lines 7030-7067), so something else paints that margin dark in
  the running app. Find out what before changing anything (see Outline), and color exactly that margin. If the margin
  turns out to be an accident (for example a leftover strip of the page's `--bg-0` that the terminal does not cover),
  the setting-off appearance stays as it is today; fixing the accident is not part of this plan.
- SPEC.md and SPEC_impl.md say the new behavior (see Outline), docs are updated, and the TODO.md entry is removed.
- Tests: the vocabulary round-trip tests cover the new words; a helm store test covers the migration (existing local
  rows read as laptop/red, a migrated database matches a freshly created one); an HTTP test covers the local host
  accepting appearance; the contrast harness (`crates/farhelm-ui/js-tests/app-css-contrast.test.js`) binds the new red
  token like the other six; a browser test (Chromium and WebKit) covers choosing the local host's color and the frame
  switch coloring and uncoloring the frame. The local laptop's mark carries `data-glyph="laptop"` like any other icon
  word; the existing browser assertions on `.host-kind-icon[data-glyph='local']` and its red color
  (`e2e/tests/sidebar.spec.ts` around lines 1065 and 2823-2824, and the local-mark checks in
  `e2e/tests/host-picker.spec.ts` and `e2e/tests/terminal-multihost.spec.ts`) are updated to that, keeping their check
  that an untouched local host is the red laptop.
- `docs/harness-marks.md`, "Host marks" ("The local laptop is a fixed red locality cue", "The six host color tokens")
  describes the new state.

## Requirement sources

**The user's decisions (2026-10-17):**

- Not the background: "Rather than changing the background color—which opens up a bunch of other problems because of
  other colors used in the terminal itself—let's just change the color of the frame. ... we can default it to use the
  same color as the host icon. In fact, let's make that the only option for now so that it always matches: one color
  choice affects both the icon and the frame color."
- The colors are the same choices as for icons ("Same as icons for now").
- The local host: "let's bring the color selection to the local host as well so it's consistent with remote hosts. But
  the default is still going to be red, but people can change the color". Later: the local host can pick its icon too,
  from the same choices as remote hosts.
- Red becomes an eighth color every host can pick, and stays the local host's default.
- Opt-in: "Let's make that whole behavior opt-in because, given the mockup that you have here [the session-details
  bottom bar, which shows the host's color prominently], the coloring is going to be pretty visible anyway. Having the
  frame also be colored might be too much depending on the person. Let's just make it default off, but a toggle you can
  turn on to get the frame colored." The toggle lives in Settings and is shared by every device.
- With the toggle on, the frame is the host color at full strength, and every host's frame is colored ("if you enable
  the setting to have frames colored, it'll always be colored"); a default-color host's frame uses the default (pale
  foreground) color.
- The local host's color choice does not depend on the toggle.
- Schema compatibility: the user accepts that this plan's helm schema changes (the local host's icon and color, the
  frame switch) leave no downgrade path; an older helm refuses the newer database, as it already does for every schema
  bump. The upgrade path must stay intact: every existing database migrates forward with no loss.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:**

- `crates/farhelm-proto/src/host_appearance.rs` module docs: words, never palette positions; each word spelled three
  times and held together by the round-trip tests; adding a word needs a helm schema bump.
- `crates/farhelm-helm/src/store.rs`: the one-transaction migration ladder, a fresh database created in its final shape,
  `a_migrated_database_matches_a_freshly_created_one`, and the rewound-fixture tolerance every recent rung has.
- SPEC.md's hosts paragraph (around line 184: "It tints only the icon, never a status or the host name. The local host
  keeps its fixed red laptop and offers no appearance choice.") and the session list paragraph around line 1328 ("A
  local row carries the red laptop"); SPEC_impl.md "Host identity appearance". These change with this plan.
- Root `AGENTS.md` "Instructions by path": changing `crates/farhelm-ui/src/icons.rs` or `crates/farhelm-ui/src/icons/`
  requires reading `docs/harness-marks.md` first.
- Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- Red gets its own identity token, `--host-icon-red`, with today's `--danger` value, rather than drawing a host mark
  with `var(--danger)`. SPEC_impl.md says the `--host-icon-*` tokens are identity colors with no state meaning, and the
  contrast harness binds them; the separate token keeps "red here means this host" apart from "red means error" in the
  code, even though the shade is the same. SPEC.md's "never a status" sentence stays true and gains a note that red,
  like the others, identifies a host and says nothing about its state.
- The laptop becomes a sixteenth icon word, `laptop`, that every host can pick, drawn with today's `LocalHostIcon`
  geometry. One vocabulary is simpler than a local-only extra choice, and it mirrors the red decision. `LocalHostIcon`
  stops being a separate component for the mark and becomes the laptop case of `HostMark`, with the hover text chosen by
  whether the host is local.
- Storage: today's `hosts` columns carry `CHECK (kind = 'ssh' OR icon IS NULL)` (and the same for `color`), and NULL
  means cloud and default. SQLite cannot drop a CHECK with `ALTER TABLE`, and rebuilding `hosts` is ruled out: about ten
  tables reference `hosts (id)` with `ON DELETE CASCADE`, every open sets `PRAGMA foreign_keys = ON` (which cannot be
  turned off inside the ladder's transaction), a `DROP TABLE hosts` cascades deletes, and `ALTER TABLE hosts RENAME`
  rewrites every referencing table's stored DDL, breaking `a_migrated_database_matches_a_freshly_created_one`. Replace
  only the two columns instead: for each of `icon` and `color`, `ADD COLUMN <c>_new TEXT`, copy with an `UPDATE`,
  `DROP COLUMN <c>`, `RENAME COLUMN <c>_new TO <c>` (SQLite allows the drop because the CHECK belongs to that column;
  the ladder already uses `DROP COLUMN` on `session_cache` and `RENAME COLUMN` on `hosts`). The fresh-create DDL moves
  `icon` and `color` to the end of `hosts` so the stored text matches, and the rung keeps the rewound-fixture tolerance
  (detect whether the old CHECK is still in `sqlite_master.sql`). Let NULL mean "this kind's default": laptop and red
  for the local row, cloud and default for SSH rows. That needs no backfill, leaves the local-row insert untouched, and
  keeps `set_appearance`'s invariant that an untouched host and an explicitly reset one have the same stored shape;
  normalization to NULL and the listing's word mapping become kind-aware. (Rejected: storing the local host's words in
  `preferences`, which splits one concept across two tables and two read paths.)
- The shared hosts snapshot: the session view does not know its host's appearance for a live session. Its own
  `host_read` in `crates/farhelm-ui/src/session_view.rs` is filled only while the session reads stale, by design (one
  fewer `/api/hosts` read per open terminal), and is reset when it stops reading stale; do not change that. The sidebar
  already keeps a live hosts snapshot: `ListView`'s `hosts` signal (`use_signal(HostsRead::default)` in
  `crates/farhelm-ui/src/list/view.rs`), refreshed by the existing feed. Lift that signal into `AppBody`
  (`crates/farhelm-ui/src/lib.rs`, which mounts both `ListView` and `SessionView`) and provide it as a context of type
  `Signal<HostsRead>`; `ListView` uses it instead of its own signal, and `SessionView` reads the session's host from it
  with `use_context`. The plan `session-details-bottom` needs the same thing and names the same shape: if it is already
  on main when you start (never base on that plan's open PRs), reuse it; otherwise add it here in exactly this shape.
  This is part of the outline, not a departure. Color picks then recolor frames live through the existing feed.
- The frame switch is one more `preferences` column and one more Settings switch, in the section whose subject fits
  (likely "sessions"; log the choice). It is not per host.
- The frame color reaches the terminal the way `HostMark` builds its tint (`crates/farhelm-ui/src/icons.rs`): with the
  host from the shared hosts snapshot above, the session view sets one CSS custom property on the element that paints
  the margin, and reads the switch from the `SharedPreferences` context that a Settings write already updates. No new
  endpoint, event or fetch.
- A side effect worth a line in the report, not a safeguard: a remote host can now look exactly like the local one (a
  red laptop). The hover text and the hidden locality word still tell them apart.

## Outline

1. `feat` PR: local host appearance and the red color. Vocabulary words (`red`, `laptop`), the schema rung and fresh
   shape, lifting `LocalHostAppearanceFixed` (store and HTTP mapping in `crates/farhelm-helm/src/lib.rs`), the red
   token, `HostMark` drawing the laptop, every local-mark call site switched to the chosen appearance, the Appearance
   section enabled for the local host, SPEC.md and SPEC_impl.md updates, `docs/harness-marks.md` "Host marks", the host
   docs page (`website/src/content/docs/docs/using/manage-hosts.mdx`), tests, a changelog fragment.
2. Investigation (no PR of its own; log it): in the running web UI, and on the desktop renderer if you can, find what
   paints the dark margin around the terminal. Record what you found as a DECISION, with the element and rule.
3. `feat` PR: the frame setting. The shared hosts snapshot (unless already on main), the preference column (its own
   rung), the Settings switch, the CSS custom property and the rule that colors the margin, SPEC.md (the Settings and
   session view text) and SPEC_impl.md, the docs, tests, a changelog fragment, and the removal of the TODO.md entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-proto` host appearance tests and the `farhelm-helm` store
and hosts tests through the recorder, `cd crates/farhelm-ui/js-tests && node --test` (the contrast harness),
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when browser tests change,
`python3 releasing/check-changelog.py format`, `dprint check` on changed Markdown, the website build when docs change,
and on Chromium and WebKit through the recorder: the new tests plus the existing tests that check host marks
(`e2e/tests/terminal-multihost.spec.ts`, `e2e/tests/sidebar.spec.ts`, `e2e/tests/host-picker.spec.ts`) and the terminal
layout tests in `e2e/tests/terminal.spec.ts`. Not the full suite.

## Fallbacks

- If what paints the dark margin cannot be found or cannot be colored without changing the terminal's size or the
  setting-off appearance, color a border of the same width drawn just outside the terminal box instead, and say so in
  the report.
- If the column swap meets an SQLite obstacle the planning review did not foresee, stop and run the scope reassessment
  before considering anything else; a `hosts` table rebuild is not an agreed fallback.

Examples of departures that need the scope reassessment: a per-host frame setting, a separate frame color choice, any
change to the terminal background or the xterm theme, a new endpoint or event for the frame or for preferences, a
`/api/hosts` read in the session view for live sessions.

Decisions to log: the storage rung's details, where the switch sits in Settings, what paints the margin, whether the
shared hosts snapshot was already on main.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-terminal-colors-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
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
  `plan/host-terminal-colors/<nn>-<short-name>` (for example `plan/host-terminal-colors/01-local-host-appearance`).
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

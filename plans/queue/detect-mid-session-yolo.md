# Notice a session switched into YOLO partway through

Written against main at 5de6290c on 2026-10-15. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

Each session row has a permission shield describing how the agent was launched: amber slashed for YOLO, green for
guarded or an approval mode, grey dashed for "the agent's own setting", amber question mark for an unclassified legacy
command. Several agents can be switched into a no-checks mode inside a running session (Grok's `/always-approve`,
Cursor's `/run-everything`, Muse's `/permissions`, Claude's bypass mode when the user's settings allow it), and the
shield then still shows the launch mode. Those agents print their current mode in their status line. Read it, and when
it says the no-checks mode, draw the YOLO shield, with hover text saying it was seen on screen.

The TODO.md entry (Near term, "Notice a session switched into YOLO partway through") is the source; read it in full.

### Acceptance criteria

- **Best effort, in SPEC terms.** In the maintainer's words: "follow screen but SPEC wise this is best effort. we do not
  promise we can do this, but we try our best within reason." SPEC.md's shield text says the screen reading is advisory
  and best effort, that it cannot see allow rules, hooks or managed policy, and that a missed or late detection is not a
  bug in the guarantee sense.
- **Follow the screen** (the maintainer's choice): the latest definite reading wins. When the status line shows the
  no-checks mode the shield is YOLO; when it later shows a non-YOLO mode again the shield goes back to the launch
  shield. A screen where the mode cannot be read (a dialog covering the status line, the line absent) keeps the last
  definite reading. The shield never shows anything safer than the launch shield.
- **Agents covered:** Claude, Grok, Muse and Cursor, by their launch's agent type, which also covers command launches
  that declare one of these agents. Codex, OMP, Goose, Pi and OpenCode show their mode only inside a command's output,
  not on the status line, and are not read. Legacy sessions with no agent type are not read.
- **The shield rule is on the drawn glyph:** when the screen says YOLO and the launch's glyph is anything other than
  YOLO or asserted-YOLO, draw the YOLO glyph. That includes Cursor's grey dashed own-setting shield (the shared YOLO
  classifier already counts own-setting as YOLO, so a condition written as "launch is not YOLO" would never flip Cursor;
  do not use it).
- **Hover and screen-reader text** say the mode was seen on the agent's status line (name the mode word that was seen),
  that this is best effort, and still show the launch mode and command as today. The row's accessible description
  carries the same information.
- **Display only.** The reading never feeds the helm's YOLO guard, permission inheritance for spawned sessions, or
  approvals. The stored launch stays the only input to those.
- **Updates promptly:** a change in the reported reading tells connected helms that sessions changed (as a status change
  does today), so the shield does not wait for the backstop poll.
- **Tests:** reader unit tests over real captured screens (below), including transcript text that contains the mode word
  above the status line and must not count; a proto serde test (absent field decodes as false; false is omitted when
  encoding); a supervisor test that the reading reaches the session info and resets on restart; a UI unit test of the
  badge, including a Cursor own-setting row and a guarded Claude row.
- **Docs:** SPEC.md's shield paragraph, SPEC_impl.md where it describes screen reading, and the docs pages
  `website/src/content/docs/docs/agents/index.mdx` (which says the shield "describes the launch choice, not a live
  audit") and `agents/cursor.md`, plus the Claude, Grok and Muse agent pages where they describe the shield, following
  `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
- **Changelog fragment**, `kind: added`.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's request and decisions (2026-10-15):** the TODO entry; "Follow the screen" with the best-effort quote above;
permission for the executor to run real Grok, Cursor, Muse and Claude in throwaway folders to capture status lines
(switching modes, no prompts sent); review gate: one fresh-context gpt-6.1-sol reviewer at high effort, no swarm;
no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** root `AGENTS.md` "Harness-specific code" (per-harness behavior lives where
`crates/farhelm-supervisor/src/agent_kind/mod.rs` maps it; no `kind == X`, `matches!` or `_` arms over harnesses in
shared code); "Driving real agent TUIs" and `.agents/real-agent-tui.md`; `PROTOCOL_VERSION`'s documented rules in
`crates/farhelm-proto/src/lib.rs` (an optional field whose omission is harmless needs no bump); SPEC.md "Upgrade
compatibility and client scale"; the changelog rule.

**Planner proposals** (reviewed by a fresh-context planning review): a bool wire field, the separate fixture tree, the
single dispatch function, and keeping the reading in the supervisor's per-run sample state.

## What planning found

- The shield is drawn by `agent_badge()` in `crates/farhelm-ui/src/list/row.rs` from the stored launch only; glyphs in
  `crates/farhelm-ui/src/icons.rs`; hover from `launch_composer::permission_help`, which already says in-session
  switches can loosen the mode.
- Screens are sampled in `crates/farhelm-supervisor/src/service/ticker.rs`; the sample is the bottom 4096 bytes of the
  visible grid, which includes the status line. `agent_kind/screen_reader.rs` `reader_for(AgentKind)` has dedicated
  readers for Claude and Codex only; Muse and Cursor run as the generic kind, so a mode reader must be keyed by the
  launch's agent type, which the supervisor has at sampling time as `entry.info.launch.agent_type()`.
- Status-line wording recorded by the guarded-mode work (2026-10-13/14; its plan is
  `git show e6c80c6c:plans/queue/guarded-permission-mode.md`, section "What planning verified on this machine"): Claude
  `⏸ manual mode on` / `auto mode on` / `⏵⏵ bypass permissions on`; Grok "auto-review" / "always-approve"; Muse
  "Auto-review" / "YOLO"; Cursor "Run Everything" (Cursor has no guarded mode). These are leads, not a spec: confirm
  each on the installed versions.
- The existing fixture tree `crates/farhelm-supervisor/tests/fixtures/screens/` has a strict contract
  (`agent_kind/screen_fixtures.rs`: only `claude` and `codex`, files named by activity state, every version holding
  working, waiting and idle screens, captured by a turn-spending script). Mode screens do not fit it. The existing
  Claude screens show `⏸ manual mode on` and can serve as not-YOLO evidence where they are.

## Outline

1. **Capture.** With `scripts/agent-tui.py` per `.agents/real-agent-tui.md`, start each of Claude, Grok, Muse and Cursor
   the way Farhelm launches it, in a fresh throwaway folder under your scratch directory, switch modes in-session
   (Shift+Tab, `/permissions`, `/always-approve`, `/run-everything`, as each agent offers), and save full visible
   screens for the no-checks mode, at least one non-YOLO mode, and a screen with a dialog covering the status line where
   you can produce one. Send no prompts. Claude's bypass mode needs the user's settings to allow it; if it is not
   reachable without changing the maintainer's settings, use a per-launch flag that allows it
   (`--allow-dangerously-skip-permissions`) rather than editing settings, or fall back to the existing derived bypass
   screen and say so in the report. Do not sign in or update any agent. Note that answering a trust prompt in a
   throwaway folder leaves an entry in that agent's own configuration for that folder; list those folders in the report.
   Check specifically whether Cursor shows anything outside Run Everything: if it does not, Cursor can only ever read as
   YOLO or unreadable, so its YOLO shield sticks until the agent restarts; record that as a best-effort limit in SPEC
   and the docs rather than guessing from absence.
2. **Fixtures:** a separate small tree, for example
   `crates/farhelm-supervisor/tests/fixtures/status-lines/<agent>/<version>/<yolo|not-yolo|unreadable>-<scenario>.txt`,
   full grids, with a small loader in the reader tests that reuses the existing personal-data checks (the email and
   home-path checks in `screen_fixtures.rs`). Record vendor version and capture date in the directory name and test
   docstrings. Do not extend `scripts/capture-agent-screens.py`.
3. **Reader:** one exhaustive function keyed by agent type in `agent_kind/screen_reader.rs` (for example
   `fn status_line_yolo(harness, text) -> Option<bool>`, with `#[warn(clippy::wildcard_enum_match_arm)]` and no `_`
   arm), small private helpers per agent, anchored to the status line so transcript text above it cannot match. Update
   the map in `agent_kind/mod.rs`'s module docs. Readers stay pure and must not panic on arbitrary bytes.
4. **Supervisor:** the existing sampling runs the reader for the session's agent type and keeps the last definite
   reading in the per-run sample state (`ticker::ActivitySample`, which a rename shares and a relaunch replaces, so a
   restart resets it). `SessionInfo` gains `screen_yolo: bool` (`#[serde(default)]`, omitted when false), filled in the
   one builder shared by listing and rename replies (`service/status.rs` `entry_info`). Hint sessions-changed when the
   reported value flips. After a supervisor restart the field is false until each session's next sample; that is
   acceptable and gets one sentence in SPEC_impl.md.
5. **Helm:** no code change expected: `SessionRow` flattens `SessionInfo`, and `session_cache::merge_cached_session`
   takes such fields as given, which is right (a restart reply correctly resets the reading). Add one doc sentence there
   if useful. Confirm this rather than assume it.
6. **UI:** the `Session` mirror in `crates/farhelm-ui/src/lib.rs` gains the field with a default; `agent_badge` applies
   the glyph rule above and appends the hover and accessible text.
7. **PRs:** (1) fixtures, reader, supervisor field and proto test; (2) UI, SPEC, SPEC_impl, docs, fragment, TODO
   removal. Adjust if the pieces come out differently, without churn.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` (supervisor code changes),
the affected supervisor, proto and UI tests through the recorder, the test-sleep checker if tests change,
`python3 releasing/check-changelog.py format`, `dprint check`, and the docs website build if a page changes. A browser
test is optional: add one only if a UI behavior is not covered by the unit tests.

## Fallbacks

- If an agent's installed version shows no mode on its status line at all, or a mode cannot be reached without changing
  the maintainer's settings, leave that agent unread, say so in SPEC/docs as a best-effort limit, and report it. That is
  an agreed fallback, not a block.
- Examples of departures that need the scope reassessment: a protocol version bump, persisting readings, reading modes
  from command output (Codex `/status`, OMP `/settings`), letting the reading affect admission or approvals, a new
  per-harness trait or registry, or widening the existing fixture contract.

Decisions to log: the exact mode words matched per agent and version, the fixture layout, the Cursor finding, and the PR
split.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-detect-mid-session-yolo-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/detect-mid-session-yolo/<nn>-<short-name>` (for example `plan/detect-mid-session-yolo/01-status-line-reader`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat`. Add a changelog fragment (`kind: added`) per root
  `AGENTS.md`.
- The last PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

# Sounds when a session waits on the user, an approval arrives, or a turn finishes

Written against main at 2af378bd on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. The `approval-card-layout` plan (in flight when this was written) moves and
reshapes the approval cards; if it lands first, rebase onto it carefully (root `AGENTS.md`, Careful rebase) and detect
new approval requests from the data, not from the cards' markup, so either layout works.

## The goal

Play a sound when a session's agent needs the user, so a user looking at something else notices without watching the
sidebar: when a session starts waiting (a question, an approval prompt inside the agent), when a new Farhelm approval
request arrives, and, if the user turns it on, when an agent finishes its turn. Each event has its own switch in
Settings.

Acceptance criteria:

- Three events, each with its own sound from the Appendix (D1, D6):
  - waiting: a session's status becomes Waiting from any other status. Only Claude and Codex sessions can report Waiting
    today (SPEC.md, Status); the plan does not change that.
  - approval: an approval request the client has not seen before appears (SPEC.md's agent approval requests, the cards
    offering Allow, Always allow and Deny).
  - turn finished: a session's status goes from Running to Idle. This covers every agent. A session going from Running
    to Waiting makes only the waiting sound, and exits, errors and interruptions make no sound.
- Settings has three on/off settings, one per event, under a heading such as "Sounds", drawn the same way as the
  dialog's other settings. On a fresh device, waiting and approval are on and turn finished is off (D2). The switches
  are remembered per device, in the browser's and the desktop app's local storage the way terminal text size is, with
  the same rules: a missing, malformed or unreadable value means the default, and nothing goes to the helm (D4).
- Quiet rules (D3): no sound for an event about the session the user is already looking at, meaning the window is active
  (the existing `data-window-active` tracking in `crates/farhelm-ui/src/lib.rs`) and that session is the one open; for
  an approval request, the session that asked (every request names it). No sound for anything already true when the page
  loads or first signs in: the first read only records the starting state.
- Every session counts, including sessions a session-list filter hides (D5). The sound code does its own unfiltered read
  of the session list on every change-feed notice, independent of the session list's own filtered read, mounted beside
  the approval cards and following their pattern (they re-read on every feed notice). The helm reads the whole fleet for
  every list request anyway, so this costs one more small reply per notice, and it keeps the sound code out of the
  session list's filtering and absence logic. A session that first appears in a read makes no sound for its starting
  status unless that status is Waiting (a session that starts waiting is a session waiting on the user); a session that
  disappears makes none. If the unfiltered read is capped (the default list is limited to 500 sessions), the cap applies
  and is noted in SPEC_impl.md.
- When several sessions change at once, one read plays at most one sound per event, and when more than one event remains
  in one read, the most urgent plays (approval, then waiting, then turn finished) (planner proposal). The per-event
  settings and the quiet rules are applied first, so a switched-off or quieted event never hides an audible one.
- The sounds are generated in the page with the Web Audio API, from the note lists in the Appendix, at the fixed level
  the Appendix gives (D6). No audio file is added. One audio context serves the page. Browsers refuse sound until the
  user has interacted with the page: create or resume the context on the first user gesture, and if it still cannot play
  when an event fires, skip that sound silently rather than queueing it. In the desktop app the webview allows playback
  without a gesture; check that on Linux, and say in the report whether macOS was checked.
- Every open client makes its own sounds. Two windows open means two sounds; the per-device switches are how a user
  silences one (D4).
- SPEC.md's Status section gains a short paragraph on sounds: the three events, that only Claude and Codex sessions can
  report waiting, the defaults, that they are per device, and the quiet rules. Its sentence that desktop or
  operating-system notifications are not part of v1 stays true and is reworded only if it now misleads. The Settings
  description lists the settings. SPEC_impl.md records the storage keys, where transitions are detected, and the
  unfiltered read. The comment on the window-activity tracking, which says the stylesheet is its only reader, is
  updated. The docs website says what the sounds are and where to turn them off, in the page that describes Settings or
  the session list's statuses (`website/src/content/docs/docs/using/session-list.mdx`).
- Tests per Validation. The rules (each event, each quiet rule, the first read, a session appearing or disappearing, the
  burst rule and its ordering after settings and quiet rules) are unit tests of the pure detection function. Browser
  tests cover the wiring: they replace `AudioContext` with a recorder through `addInitScript`, as
  `e2e/tests/helpers/fleet.ts` already does for other globals, and check that a status change and an approval each play
  their sound, that a session a filter hides still sounds, that the open session in an active window stays quiet, and
  that the settings survive a reload.
- The last code PR removes the TODO.md entry "Audio signal when an agent is waiting on input." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Audio signal when an agent is waiting on input. Play a sound when a session's agent gets stuck waiting for the user (a
question, an approval prompt), so a user looking at something else notices without watching the sidebar. Possibly other
events too; which ones, and the sound, volume, and any setting to turn it off, are to be decided when this is picked
up."

**The user's decisions (2026-10-09):**

- D1. Events: waiting, approval requests, and turn finished, in the maintainer's words "waiting, approvals, turn
  finished - but settings dialog must allow opt-in/opt-out".
- D2. Defaults: waiting and approval on, turn finished off, because it fires much more often.
- D3. Quiet when the user is already looking: the window is in front and the session in question is the one open. No
  sound for sessions already in that state when the page loads.
- D4. The switches are per device, like terminal text size, which also lets a user silence one of two open windows.
- D5. All sessions, including ones a filter hides: a hidden session waiting on the user is the case where a sound helps
  most.
- D6. Distinct sounds per event, generated in the page with no audio files, chosen by ear from a set of twelve
  candidates: waiting "Bell call", approval "Ding-dong", turn finished "Soft pluck". The volume is fixed at the level
  the audition page started at.
- D7. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- Transition detection is a pure function from the previous and the new per-session statuses (and the previous and the
  new approval ids) to the events to play, unit-tested in whichever layer it lives in. The sound engine and the
  switches' storage live in a small asset script beside `terminal.js`, registered like the others in `lib.rs`, because
  local storage and Web Audio are JS-side in both builds, as text size already is.
- One always-on unfiltered read owned by the sound code, rather than reusing the session list's filtered listing, so the
  sound code never depends on the list's filter or absence handling.
- One sound per event per read, most urgent first, so a burst of changes (a helm reconnect, several sessions ending a
  turn together) does not machine-gun.

**What the planner found (at 2af378bd).**

- Statuses: `farhelm_proto::SessionStatus` (`crates/farhelm-proto/src/lib.rs`): Unknown, Running, Waiting, Idle, Exited,
  Interrupted, Error. The supervisor's ticker classifies sessions and hints the helm, which bumps the change feed
  (`/api/events`, revision numbers only); the client re-reads the session list (`assets/events.js`,
  `feed.rs::use_feed_reader`, a 3 s poll fallback while the socket is down). The natural place to compare statuses is
  `ListView` (`crates/farhelm-ui/src/list/view.rs`), where `commit_listing` stores each fetched listing; it follows the
  user's `SessionFilter` (`api.rs`), and the absence handling there uses `authoritative` / `omits_fleet_members()` to
  tell a filtered listing from the whole fleet.
- Approval requests: `crates/farhelm-ui/src/approvals.rs` (`ApprovalCards`, mounted in `lib.rs`) re-reads
  `api::fetch_approvals` on every feed notice; every card names the session that asked (`card.session`). A request
  expires after nine minutes; with no GUI connected the helm refuses at once.
- Window activity: `install_window_activity_tracking` in `lib.rs` sets `data-window-active` from `hasFocus()` and
  `visibilityState`.
- Settings: `crates/farhelm-ui/src/settings.rs` (`SettingsDialog`, opened by the gear beside the sidebar's version),
  defined in SPEC.md. Per-device storage pattern: `FONT_SIZE_KEY` and `storedFontSize()` in
  `crates/farhelm-ui/assets/terminal.js`; SPEC_impl.md's text-size paragraph notes desktop persistence relies on the
  webview keeping local storage (verified on Linux, not macOS); the same caveat applies here.
- No sound, notification, title or badge code exists anywhere today. The desktop webview (wry) allows autoplay by
  default. Desktop assets are served without HTTP range support, one more reason to generate sounds instead of shipping
  files.
- Browser test seams: `e2e/tests/notifications.spec.ts` and `sort.spec.ts` stub `/api/sessions`; `helpers/fleet.ts`
  (`stubFeed`) controls feed revisions; `helpers/approvals.ts` drives approval cards; `settings.spec.ts` covers the
  Settings dialog.

**Binding repository constraints:** root `AGENTS.md`: Finishing work (targeted validation; browser specs on Chromium and
WebKit through the recorder; the JS harness; the test-sleep check when browser tests change;
`scripts/check-desktop-assets.sh` if an asset file is added), Releases and the changelog (a fragment for `feat`), Docs
website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` first), Harness-specific code (do not branch on which
agent a session runs; which agents can report Waiting is already answered per harness), Desktop/web UI bug triage,
Testability, Sharing the machine, Agent scratch space, The live install is off-limits. `.agents/test-authoring.md` for
any test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: sounds for waiting sessions, approvals and finished turns

`feat:` with a changelog fragment. One PR: the parts are small and only useful together. The sound engine with the three
Appendix sounds; the unfiltered read beside the approval cards; the pure detection function with its first-read, quiet
and one-sound-per-read rules; the approval-id tracking; the three settings with their defaults and per-device storage;
SPEC.md, SPEC_impl.md and the docs page; unit tests for the rules and browser tests for the wiring. Remove the TODO.md
entry.

### Out of scope

Operating-system or desktop notifications, a dock or tab badge, a volume control, per-session or per-host muting, sounds
for exits and errors, custom sound files, and changing which agents can report Waiting.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the UI crate's unit tests
through the recorder, `scripts/check-desktop-assets.sh` if a new asset file is added,
`cd crates/farhelm-ui/js-tests && node --test`, the relevant Playwright specs (the new ones, `settings.spec.ts`, and the
approval specs) on Chromium and WebKit through the recorder after `cargo build` and the `dx` web build, the test-sleep
check, `cd website && bun install --frozen-lockfile && bun run build` when the docs page changes, `dprint check`, and
`python3 releasing/check-changelog.py format`. Listen to the three sounds yourself in a real browser if the machine has
audio; if it has none, say so in the report instead of claiming you heard them.

## Appendix: the three sounds

Each sound is a list of notes played through one master gain of 0.3 into the audio destination. A note starts `t`
seconds after the sound begins, at frequency `f` Hz, as a sine wave unless a wave type is given. Its gain ramps
exponentially from 0.0001 to its peak `a` (0.5 if not given) over 5 ms, then exponentially back to 0.0001 at `t + d`,
and the oscillator stops 50 ms after that. A note with overtones adds, for each `[ratio, relative]` pair, another
oscillator at `f × ratio` whose peak is `a × relative`, on the same envelope. The bell overtones are
`[[2.0, 0.25], [3.0, 0.08]]`. Start the sound 20 ms after the context's current time.

- waiting, "Bell call": `{f: 784, t: 0, d: 0.7, a: 0.4, bell overtones}`,
  `{f: 1047, t: 0.18, d: 0.9, a: 0.35, bell
  overtones}`.
- approval, "Ding-dong": `{f: 880, t: 0, d: 0.6, a: 0.4, bell overtones}`,
  `{f: 698, t: 0.25, d: 0.9, a: 0.4, bell
  overtones}`.
- turn finished, "Soft pluck": `{f: 523, t: 0, d: 0.35, wave: triangle, a: 0.45}`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-waiting-sound-log.md` in
the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before you
start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/waiting-sound/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (audio asset files, operating-system notifications,
sharing the switches through the helm, a new helm endpoint or protocol change for transitions, a volume setting; these
are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where transition detection and the sound engine live, the storage keys, how the
unfiltered read is triggered, how the first read and reconnects are treated, what counts as the open session for an
approval, and how blocked autoplay is handled, and every review finding you decided not to follow. The user will ask for
these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.

## Decisions

### 2026-10-10: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-10-10 (claim 88c8ce).
>
> ### Sounds make extra session-list reads that break existing browser tests
>
> The plan's sounds work (#1768, not merged; nothing of this plan is on main) watches the whole fleet for sessions that
> start waiting, new approval requests and finished turns. To see every session, including ones the sidebar's host
> filter hides (decision D5), it makes its own read of the session list, separate from the sidebar's, every time the
> helm says something changed, and once when the page opens. That separate read was the planner's proposal, not one of
> your decisions.
>
> The landing's browser run found that several existing browser tests assume the sidebar is the only thing in the page
> that reads the session list, and they now fail with the sounds in place (run `d0aa9ade`, on Chromium and WebKit):
>
> - The sort tests check that after a reload every list read asks for the order the user chose. The sounds' read always
>   asks for the default order (most recently active first), so "the chosen order survives a reload" and "the list is
>   not drawn until the remembered order is known" fail.
> - A test that holds list reads (it deliberately delays the helm's replies) and releases them one at a time, to prove
>   an old read cannot bring back a session's status from before it was stopped, now releases the sounds' read where it
>   expects the sidebar's, and fails.
> - A test that makes one held list read fail, to check the sidebar shows that failure, can hit the sounds' read
>   instead, so it fails or passes depending on which read it catches; in this run it failed on both engines.
>
> Nothing here is broken for a user: the sounds read the list correctly, and the sidebar still behaves the same. The
> failures are tests whose assumption the plan made untrue. Browser tests do not run in CI, so the executor's runs
> (which covered only the sounds, settings and change-feed tests) did not show it. They have to be settled before the
> plan lands, and how depends on whether the sounds keep a read of their own.
>
> ### Options
>
> 1. Keep the separate read, and make it recognisable to the tests: the sounds' request carries a marker the helm
>    ignores (an extra request parameter, say), and the affected tests and their shared helpers skip marked reads. No
>    change in behaviour: the marker exists only so tests can tell the two readers apart. The cost is a test-only tag on
>    a shipped request, and that every test that counts or holds list reads, now and later, must skip marked reads.
> 2. Share one read: the sidebar fetches the whole fleet and applies its host filter in the page, and the sounds use
>    that same listing. One read per change instead of two (the second read is a cheap local read on the helm, so this
>    saves little), and the tests keep their assumption. It is a larger change to the sidebar's reading, filtering and
>    failure handling, in an area those same tests guard closely, and the planner chose the separate read so the sounds
>    would not depend on the sidebar's filter.
> 3. Keep the separate read, but have it ask for the same order as the sidebar, and rework each held-read test by hand
>    (for example, by waiting for both readers' requests and releasing them in a known order). That fixes the sort tests
>    cheaply, but there is no single clean rule for the held-read tests, and each future one meets the same problem.
>
> Recommendation: option 1. It keeps your D5 behaviour with the smallest change, does not touch the sidebar, and gives
> every current and future test one rule for telling the sidebar's reads from the sounds', at the price of a marker that
> only tests use.
>
> ### Also for the next round
>
> The landing review found three smaller problems the next round should fix whatever the answer:
>
> - On a phone or tablet, sound never becomes allowed. The page enables audio as soon as a mouse button, pen or finger
>   goes down, but on touch screens browsers grant audio permission only when the finger lifts (or on a click), so every
>   sound on a touch device is dropped. Desktop browsers and the desktop app are not affected.
> - In SPEC.md's Settings paragraph, "the other two choices apply to every host and client" no longer says which two now
>   that the dialog also has three sound switches, and its list of per-device exceptions names sidebar width and
>   terminal text size but not the sound switches.
> - One unit test (a refused storage write does not undo the switches) checks that no sound plays without first checking
>   that one plays when it should, so it would also pass if audio never started in the test.
>
> The landing already rebased the plan's branch onto the sidebar-resizing work, which conflicted in SPEC.md,
> SPEC_impl.md and the list of scripts the page loads (each now holds both changes), restored a paragraph break in
> SPEC_impl.md that the rebase had dropped, and pushed the result to #1768, so the next round starts from it.

The maintainer's answer:

The maintainer's answer, verbatim:

> Let's adjust the plan to only do beeps for what the current filter view is showing. I think this should completely
> eliminate the tension here and not add a bunch more complexity nor require a bunch of refactoring? I was on the fence
> on this originally during design anyway. As a UX perspective therea re downsides to beeping for anything.

Agreed restatement:

- This replaces D5. Sounds are for what the session list currently shows: the sessions in the sidebar's current listing,
  under its host selector and filters. The sound code reads that same listing rather than making its own session-list
  request, so the page keeps a single reader of the session list and the existing browser tests' assumption that the
  sidebar is the only one holds. The separate always-on unfiltered read is dropped.
- Approval requests keep sounding for every new request, since their cards appear whatever the filter is.
- Sessions that come into view because the user changed the filter or host selector are treated like the first load: no
  sound for a state they were already in, only for changes seen after that.
- The next round also fixes the three smaller problems from the landing review: sound must become allowed on touch
  screens (unlock on the end of a touch or on a click, not only on a press); SPEC.md's Settings paragraph says which
  choices are shared by every host and client and lists the sound switches among the per-device settings; and the
  storage-refusal unit test first checks that a sound plays when it should.

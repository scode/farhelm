# A calm view while a session's supervisor is unavailable

Written against main at 0e7c3dd5 on 2026-10-16. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term, "A calm message while a supervisor upgrades") asks to replace the yellow text that appears
while a host's supervisor is upgraded with one clear message that looks intentional. The maintainer clarified while
planning: nothing is broken, and it happens on every supervisor update; the problem is that the session pane "barfs
yellowish text in the terminal area with errors/warnings" from the early days of Farhelm instead of a sensible view like
"session unavailable — waiting for supervisor". They also decided against tracking updates: whenever a session's
supervisor is out of reach, for whatever reason, show one calm, intentional view, with the actual error still available
below it in smaller text, in a look consistent with the rest of the UI.

The design was settled on a mockup the maintainer approved as drawn (proposal A, A2, and the neutral list mark):
https://snippets.scode.org/s/farhelm-session-unavailable-mockup/. The mockup is a reference only; this section is the
whole of the agreed design.

### What appears today (from code; confirm by reproducing)

When the helm loses a session's supervisor, the open session's pane shows, in turn: a red banner "Detached: supervisor
connection lost" at the top of the terminal (`crates/farhelm-ui/assets/terminal.js`, `.banner`); an amber overlay over
the frozen terminal, "connection lost — reconnecting in Ns (attempt n of N)…" with a "reconnect now" button, repainted
on every retry (`terminal.js`, `.terminal-connecting.terminal-reconnecting`, text in
`crates/farhelm-ui/src/reconnect.rs`); then, once the listing marks the session stale, the tab strip and terminal are
removed and an amber paragraph takes their place, "<host>: connecting, so there is no terminal to show — everything
below is the helm's last-known record of this session. attempt 4 is in flight; the last one failed: <error>"
(`stale_session_notice` and `state_detail` in `crates/farhelm-ui/src/hosts.rs`, rendered in
`crates/farhelm-ui/src/session_view.rs`, `.host-stale-notice`), changing on every retry; on reconnect, "<host> has
reconnected — refreshing this session's state." Every session row on that host gets an amber "stale" mark
(`crates/farhelm-ui/src/list/row.rs`, `.stale-badge`).

### Acceptance criteria

- **Reproduce first.** Before changing anything, reproduce a supervisor restart with a session open (the browser
  harness's `killRemoteSupervisor` / `restoreRemoteSupervisor` in `e2e/tests/terminal-multihost.spec.ts` restart a real
  second supervisor), and record in the working log exactly which messages appear, where, and in what order. Correct
  this file's list above in the log if it is wrong, and cover whatever you actually see. Also note how long a real
  update keeps the supervisor away, since the browser tests shorten the retry timings.
- **One card for the outage (proposal A).** While a session's terminal cannot reach its supervisor, the pane shows one
  card in the style of Delete's existing "Stopping agent…" overlay (`.terminal-delete-overlay` /
  `.terminal-delete-overlay-card`: accent border on the panel background, the accent spinner, over the scrim), centered
  in the terminal area, in a variant whose button can be clicked (the delete overlay ignores pointer events by design).
  The card holds, top to bottom:
  1. the spinner and the main line, in the card's existing 15px semibold accent text, "Session unavailable — <cause>";
  2. a second line in the muted foreground at 12px;
  3. small print in the dimmer foreground at about 11.5px with the details;
  4. a quiet "reconnect now" button (neutral, not amber) only where a manual retry can help (see below).

  The main line stays the same for as long as the cause does; only the small print changes as retries happen. When the
  connection is back, the card goes away and the terminal works again, with no amber "has reconnected" paragraph in
  between (a refreshing moment, if one stays visible, uses the same card with a calm line).
- **Where the cause comes from.** Two situations reach the pane, and the card covers both:
  - **The helm cannot reach the session's host** (the listing marks the session stale). The cause comes from one small
    mapping from the host's state to a short phrase, used both for the card's main line and for the list mark below:
    "waiting for supervisor" for the transient states (connecting, retrying, unreachable, no supervisor running on this
    machine), and a plain phrase naming each persistent state (for example "host needs update", "host is too new", "host
    identity mismatch", "host removed"). The second line is the helm's existing remedy text, shown verbatim as today's
    notice does (do not paraphrase it: it already says, for instance, that a "too new" host needs the helm updated, not
    the host), or, when there is none, a fixed reassurance such as "<host> is not answering. The session keeps running
    there; this view reattaches by itself." The small print is today's detail text (the helm's attempt number and last
    error from `state_detail`), escaped as host-supplied text. "State not read yet" is a calm "checking <host>…" line;
    "state could not be refreshed" is the helm's own read failing and says that, not "waiting for supervisor". No
    "reconnect now" button here: it cannot help while the helm cannot reach the host.
  - **The browser lost its own connection** (the terminal script's reconnect overlay: a helm restart, a network drop, a
    laptop waking, a heartbeat timeout, as well as the moments before the listing marks the session stale). The terminal
    script draws the same card, with a neutral main line, "Session unavailable — reconnecting", the existing "reconnect
    now" button, and the browser's own attempt text as small print. The "this page and the helm report different builds"
    case keeps its own wording inside the card. Do not try to merge the browser's attempt counter with the helm's: they
    count different things, and the UI code cannot read the browser's on the desktop app.
- **The red banner goes for recoverable drops.** Show "Detached: supervisor connection lost" (and its "Connection
  closed" / "Connection error" siblings) only when the disconnect is final (another client took control, with its "take
  control" button, or a stall the retry loop will not recover from); a drop the retry loop is recovering from shows the
  card instead. Only one overlay is visible at a time: when the stale card is up, a mounted terminal's own reconnect
  card is hidden under it.
- **The last screen stays, dimmed.** When a session that was showing its terminal goes stale, the pane keeps the tab
  strip and the last screen under the scrim instead of unmounting them (today `terminal_absence` returning `Stale`
  unmounts them). This needs:
  - **a prompt reattach**: when the session goes live again, trigger one immediate automatic retry in each mounted
    terminal, so the pane does not sit for up to 30 seconds waiting for the terminal's own slowed-down retry. It must
    not take control from another client (so not the user's "reconnect now" path), and it must leave a session whose
    host rebooted on today's interrupted card;
  - **inert tab controls while stale**: opening or closing tabs is not offered against an unreachable host;
  - a session opened while its host is already unreachable has no last screen: it shows the card on an empty pane, with
    no tab strip, as today.

  The existing browser test that asserts zero terminals and no tab strip for a stale session changes to assert the new
  behavior for a session that was showing its terminal, and keeps asserting the empty pane for one opened while stale;
  that is the intended change, not a weakening.

  Fallback, if the prompt reattach cannot be done safely within this plan: keep today's unmounting (the screen goes away
  when the session turns stale, and the tab strip with it) and show the card on the empty pane in every case. Log it as
  a DECISION and state it in the report's "things you should know", since it departs from the approved mockup.
- **The last-known status band stays** where it is today (`.stale-metadata`, the session's last-known status and its
  age): SPEC.md and an existing test require it in exactly one place, and the card does not repeat it.
- **The session list's mark becomes neutral.** Rows of sessions on a host the helm cannot reach replace the amber
  "stale" with the cause phrase from the mapping above, in the row's muted foreground (for example "waiting for
  supervisor", "host needs update"); the session's own status dot is greyed and does not pulse, as in the approved
  mockup. The tooltip still says this is the session's last-known state. Compact rows (a glyph with a hidden "stale"
  word and a tooltip) get the same cause in their hidden text and tooltip, and a neutral glyph color. The rows get the
  host states from the host list the session list already holds, passed down; no new fetch.
- **Unchanged:** the host row's "updating…" progress during an Update; a failed update still expanding the host row in
  red with its failed step; lifecycle operations against an unreachable host still refused with a clear error; the
  interrupted card for a session whose host rebooted.
- **Tests:** a browser test on Chromium and WebKit that opens a session on the second host, stops that host's
  supervisor, and checks the card appears with the right main line, no red banner and no amber text, the last screen
  still present under it, the row shows the neutral mark, and after restoring the supervisor the card goes away and the
  terminal works again promptly; unit tests for the cause mapping and the card's text per host state; update the
  existing tests of `stale_session_notice`, the reconnect texts, and browser tests that assert today's red banner, amber
  overlay text, "stale" text or the unmounted pane for this situation (search `e2e/tests` for "supervisor connection
  lost", "connection lost —", "stale" and `.host-stale-notice`) so they assert the new view without weakening what they
  check about behavior.
- **Spec and docs:** SPEC.md's host-list paragraph says sessions on an unreachable host are "clearly marked stale" and
  that opening one shows its metadata "behind a clear host-unreachable notice; there is no terminal to show and no
  pretense of one". Rewrite it for the new view: a calm card naming the cause, the last screen kept dimmed and inert
  under it while the outage lasts and clearly not live, and the neutral list mark. Update SPEC.md's Status section (the
  dot colors, and "a still dot whatever its status says") for the greyed dot of a session on an unreachable host,
  SPEC.md's session list text about the stale mark, and SPEC_impl.md wherever it describes the banner, the reconnect
  overlay or the stale notice. The docs website, if a page describes these messages (`website/AGENTS.md` governs it).
- **Changelog fragment**, `kind: changed`.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's words and decisions (2026-10-16):** "yes this is about supervisors. flicker was imprecise. it wasn't
broken. it was just 'barf some yellowish text in the terminal area with errors/warnings' from the early days of farhelm
instead of a reasonably sensible nice view like 'session unavailable - waiting for supervisor' or something. it happens
every time"; "make it fit given what we already do when sessions are being stopped"; "don't add complexity to track
updates. just make it so that whenever the issues happen ... we just show something that looks sensible and not just
like the app crapped out"; "we can still show the actual error messages if it's below in smaller font than the main calm
point that feels UI theme consistent". From the mockup: proposal A as drawn (including the dimmed last screen), A2 (the
same card naming non-transient causes), the neutral list mark with a greyed dot. Review gate: one fresh-context
gpt-6.1-sol reviewer at high effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md's host-list paragraph and Status section (changed by this plan as above);
SPEC.md's rule that host-supplied text such as error strings and host names is shown escaped outside a terminal (the
small print carries them); SPEC.md's rule that a supervisor restart must not interrupt its sessions (unchanged);
`.agents/test-authoring.md`.

**Planner proposals** (reviewed by a fresh-context planning review): reusing the delete overlay's look for the card; one
cause mapping shared by the card and the list mark; the helm's remedy text as the second line; the neutral
"reconnecting" line for browser-side drops; the prompt reattach; the fallback above.

## What planning found

- Plain Stop has no transitional view of its own (the badge waits for the server; header buttons dim). "Stopping
  agent…", "Stopping tabs…" and "Deleting…" are Delete's overlay (`.terminal-delete-overlay`), which is the look the
  maintainer meant by "what we already do when sessions are being stopped".
- `terminal_absence` in `session_view.rs` decides whether terminals mount; `Stale` currently unmounts them. Before the
  session turns stale, terminals stay mounted under the browser's retry loop.
- The browser's reconnect ladder lives in the terminal script and `reconnect.rs`: six retries (500 ms to 15 s), then a
  probe every 30 s, and a manual-only phase when the page and helm builds differ. It runs for any lost connection, not
  only a lost supervisor.
- `stale_session_notice` and `state_detail` (`hosts.rs`) already distinguish the host states and carry the helm's remedy
  text, which the UI shows verbatim on purpose.
- The colors: `--warn` / `--warn-bright` are the amber, `--danger-bg` the red banner, `--accent` the card, `--fg-1` and
  `--fg-2` the muted and dimmer foregrounds (`crates/farhelm-ui/assets/app.css`, dark theme only).
- The host row already shows a steady "updating…" with step and elapsed time during an Update and is not part of this
  change.

## Outline

1. **Reproduce and record** what appears.
2. **Pane:** the card component and the cause mapping; the terminal script's card for browser-side drops and the red
   banner's narrower condition; keeping terminals mounted while stale with the prompt reattach and inert tabs (or the
   fallback).
3. **Rows:** the neutral mark, regular and compact.
4. **Tests, SPEC, SPEC_impl, docs, fragment, TODO removal.**

PRs: (1) cause mapping, pane card and terminal-script card with tests; (2) keeping the last screen and the prompt
reattach; (3) rows; (4) spec/docs and TODO removal, or fewer PRs if they stay reviewable. Each PR carries the spec text
for what it changes where that keeps it coherent.

### Validation

Per `.agents/validation.md`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, the `farhelm-ui` unit tests through the recorder, the JS unit harness if
terminal script logic changes, the new and changed browser specs on Chromium and WebKit through the recorder (with the
builds they need), the test-sleep checker, `python3 releasing/check-changelog.py format`, `dprint check`, and the docs
website build if a docs page changed.

## Fallbacks

- The card on the empty pane in every case (today's unmounting kept), if the prompt reattach cannot be done safely; see
  the criterion.

Departures that need the scope reassessment (examples, not a blacklist): tracking updates as their own state, a new helm
or protocol field, changing the reconnect ladder's timing, bridging the browser's attempt counter into the UI code,
changing the host row, or changing what lifecycle operations do on an unreachable host.

Decisions to log: the reproduction's findings, the cause phrases per host state, whether the last screen stays mounted
or the fallback was taken, how the prompt reattach avoids displacing other clients, and the red banner's new condition.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-calm-session-unavailable-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
("Sub-agents this file requires") requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from the no-delegation demand and run as that file says, not through galaxy-brain.

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
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/calm-session-unavailable/<nn>-<short-name>` (for example `plan/calm-session-unavailable/01-pane-card`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `feat` (a changed user-visible view). Add a changelog
  fragment (`kind: changed`) per root `AGENTS.md`.
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
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing one plan, steps
11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.

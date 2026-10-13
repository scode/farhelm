# A click on a link Farhelm recognizes opens it once

Written against main at d1fe461c on 2026-10-13. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Fix duplicate link opening": one click on a link in Farhelm opens two separate browser
tabs for the same URL. Fix it so one click opens only one intended browser tab.

The cause, as diagnosed in planning and reproduced by the maintainer in a Codex 0.162.1 session on a remote Linux host
(a link in Codex's answer, one click, two tabs): Codex's full-screen interface turns on terminal mouse reporting, so the
terminal sends the click to Codex as well, and Codex opens links you click on itself through the Rust `webbrowser`
crate. Farhelm's terminal link handler opens the same link on the same click. On a remote host Codex's own open used to
fail silently (no browser, no `BROWSER`); since agent link forwarding landed (#1944, #1948, #1950) it succeeds through
`farhelm agent open-url`. On the Mac's own machine Codex opens through macOS directly (the crate never reads `BROWSER`
on macOS), so a local session there opened twice even before forwarding.

The fix, decided by the maintainer: when a click lands on a link Farhelm recognizes, Farhelm opens it and does not send
that click to the program at all, so the program has nothing to open. A setting lets the user choose otherwise.

Acceptance criteria:

- **Interception (the default).** When the pane's program has mouse reporting on and the user presses the primary button
  on a web link Farhelm would open (a plain-text URL or an OSC 8 hyperlink that passes the same checks the open path
  applies today), Farhelm opens the link on release as it does today and the program receives neither the press nor the
  release. A press anywhere else, other buttons, moves, the wheel, and every other mouse event reach the program exactly
  as today. When the program has no mouse reporting on, nothing changes: there is no click to withhold.
- **Web links only.** File links (the download hover) are not intercepted: plain-text path recognition is broad, so
  intercepting them would withhold ordinary clicks in agent transcripts, and nothing opens them twice. Their behavior is
  unchanged under every setting value.
- **Drags and selection.** A primary-button press on an openable web link belongs to the link, so a drag that starts on
  one sends the program nothing; the forced-selection modifier (Shift, Option on macOS; SPEC.md "Terminal experience",
  the copy-on-select bullet) still selects locally from there, as today. Do not build a replay of withheld presses. A
  withheld press still dismisses the copy-on-select drag notice (SPEC says it closes on the next press in that terminal)
  and never shows it.
- **The setting**, "clicking a link in a terminal" (or close to it), one choice for every client, stored on the helm
  with the other shared preferences, with three values:
  - **open in Farhelm, don't pass the click on** (the default, the interception above);
  - **open in Farhelm and also pass the click on** (today's behavior: some programs may open the link a second time);
  - **pass the click on only** (Farhelm does not open links on click; the program gets every click, and without mouse
    reporting a click on a link does nothing, as in a plain terminal). The settings dialog is already organized into
    sections (#1949): add a "terminal links" section of its own, with the three choices as radio buttons, each with a
    short explanation, and a note that the choice covers every client of this helm (so it does not sit under the sounds
    note, which says "this device"). Existing installations get the default. Other open clients pick the change up the
    way they pick up the host choices.
- **Forwarding unchanged.** `BROWSER` forwarding, its cards and its settings stay as they are, including sessions on
  this machine getting no `BROWSER`: an agent that opens a link on its own still goes through forwarding.
- **Docs**: SPEC.md "Terminal experience" (the link bullets, which say a link opens on click, and the mouse reporting
  rules) and the settings description; SPEC_impl.md where it describes link activation; the website page that covers
  links in the terminal and the settings page. Say what the default does and when to pick each other choice.
- Tests as listed under Outline.
- A linear stack of draft PRs, each passing its review gate, with a changelog fragment, the last one removing the
  TODO.md entry. No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-13):** plan this TODO entry. The maintainer suspected the forwarding change and that Codex
handles the click itself in its full-screen mode; planning confirmed both from the code and the Codex binary, and the
maintainer reproduced it.

**The user's decisions (2026-10-13), in order:**

- Planner decisions stated to the maintainer with the plan: intercept web links only, not file links; a press on a web
  link belongs to the link (no replay of withheld presses for drags), and the forced-selection modifier still selects.
- First choice, superseded: deduplicate by having the page report each click and the helm drop the agent's echo, with
  `BROWSER` also on this machine. Planning then found that the `webbrowser` crate ignores `BROWSER` on macOS, so neither
  could fix a local session on the Mac.
- The decision this plan builds, in the maintainer's words: "What do you think about not sending the click at all when
  the click is such that it was identified as a URL click? I think that is way more likely to match the vast majority of
  people's experiences. Like if somebody's using something where it is a link and they want to click it, and it's also a
  link recognized by Farhelm, that seems like an edge case, and we can still make a setting to disable this behavior.
  That somebody can click a button and say either disable click to open entirely or to disable the interception. So they
  can either get no Farhelm-assisted clicking at all, or they can get Farhelm-assisted clicking with the duplication
  risk, but it unblocks whatever their use case is, or they get the default behavior, which is that we intercept and we
  have the Farhelm-assisted clicks."
- The setting is one choice for every client, in app settings, stored on the helm (offered and not chosen: per device,
  like the sounds).
- No echo deduplication, no click report to the helm, and no change to `BROWSER` on this machine.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Terminal experience": links open their http(s) target on click with no confirmation, the hover display is the
  whole safeguard, and the copy-on-select and mouse reporting rules; this plan amends the link and mouse reporting rules
  as decided and keeps the rest.
- SPEC.md "Upgrade compatibility and client scale" for the new shared preference (a migration and a default for existing
  installations).
- Root `AGENTS.md`: Conventional Commits; Finishing work (browser tests through the recorder on Chromium and WebKit,
  `scripts/check-test-sleeps.py` for test delays); Releases and the changelog; Agent scratch space; Sharing the machine
  with other agents; Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before touching a page).
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main and corrected by a
fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/assets/terminal.js`: the xterm.js terminal (vendored 6.0.0, minified in
  `crates/farhelm-ui/assets/vendor/xterm.js`). OSC 8 links go through `linkHandler.activate` (a selection guard, then
  `farhelmTerminalFiles.oscTarget`, web targets to `openTerminalUrl`); plain-text URLs through the `WebLinksAddon`
  callback (selection guard, `isPlainWebUrl`, `openTerminalUrl`). xterm activates a link on mouse release (of any
  button), and reports the press to a mouse-reporting program as soon as it happens, so withholding the click means
  deciding at press time. The terminal knows the hovered link for OSC 8 links and file links, but the web-links addon is
  created with no hover or leave callbacks, so plain-text URLs need them added. It reads `term.modes.mouseTrackingMode`
  for the copy-on-select policy.
- In the vendored xterm, the link layer records the pressed link in its own press listener on the screen element and
  never clears it after a release, and xterm offers no supported way to veto a mouse report. A capture-phase stop on the
  terminal element hides the press from that listener, and a stale record then lets xterm open the link a second time
  (Shift-click a link, then plain-click it). A listener on the screen element added after `term.open()` runs after
  xterm's link listener: stopping the press there (with `preventDefault()` and `term.focus()`) keeps xterm from
  reporting it, because xterm attaches its release and drag reporters inside the press handler that this skips, while
  xterm's own release handling still opens the link through the existing callbacks. That is the planner's preferred
  mechanism; verify it against the vendored build.
- `crates/farhelm-ui/assets/terminal-links.js`: `openTerminalUrl` (desktop navigation on the `dioxus:` page, web
  `window.open`) and `isPlainWebUrl`.
- Shared preferences: `crates/farhelm-ui/src/api.rs` (`Preferences`, `PreferenceField`, the `PUT /api/preferences`
  patch) and the helm's preferences table, whose migrations live in `crates/farhelm-helm/src/store.rs`. A nullable text
  column where empty means the default needs no backfill. The downgrade fixture tests in `checkout_config.rs`, `lib.rs`
  and `store.rs` drop preference columns and must drop the new one too. The settings dialog is
  `crates/farhelm-ui/src/settings.rs`.

Here's the work, as a proposal:

1. **Interception**, by the screen-element listener described above, only for the primary button, only when the hovered
   link is an openable web link, and only under the default setting; mirror the open path's checks so a click Farhelm
   would refuse to open is never swallowed. Do not patch the vendored build. Log the mechanism as a DECISION.
2. **The setting**, as one page-level mode in `terminal.js` that Rust sets from the shared preferences and every
   terminal reads at press time; "pass the click on only" makes the web open callbacks return early.
3. **Docs** as in the acceptance criteria.
4. **Tests**:
   - JavaScript unit tests for the decision (link under pointer, reporting on or off, the three setting values, a drag),
     where it can be kept pure;
   - Playwright, on Chromium and WebKit, against the real stack with the fake agent's existing `mouse-modes` script (it
     turns on mouse reporting and echoes every input byte) plus a new cue that prints a plain-text link and an OSC 8
     link: with the default, a click on each link calls the opener exactly once (the existing `recordWindowOpen` pattern
     in `e2e/tests/terminal-links.spec.ts`) and the program receives no report for it, and a click beside the link
     afterwards does reach the program (proving reports were flowing); with "also pass the click on", the opener runs
     once and the program receives the click; with "pass the click on only", the opener does not run and the program
     receives the click; a Shift-click then a plain click on the same link opens it once each, never twice; the
     forced-selection modifier still selects; a file link's behavior is unchanged;
   - the preference's migration default and its round trip, as the existing host choices are tested, and the three
     downgrade fixtures;
   - existing terminal link, mouse mode and copy-on-select tests keep passing.
5. **Bookkeeping**: changelog fragment, `kind: fixed` (one click on a link in a program like Codex opens one tab, with a
   setting for programs that need the click). Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`;
- through the recorder: the touched crates' unit tests (the preference migration);
- `cd crates/farhelm-ui/js-tests && node --test`;
- the changed Playwright specs (terminal links, mouse modes, copy-on-select, settings) on Chromium and WebKit through
  the recorder;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and the website build for changed
  pages.

Do not run the full battery without a specific risk that calls for it. The report asks the maintainer to try the real
case again in a Codex session (one click on a link in Codex's answer, on a remote host and on the Mac) once it ships.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-link-click-intercept-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/link-click-intercept/<nn>-<short-name>` (for example `plan/link-click-intercept/01-intercept-link-clicks`).
- Bite-sized PRs, each leaving main correct on its own. Avoid churn: code added in one PR and deleted in a later one
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix` for the
  interception, `feat` for the setting if it lands separately). Such a PR adds a changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

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

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that with the default, a program under mouse reporting receives neither press nor release of a click on a recognized
  link, and receives every other mouse event unchanged;
- that each of the three setting values behaves as specified, in every mounted terminal, without a remount;
- that drags, selection and the forced-selection modifier behave as before;
- that the preference's migration gives existing installations the default;
- that the browser tests observe the program's received mouse reports, not just the opener.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example patching the vendored xterm build,
  deduplicating opens at the helm, changing `BROWSER` forwarding, or a per-program list of exceptions (examples, not a
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
alternatives considered. In particular:

- the interception mechanism;
- what a drag starting on a link sends;
- whether file links follow the same rule;
- the setting's label, values' wording and default migration;
- the stack's shape;
- the SPEC.md, SPEC_impl.md and website wording;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: intercepting other mouse events, a per-device
setting, changing forwarding or `BROWSER`, or deduplicating at the helm instead.

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

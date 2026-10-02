# Looping animations step instead of interpolating, and stop while the window is inactive

Written against main at 25c6a731 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan.

## The goal

On the maintainer's MacBook, the Farhelm desktop app kept WindowServer at roughly 40-50% CPU and drained the battery,
with the visible terminal idle and even with the Farhelm window not visible, whenever some sessions were running. The
planning session diagnosed it: with sessions running, the sidebar shows their green status dot pulsing via
`farhelm-status-pulse` in `crates/farhelm-ui/assets/app.css` (`2s ease-in-out infinite`, opacity 1 to 0.45). An
animation whose timing function interpolates (`ease-in-out`, `linear`, and so on) produces a new frame on every display
refresh for as long as it runs, so an infinite one makes the compositor redraw the window at the display's full rate
(120 Hz on that machine) forever and keeps the display out of its low-power idle refresh. The page's own work for it is
cheap; the cost lands in WindowServer. The maintainer confirmed the diagnosis: turning macOS Reduce Motion on (which
`app.css` already honors by removing the animation) made the load go away, and turning it off brought it back. That is
one observation on one 120 Hz Mac, not a measured constant, but it is conclusive about the cause.

The fix has three parts: a spec rule that forbids this animation strategy everywhere, stepped timing for every looping
animation the app has today, and pausing them all while the window is not active.

Acceptance criteria:

- SPEC_impl.md states the rule precisely enough that an agent writing new UI code knows what is forbidden without having
  to interpret the word "smooth" (see U1 and the wording requirements below). SPEC.md states the user-facing half.
- Every looping animation in `app.css` uses stepped timing within the rate cap: the running-status pulse
  (`.status-badge.running .status-dot`), the host update dot (`.host-update-dot`), the delete header dot
  (`.delete-progress-dot`), and the delete spinner (`.delete-spinner`). The pulse changes at about 4 visual steps per
  second over its existing 2 s cycle and visual range; the spinner at 8 steps per 0.8 s.
- While the window or tab lacks keyboard focus or is hidden, every CSS animation on the page is paused, and it resumes
  when the window is active again. This holds in both the web and desktop builds.
- Under Reduce Motion (`prefers-reduced-motion: reduce`), every one of these indicators is still static, as today.
- A `node --test` check in `crates/farhelm-ui/js-tests/` enforces the stepped-timing rule on `app.css`.
- The `app.css` comments that call the pulse "the only animation in the app" and say opacity is the one property "a
  compositor can animate without touching" layout or color are rewritten; both are wrong or misleading now (there are
  four looping animations, and compositor-only does not mean free).
- The TODO.md entry is removed, and this plan's `plans/INDEX.md` line is marked `[executed]`.

## Requirement sources

**The user's request (2026-10-02), verbatim:**

> lets use the planning system to plan it out and schedule it.
>
> - SPEC should lcearly say that nothing must use the "smooth" animation strategy for CPU/battery reasons. make sure the
>   language is specific enough that agents understand what is meant, don't just use the word smooth.
> - For this particular animation, change it as you propose
> - pause animation when window is not active.

> ideally still honor "reduce motion" if that's possible esily

"Change it as you propose" refers to the planning session's proposal: give the pulse `steps()` timing so it changes a
few discrete times per second instead of on every frame, which reads as a slightly chunkier heartbeat that is still
clearly alive, and pause looping animations with `animation-play-state: paused` while the window is not active.

**The user's decisions (2026-10-02):**

- U1. The rule goes in the specs, worded so agents understand exactly which strategy is banned and why (CPU and
  battery), not by the word "smooth".
- U2. "Window not active" means the window (or browser tab) does not have keyboard focus, or is hidden or minimized. A
  Farhelm window visible beside another app that has focus counts as inactive, and its dots freeze until the user
  returns. Web and desktop builds behave the same.
- U3. Scope: all looping animations get stepped timing and the pause, not only the status pulse. The planning session
  named three; the planning review found a fourth (`.delete-progress-dot`), and U3 covers it.
- U4. Rate: a cap of 10 visual changes per second for anything that repeats on its own. The pulse uses 8 steps per 2 s
  (4 per second); the spinner 8 steps per 0.8 s (10 per second), like a classic stepped spinner.
- U5. Reduce Motion stays honored: under it, every looping indicator is static.
- U6. Verification: you prove the contract with tests. You do not measure WindowServer; the maintainer checks the real
  effect on macOS after merge. If stepped CSS turns out not to help in practice, that is a follow-up, not part of this
  plan.
- U7. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter (below).
- U8. No-workhorse mode: you do all the work yourself (see How to run).

**Binding repository constraints:**

- SPEC.md is product behavior and implementation-neutral; SPEC_impl.md holds implementation choices and their
  motivations (root `AGENTS.md`). So the precise rule lives in SPEC_impl.md, under "GUI: Dioxus" near the other visual
  styling rules (the surface ladder and the closed accent list), and SPEC.md gets the product-level statement.
- SPEC.md already says "running pulses" (the Status section, "How a status is DRAWN") and, for host updates,
  "Reduced-motion settings replace the animated indicator with a static one". Both stay true; amend around them rather
  than contradicting them.
- `app.css` declares the status pulse's reduced-motion override early and the other dots' override after their own
  rules, with a comment explaining that the order is load-bearing (same specificity). Keep that property intact for
  whatever overrides remain.
- A `fix:` PR carries a changelog fragment under `releasing/changelog.d/` in the same commit (root `AGENTS.md`, Releases
  and the changelog).
- `crates/farhelm-ui/js-tests/app-css-tokens.test.js` already parses `app.css` statically; it is the precedent for the
  new check. Run the JS harness from inside that directory with `node --test` (root `AGENTS.md`).

**Planner choices (after the planning review):**

- P1. One universal pause rule rather than a list of selectors:
  `:root[data-window-active="false"] *, :root[data-window-active="false"] *::before, :root[data-window-active="false"] *::after { animation-play-state: paused !important; }`
  (or an equivalent). It covers current and future animations, so the next looping indicator cannot forget to pause.
  Nothing else in the stylesheet sets `animation-play-state`, so `!important` costs nothing. It also pauses xterm.js's
  own stepped cursor blink, which only runs while the terminal is focused anyway.
- P2. The attribute is set by about six lines of JS installed once from the app root (`App` in
  `crates/farhelm-ui/src/lib.rs`), for example a `use_hook` running one `document::eval` that adds `focus`, `blur` (on
  `window`) and `visibilitychange` (on `document`) listeners, each setting
  `document.documentElement.dataset.windowActive` from `document.hasFocus() && document.visibilityState !== "hidden"`,
  plus one immediate call so the initial state is right. No new asset file: an asset would need a `declare_assets!`
  entry, script tags in both the web and desktop roots, desktop asset parity, and a unit test, all for one boolean
  expression. The listeners never talk back to Rust, so the eval channel finishing does not matter. If you find a
  concrete reason this does not work in one of the builds (for example the eval runs before `<html>` exists, or the
  desktop build rejects it), choose the next-smallest place and log it as a DECISION.
- P3. The `node --test` check asserts that every `animation` (or `animation-timing-function`) in `app.css` whose
  iteration count is `infinite` uses `steps(n)`, `step-start` or `step-end`, and that its change rate (steps divided by
  duration) is at most 10 per second. Keep it narrow: no coverage checks for the pause rule (P1 makes them unnecessary),
  no detection of long non-infinite animations (none exist). It reads `app.css` only; the vendored `xterm.css` is not
  ours to lint.
- P4. No Playwright spec. Playwright reports pages as focused, and a synthetic `blur` does not change
  `document.hasFocus()`, so a browser test would either fail against a correct implementation or push the code into
  trusting events over `hasFocus()`. The literal CSS values are already proven by P3, and the real acceptance test is
  the maintainer's macOS check (U6), which is the only one that exercises WKWebView's actual focus and visibility
  behavior.

## Wording requirements for the spec rule

The SPEC_impl.md text must let an agent decide mechanically whether its change is allowed. Write it in your own words,
but it must cover all of this:

- Stated by effect first, so it covers more than CSS: nothing on screen may redraw on its own, without new content to
  show, more than 10 times per second. "On its own" means an indicator or decoration that moves, fades, spins or blinks
  while nothing about the underlying state changed. Rendering new content in response to new data (terminal output
  arriving, a list update) is not animation and is not covered.
- Then how that is spelled in this codebase: a CSS animation that repeats (`infinite`, or any iteration count that makes
  it run on indefinitely) must use a stepped timing function (`steps(n)`, `step-start`, `step-end`) with at most 10
  steps per second. Interpolating timing functions (`linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`,
  `cubic-bezier(...)`) on such an animation are forbidden, and so are `requestAnimationFrame` loops and JS timers that
  change what is drawn more than 10 times per second. The list explains the rule; it is not a whitelist, and a new
  technique that redraws per frame is just as forbidden.
- One-shot transitions and animations triggered by a user action or a state change, that finish within about 1 s, are
  allowed (for example the 100 ms hover background transitions, and xterm.js's 800 ms one-shot scrollbar fade).
- Every looping animation pauses while the window is not active (U2), and is static under Reduce Motion (U5).
- Why: an interpolated animation produces a new frame on every display refresh for as long as it runs. On macOS the
  compositor (WindowServer) then redraws the window at the display's full rate, up to 120 Hz, and the display cannot
  drop to its low-power idle rate. One running-status pulse was observed to hold WindowServer at roughly 40-50% CPU on a
  120 Hz MacBook, even with the window hidden. Present that number as one observation, not as a constant.
- Why not just rely on the compositor: animating only opacity or transform keeps the page's own main thread idle, but
  the compositor still redraws every frame, so "compositor-only" does not make a looping animation cheap.

SPEC.md gets a short product-level statement, in the Status section where it says "running pulses" or wherever reads
best: looping indicators such as the running pulse change in discrete steps rather than continuously, and they stop
while the Farhelm window is not focused or not visible, to save CPU and battery. Keep it to one or two sentences.

## Implementation outline

One PR (`fix:`). Line numbers drift; find code by name.

- SPEC_impl.md and SPEC.md per the wording requirements above.
- `app.css`:
  - The pulse keyframes and the four `animation:` declarations get stepped timing (U4). How to keep the pulse reading as
    a pulse is your visual call: for example keep the 1 to 0.45 to 1 opacity range and use `steps(8)` over 2 s, or write
    explicit keyframes at eighths with `step-end`. Look at the result before settling; if you can render the page (the
    web build in a browser, or any local means), do so, but do not build a full stack just to look.
  - The spinner gets `steps(8)` over its 0.8 s rotation.
  - The universal pause rule (P1).
  - The comments above the pulse and the delete indicators are rewritten to say why the timing is stepped and why the
    pause exists, pointing at the SPEC_impl.md rule; the reduced-motion overrides stay and keep their ordering comments.
- `crates/farhelm-ui/src/lib.rs` (or the place P2's fallback picks): the window-activity listener install, with a doc
  comment that says what the attribute means, which events drive it, and that nothing reads it from Rust.
- `crates/farhelm-ui/js-tests/`: the P3 check, in a new test file or beside `app-css-tokens.test.js`, with a docstring
  saying why the rule exists (the WindowServer observation) and what it asserts.
- Changelog fragment, `kind: fixed`: running sessions no longer keep the Mac's window server busy and drain the battery;
  their status dots pulse in steps and stop while Farhelm is in the background. Follow
  `releasing/EDITORIAL_GUIDANCE.md`.
- Remove the TODO.md entry. Mark this plan's line in `plans/INDEX.md` `[executed]`.

### What not to build

No Playwright spec (P4). No new asset file unless P2's fallback forces it. No JS-timer-driven replacement for the CSS
animations: if stepped CSS does not help in practice, that is the maintainer's follow-up (U6). No WindowServer
measurement, no desktop build for measuring, and no change to xterm.js or its vendored CSS. No new user preference for
animation.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-stepped-animations-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain.

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

- Use the `jjstack` skill. The stack's base is not main but the tip of the plan stack, set up per `plans/AGENTS.md`
  (Executing, step 4). PRs already in the plan stack, from earlier plans or an earlier blocked run of this one, are the
  base and are not rewritten.
- One commit, bookmark and draft PR. Within this run, if it needs correcting, restructure it rather than stacking a
  correction on top.
- Commit message and PR title use Conventional Commits; the PR is `fix:`. It adds its changelog fragment in the same
  commit; validate with `python3 releasing/check-changelog.py format`.
- Run the commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. The PR
  description should say that the real WindowServer effect is for the maintainer to confirm on macOS (U6).
- The PR stays a draft. Never mark it ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression. Typical choices, not
a checklist: `cd crates/farhelm-ui/js-tests && node --test`; `cargo fmt --all -- --check`;
`cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` if the desktop
dependencies are available (the listener install touches code both builds compile; if the desktop check cannot run on
your machine, say so in the report); `dprint check` on changed Markdown and CSS; and
`python3 releasing/check-changelog.py format`. Apply `.agents/test-authoring.md` to the new JS test. No browser tests
(P4) and no Rust test battery unless your change touches Rust beyond the listener install.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort (U7), shelled out to the other harness if the executing one cannot reach
that model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: The goal, U1-U8, P1-P4 and the wording requirements. Include
the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test changes. Address what the
reviewer finds before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or
in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new asset file, a JS-driven animation, a
Playwright spec, a user preference, a change to xterm.js; these are examples, not a blacklist), and whenever the same
component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the request, the user decisions above, this outline, the current diff and the proposed departure
(what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the pulse's exact stepped form and why it still reads as a pulse, where the
listener install lives, the final spec wording's placement, how the CSS check parses `app.css`, and every reviewer
finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. The one
agreed fallback is P2's (a different install location for the listeners). Anything else that needs a decision: record
the concrete tradeoff and block per `plans/AGENTS.md` (Executing, step 7).

## Done criterion

The plan is complete when its one draft PR exists on the plan stack's tip, satisfies the acceptance criteria, has passed
the review gate, has removed the TODO.md entry, and has marked this plan's `plans/INDEX.md` line `[executed]`. Open, not
merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing, step 8): write its report,
write a closing entry in its log, and stop the watchdog.

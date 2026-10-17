# Make input-method guards work in WebKit

Written against main at 0e7c3dd5 on 2026-10-16. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term, "Make input-method guards work in WebKit"), verbatim:

> Escape or Enter that ends an input-method composition should only end the composition, but WebKit (the Mac app and
> Safari) ends composition before the keydown of the key that ends it, so that key arrives with `isComposing` false and
> acts as a normal key (WebKit bug 165004). The usual extra check, key code 229, is not available through Dioxus on the
> desktop renderer. So the template panel's composing-Escape fix (#2115) very likely does nothing in the Mac app, and
> the same gap affects about 25 other input-method guards across the UI, including that panel's earlier composing-Enter
> fix. Dig into a WebKit-proof approach and apply it everywhere. Browser tests that fake a keydown with
> `isComposing: true` do not show the Mac app is fixed, because real WebKit never sends one.

### Acceptance criteria

- **Every input-method guard in the UI holds in WebKit**: a key that ends a composition (Enter, Escape, or any other key
  a guard checks) only ends the composition, in the Mac app, Safari, and Chromium alike. All guards, not only Enter and
  Escape (the maintainer's choice): planning counted 35 (29 Rust `is_composing()` checks under `crates/farhelm-ui/src`,
  6 JS `isComposing` checks); recount them yourself and cover every one outside the terminal, including the two shared
  helpers `guard_confirmation_enter` (`crates/farhelm-ui/src/lib.rs`) and `enter_choice`
  (`crates/farhelm-ui/src/launch_controls.rs`). The one deliberate exclusion is `assets/shift-enter-key.js`: it runs
  only inside the terminal (out of scope below) and already checks `keyCode === 229` itself. The window-level chord
  guards (Cmd/Ctrl+Shift+K and +P in `lib.rs`, the template number keys) are in scope.
- **A plain Enter with an input method on but nothing composed still works.** macOS Pinyin, for one, stamps
  `keyCode === 229` on an ordinary Enter even when nothing is being composed, so 229 alone must never mark a key as
  composing.
- **A real key right after a composition still works**: an Escape or Enter pressed after the composition has ended (a
  separate keystroke) acts normally in every engine. In Chromium, `compositionend` arrives after the keydown that ends
  composition, so a "recently ended" window must not swallow the user's next real key there.
- **The terminal is out of scope**: xterm.js has its own composition handling, and nothing in this change alters input
  to the terminal's helper textarea.
- **Tests:**
  - unit tests for any pure decision logic, in the existing JS harness (`crates/farhelm-ui/js-tests`, run with
    `node --test` from inside it) or in Rust, whichever holds the logic;
  - browser tests, on Chromium and WebKit, that replay WebKit's real order by hand (`compositionstart`, `input`,
    `compositionend`, then the ending key's `keydown` with `isComposing: false`, and with `keyCode: 229` too if the
    engine lets a test set it) against a representative set of guards: at least the template panel's Escape and Enter
    (#2115 and its earlier Enter fix), rename's Enter and Escape, and one text field whose Enter goes through
    `enter_choice`. (`guard_confirmation_enter` guards Confirm buttons, which cannot host a composition, so a browser
    test of it would prove nothing real.) Each also checks that a separate real Escape or Enter afterwards still acts,
    and one checks that an Enter carrying `keyCode: 229` with no composition before it still acts. Keep the existing
    `isComposing: true` tests; they cover Chromium's order.
  - Say in the test docstrings that these replay WebKit's order rather than prove the Mac app.
- **Verification on the Mac is the maintainer's** (their choice): you run unattended on Linux and cannot type with a
  real input method on a Mac. The report's "things you should know" gives exact manual steps for the Mac app (for
  example: turn on a Japanese input method, type in the template panel's name field and in rename, end the composition
  with Escape and with Enter, and confirm the dialog stays open and nothing is submitted; then press Escape again and
  confirm it closes).
- **Docs:** SPEC_impl.md gains a short section on how the UI tells a composition-ending key from a real one and why
  (WebKit bug 165004, the missing key code through Dioxus on desktop, Chromium's opposite order). SPEC.md only if its
  existing composing-Enter wording needs adjusting.
- **Changelog fragment**, `kind: fixed`, in product terms (input methods in the Mac app and Safari: ending a composition
  with Enter or Escape no longer submits or closes the dialog).
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-16):** cover every guard; verify with browser tests that replay WebKit's event order,
and leave the real-Mac check to the maintainer after landing, with exact steps in the report; review gate: one
fresh-context gpt-6.1-sol reviewer at high effort, no swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** `scripts/check-desktop-assets.sh` (every asset exists in both the web and the
desktop build); `.agents/validation.md`'s rule that browser specs run on Chromium and WebKit;
`.agents/test-authoring.md` for the tests; SPEC.md's existing composing-Enter wording ("held or composing Enter never
restarts", "Repeated or composing Enter cannot answer the new question").

**Planner proposal, checked against the Dioxus source by the planning review:** one small capture-phase listener pair on
`window`, for `compositionend` and `keydown`. The rule: a keydown is marked as composing only if it is not already
composing, a `compositionend` fired on the same target since the previous keydown, and it either carries
`keyCode === 229` or arrives within a short window after that `compositionend` (newer Safari is reported to send the
real key code, 13, on the ending keydown, so 229 alone does not catch it; and 229 alone must never mark a key, per the
Pinyin criterion). Marking means defining an own `isComposing` property with value `true` on the event instance, so
every existing guard sees `is_composing()` true and needs no change. It never calls `stopPropagation` or
`preventDefault`, and it skips xterm's helper textarea (xterm does not read `isComposing`, so this costs nothing).
Install it the way the template number keys install their window listener, with `document::eval` from a hook at the top
of `AppBody`, placed before the hooks that install the other window capture listeners, so it is registered first on
every load; an asynchronously loaded `document::Script` asset would not guarantee that order. Keep the decision logic as
a pure function the JS harness can test. Choose the window's length from the event timing you can observe and justify it
in a DECISION; it must stay short enough that the browser tests' "real key afterwards" cases pass in Chromium, where
`compositionend` arrives after the ending keydown.

## What planning found

- No helper centralizes the composing check; the guards each call `evt.is_composing()` (Rust) or read `isComposing`
  (JS). `crates/farhelm-ui/assets/shift-enter-key.js` is the one place that also checks `keyCode === 229`, with a
  comment explaining why.
- Dioxus 0.7.10, checked against its source: the desktop interpreter's `serialize.ts` reads `event.isComposing` when it
  sends the event, from a bubbling listener on the root element, over a synchronous XHR, so a `window` capture listener
  always runs first. `dioxus-web` calls web-sys `is_composing()` at handler time, which compiles to a plain property
  read, so an own property on the event wins. Defining `isComposing` on a KeyboardEvent instance is allowed (only
  `isTrusted` is protected). The Rust `KeyboardData` has no getter for `keyCode`, so 229 is unreadable from Rust on
  desktop. Still confirm the mechanism in a browser test before building the rest on it.
- The other window capture listeners that check composing (the Cmd/Ctrl+Shift+K and +P chords in `lib.rs`, the template
  number keys) are installed with `document::eval` from hooks near the top of `AppBody`. `assets/sidebar-width.js`
  listens on the resize handle, not the window.
- Existing composition tests all dispatch a fake `isComposing: true` keydown (`e2e/tests/templates.spec.ts`,
  `restart-with.spec.ts`, `terminal-replay-rename.spec.ts`, `js-tests/shift-enter-key.test.js`,
  `js-tests/template-number-shortcuts.test.js`). No test drives a real input method; Playwright cannot on WebKit.
- The Linux desktop build renders with WebKitGTK, which may order these events like WKWebView or not; the approach must
  hold for both orders.

## Outline

1. **Prove the mechanism** first: a browser test on Chromium and WebKit showing that a guard's handler sees
   `is_composing()` true for a keydown the listener marked. Log the result as a DECISION.
2. **The listener and its installation**, with the decision logic as a pure, unit-tested function.
3. **Browser tests** as in the acceptance criteria, then SPEC_impl, the changelog fragment and the TODO removal.

PRs: (1) script, wiring and tests; (2) docs and TODO removal, or one PR if it stays reviewable.

### Validation

Per `.agents/validation.md`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `scripts/check-desktop-assets.sh` if an asset was added, the JS unit
harness, the new and existing composition browser specs on Chromium and WebKit through the recorder (with the builds
they need), the test-sleep checker, `python3 releasing/check-changelog.py format`, and `dprint check`.

## Fallbacks

- **If marking the event cannot reach the handlers** on a renderer: expose the "this keydown ends a composition" signal
  another way and route every guard through one Rust helper (for example `crate::ime::composing(&evt)`), at every site.
  This is agreed as the fallback; log which renderer forced it.
- **If no window length both catches WebKit's ending key and spares Chromium's next real key** in the browser tests,
  prefer sparing the real key (a guard that misses is today's behavior; a guard that swallows a real Escape or Enter is
  a new bug), cover what can be covered, and say plainly in the report which case is left.

Departures that need the scope reassessment (examples, not a blacklist): touching the terminal's input handling,
changing what any guarded key does when no composition is involved, or adding per-component composition state.

Decisions to log: the mechanism and how it was proved, the window length and why, the guard count you found, and the
representative guards the browser tests cover.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-webkit-ime-guards-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/webkit-ime-guards/<nn>-<short-name>` (for example `plan/webkit-ime-guards/01-composition-tracker`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `fix`. Add a changelog fragment (`kind: fixed`) per root
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

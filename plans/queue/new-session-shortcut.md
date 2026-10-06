# Cmd+N opens a new session in the Mac desktop app

Written against main at b3003cef on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out this TODO.md entry (Near term bucket), as narrowed by the user's decision below, and remove it in the same PR:

> **Cmd+N for a new session.** Cmd+N on macOS, and the equivalent elsewhere, does what clicking the new session button
> does. Pick the non-Mac chord with care: Ctrl+N is next-history in readline and emacs inside the terminal, and browsers
> keep Ctrl+N (and Cmd+N in the web UI on a Mac) for a new window. The terminal text-size shortcut in SPEC.md (Cmd+Shift
> on macOS, Ctrl+Shift elsewhere) is the precedent to follow.

The user decided (2026-10-05) "Desktop app only", then "Mac desktop app only": Cmd+N in the Mac desktop app, and no
shortcut on the (unshipped) Linux desktop build or in the web UI. That decision replaces the entry's "equivalent
elsewhere"; do not add a chord for any other platform.

Acceptance criteria:

- In the desktop app on macOS, Cmd+N (Command held, no Shift, Alt or Control) opens the new-session form exactly as
  clicking the session list's New button does, including its prefill, wherever focus is, terminal included. The key
  never reaches the terminal's program.
- While any modal dialog is open (the new-session form itself is one), Cmd+N does nothing; while the New button is
  disabled (the list is busy), Cmd+N does nothing.
- The web UI and the Linux desktop build are unchanged.
- SPEC.md says so in one or two sentences, and `docs/manual-mac-checklist.md` gains a bullet to check it on a Mac.
- One draft PR, `feat:`, with a changelog fragment (`kind: added`), reviewed per the review gate, removing the TODO
  entry.

## Requirement sources

**The user's request (2026-10-05):** "lets plan 'trailing slash in ~', 'cmd-N for new session', 'installer feedback
prompt' - a group of easy ones", planned as three separate plans at the user's choice; this is one.

**The user's plan-time decisions (2026-10-05):** Mac desktop app only, as above; review gate "opus 5.5 and astra high
(no swarm)" (see Review gate); no-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the outline below. A fresh-context planning review checked it against the code on b3003cef and
cut it down to the design described.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog;
The live install is off-limits), `plans/AGENTS.md` (Executing one plan).

## Outline

Line numbers drift; find the code by name. Verified on b3003cef:

- The New button is in `crates/farhelm-ui/src/list/view.rs` (class `new-session-button`). It is `disabled: busy`,
  re-checks `ops.busy_now()` in its click handler, toggles the form, and computes the prefill on open.
  `focus_new_session_button` in the same file already addresses it by that selector.
- The new-session form is `role="dialog"` with `aria-modal="true"` (`crates/farhelm-ui/src/list/create_form.rs`), the
  same marker `crates/farhelm-ui/assets/terminal.js` uses for open modals (`OPEN_MODAL_SELECTOR`). So "the form is
  already open" and "a modal owns the keyboard" are one check, and with no modal open the button's click always opens
  the form, never closes it.
- The text-size shortcut in `terminal.js` is the precedent: a `window` keydown listener in the capture phase, ahead of
  xterm's own handler, with `preventDefault` and `stopPropagation`.
- Desktop versus web is a compile-time distinction (`feature = "desktop"` in `farhelm-ui`). The page's JS has NO runtime
  desktop flag: `IS_MAC` in `terminal.js` reads `navigator.platform` and is also true in a browser on a Mac. Do not gate
  the shortcut on `IS_MAC` alone in shared JS.
- Dioxus desktop 0.7.10's default macOS menu has no Cmd+N item, so the key reaches the page.

The design: a capture-phase `window` keydown listener of about ten lines that acts on Cmd+N (match `event.key`
case-insensitively against `n`, which follows the Mac convention for non-QWERTY layouts such as Dvorak; Meta held,
Shift, Alt and Control not held). It returns without acting when an open modal matches
`[role="dialog"][aria-modal="true"]`; otherwise it calls `preventDefault` and `stopPropagation` and clicks
`.new-session-button`. Clicking a disabled button does nothing, and the button's own handler re-checks busy, so the
shortcut gets the button's exact behavior with no duplicated logic. Rust installs the listener once (for example with
`document::eval` from a component that mounts once, such as the app root or the session list), only in the desktop build
on macOS. Gate it so the code still compiles on Linux: `#[cfg(feature = "desktop")]` around it is fine, but use
`if cfg!(target_os = "macos")` rather than `#[cfg(target_os = "macos")]` for the macOS part, so the Linux desktop
compile check type-checks it. If the listener can be installed more than once (a remount), make that harmless.

Not wanted: refactoring the button's handler into a shared closure, a JS-to-Rust event channel, a native menu item with
a Cmd+N accelerator (more Mac-idiomatic, since the shortcut would show in the menu bar, but not asked for; mention it in
the report as a possible follow-up), and a unit test for the key predicate (it would only assert on a string).

SPEC.md: in the session list section, near "The Templates control sits beside New in the session list header", add that
Cmd+N in the Mac desktop app opens New, and that the web UI has no such shortcut because browsers keep Cmd/Ctrl+N for a
new window. Leave the unshipped Linux desktop build unmentioned.

`docs/manual-mac-checklist.md`: add a bullet under the most fitting section (or a short new one): in the Mac desktop
app, Cmd+N opens the new-session form with focus in the sidebar and with focus in a terminal (the terminal program gets
no key), does nothing while the form or another dialog is open, and does nothing while the list is busy; in a browser on
a Mac, Cmd+N still opens a new browser window.

The executing machine is Linux and cannot run the Mac desktop app. The report must say plainly that the shortcut was not
exercised at runtime before landing and that the checklist bullet is how it gets verified.

### Validation

Follow root `AGENTS.md` "Finishing work". Typical choices, not a checklist: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` scoped to what is affordable, `cargo check -p farhelm-ui --features desktop`
(this is what type-checks the new code on Linux, which is why the macOS gate must be `cfg!`), and
`scripts/check-desktop-assets.sh` only if a new asset file is added. `dprint check` on the changed Markdown. No browser
tests: the web UI does not change.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-new-session-shortcut-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs)
rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on, or
that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
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
  `plan/new-session-shortcut/<nn>-<short-name>`.
- This plan is one PR. If the work turns out to want more than one, split it into a linear stack of bite-sized PRs
  without churn (nothing added in one PR and removed in a later one), and log the split as a DECISION. Within this run,
  if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect. A `feat:` or
  `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases
  and the changelog), written for someone running Farhelm per `releasing/EDITORIAL_GUIDANCE.md`. Validate with
  `python3 releasing/check-changelog.py format`.
- The PR removes this plan's TODO.md entry (named under The goal) in the same commit. It never touches `plans/`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; the planning system's monitor lands the plan.

### Review gate

Before finishing each PR that changes code, scripts or tests, use the active galaxy-brain skill to delegate two
independent reviews of that PR's changes, and address what both find before moving on. The user demands exactly these
reviewers, and no review swarm ("opus 5.5 and astra high (no swarm)"):

- a fresh-context agent on Opus 5.5 at high effort;
- a fresh-context agent on gpt-6-astra at high effort, shelled out to the harness that serves that model when the
  executing one cannot reach it natively.

Both get the same prompt, carrying the full charter, because neither reviewer has anything else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a separate
findings file per reviewer (in the scratch directory), and the acceptance criteria: The goal and the user's decisions
from this file, quoted. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the DECISION. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new shared abstraction,
a new persisted field, a change to a protocol message, a JS-to-Rust event channel, a rewrite of a flow rather than a
small change in it), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the user's request and decisions, this outline, the current
diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the Conventional Commit type and changelog kind, anything the outline left to
you, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the user's requirements still hold. A material scope expansion, a weakened guarantee, an omitted
required behavior, or a change to text the user agreed verbatim needs the user's decision; a review finding or a log
entry is not authorization. If the work needs such a decision, record the concrete tradeoff and block per
`plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its draft PR (or linear stack, if split) exists, does what The goal says, has passed the
review gate, and removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver
its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/`
yourself.
